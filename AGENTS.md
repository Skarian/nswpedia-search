# Agent notes

Native Rust/GPUI desktop app. See README.md for features, build and install
details. This file holds the rules agents must follow.

## Before any release (mandatory)

A release run takes about 30 minutes, and a failure in the last step wastes all
of it. Before pushing a version tag, check everything that can be checked
locally:

1. `Cargo.toml` has the release version (`X.Y.Z`, or `X.Y.Z-rc.N` for a
   prerelease). `bash scripts/version.sh` must accept it.
2. The release commit is merged to `main` (the user merges PRs), and CI on
   `main` is green: `gh run list -b main -L 1`.
3. **Publish dry run:** `bash scripts/test-release-publish.sh`. It runs the
   "Check assets", "Release notes" and "Publish release" steps from
   `.github/workflows/release.yml` verbatim against the private scratch repo
   `Skarian/nswpedia-release-test`, using dummy files. It checks that a stable
   tag becomes Latest with all assets, that an rc tag becomes a prerelease
   without replacing Latest, and that re-running a published tag is refused. It
   deletes its test releases afterwards. Run it again after **any** change to
   `release.yml` or `.github/release-notes.md`.
4. If the installer, `build.rs` or packaging scripts changed, run the Windows
   installer test locally (installs into your profile, then removes it):
   `.\scripts\Build-Windows.ps1 -Release -Installer -Name windows-rc`, then
   `.\scripts\Test-WindowsInstaller.ps1 -Setup dist\release\<setup>.exe -Launch`.
   Never pass `-SeedCredential` on the user's PC: a real Real-Debrid token is
   stored there. The script refuses to overwrite it anyway.
5. Only then: `git tag -a vX.Y.Z -m "NSWpedia Search X.Y.Z"` on `main` and
   `git push origin vX.Y.Z`. The workflow publishes automatically once every
   job passes. Report the release URL when it finishes.

If a release run fails before publishing, nothing is public. Fix the problem,
merge the fix, then move the tag to the new `main` commit
(`git push origin :refs/tags/vX.Y.Z`, re-tag, push).

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
- `cargo fmt --all --check` and both test commands from the README must pass
  before pushing.
