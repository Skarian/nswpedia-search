#!/bin/bash
# Installs the DMG from dist/release into a temporary Applications folder, checks
# the signature and CPU architecture, and launches the app for 10 seconds.
#   bash scripts/smoke-macos.sh arm64|x64
set -euo pipefail
cd "$(dirname "$0")/.."
eval "$(bash scripts/version.sh)"
arch=$1
case $arch in
  arm64) lipo_arch=arm64 ;;
  x64) lipo_arch=x86_64 ;;
  *) echo "Use arm64 or x64" >&2; exit 1 ;;
esac
dmg="dist/release/nswpedia-search-$VERSION-macos-$arch.dmg"
mnt=$(mktemp -d)
apps=$(mktemp -d)
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$mnt" >/dev/null
ditto "$mnt/NSWpedia Search.app" "$apps/NSWpedia Search.app"
hdiutil detach "$mnt" -quiet
app="$apps/NSWpedia Search.app"
exe="$app/Contents/MacOS/nswpedia-search"
[[ $(lipo -archs "$exe") == "$lipo_arch" ]] || { echo "Expected $lipo_arch, got $(lipo -archs "$exe")" >&2; exit 1; }
codesign --verify --strict --verbose=2 "$app"
# Expected to be rejected: ad-hoc signed and not notarized.
spctl --assess --type execute --verbose=2 "$app" || true
"$exe" &
pid=$!
sleep 10
kill -0 "$pid" 2>/dev/null || { echo "App exited during startup" >&2; exit 1; }
kill "$pid"
echo "App ($lipo_arch on $(uname -m)) started and stayed running"
