# Development

How to build, test and package NSWpedia Search, and how it works inside. For
CI and publishing see [releases.md](releases.md); for installing a release see
[install.md](install.md). Agents also follow [AGENTS.md](../AGENTS.md).

## Build

Rust 1.92 or newer (CI and releases use the pinned 1.99.1). Dependencies are
pinned by Cargo.lock. GPUI Kit 0.7.3 provides GPUI and native input
components. See https://gpui-kit.com/docs/installation/.

The version in `Cargo.toml` is the single source for the executable metadata,
installer, app bundle and file names. Allowed forms are `X.Y.Z` and
`X.Y.Z-rc.N`. Where a platform needs plain numbers, an ordered mapping is used
(`build.rs`, `scripts/version.sh`, `scripts/Version.ps1`): `0.2.0-rc.1` →
Windows file version 0.2.0.1, macOS `CFBundleShortVersionString` 0.2.0 and
`CFBundleVersion` 0.2.1; `0.2.0` → 0.2.0.1000, 0.2.0 and 0.2.1000.

Windows 10/11 x64: install Rust's MSVC toolchain and Visual Studio 2022 Build
Tools with the C++ desktop workload, Windows SDK and CMake. For the installer
also install [Inno Setup 6](https://jrsoftware.org/isinfo.php) (or set
`$env:ISCC` to the path of `ISCC.exe`). Run:

```powershell
.\scripts\Build-Windows.ps1
# Optimized build plus dist\nswpedia-search-windows.zip:
.\scripts\Build-Windows.ps1 -Release
.\dist\windows\nswpedia-search.exe
# Package into another folder (e.g. while a copy is running from dist\windows):
.\scripts\Build-Windows.ps1 -Release -Name windows-test
# Release assets in dist\release (setup .exe and portable zip):
.\scripts\Build-Windows.ps1 -Release -Installer
# Silent install/upgrade/uninstall check (installs into your profile, then removes it):
.\scripts\Test-WindowsInstaller.ps1 -Setup dist\release\nswpedia-search-<version>-windows-x64-setup.exe -Launch
```

The script embeds the icon and version resources, checks the executable's
version against Cargo.toml, and bundles every Visual C++ runtime DLL the
executable imports (currently only `vcruntime140.dll`); other DLLs are Windows
system components. The installer is defined in
`installer/windows/nswpedia-search.iss`; its `AppId` GUID is the upgrade
identity and must never change.

macOS 15 or newer: install Rust and Xcode (or its command-line tools), then:

```sh
bash scripts/package-macos.sh          # host CPU
bash scripts/package-macos.sh x64      # or arm64; other CPU needs: rustup target add x86_64-apple-darwin
open 'dist/macos-arm64/NSWpedia Search.app'
```

It writes the app bundle (icon, versioned `Info.plist`, bundle ID
`com.nswpedia.search`) and `dist/release/nswpedia-search-<version>-macos-<arch>.dmg`,
ad-hoc signs the app, and verifies the signature, CPU architecture and DMG
contents.

## Icon

`assets/icon/icon.svg` is the editable source (`icon-small.svg` is used for
16–32 px, `icon-macos.svg` follows the macOS icon grid). After editing,
regenerate the committed `.ico`, `.icns` and preview PNG:

```sh
cargo run --release --manifest-path tools/icon-gen/Cargo.toml -- assets/icon
```

## Tests and checks

```sh
cargo test --locked --no-default-features --lib
cargo run --locked --no-default-features --bin nswpedia-check -- Celeste
cargo run -- --search Celeste
```

The checker uses the same Rust resolver as the GUI. Its optional second argument
is a SQLite path, useful for measuring a fresh cache. It reports time to the
first landing link, not download speed. Unit tests cover parser boundaries,
ad rejection, cache persistence/expiry, concurrent deduplication and cancellation.
Real-Debrid and download tests run against a local mock HTTP server: request
shape and bearer header, error-code mapping (token never echoed), HTTPS
enforcement, safe names, no-overwrite saving, duplicate suppression, resume
after failure, and cancellation. No account or network is needed.

## How it works

- Public WordPress search → `/download/{slug}-{id}` → source-listed
  `download_list_aff` pages → `a#download-link`. No JavaScript/ad scripts run.
- Three concurrent requests, starts at least 250 ms apart, 60-second cooldown
  on HTTP 403/429, bounded response sizes and 20-second deadlines.
- Cache lifetimes: search 15 minutes, file lists 1 hour, landing links 12 hours.
  Stored as SQLite in the OS application cache directory.
- Resolve 24 files per batch, stream results immediately, retain resolved links
  while loading more. Queued work cancels on selection/search changes. Generation
  IDs reject results from obsolete work; in-flight requests may finish in the
  background within the timeout.
- UI work stays on the main thread; HTTP/parsing/cache work uses worker threads.
  Only NSWpedia source URLs and HTTPS 1fichier landing URLs are accepted.
- Source-page layout changes surface errors and a retry action. No automated
  crawl or alternate mirror probing. The only authenticated API is Real-Debrid,
  used only when you press download or verify.

The interface is a restrained, terminal-inspired dark design: monospace type,
one muted green accent, and brief fades plus small spinners while work is in
flight. Nothing animates while the window is idle.

No startup registration is created. Closing the window quits the app.

## Real-Debrid downloads in detail

**download** next to a ready link, or **download all**, sends each 1fichier link
to Real-Debrid (`POST /rest/1.0/unrestrict/link`) and streams the returned file
to `<download folder>\<result title>\<file name>`. The **↓** button opens the
queue with progress, speed, cancel, retry, and show-in-folder actions.

- Two downloads run at once; the rest wait in the queue. Re-clicking a link that
  is queued, running or already saved does nothing; failed or cancelled jobs
  restart.
- Folder and file names are sanitized to a single safe path component (no
  separators, `..`, drive colons, wildcards, reserved device names or trailing
  dots). Existing files are never replaced: a name such as `Game (1).nsp` is
  used instead.
- Data is written to `<file>.<hash>.part` and renamed when complete and the size
  matches. Failed downloads keep the part file and resume with an HTTP range
  request on retry, including after a restart; cancelled ones delete it.
- The token is only sent to `api.real-debrid.com` in the `Authorization` header.
  Download links must be HTTPS and redirects must stay HTTPS.
- The queue is in memory: closing the app stops active downloads and clears
  the list (part files remain and resume when the same link is downloaded
  again).

