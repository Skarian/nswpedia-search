//! Minimal Real-Debrid REST client: account check and link unrestriction.
//!
//! See https://api.real-debrid.com/ — `POST /unrestrict/link` takes a form
//! field `link` and returns `{ filename, filesize, download, … }`; errors are
//! 4xx/5xx with `{ "error": "…", "error_code": n }`. The API token is sent only
//! in the `Authorization` header, never in a URL, and never in error text.
use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::blocking::Client;
use serde::Deserialize;
use std::{io::Read, time::Duration};
use url::Url;

pub const API_BASE: &str = "https://api.real-debrid.com/rest/1.0";
pub const TOKEN_PAGE: &str = "https://real-debrid.com/apitoken";
const MAX_BODY: u64 = 256_000;

/// Accepts the shape of Real-Debrid private API tokens (alphanumeric), which
/// also keeps anything header-unsafe out of the `Authorization` value.
pub fn valid_token(token: &str) -> bool {
    (20..=128).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_alphanumeric())
}

#[derive(Clone, Debug, PartialEq)]
pub struct Unrestricted {
    pub filename: String,
    pub filesize: Option<u64>,
    pub download: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub username: String,
    pub premium: bool,
    pub premium_days: u64,
}

pub struct Debrid {
    client: Client,
    base: String,
    require_https: bool,
}

impl Debrid {
    pub fn new() -> Result<Self> {
        Self::with_base(API_BASE, true)
    }

    pub(crate) fn with_base(base: &str, require_https: bool) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("NSWpediaSearch/0.1")
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(30))
                .build()?,
            base: base.trim_end_matches('/').to_string(),
            require_https,
        })
    }

    fn call(&self, request: reqwest::blocking::RequestBuilder, token: &str) -> Result<String> {
        ensure!(
            valid_token(token),
            "The Real-Debrid API key looks invalid. Check it in settings."
        );
        let response = request
            .bearer_auth(token)
            .header("Accept", "application/json")
            .send()
            .map_err(|e| anyhow!("Could not reach Real-Debrid: {}", e.without_url()))?;
        let status = response.status().as_u16();
        let mut bytes = Vec::new();
        response
            .take(MAX_BODY)
            .read_to_end(&mut bytes)
            .context("Real-Debrid response was interrupted")?;
        let body = String::from_utf8_lossy(&bytes).into_owned();
        if !(200..300).contains(&status) {
            return Err(api_error(status, &body));
        }
        Ok(body)
    }

    pub fn account(&self, token: &str) -> Result<Account> {
        #[derive(Deserialize)]
        struct User {
            username: String,
            #[serde(rename = "type")]
            kind: String,
            #[serde(default)]
            premium: u64,
        }
        let body = self.call(self.client.get(format!("{}/user", self.base)), token)?;
        let user: User =
            serde_json::from_str(&body).context("Unexpected Real-Debrid account response")?;
        Ok(Account {
            username: user.username,
            premium: user.kind == "premium",
            premium_days: user.premium / 86_400,
        })
    }

    pub fn unrestrict(&self, token: &str, link: &str) -> Result<Unrestricted> {
        #[derive(Deserialize)]
        struct Link {
            #[serde(default)]
            filename: String,
            filesize: Option<u64>,
            download: String,
        }
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Reply {
            One(Link),
            Many(Vec<Link>),
        }
        let body = self.call(
            self.client
                .post(format!("{}/unrestrict/link", self.base))
                .form(&[("link", link)]),
            token,
        )?;
        let link = match serde_json::from_str(&body).context("Unexpected Real-Debrid response")? {
            Reply::One(link) => link,
            Reply::Many(links) => links
                .into_iter()
                .next()
                .context("Real-Debrid returned no download links")?,
        };
        let url = Url::parse(&link.download).context("Real-Debrid returned an invalid link")?;
        if url.scheme() != "https" && (self.require_https || url.scheme() != "http") {
            bail!("Real-Debrid returned a non-HTTPS download link");
        }
        Ok(Unrestricted {
            filename: link.filename,
            filesize: link.filesize.filter(|&size| size > 0),
            download: link.download,
        })
    }
}

/// Turns a Real-Debrid error reply into a short, actionable message.
pub fn api_error(status: u16, body: &str) -> anyhow::Error {
    #[derive(Deserialize)]
    struct Fault {
        #[serde(default)]
        error: String,
        error_code: Option<i64>,
    }
    let fault = serde_json::from_str::<Fault>(body).ok();
    let code = fault.as_ref().and_then(|f| f.error_code);
    let message = match (status, code) {
        (_, Some(8)) | (401, _) => {
            "Real-Debrid rejected the API key. Check it in settings.".to_string()
        }
        (_, Some(14)) => "Real-Debrid account is locked.".into(),
        (_, Some(15)) => "Real-Debrid account is not activated.".into(),
        (_, Some(20)) => "This host needs a Real-Debrid premium account.".into(),
        (_, Some(9)) | (403, _) => {
            "Real-Debrid denied access (account locked or not premium).".into()
        }
        (_, Some(16)) => "Real-Debrid does not support this host.".into(),
        (_, Some(17 | 19)) => {
            "The host is temporarily unavailable on Real-Debrid. Retry later.".into()
        }
        (_, Some(18)) => "Real-Debrid's quota for this host is used up. Retry later.".into(),
        (_, Some(21)) => "Too many active Real-Debrid downloads. Retry when one finishes.".into(),
        (_, Some(22)) => "Real-Debrid does not allow downloads from this IP address.".into(),
        (_, Some(23)) => "Real-Debrid traffic quota exhausted.".into(),
        (_, Some(36)) => "Real-Debrid fair-usage limit reached.".into(),
        (_, Some(24)) => "The file is unavailable on the host (removed or expired).".into(),
        (_, Some(35)) => "Real-Debrid blocked this file.".into(),
        (_, Some(5 | 34)) | (429, _) => {
            "Real-Debrid rate limit reached. Wait a minute and retry.".into()
        }
        (503, _) => "The file or Real-Debrid service is unavailable. Retry later.".into(),
        _ => {
            let detail = fault
                .map(|f| f.error)
                .filter(|e| !e.is_empty())
                .map(|e| e.chars().take(120).collect::<String>())
                .unwrap_or_else(|| "unexpected reply".into());
            match code {
                Some(code) => format!("Real-Debrid error {code}: {detail} (HTTP {status})"),
                None => format!("Real-Debrid error: {detail} (HTTP {status})"),
            }
        }
    };
    anyhow!(message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockServer, Reply};

    const TOKEN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMNOP";

    #[test]
    fn token_shape_is_checked_before_sending() {
        assert!(valid_token(TOKEN));
        for bad in [
            "",
            "short",
            "ABCDEFGHIJKLMNOPQRSTU\r\nX: y",
            "ABCDEFGHIJKLMNOPQRSTU V",
        ] {
            assert!(!valid_token(bad), "{bad:?}");
        }
    }

    #[test]
    fn unrestrict_posts_form_with_bearer_header() {
        let server = MockServer::start(|req| {
            assert_eq!(req.method, "POST");
            assert_eq!(req.path, "/unrestrict/link");
            assert_eq!(req.header("authorization"), Some(format!("Bearer {TOKEN}")));
            assert!(!req.path.contains(TOKEN));
            assert_eq!(
                req.body,
                b"link=https%3A%2F%2F1fichier.com%2F%3Fabcdefghijkl%26af%3D1".to_vec()
            );
            Reply::json(
                200,
                r#"{"id":"X","filename":"Game.nsp","mimeType":"application/octet-stream","filesize":1234,"link":"https://1fichier.com/?abcdefghijkl","host":"1fichier.com","chunks":16,"crc":1,"download":"https://abc.download.real-debrid.com/d/X/Game.nsp","streamable":0}"#,
            )
        });
        let debrid = Debrid::with_base(&server.url(), false).unwrap();
        let link = debrid
            .unrestrict(TOKEN, "https://1fichier.com/?abcdefghijkl&af=1")
            .unwrap();
        assert_eq!(link.filename, "Game.nsp");
        assert_eq!(link.filesize, Some(1234));
        assert!(link.download.starts_with("https://"));
    }

    #[test]
    fn rejects_insecure_download_links() {
        let server = MockServer::start(|_| {
            Reply::json(
                200,
                r#"{"filename":"a","download":"http://evil.example/a"}"#,
            )
        });
        let mut debrid = Debrid::with_base(&server.url(), false).unwrap();
        debrid.require_https = true;
        let error = debrid
            .unrestrict(TOKEN, "https://1fichier.com/?x")
            .unwrap_err();
        assert!(error.to_string().contains("non-HTTPS"));
    }

    #[test]
    fn errors_are_mapped_and_never_echo_the_token() {
        let cases = [
            (
                401,
                r#"{"error":"bad_token","error_code":8}"#,
                "rejected the API key",
            ),
            (
                403,
                r#"{"error":"permission_denied","error_code":9}"#,
                "denied access",
            ),
            (
                503,
                r#"{"error":"unavailable_file","error_code":24}"#,
                "unavailable on the host",
            ),
            (
                503,
                r#"{"error":"hoster_unavailable","error_code":19}"#,
                "temporarily unavailable",
            ),
            (429, "", "rate limit"),
            (
                400,
                r#"{"error":"weird","error_code":99}"#,
                "error 99: weird",
            ),
            (500, "<html>", "unexpected reply"),
        ];
        for (status, body, expected) in cases {
            let server = MockServer::start(move |_| Reply::json(status, body));
            let debrid = Debrid::with_base(&server.url(), false).unwrap();
            let message = debrid
                .unrestrict(TOKEN, "https://1fichier.com/?x")
                .unwrap_err()
                .to_string();
            assert!(message.contains(expected), "{status}: {message}");
            assert!(!message.contains(TOKEN));
        }
    }

    #[test]
    fn account_reports_premium_days() {
        let server = MockServer::start(|req| {
            assert_eq!(req.path, "/user");
            Reply::json(
                200,
                r#"{"id":1,"username":"neo","email":"x","points":1,"locale":"en","avatar":"","type":"premium","premium":864000,"expiration":"2027-01-01T00:00:00.000Z"}"#,
            )
        });
        let debrid = Debrid::with_base(&server.url(), false).unwrap();
        let account = debrid.account(TOKEN).unwrap();
        assert_eq!(
            account,
            Account {
                username: "neo".into(),
                premium: true,
                premium_days: 10
            }
        );
    }
}
