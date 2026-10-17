use anyhow::{Context, Result, bail, ensure};
use regex::Regex;
use reqwest::blocking::Client;
use rusqlite::{Connection, OptionalExtension, params};
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    io::Read,
    path::Path,
    sync::{
        Arc, Condvar, LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use url::Url;

const ORIGIN: &str = "https://nswpedia.com";
pub const BATCH_SIZE: usize = 24;
const MAX_BODY: u64 = 3_000_000;
static SLUG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[a-z0-9-]{1,200}$").unwrap());
static FILE_PATH: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^/download/[a-z0-9-]+/\d+/download_list_aff/?$").unwrap());
static FILE_QUERY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[A-Za-z0-9]{10,64}(?:[?&]af=\d+)?$").unwrap());

#[derive(Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);
impl Cancel {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
    pub fn check(&self) -> Result<()> {
        ensure!(!self.is_cancelled(), "Cancelled");
        Ok(())
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Game {
    pub id: u64,
    pub title: String,
    pub slug: String,
    pub source_url: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SearchPage {
    pub games: Vec<Game>,
    pub page: u32,
    pub total_pages: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct FileEntry {
    pub key: String,
    pub name: String,
    pub size: String,
    pub format: String,
    pub path: String,
}

#[derive(Clone, Debug)]
pub enum ResolveEvent {
    Files(Vec<FileEntry>),
    Link {
        key: String,
        url: String,
    },
    Error {
        key: String,
        message: String,
    },
    Done {
        next_offset: Option<usize>,
        failures: usize,
    },
}

pub fn source_url(value: &str) -> Option<Url> {
    let url = Url::parse(value).ok()?;
    (url.scheme() == "https"
        && url.host_str() == Some("nswpedia.com")
        && url.username().is_empty()
        && url.password().is_none()
        && url.port().is_none())
    .then_some(url)
}

pub fn fichier_url(value: &str) -> Option<String> {
    let value = html_escape::decode_html_entities(value);
    let url = Url::parse(&value).ok()?;
    let authority = value.strip_prefix("https://")?.split('/').next()?;
    if !matches!(authority, "1fichier.com" | "www.1fichier.com")
        || !matches!(url.host_str(), Some("1fichier.com" | "www.1fichier.com"))
        || url.path() != "/"
        || url.fragment().is_some()
        || !FILE_QUERY.is_match(url.query()?)
    {
        return None;
    }
    Some(value.into_owned())
}

fn text(element: scraper::ElementRef<'_>) -> String {
    element
        .text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn parse_files(body: &str) -> Vec<FileEntry> {
    let doc = Html::parse_document(body);
    let rows = Selector::parse("tr").unwrap();
    let cells = Selector::parse("td, th").unwrap();
    let anchors = Selector::parse("a[href]").unwrap();
    let mut seen = HashSet::new();
    let mut files = Vec::new();
    for row in doc.select(&rows) {
        let columns: Vec<_> = row.select(&cells).map(text).collect();
        for anchor in row.select(&anchors) {
            let Some(url) = anchor.value().attr("href").and_then(source_url) else {
                continue;
            };
            if !FILE_PATH.is_match(url.path()) || !seen.insert(url.path().to_owned()) {
                continue;
            }
            let digest = format!("{:x}", Sha256::digest(url.path().as_bytes()));
            files.push(FileEntry {
                key: digest[..16].to_string(),
                name: text(anchor),
                size: columns.get(1).cloned().unwrap_or_default(),
                format: columns.get(2).cloned().unwrap_or_default(),
                path: url.path().to_string(),
            });
        }
    }
    files
}

pub fn parse_landing(body: &str) -> Result<String> {
    let doc = Html::parse_document(body);
    let selector = Selector::parse("a#download-link[href]").unwrap();
    doc.select(&selector)
        .find_map(|a| a.value().attr("href").and_then(fichier_url))
        .context("No 1fichier link found. The source page may have changed.")
}

fn parse_file_listing(body: &str) -> Result<Vec<FileEntry>> {
    ensure!(
        body.contains("table-download") || body.contains("List of available versions"),
        "The file-list page changed or is unavailable"
    );
    let files = parse_files(body);
    // A marker alone does not establish that the listing was parsed correctly.
    // Until the source has a recognized empty-state contract, do not cache an
    // empty parse as a successful listing.
    ensure!(
        !files.is_empty(),
        "No supported file links found. The listing may be empty or the source page may have changed."
    );
    Ok(files)
}

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs_f64()
}

struct Cache {
    db: Mutex<Connection>,
    flights: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}
impl Cache {
    fn new(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let db = Connection::open(path)?;
        db.busy_timeout(Duration::from_secs(3))?;
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE IF NOT EXISTS cache (key TEXT PRIMARY KEY, value TEXT NOT NULL, expires REAL NOT NULL);")?;
        db.execute("DELETE FROM cache WHERE expires < ?", [now()])?;
        Ok(Self {
            db: Mutex::new(db),
            flights: Mutex::new(HashMap::new()),
        })
    }
    fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        let value: Option<String> = self
            .db
            .lock()
            .unwrap()
            .query_row(
                "SELECT value FROM cache WHERE key=? AND expires>?",
                params![key, now()],
                |r| r.get(0),
            )
            .optional()?;
        // Ignore incompatible cache data after an application update.
        Ok(value.and_then(|v| serde_json::from_str(&v).ok()))
    }
    fn load<T: Serialize + DeserializeOwned>(
        &self,
        key: &str,
        ttl: f64,
        cancel: &Cancel,
        load: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        cancel.check()?;
        if let Some(value) = self.get(key)? {
            return Ok(value);
        }
        let lock = self
            .flights
            .lock()
            .unwrap()
            .entry(key.to_string())
            .or_default()
            .clone();
        let _guard = lock.lock().unwrap();
        cancel.check()?;
        if let Some(value) = self.get(key)? {
            return Ok(value);
        }
        let value = load()?;
        self.db.lock().unwrap().execute(
            "INSERT OR REPLACE INTO cache VALUES (?,?,?)",
            params![key, serde_json::to_string(&value)?, now() + ttl],
        )?;
        Ok(value)
    }
}

struct GateState {
    active: usize,
    next: Instant,
    cooldown: Instant,
}
struct Gate {
    state: Mutex<GateState>,
    changed: Condvar,
}
struct Permit<'a>(&'a Gate);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        self.0.state.lock().unwrap().active -= 1;
        self.0.changed.notify_all();
    }
}
impl Gate {
    fn new() -> Self {
        Self {
            state: Mutex::new(GateState {
                active: 0,
                next: Instant::now(),
                cooldown: Instant::now(),
            }),
            changed: Condvar::new(),
        }
    }
    fn enter(&self, cancel: &Cancel) -> Result<Permit<'_>> {
        let mut state = self.state.lock().unwrap();
        loop {
            cancel.check()?;
            ensure!(
                Instant::now() >= state.cooldown,
                "Source requests paused for a minute after a refusal. Try again shortly."
            );
            if state.active < 3 && Instant::now() >= state.next {
                state.active += 1;
                state.next = Instant::now() + Duration::from_millis(250);
                return Ok(Permit(self));
            }
            state = self
                .changed
                .wait_timeout(state, Duration::from_millis(50))
                .unwrap()
                .0;
        }
    }
}

pub struct Resolver {
    client: Client,
    cache: Cache,
    gate: Gate,
}
impl Resolver {
    pub fn new(cache_path: &Path) -> Result<Self> {
        Ok(Self {
            client: Client::builder()
                .user_agent("NSWPortal/0.1 (personal lookup)")
                .redirect(reqwest::redirect::Policy::none())
                .connect_timeout(Duration::from_secs(10))
                .timeout(Duration::from_secs(20))
                .pool_max_idle_per_host(3)
                .build()?,
            cache: Cache::new(cache_path)?,
            gate: Gate::new(),
        })
    }

    fn get(&self, path: &str, cancel: &Cancel) -> Result<(String, reqwest::header::HeaderMap)> {
        ensure!(
            path.starts_with('/') && !path.starts_with("//") && !path.contains(['\r', '\n', '\\']),
            "Invalid source path"
        );
        let _permit = self.gate.enter(cancel)?;
        let response = self
            .client
            .get(format!("{ORIGIN}{path}"))
            .header("Accept", "application/json,text/html")
            .send()
            .context("Could not connect to NSWpedia")?;
        let status = response.status();
        if status.as_u16() == 403 || status.as_u16() == 429 {
            self.gate.state.lock().unwrap().cooldown = Instant::now() + Duration::from_secs(60);
            bail!("NSWpedia refused or rate-limited the request. Wait a minute before retrying.");
        }
        ensure!(
            status.is_success(),
            "NSWpedia returned HTTP {}",
            status.as_u16()
        );
        let headers = response.headers().clone();
        let mut bytes = Vec::new();
        response.take(MAX_BODY + 1).read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 <= MAX_BODY,
            "Source response exceeded the size limit"
        );
        cancel.check()?;
        Ok((String::from_utf8_lossy(&bytes).into_owned(), headers))
    }

    pub fn search(&self, query: &str, page: u32, cancel: &Cancel) -> Result<SearchPage> {
        let query = query.trim();
        ensure!(
            (2..=120).contains(&query.chars().count()),
            "Enter 2–120 characters"
        );
        ensure!((1..=100).contains(&page), "Invalid search page");
        self.cache.load(
            &format!("search:{}:{page}", query.to_lowercase()),
            900.,
            cancel,
            || {
                let params = url::form_urlencoded::Serializer::new(String::new())
                    .append_pair("search", query)
                    .append_pair("page", &page.to_string())
                    .append_pair("per_page", "20")
                    .append_pair("type", "post")
                    .append_pair("subtype", "post")
                    .append_pair("_fields", "id,title,url")
                    .finish();
                let (body, headers) =
                    self.get(&format!("/wp-json/wp/v2/search?{params}"), cancel)?;
                #[derive(Deserialize)]
                struct Row {
                    id: u64,
                    title: String,
                    url: String,
                }
                let rows: Vec<Row> =
                    serde_json::from_str(&body).context("Unexpected search response")?;
                let mut games = Vec::new();
                for row in rows {
                    let Some(url) = source_url(&row.url) else {
                        continue;
                    };
                    if !url.path().starts_with("/nintendo-switch-roms/") {
                        continue;
                    }
                    let slug = url
                        .path()
                        .trim_end_matches('/')
                        .rsplit('/')
                        .next()
                        .unwrap_or_default();
                    if !SLUG.is_match(slug) {
                        continue;
                    }
                    games.push(Game {
                        id: row.id,
                        title: html_escape::decode_html_entities(&row.title).into_owned(),
                        slug: slug.to_owned(),
                        source_url: url.to_string(),
                    });
                }
                let normalized = |s: &str| {
                    s.chars()
                        .filter(|c| c.is_alphanumeric())
                        .flat_map(char::to_lowercase)
                        .collect::<String>()
                };
                let needle = normalized(query);
                games.sort_by_key(|g| normalized(&g.title) != needle);
                let total_pages = headers
                    .get("x-wp-totalpages")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(1);
                Ok(SearchPage {
                    games,
                    page,
                    total_pages,
                })
            },
        )
    }

    pub fn files(&self, game: &Game, cancel: &Cancel) -> Result<Vec<FileEntry>> {
        ensure!(
            game.id > 0 && SLUG.is_match(&game.slug),
            "Invalid game identifier"
        );
        self.cache.load(
            &format!("files:v2:{}:{}", game.id, game.slug),
            3600.,
            cancel,
            || {
                let (body, _) =
                    self.get(&format!("/download/{}-{}", game.slug, game.id), cancel)?;
                parse_file_listing(&body)
            },
        )
    }

    fn link(&self, file: &FileEntry, cancel: &Cancel) -> Result<String> {
        ensure!(FILE_PATH.is_match(&file.path), "Invalid file page");
        self.cache
            .load(&format!("link:{}", file.path), 43200., cancel, || {
                let (body, _) = self.get(&file.path, cancel)?;
                parse_landing(&body)
            })
    }

    pub fn resolve(
        &self,
        game: &Game,
        offset: usize,
        cancel: &Cancel,
        emit: &(impl Fn(ResolveEvent) + Sync),
    ) -> Result<()> {
        let files = self.files(game, cancel)?;
        ensure!(offset <= files.len(), "Invalid file offset");
        emit(ResolveEvent::Files(files.clone()));
        let end = (offset + BATCH_SIZE).min(files.len());
        let mut queue = VecDeque::new();
        for file in &files[offset..end] {
            if let Some(url) = self.cache.get::<String>(&format!("link:{}", file.path))? {
                emit(ResolveEvent::Link {
                    key: file.key.clone(),
                    url,
                });
            } else {
                queue.push_back(file.clone());
            }
        }
        let queue = Mutex::new(queue);
        let failures = Mutex::new(0usize);
        thread::scope(|scope| {
            for _ in 0..3 {
                scope.spawn(|| {
                    loop {
                        if cancel.is_cancelled() {
                            break;
                        }
                        let Some(file) = queue.lock().unwrap().pop_front() else {
                            break;
                        };
                        let result = self.link(&file, cancel);
                        if cancel.is_cancelled() {
                            break;
                        }
                        match result {
                            Ok(url) => emit(ResolveEvent::Link { key: file.key, url }),
                            Err(error) => {
                                *failures.lock().unwrap() += 1;
                                emit(ResolveEvent::Error {
                                    key: file.key,
                                    message: error.to_string(),
                                });
                            }
                        }
                    }
                });
            }
        });
        cancel.check()?;
        emit(ResolveEvent::Done {
            next_offset: (end < files.len()).then_some(end),
            failures: *failures.lock().unwrap(),
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unrecognized_listing_is_not_cached_and_can_be_retried() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(&dir.path().join("cache.sqlite")).unwrap();
        let cancel = Cancel::default();
        let key = "files:v2:1:test";
        for body in [
            "<table id='table-download'></table>",
            "<h2>List of available versions</h2><div>Changed layout</div>",
            "<html>Unavailable</html>",
        ] {
            assert!(
                cache
                    .load(key, 3600., &cancel, || parse_file_listing(body))
                    .is_err()
            );
            assert!(cache.get::<Vec<FileEntry>>(key).unwrap().is_none());
        }
        let body = "<table id='table-download'><tr><td><a href='https://nswpedia.com/download/test-1/1/download_list_aff'>Game</a></td><td>3 GB</td><td>nsp</td></tr></table>";
        let files = cache
            .load(key, 3600., &cancel, || parse_file_listing(body))
            .unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(cache.get::<Vec<FileEntry>>(key).unwrap().unwrap().len(), 1);
    }

    #[test]
    fn parse_files_keeps_metadata_ignores_ads_and_other_mirrors() {
        let files = parse_files(
            r#"<script>"<a href='https://1fichier.com/?abcdefghijk'>ad</a>"</script>
        <table><tr><td><a href='https://nswpedia.com/download/test-1/1/download_list_aff'>Game &amp; update</a></td><td>3 GB</td><td>nsp</td></tr>
        <tr><td><a href='https://nswpedia.com/download/test-1/1/download_list_aff_pro'>Other</a></td></tr>
        <tr><td><a href='https://evil.example/download/test-1/1/download_list_aff'>Ad</a></td></tr>
        <tr><td><a href='https://nswpedia.com/download/test-1/1/download_list_aff'>duplicate</a></td></tr></table>"#,
        );
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "Game & update");
        assert_eq!(files[0].size, "3 GB");
    }
    #[test]
    fn landing_link_is_specific_and_host_restricted() {
        let valid = "https://1fichier.com/?abcdefghijklmnopqrst?af=5006637";
        assert_eq!(
            parse_landing(&format!(
                "<a href='https://ads.example'>Ad</a><a id='download-link' href='{valid}'>Go</a>"
            ))
            .unwrap(),
            valid
        );
        for bad in [
            "https://1fichier.com.evil.example/?abcdefghijk",
            "javascript:alert(1)",
            "http://1fichier.com/?abcdefghijk",
            "https://1fichier.com:443/?abcdefghijk",
            "https://user@1fichier.com/?abcdefghijk",
            "https://1fichier.com/download/file",
        ] {
            assert!(fichier_url(bad).is_none(), "{bad}");
        }
        assert!(parse_landing("<a id='download-link' href='https://ads.example'>Go</a>").is_err());
    }
    #[test]
    fn cache_persists_expires_and_deduplicates() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("cache.sqlite");
        let cache = Arc::new(Cache::new(&path).unwrap());
        let calls = std::sync::atomic::AtomicUsize::new(0);
        thread::scope(|scope| {
            for _ in 0..6 {
                scope.spawn(|| {
                    let n: u32 = cache
                        .load("same", 60., &Cancel::default(), || {
                            calls.fetch_add(1, Ordering::Relaxed);
                            thread::sleep(Duration::from_millis(30));
                            Ok(42u32)
                        })
                        .unwrap();
                    assert_eq!(n, 42);
                });
            }
        });
        assert_eq!(calls.load(Ordering::Relaxed), 1);
        let second = Cache::new(&path).unwrap();
        assert_eq!(second.get::<u32>("same").unwrap(), Some(42));
        cache
            .load("old", -1., &Cancel::default(), || Ok(1u32))
            .unwrap();
        assert!(cache.get::<u32>("old").unwrap().is_none());
    }
    #[test]
    fn cancellation_stops_cached_and_queued_work() {
        let cancel = Cancel::default();
        cancel.cancel();
        assert!(cancel.check().is_err());
        assert!(Gate::new().enter(&cancel).is_err());
    }
}
