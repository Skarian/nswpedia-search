#!/bin/bash
# Prints release version metadata derived from Cargo.toml (the single source
# of truth). Usage: eval "$(scripts/version.sh)"
#   VERSION        0.2.0-rc.1   (Cargo/semver, used in file names and tags)
#   BASE_VERSION   0.2.0        (CFBundleShortVersionString)
#   BUILD          1            (rc.N → N, release → 1000; see build.rs)
#   BUNDLE_VERSION 0.2.1        (CFBundleVersion: X.Y.(Z*1000+BUILD), ordered)
#   PRERELEASE     true|false
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\(.*\)"$/\1/p' Cargo.toml | head -n 1)
if [[ ! $version =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)(-rc\.([0-9]+))?$ ]]; then
  echo "Unsupported version '$version': use X.Y.Z or X.Y.Z-rc.N" >&2
  exit 1
fi
major=${BASH_REMATCH[1]} minor=${BASH_REMATCH[2]} patch=${BASH_REMATCH[3]} rc=${BASH_REMATCH[5]}
if [[ -n $rc ]]; then
  if (( 10#$rc < 1 || 10#$rc > 999 )); then echo "rc number must be 1-999" >&2; exit 1; fi
  build=$((10#$rc)) prerelease=true
else
  build=1000 prerelease=false
fi
echo "VERSION=$version"
echo "BASE_VERSION=$major.$minor.$patch"
echo "BUILD=$build"
echo "BUNDLE_VERSION=$major.$minor.$((10#$patch * 1000 + build))"
echo "PRERELEASE=$prerelease"
