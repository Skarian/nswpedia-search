//! Download queue: unrestricts 1fichier links through Real-Debrid and streams
//! the files to `<root>/<safe title>/<safe filename>`.
//!
//! Data is written to a `.part` file named after the source link, so a retry
//! (or the same link queued again after a restart) resumes with an HTTP range
//! request. Finished files never replace existing ones: a free name such as
//! `Game (1).nsp` is reserved with `create_new` before the rename.
use crate::{
    debrid::{Account, Debrid},
    resolver::{Cancel, fichier_url},
};
use anyhow::{Context, Result, anyhow, bail, ensure};
use reqwest::{
    blocking::Client,
    header::{CONTENT_LENGTH, CONTENT_RANGE, RANGE},
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread,
    time::{Duration, Instant},
};

/// Parallel downloads. Real-Debrid limits active downloads per account, so
/// keep this small.
pub const WORKERS: usize = 2;
const MAX_NAME: usize = 150;
const PROGRESS_EVERY: Duration = Duration::from_millis(200);

pub struct Request {
    pub title: String,
    pub name: String,
    pub link: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    Queued,
    Resolving,
    Downloading,
    Done,
    Failed(String),
    Cancelled,
}
impl Status {
    pub fn is_active(&self) -> bool {
        matches!(self, Self::Queued | Self::Resolving | Self::Downloading)
    }
}

#[derive(Clone, Debug)]
pub struct Job {
    pub id: u64,
    pub title: String,
    pub name: String,
    pub link: String,
    pub root: PathBuf,
    pub status: Status,
    pub received: u64,
    pub total: Option<u64>,
    /// Bytes per second, smoothed.
    pub rate: f64,
    /// Final file once done, otherwise the destination folder once known.
    pub path: Option<PathBuf>,
}

struct Entry {
    job: Job,
    cancel: Cancel,
    /// Incremented on every retry so a stale worker cannot overwrite state.
    run: u64,
    /// A worker is still processing an earlier run of this entry.
    busy: bool,
}

struct State {
    entries: Vec<Entry>,
    next_id: u64,
    token: Option<String>,
    shutdown: bool,
}

struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    debrid: Debrid,
    http: Client,
    notify: Box<dyn Fn() + Send + Sync>,
}
impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }
    fn update(&self, id: u64, run: u64, apply: impl FnOnce(&mut Job)) {
        if let Some(entry) = self
            .lock()
            .entries
            .iter_mut()
            .find(|e| e.job.id == id && e.run == run)
        {
            apply(&mut entry.job);
        }
        (self.notify)();
    }
}

pub struct Downloads {
    shared: Arc<Shared>,
}

impl Downloads {
    /// `notify` is called from worker threads whenever job state changes.
    pub fn new(notify: impl Fn() + Send + Sync + 'static) -> Result<Self> {
        Self::build(Debrid::new()?, true, Box::new(notify))
    }

    fn build(
        debrid: Debrid,
        require_https: bool,
        notify: Box<dyn Fn() + Send + Sync>,
    ) -> Result<Self> {
        let http = Client::builder()
            .user_agent("NSWpediaSearch/0.1")
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() >= 5 {
                    attempt.error("Too many redirects")
                } else if require_https && attempt.url().scheme() != "https" {
                    attempt.error("Refused a non-HTTPS redirect")
                } else {
                    attempt.follow()
                }
            }))
            .connect_timeout(Duration::from_secs(15))
            // Blocking reads apply this per read, so it is an idle timeout.
            .timeout(Duration::from_secs(60))
            .build()?;
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                entries: vec![],
                next_id: 1,
                token: None,
                shutdown: false,
            }),
            wake: Condvar::new(),
            debrid,
            http,
            notify,
        });
        for _ in 0..WORKERS {
            let shared = shared.clone();
            thread::spawn(move || worker(&shared));
        }
        Ok(Self { shared })
    }

    pub fn set_token(&self, token: Option<String>) {
        self.shared.lock().token = token;
    }

    pub fn has_token(&self) -> bool {
        self.shared.lock().token.is_some()
    }

    /// Checks `token`, or the current key if `None`, against Real-Debrid.
    /// Blocks on the network; call it off the UI thread.
    pub fn check_account(&self, token: Option<String>) -> Result<Account> {
        let token = match token {
            Some(token) => token,
            None => self
                .shared
                .lock()
                .token
                .clone()
                .context("No API key saved")?,
        };
        self.shared.debrid.account(&token)
    }

    pub fn jobs(&self) -> Vec<Job> {
        self.shared
            .lock()
            .entries
            .iter()
            .map(|e| e.job.clone())
            .collect()
    }

    /// Queues downloads, skipping links that are already queued, running or
    /// saved. Failed or cancelled jobs for the same link are restarted.
    /// Returns how many were queued.
    pub fn enqueue(&self, root: &Path, requests: Vec<Request>) -> Result<usize> {
        ensure!(
            self.has_token(),
            "Add your Real-Debrid API key in settings to download."
        );
        ensure!(
            root.is_absolute(),
            "The download folder must be a full path"
        );
        let mut queued = 0;
        {
            let mut state = self.shared.lock();
            for request in requests {
                if fichier_url(&request.link).is_none() {
                    continue;
                }
                if let Some(entry) = state
                    .entries
                    .iter_mut()
                    .find(|e| e.job.link == request.link && e.job.root == root)
                {
                    let saved = entry.job.status == Status::Done
                        && entry.job.path.as_deref().is_some_and(Path::exists);
                    if entry.job.status.is_active() || saved {
                        continue;
                    }
                    restart(entry);
                } else {
                    let id = state.next_id;
                    state.next_id += 1;
                    state.entries.push(Entry {
                        job: Job {
                            id,
                            title: request.title,
                            name: request.name,
                            link: request.link,
                            root: root.to_path_buf(),
                            status: Status::Queued,
                            received: 0,
                            total: None,
                            rate: 0.,
                            path: None,
                        },
                        cancel: Cancel::default(),
                        run: 0,
                        busy: false,
                    });
                }
                queued += 1;
            }
        }
        self.shared.wake.notify_all();
        (self.shared.notify)();
        Ok(queued)
    }

    pub fn retry(&self, id: u64) {
        if let Some(entry) = self
            .shared
            .lock()
            .entries
            .iter_mut()
            .find(|e| e.job.id == id && !e.job.status.is_active())
        {
            restart(entry);
        }
        self.shared.wake.notify_all();
        (self.shared.notify)();
    }

    pub fn cancel(&self, id: u64) {
        if let Some(entry) = self
            .shared
            .lock()
            .entries
            .iter_mut()
            .find(|e| e.job.id == id && e.job.status.is_active())
        {
            entry.cancel.cancel();
            entry.job.status = Status::Cancelled;
            entry.job.rate = 0.;
        }
        (self.shared.notify)();
    }

    /// Removes finished, failed and cancelled jobs from the list.
    pub fn clear_finished(&self) {
        self.shared
            .lock()
            .entries
            .retain(|e| e.job.status.is_active() || e.busy);
        (self.shared.notify)();
    }
}

impl Drop for Downloads {
    fn drop(&mut self) {
        let mut state = self.shared.lock();
        state.shutdown = true;
        for entry in &state.entries {
            entry.cancel.cancel();
        }
        drop(state);
        self.shared.wake.notify_all();
    }
}

fn restart(entry: &mut Entry) {
    entry.run += 1;
    entry.cancel = Cancel::default();
    entry.job.status = Status::Queued;
    entry.job.received = 0;
    entry.job.total = None;
    entry.job.rate = 0.;
    entry.job.path = None;
}

fn worker(shared: &Shared) {
    loop {
        let (job, cancel, run, token) = {
            let mut state = shared.lock();
            loop {
                if state.shutdown {
                    return;
                }
                let token = state.token.clone();
                if let Some(entry) = state
                    .entries
                    .iter_mut()
                    .find(|e| e.job.status == Status::Queued && !e.busy)
                {
                    entry.busy = true;
                    entry.job.status = Status::Resolving;
                    break (entry.job.clone(), entry.cancel.clone(), entry.run, token);
                }
                state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        };
        (shared.notify)();
        let mut part = None;
        let result = match token {
            Some(token) => run_job(shared, &job, run, &token, &cancel, &mut part),
            None => Err(anyhow!(
                "Add your Real-Debrid API key in settings to download."
            )),
        };
        // A cancelled job discards its partial data; a failed one keeps it so
        // a retry can resume.
        if cancel.is_cancelled() {
            if let Some(part) = &part {
                let _ = fs::remove_file(part);
            }
        }
        {
            let mut state = shared.lock();
            if let Some(entry) = state.entries.iter_mut().find(|e| e.job.id == job.id) {
                entry.busy = false;
                if entry.run == run {
                    entry.job.rate = 0.;
                    match result {
                        Ok(path) => {
                            entry.job.status = Status::Done;
                            entry.job.path = Some(path);
                        }
                        Err(_) if cancel.is_cancelled() => entry.job.status = Status::Cancelled,
                        Err(error) => entry.job.status = Status::Failed(error.to_string()),
                    }
                }
            }
        }
        shared.wake.notify_all();
        (shared.notify)();
    }
}

fn run_job(
    shared: &Shared,
    job: &Job,
    run: u64,
    token: &str,
    cancel: &Cancel,
    part_out: &mut Option<PathBuf>,
) -> Result<PathBuf> {
    let link = shared.debrid.unrestrict(token, &job.link)?;
    cancel.check()?;
    let dir = job
        .root
        .join(safe_name(&job.title).unwrap_or_else(|| "download".into()));
    fs::create_dir_all(&dir)
        .with_context(|| format!("Could not create folder {}", dir.display()))?;
    let filename = safe_name(&link.filename)
        .or_else(|| safe_name(&job.name))
        .unwrap_or_else(|| "download.bin".into());
    let part = dir.join(part_name(&filename, &job.link));
    *part_out = Some(part.clone());
    shared.update(job.id, run, |j| {
        if j.status == Status::Resolving {
            j.status = Status::Downloading;
        }
        j.total = link.filesize;
        j.path = Some(dir.clone());
    });
    fetch(
        shared,
        &link.download,
        link.filesize,
        &part,
        cancel,
        |received, total, rate| {
            shared.update(job.id, run, |j| {
                j.received = received;
                j.total = total;
                j.rate = rate;
            })
        },
    )
    .map_err(|e| anyhow!(e.to_string().replace(&link.download, "the download link")))?;
    cancel.check()?;
    finalize(&part, &dir, &filename)
}

/// Streams `url` into `part`, resuming from its current length if possible.
fn fetch(
    shared: &Shared,
    url: &str,
    expected: Option<u64>,
    part: &Path,
    cancel: &Cancel,
    mut progress: impl FnMut(u64, Option<u64>, f64),
) -> Result<()> {
    let mut offset = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
    if expected.is_some_and(|size| offset > size) {
        offset = 0;
    }
    let mut request = shared.http.get(url);
    if offset > 0 {
        request = request.header(RANGE, format!("bytes={offset}-"));
    }
    let response = request
        .send()
        .map_err(|e| anyhow!("Could not reach the download server: {}", e.without_url()))?;
    let status = response.status().as_u16();
    match status {
        206 if offset > 0 => {
            let range = response
                .headers()
                .get(CONTENT_RANGE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default();
            if !range.starts_with(&format!("bytes {offset}-")) {
                let _ = fs::remove_file(part);
                bail!("The server resumed at the wrong position. Retry to start over.");
            }
        }
        200 => offset = 0,
        416 if offset > 0 && expected == Some(offset) => return Ok(()),
        416 => {
            let _ = fs::remove_file(part);
            bail!("The partial download no longer matches. Retry to start over.");
        }
        _ => bail!("The download server returned HTTP {status}"),
    }
    let total = response
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
        .map(|length| length + offset)
        .or(expected);
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(offset > 0)
        .truncate(offset == 0)
        .open(part)
        .with_context(|| format!("Could not write {}", part.display()))?;
    let mut response = response;
    let mut buffer = vec![0; 256 * 1024];
    let mut received = offset;
    let (mut last, mut last_bytes, mut rate) = (Instant::now(), received, 0.);
    progress(received, total, 0.);
    loop {
        cancel.check()?;
        let n = match response.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == ErrorKind::Interrupted => continue,
            Err(e) => bail!("Download interrupted ({e}). Retry to resume."),
        };
        file.write_all(&buffer[..n])
            .with_context(|| format!("Could not write {}", part.display()))?;
        received += n as u64;
        let elapsed = last.elapsed();
        if elapsed >= PROGRESS_EVERY {
            let now = (received - last_bytes) as f64 / elapsed.as_secs_f64();
            rate = if rate == 0. {
                now
            } else {
                rate * 0.7 + now * 0.3
            };
            (last, last_bytes) = (Instant::now(), received);
            progress(received, total, rate);
        }
    }
    file.flush()?;
    file.sync_all()?;
    progress(received, total, rate);
    if let Some(total) = total {
        ensure!(
            received == total,
            "Download ended early ({received} of {total} bytes). Retry to resume."
        );
    }
    Ok(())
}

/// Moves `part` to `dir/filename`, or `name (n).ext` if that name is taken.
pub fn finalize(part: &Path, dir: &Path, filename: &str) -> Result<PathBuf> {
    let (stem, ext) = split_ext(filename);
    for n in 0..1000 {
        let name = if n == 0 {
            filename.to_string()
        } else {
            format!("{stem} ({n}){ext}")
        };
        let path = dir.join(name);
        match OpenOptions::new().write(true).create_new(true).open(&path) {
            Ok(placeholder) => {
                drop(placeholder);
                return match fs::rename(part, &path) {
                    Ok(()) => Ok(path),
                    Err(error) => {
                        let _ = fs::remove_file(&path);
                        Err(error).with_context(|| format!("Could not save {}", path.display()))
                    }
                };
            }
            Err(error) if error.kind() == ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(error).with_context(|| format!("Could not save {}", path.display()));
            }
        }
    }
    bail!("Too many files named {filename} in {}", dir.display())
}

fn split_ext(name: &str) -> (&str, &str) {
    match name.rfind('.') {
        Some(i) if i > 0 && name.len() - i <= 16 => name.split_at(i),
        _ => (name, ""),
    }
}

/// Partial-file name, unique per source link so retries find it again.
fn part_name(filename: &str, link: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(link.as_bytes()));
    format!("{filename}.{}.part", &digest[..8])
}

/// Makes one safe path component: no separators, drive colons, wildcards,
/// control characters, leading dots (`..`, hidden files), trailing dots or
/// spaces, or Windows device names. Returns `None` if nothing is left.
pub fn safe_name(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .filter_map(|ch| match ch {
            '/' | '\\' | ':' | '|' => Some(' '),
            '<' | '>' | '"' | '?' | '*' => None,
            ch if ch.is_control() => Some(' '),
            ch => Some(ch),
        })
        .collect();
    let mut name = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(['.', ' '])
        .to_string();
    if name.is_empty() {
        return None;
    }
    if name.chars().count() > MAX_NAME {
        let (stem, ext) = split_ext(&name);
        let keep = MAX_NAME - ext.chars().count();
        let stem: String = stem.chars().take(keep).collect();
        name = format!("{}{ext}", stem.trim_end_matches(['.', ' ']));
    }
    let device = name
        .split('.')
        .next()
        .unwrap_or_default()
        .trim_end()
        .to_ascii_uppercase();
    let reserved = matches!(device.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (device.len() == 4
            && (device.starts_with("COM") || device.starts_with("LPT"))
            && device.as_bytes()[3].is_ascii_digit());
    if reserved {
        name.insert(0, '_');
    }
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockServer, Reply};
    use std::{
        path::Component,
        sync::atomic::{AtomicUsize, Ordering},
    };

    const TOKEN: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789ABCDEFGHIJKLMNOP";
    const LINK: &str = "https://1fichier.com/?abcdefghijkl";

    fn data() -> Vec<u8> {
        (0..700_000u32).map(|i| (i % 251) as u8).collect()
    }

    /// Mock Real-Debrid + download host. `file` answers `GET /file`.
    fn server(
        filename: &'static str,
        file: impl Fn(&crate::mock::Request) -> Reply + Send + Sync + 'static,
    ) -> MockServer {
        let base = Arc::new(Mutex::new(String::new()));
        let url = base.clone();
        let server = MockServer::start(move |req| match req.path.as_str() {
            "/unrestrict/link" => Reply::json(
                200,
                &format!(
                    r#"{{"filename":"{filename}","filesize":{},"download":"{}/file"}}"#,
                    data().len(),
                    url.lock().unwrap()
                ),
            ),
            "/file" => file(req),
            _ => Reply::json(404, "{}"),
        });
        *base.lock().unwrap() = server.url();
        server
    }

    fn manager(server: &MockServer) -> Downloads {
        let debrid = Debrid::with_base(&server.url(), false).unwrap();
        let downloads = Downloads::build(debrid, false, Box::new(|| {})).unwrap();
        downloads.set_token(Some(TOKEN.into()));
        downloads
    }

    fn request(title: &str) -> Vec<Request> {
        vec![Request {
            title: title.into(),
            name: "Game".into(),
            link: LINK.into(),
        }]
    }

    fn wait(downloads: &Downloads, done: impl Fn(&Job) -> bool) -> Job {
        let start = Instant::now();
        loop {
            if let Some(job) = downloads.jobs().into_iter().find(|j| done(j)) {
                return job;
            }
            assert!(
                start.elapsed() < Duration::from_secs(10),
                "{:?}",
                downloads.jobs()
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    #[test]
    fn safe_names_cannot_escape_or_hit_reserved_names() {
        let cases = [
            ("Zelda: Tears of the Kingdom", "Zelda Tears of the Kingdom"),
            ("../../etc/passwd", "etc passwd"),
            ("..\\..\\Windows\\x.nsp", "Windows x.nsp"),
            ("C:\\temp\\a.nsp", "C temp a.nsp"),
            ("what?*<>\"|.nsp", "what .nsp"),
            ("trailing dots... ", "trailing dots"),
            (".hidden", "hidden"),
            ("con", "_con"),
            ("NUL.txt", "_NUL.txt"),
            ("com1.nsp", "_com1.nsp"),
            ("Console", "Console"),
            ("tab\tnew\nline", "tab new line"),
            ("Pokémon™ [NSP]", "Pokémon™ [NSP]"),
        ];
        for (raw, expected) in cases {
            let name = safe_name(raw).unwrap();
            assert_eq!(name, expected, "{raw:?}");
            let mut parts = Path::new(&name).components();
            assert!(
                matches!(parts.next(), Some(Component::Normal(_))),
                "{raw:?}"
            );
            assert!(parts.next().is_none(), "{raw:?}");
        }
        for empty in ["", "..", " . ", "///", "???"] {
            assert_eq!(safe_name(empty), None, "{empty:?}");
        }
        let long = format!("{}.nsp", "a".repeat(400));
        let name = safe_name(&long).unwrap();
        assert_eq!(name.chars().count(), MAX_NAME);
        assert!(name.ends_with("a.nsp"));
    }

    #[test]
    fn finalize_never_overwrites() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("Game.nsp"), "old").unwrap();
        fs::write(dir.path().join("Game (1).nsp"), "older").unwrap();
        let part = dir.path().join("x.part");
        fs::write(&part, "new").unwrap();
        let path = finalize(&part, dir.path(), "Game.nsp").unwrap();
        assert_eq!(path, dir.path().join("Game (2).nsp"));
        assert_eq!(fs::read_to_string(&path).unwrap(), "new");
        assert_eq!(
            fs::read_to_string(dir.path().join("Game.nsp")).unwrap(),
            "old"
        );
        assert!(!part.exists());
    }

    #[test]
    fn downloads_into_title_folder_and_skips_duplicates() {
        let server = server("../Game.nsp", |_| Reply::bytes(200, data()));
        let downloads = manager(&server);
        let root = tempfile::tempdir().unwrap();
        assert_eq!(
            downloads
                .enqueue(root.path(), request("../Hollow: Knight"))
                .unwrap(),
            1
        );
        // A second click while active (or after it is saved) does nothing.
        assert_eq!(
            downloads
                .enqueue(root.path(), request("../Hollow: Knight"))
                .unwrap(),
            0
        );
        let job = wait(&downloads, |j| !j.status.is_active());
        assert_eq!(job.status, Status::Done);
        let path = job.path.unwrap();
        assert_eq!(path, root.path().join("Hollow Knight").join("Game.nsp"));
        assert_eq!(fs::read(&path).unwrap(), data());
        assert_eq!(job.received, data().len() as u64);
        assert_eq!(
            downloads
                .enqueue(root.path(), request("../Hollow: Knight"))
                .unwrap(),
            0
        );
        assert_eq!(downloads.jobs().len(), 1);
        // No leftover partial files.
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 1);
    }

    #[test]
    fn rejects_non_fichier_links_and_missing_token() {
        let server = server("a", |_| Reply::bytes(200, vec![]));
        let downloads = manager(&server);
        let root = tempfile::tempdir().unwrap();
        let bad = vec![Request {
            title: "t".into(),
            name: "n".into(),
            link: "https://evil.example/?abcdefghijkl".into(),
        }];
        assert_eq!(downloads.enqueue(root.path(), bad).unwrap(), 0);
        assert!(
            downloads
                .enqueue(Path::new("relative"), request("t"))
                .is_err()
        );
        downloads.set_token(None);
        assert!(downloads.enqueue(root.path(), request("t")).is_err());
    }

    #[test]
    fn failure_keeps_partial_and_retry_resumes_with_range() {
        let calls = Arc::new(AtomicUsize::new(0));
        let ranges = Arc::new(Mutex::new(Vec::new()));
        let (count, seen) = (calls.clone(), ranges.clone());
        let server = server("Game.nsp", move |req| {
            seen.lock().unwrap().push(req.header("range"));
            if count.fetch_add(1, Ordering::SeqCst) == 0 {
                return Reply::json(503, "{}");
            }
            let full = data();
            let start: usize = req
                .header("range")
                .and_then(|r| r.strip_prefix("bytes=")?.strip_suffix('-')?.parse().ok())
                .unwrap_or(0);
            Reply::bytes(206, full[start..].to_vec()).header(
                "Content-Range",
                format!("bytes {start}-{}/{}", full.len() - 1, full.len()),
            )
        });
        let downloads = manager(&server);
        let root = tempfile::tempdir().unwrap();
        downloads.enqueue(root.path(), request("Game")).unwrap();
        let job = wait(&downloads, |j| !j.status.is_active());
        let Status::Failed(message) = &job.status else {
            panic!("{job:?}")
        };
        assert!(message.contains("HTTP 503"), "{message}");
        // Simulate data from an interrupted earlier attempt.
        let dir = root.path().join("Game");
        let part = dir.join(part_name("Game.nsp", LINK));
        fs::write(&part, &data()[..300_000]).unwrap();
        downloads.retry(job.id);
        let job = wait(&downloads, |j| j.status == Status::Done);
        assert_eq!(fs::read(job.path.unwrap()).unwrap(), data());
        assert!(!part.exists());
        assert_eq!(
            ranges.lock().unwrap().last().cloned().flatten().as_deref(),
            Some("bytes=300000-")
        );
    }

    #[test]
    fn cancel_stops_job_and_removes_partial() {
        let server = server("Game.nsp", |_| {
            thread::sleep(Duration::from_millis(400));
            Reply::bytes(200, data())
        });
        let downloads = manager(&server);
        let root = tempfile::tempdir().unwrap();
        downloads.enqueue(root.path(), request("Game")).unwrap();
        let job = wait(&downloads, |j| j.status == Status::Downloading);
        downloads.cancel(job.id);
        assert_eq!(downloads.jobs()[0].status, Status::Cancelled);
        thread::sleep(Duration::from_millis(800));
        let job = &downloads.jobs()[0];
        assert_eq!(job.status, Status::Cancelled);
        let dir = root.path().join("Game");
        assert_eq!(
            fs::read_dir(&dir).unwrap().count(),
            0,
            "{:?}",
            fs::read_dir(&dir)
        );
        // Retrying after cancel downloads it again.
        downloads.retry(job.id);
        let job = wait(&downloads, |j| j.status == Status::Done);
        assert_eq!(fs::read(job.path.unwrap()).unwrap(), data());
        downloads.clear_finished();
        assert!(downloads.jobs().is_empty());
    }
}
