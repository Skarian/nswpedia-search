# CI and releases

Build once, test those exact files, publish the same files.

`.github/workflows/ci.yml` is the single validation run for pull requests and
`main`. It always checks formatting and runs the resolver tests. When a pull
request changes packaging inputs (`Cargo.toml`, `build.rs`, `assets/`,
`installer/`, `scripts/`, `.github/`), the same run builds and tests the
installers with `.github/workflows/build.yml`; otherwise it runs
release-profile tests on Windows and Apple Silicon. After a merge, the run on
`main` is skipped if that exact source tree already passed in this repository.

`build.yml` compiles each platform once (tests reuse the release build).
Windows builds the installer and runs a silent install → upgrade → uninstall
test (settings and credential preservation, launching the installed app).
Both macOS DMGs compile on the Apple Silicon runner (`macos-15`); the arm64
DMG is installed and launched there, the Intel DMG on a real Intel runner
(`macos-15-intel`). The tested files are kept for 30 days as the artifact
`release-assets-<source tree>`, with `SHA256SUMS.txt` and `provenance.txt`
(version, source tree, commit and run that built them).

To release, follow the checklist in AGENTS.md: bump the version in the pull
request, merge it, then run the Release workflow on `main`:

```sh
gh workflow run release.yml --ref main
```

`.github/workflows/release.yml` refuses if the version is already tagged,
reuses the tested assets for main's exact source tree (only from successful
runs of this repository's own workflows, never fork pull requests), or builds
and tests them once if there are none. It verifies the version, source tree
and every checksum, then creates the tag `vX.Y.Z` at main's commit and
publishes those files with notes from `.github/release-notes.md`. A stable
version becomes the repository's **Latest** release; `-rc` versions are
**Pre-release** and don't replace Latest. Nothing is tagged or published
unless every step passes. Only the final job has `contents: write`; actions
are pinned to commit SHAs; the Rust toolchain and Inno Setup (SHA-256-checked)
are pinned; caches are only written from `main`. Artifacts contain only the
built app; no credentials or API keys are involved beyond the workflow's own
short-lived GitHub token.

