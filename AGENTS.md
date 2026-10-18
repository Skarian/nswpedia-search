# Agent notes

Native Rust/GPUI desktop app. See README.md for features, docs/install.md for
installing, docs/development.md for building and internals, and
docs/releases.md for CI and publishing. This file holds the rules agents must
follow.

## How validation and releases flow

Build once, test those exact files, publish the same files. Never rebuild what
was already validated, and never move a tag.

1. **Pull request:** `ci.yml` is the only validation. It always runs
   formatting and resolver tests. If packaging inputs changed (`Cargo.toml`,
   `build.rs`, `assets/`, `installer/`, `scripts/`, `.github/`), the same run
   builds and tests the installers (`build.yml`: Windows install/upgrade/
   uninstall, both DMGs installed and launched on Apple Silicon and Intel) and
   stores them as `release-assets-<source tree>` with `SHA256SUMS.txt` and
   `provenance.txt`. Otherwise it runs release-profile tests on Windows and
   Apple Silicon. Don't start a second overlapping run (for example a manual
   Build run) for the same commit.
2. **Merge to main** (the user merges): `ci.yml` on `main` is skipped when the
   identical source tree already passed in this repository
   (`scripts/find-validated.sh`). It only runs if `main` moved in between.
3. **Release:** Actions → Release → Run workflow on `main`
   (`gh workflow run release.yml --ref main`). It refuses if `vX.Y.Z` already
   exists, reuses `release-assets-<tree>` for main's exact tree from a
   successful same-repository run (fork PR runs are never trusted), or builds
   and tests once in the release run if there are none. It verifies the
   version, source tree and every checksum, then creates the tag at main's
   commit and publishes the same files. X.Y.Z becomes Latest; X.Y.Z-rc.N is a
   prerelease. A tag exists only for a published release.

If a release run fails, nothing is tagged or public. Fix, merge, run again.

## Before a release (mandatory)

1. `Cargo.toml` has the new version (`X.Y.Z` or `X.Y.Z-rc.N`) and
   `bash scripts/version.sh` accepts it. Bump it in the PR you intend to
   release, so that PR's validation produces the release assets.
2. If `release.yml`, `.github/release-notes.md`, `scripts/bundle-assets.sh` or
   `scripts/test-release-publish.sh` changed, run the publish dry run before
   pushing: `bash scripts/test-release-publish.sh`. It runs the publish steps
   of `release.yml` verbatim against the private scratch repo
   `Skarian/nswpedia-release-test` with dummy assets: drafts don't create the
   tag, publishing creates it at the release commit, stable becomes Latest, rc
   becomes a prerelease, notes state where the assets were built, a modified
   asset or an already-published version is refused. It cleans up after itself.
3. If the installer, `build.rs` or packaging scripts changed, run the Windows
   installer test locally (installs into your profile, then removes it):
   `.\scripts\Build-Windows.ps1 -Release -Installer -Name windows-rc`, then
   `.\scripts\Test-WindowsInstaller.ps1 -Setup dist\release\<setup>.exe -Launch`.
   Never pass `-SeedCredential` on the user's PC: a real Real-Debrid token is
   stored there. The script refuses to overwrite it anyway.
4. After the user merges, start the release run and report the release URL.
   The release notes say which run and commit built the assets; if that commit
   is the PR's merge preview rather than main's commit, they say the source
   trees are identical. Don't hide or "fix" that by rebuilding.

## CI cost and speed

- Never trigger on every branch push and never add a second validation
  workflow. An early setup that did burned through the account's Actions
  minutes in a day (private-repo macOS minutes bill at 10x, Windows at 2x).
- Each platform compiles once in the release profile; tests reuse it. Keep
  `cargo test` flags and the rust-cache `shared-key` aligned with the
  packaging build, or the cache stops helping.
- Compile macOS on the Apple Silicon runner (both architectures); the Intel
  runner only runs `scripts/smoke-macos.sh x64` on the finished DMG.
- Validate workflow edits locally first: `actionlint` (no errors) and the
  publish dry run where it applies.

## Identities that must never change

They tie upgrades to existing installs and keep user data reachable:

- Inno Setup `AppId` `{918D6931-5C88-4EEE-9F8B-85E0D2EA2C30}` in
  `installer/windows/nswpedia-search.iss`.
- macOS bundle identifier `com.nswpedia.search`.
- Keyring service `NSWpedia Search`, account `real-debrid-api-key`
  (`src/secrets.rs`).
- `ProjectDirs::from("com", "NSWpedia", "NSWpedia Search")` (settings and
  cache paths, `src/main.rs`).

Installers and uninstallers must not touch settings, cache, credentials or
downloads.

## Working on this machine

- Git needs `-c safe.directory=C:/Users/Neil/projects/nswpedia-search` (the
  folder is owned by another account). `GIT_CONFIG_COUNT=1
  GIT_CONFIG_KEY_0=safe.directory GIT_CONFIG_VALUE_0=C:/Users/Neil/projects/nswpedia-search`
  also covers `gh`.
- Cargo is in `C:\Users\Neil\.cargo\bin`. The GitHub CLI may only be at
  `$env:TEMP\nswpedia-gh\bin\gh.exe`.
- Packaged copies in `dist\` may be running. Use `Build-Windows.ps1 -Name <new>`
  instead of overwriting locked files.
- Work on a feature branch and open a PR; the user merges to `main`.
- `cargo fmt --all --check` and the test commands in docs/development.md must pass
  before pushing.
