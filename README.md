# NSWpedia Search

Native Rust/GPUI desktop application. Direct HTTPS requests, local SQLite cache,
native text entry and clipboard. No webview, localhost service, Python, Node,
SSH, VM, or relay is needed to run the executable. The desktop replacement was
accepted on October 17, 2026; the web version has been retired.

Project location: C:\Users\Neil\projects\nswpedia-search.

Search with Enter or the EXEC button. The first result resolves automatically.
Select another title to resolve it; source-page buttons open the original page
in your default browser. Copy individual URLs or all ready URLs. Ctrl+L (Cmd+L
on macOS) focuses search. No recent-search UI. URLs are 1fichier landing links;
the app does not download files or verify their availability.

## Build

Rust 1.92 or newer. Dependencies are pinned by Cargo.lock. GPUI Kit 0.7.3 provides
GPUI and native input components. See https://gpui-kit.com/docs/installation/.

Windows 10/11 x64: install Rust's MSVC toolchain and Visual Studio 2022 Build
Tools with the C++ desktop workload, Windows SDK and CMake. Run:

```powershell
.\scripts\Build-Windows.ps1
# Optimized distribution build:
.\scripts\Build-Windows.ps1 -Release
.\dist\windows\nswpedia-search.exe
```

The Windows output folder/zip includes Microsoft's redistributable
`vcruntime140.dll`; keep it alongside the executable when copying the app to
another PC. Other DLL dependencies are Windows system components.

macOS 15 or newer: install Rust and Xcode command-line tools, then:

```sh
bash scripts/package-macos.sh
open 'dist/NSWpedia Search.app'
```

The Mac script builds for the host CPU, produces an app bundle and zip, and
applies an ad-hoc signature for local use. Developer ID signing/notarization
for distribution is not configured. The included CI workflow assumes this
directory is the repository root; it has not been published or run remotely.
Actual Mac build and interaction testing still require a Mac.

## Checks

```sh
cargo test --locked --no-default-features --lib
cargo run --locked --no-default-features --bin nswpedia-check -- Celeste
cargo run -- --search Celeste
```

The checker uses the same Rust resolver as the GUI. Its optional second argument
is a SQLite path, useful for measuring a fresh cache. It reports time to the
first landing link, not download speed. Unit tests cover parser boundaries,
ad rejection, cache persistence/expiry, concurrent deduplication and cancellation.

## Implementation

- Public WordPress search → `/download/{slug}-{id}` → source-listed
  `download_list_aff` pages → `a#download-link`. No JavaScript/ad scripts run.
- Three concurrent requests, starts at least 250 ms apart, 60-second cooldown
  on HTTP 403/429, bounded response sizes and 20-second deadlines.
- Cache lifetimes: search 15 minutes, file lists 1 hour, landing links 12 hours.
  Stored in the OS application cache directory, separate from the web cache.
- Resolve 24 files per batch, stream results immediately, retain resolved links
  while loading more. Queued work cancels on selection/search changes. Generation
  IDs reject results from obsolete work; in-flight requests may finish in the
  background within the timeout.
- UI work stays on the main thread; HTTP/parsing/cache work uses worker threads.
  Only NSWpedia source URLs and HTTPS 1fichier landing URLs are accepted.
- Source-page layout changes surface errors and a retry action. No automated
  crawl, alternate mirror probing, credentials, or authenticated API use.

The interface is a terminal-style, neon-on-black design (monospace type,
scanlines, glitch/decode/typewriter motion). Animations follow the OS
reduce-motion setting; the idle window repaints only for the cursor blink.

No startup registration is created. Closing the window quits the app.

## Verified on this Windows PC — October 17, 2026

- Debug and optimized builds passed; four resolver unit tests passed.
- Live lookup: Celeste 1/1, Hollow Knight 3/3, No Man’s Sky 2/2 landing links.
  Cold first-link times were 1.27 s, 1.44 s and 1.20 s respectively. Cached
  Celeste completed in 1 ms. Single-run backend observations, not guarantees.
- Native window rendered and Copy placed the exact expected URL on the clipboard.
  Packaged release loaded Hollow Knight with three ready Copy controls and Copy
  all. It was responsive, used about 75 MB RAM, opened no listening TCP ports,
  and produced no startup stderr. The user subsequently accepted the desktop replacement.
- Windows output: approximately 31 MB executable, 12 MB zipped with its runtime.
- macOS has not been built or tested here. The old web portal is stopped.

Evidence is in `reports/`. The temporary relay VM was deleted. A recovery archive
of the retired web project is kept at
`C:\Users\Neil\projects\pc-manager\archives\nsw-portal-retired-2026-10-17.zip`.
