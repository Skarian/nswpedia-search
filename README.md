# NSWpedia Search

Native Rust/GPUI desktop application. Direct HTTPS requests, local SQLite cache,
native text entry and clipboard. No webview, localhost service, Python, Node,
SSH, VM, or relay is needed to run the executable. The desktop replacement was
accepted on October 17, 2026; the web version has been retired.

Project location: C:\Users\Neil\projects\nswpedia-search.

Search with Enter or the search button. The first result resolves automatically.
Select another title to resolve it; source-page buttons open the original page
in your default browser. Copy individual URLs or all ready URLs. Ctrl+L (Cmd+L
on macOS) focuses search. No recent-search UI. URLs are 1fichier landing links.

## Install

Releases are on the repository's GitHub **Releases** page. The repository is
private: sign in to GitHub in your browser, or use
`gh release download <tag> -R Skarian/nswpedia-search`. Each release has a
`SHA256SUMS.txt`; compare with `Get-FileHash <file>` (Windows) or
`shasum -a 256 <file>` (macOS).

These builds are free and unsigned: no paid Windows code-signing certificate,
no Apple Developer ID, no notarization. The operating system therefore warns
once, as described below. Only continue for files downloaded from this
repository's releases.

### Windows 10/11 (x64)

1. Download `nswpedia-search-<version>-windows-x64-setup.exe`. Edge or Chrome
   may say the file *isn't commonly downloaded*; choose **Keep** (in Edge:
   **...** menu → **Keep** → **Show more** → **Keep anyway**).
2. Run it. SmartScreen shows **Windows protected your PC** (*Unknown
   publisher*). Click **More info**, then **Run anyway**.
3. The installer needs no administrator rights. It installs for the current
   user into `%LOCALAPPDATA%\Programs\NSWpedia Search`, adds a Start menu
   shortcut and, optionally, a desktop shortcut. Uninstall from **Settings →
   Apps → Installed apps** (or Control Panel).

If **Smart App Control** is on (Windows 11), it blocks unsigned apps and
offers no **Run anyway** button. The only way around it is turning Smart App
Control off in **Windows Security → App & browser control**.

Portable alternative: unzip `nswpedia-search-<version>-windows-x64-portable.zip`
anywhere and run `nswpedia-search.exe` (keep `vcruntime140.dll` next to it).
SmartScreen may show the same warning. Settings and the token are shared with
the installed copy.

**Upgrade:** run the newer setup. It reuses the existing installation folder
and shortcut choices and asks to close the app if it is running.

### macOS 15 or newer

Pick the DMG for your Mac (Apple menu → **About This Mac**): **Chip: Apple M...**
→ `nswpedia-search-<version>-macos-arm64.dmg`; **Processor: Intel** →
`nswpedia-search-<version>-macos-x64.dmg`.

1. Open the DMG and drag **NSWpedia Search** onto **Applications**.
2. Open the app from Applications. macOS shows **"NSWpedia Search" Not
   Opened** (*Apple could not verify "NSWpedia Search" is free of malware...*).
   Click **Done**, not **Move to Trash**.
3. Open **System Settings → Privacy & Security**, scroll to **Security**,
   click **Open Anyway** next to *"NSWpedia Search" was blocked...*, enter
   your password, then click **Open Anyway** again. Later launches open
   normally.

Since macOS 15, Control-click → **Open** no longer skips this check; use
**Open Anyway** as above. The Terminal equivalent removes the download
quarantine flag: `xattr -dr com.apple.quarantine "/Applications/NSWpedia Search.app"`.
Files fetched with `gh release download` are normally not quarantined, so the
prompt usually doesn't appear for them.

**Upgrade:** drag the new version onto Applications and choose **Replace**.
The **Open Anyway** step is needed once per downloaded version. Each build has
a new ad-hoc signature, so the first time a new version reads the saved
Real-Debrid token macOS may ask whether *NSWpedia Search* may use confidential
information stored in your keychain: enter your login password and click
**Always Allow**. The token itself is kept.

**Uninstall:** drag the app to the Trash.

### Your data across upgrades and uninstalls

Installing, upgrading and uninstalling only change the app's own files. These
are kept (delete them by hand for a clean slate):

| | Windows | macOS |
| --- | --- | --- |
| Settings (`settings.json`) | `%APPDATA%\NSWpedia\NSWpedia Search\config` | `~/Library/Application Support/com.NSWpedia.NSWpedia-Search` |
| Search cache | `%LOCALAPPDATA%\NSWpedia\NSWpedia Search\cache` | `~/Library/Caches/com.NSWpedia.NSWpedia-Search` |
| Real-Debrid token | Credential Manager → Windows Credentials → `real-debrid-api-key.NSWpedia Search` | Keychain Access → login → `NSWpedia Search` |
| Downloads | your chosen download folder | your chosen download folder |

There is no automatic updater; check the Releases page for new versions.

## Real-Debrid downloads

Open **settings** (top right), paste your private API token from
https://real-debrid.com/apitoken, optionally press **verify**, and **save**. The
token is stored in Windows Credential Manager (macOS Keychain on Mac), never in
files or logs; the input is masked and is cleared when the dialog closes.
The download folder defaults to your Downloads folder; settings are saved in
the OS config directory as `settings.json`.

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

### Icon

`assets/icon/icon.svg` is the editable source (`icon-small.svg` is used for
16–32 px, `icon-macos.svg` follows the macOS icon grid). After editing,
regenerate the committed `.ico`, `.icns` and preview PNG:

```sh
cargo run --release --manifest-path tools/icon-gen/Cargo.toml -- assets/icon
```

## CI and releases

`.github/workflows/ci.yml` runs on every branch push: formatting, unit tests on
all platforms, the Windows installer with a silent install → upgrade →
uninstall test (settings and credential preservation, launching the installed
app), and DMGs for Apple Silicon (`macos-15`) and Intel (`macos-15-intel`)
runners that are installed from the image and launched. Packages are kept as
workflow artifacts for 14 days.

To release, set the version in Cargo.toml, commit, and push a matching tag:

```sh
git tag v0.1.1
git push origin v0.1.1
```

`.github/workflows/release.yml` checks that the tag equals `v` + the
Cargo.toml version, rebuilds and tests every platform, and only if all of that
passes publishes a GitHub Release with the four installers, `SHA256SUMS.txt`
and notes from `.github/release-notes.md`. A stable version (`X.Y.Z`) becomes
the repository's **Latest** release in the sidebar; `-rc` tags are published
as **Pre-release** and don't replace Latest. If any job fails, nothing is
published. Only the final job has `contents: write`;
actions are pinned to commit SHAs; the Rust toolchain and Inno Setup
(SHA-256-checked) are pinned; release builds don't use the dependency cache.
Artifacts contain only the built app; no credentials or API keys are involved
beyond the workflow's own short-lived GitHub token.

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
Real-Debrid and download tests run against a local mock HTTP server: request
shape and bearer header, error-code mapping (token never echoed), HTTPS
enforcement, safe names, no-overwrite saving, duplicate suppression, resume
after failure, and cancellation. No account or network is needed.

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
  crawl or alternate mirror probing. The only authenticated API is Real-Debrid,
  used only when you press download or verify.

The interface is a restrained, terminal-inspired dark design: monospace type,
one muted green accent, and brief fades plus small spinners while work is in
flight. Nothing animates while the window is idle.

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
