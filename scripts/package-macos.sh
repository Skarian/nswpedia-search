#!/bin/bash
# Builds the macOS app bundle and drag-to-Applications DMG for one CPU.
#   bash scripts/package-macos.sh              host CPU
#   bash scripts/package-macos.sh arm64|x64    explicit Apple Silicon / Intel
# Output: dist/macos-<arch>/NSWpedia Search.app and
#         dist/release/nswpedia-search-<version>-macos-<arch>.dmg
# The app is ad-hoc signed (free, no Apple Developer ID) and not notarized.
set -euo pipefail
cd "$(dirname "$0")/.."
eval "$(bash scripts/version.sh)"

arch=${1:-$(uname -m)}
case $arch in
  arm64 | aarch64) arch=arm64 target=aarch64-apple-darwin ;;
  x64 | x86_64) arch=x64 target=x86_64-apple-darwin lipo_arch=x86_64 ;;
  *) echo "Unknown architecture '$arch' (use arm64 or x64)" >&2; exit 1 ;;
esac
lipo_arch=${lipo_arch:-arm64}

export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build --locked --release --bin nswpedia-search --target "$target"

name="NSWpedia Search"
stage="dist/macos-$arch"
app="$stage/$name.app"
rm -rf "$stage"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources" dist/release
cp "target/$target/release/nswpedia-search" "$app/Contents/MacOS/nswpedia-search"
cp assets/icon/nswpedia-search.icns "$app/Contents/Resources/AppIcon.icns"
# Bundle identifier is part of the app's identity (keychain access, settings);
# keep it stable across releases.
cat > "$app/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>$name</string>
<key>CFBundleDisplayName</key><string>$name</string>
<key>CFBundleIdentifier</key><string>com.nswpedia.search</string>
<key>CFBundleExecutable</key><string>nswpedia-search</string>
<key>CFBundleIconFile</key><string>AppIcon</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleInfoDictionaryVersion</key><string>6.0</string>
<key>CFBundleShortVersionString</key><string>$BASE_VERSION</string>
<key>CFBundleVersion</key><string>$BUNDLE_VERSION</string>
<key>NSHumanReadableCopyright</key><string>Version $VERSION. MIT License.</string>
<key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
plutil -lint "$app/Contents/Info.plist"

actual=$(lipo -archs "$app/Contents/MacOS/nswpedia-search")
[[ $actual == "$lipo_arch" ]] || { echo "Built for '$actual', expected '$lipo_arch'" >&2; exit 1; }

# Ad-hoc signature: required for Apple Silicon, free, but not trusted by
# Gatekeeper. See README "Install" for the first-launch steps.
codesign --force --sign - "$app"
codesign --verify --strict --verbose=2 "$app"
codesign --display --verbose=2 "$app" 2>&1 | grep -E '^(Identifier|Format|Signature|CodeDirectory)'

dmg="dist/release/nswpedia-search-$VERSION-macos-$arch.dmg"
dmgroot="$stage/dmg"
mkdir -p "$dmgroot"
ditto "$app" "$dmgroot/$name.app"
ln -s /Applications "$dmgroot/Applications"
rm -f "$dmg"
hdiutil create -volname "$name" -srcfolder "$dmgroot" -fs HFS+ -format UDZO -ov "$dmg" >/dev/null
rm -rf "$dmgroot"
hdiutil verify "$dmg" >/dev/null

# Check the image contents as a user would see them.
mnt=$(mktemp -d)
hdiutil attach "$dmg" -readonly -nobrowse -mountpoint "$mnt" >/dev/null
trap 'hdiutil detach "$mnt" -quiet || true' EXIT
[[ -L "$mnt/Applications" ]] || { echo "DMG is missing the Applications link" >&2; exit 1; }
codesign --verify --strict "$mnt/$name.app"
[[ $(defaults read "$mnt/$name.app/Contents/Info" CFBundleShortVersionString) == "$BASE_VERSION" ]]
hdiutil detach "$mnt" -quiet
trap - EXIT

echo "$app"
echo "$dmg"
