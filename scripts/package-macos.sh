#!/bin/bash
set -euo pipefail
cd "$(dirname "$0")/.."
export MACOSX_DEPLOYMENT_TARGET=15.0
cargo build --locked --release --bin nswpedia-search
app="dist/NSWpedia Search.app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp target/release/nswpedia-search "$app/Contents/MacOS/nswpedia-search"
cat > "$app/Contents/Info.plist" <<'PLIST'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>CFBundleName</key><string>NSWpedia Search</string>
<key>CFBundleDisplayName</key><string>NSWpedia Search</string>
<key>CFBundleIdentifier</key><string>com.nswpedia.search</string>
<key>CFBundleExecutable</key><string>nswpedia-search</string>
<key>CFBundlePackageType</key><string>APPL</string>
<key>CFBundleShortVersionString</key><string>0.1.0</string>
<key>CFBundleVersion</key><string>1</string>
<key>LSMinimumSystemVersion</key><string>15.0</string>
<key>NSHighResolutionCapable</key><true/>
</dict></plist>
PLIST
codesign --force --deep --sign - "$app"
ditto -c -k --sequesterRsrc --keepParent "$app" dist/nswpedia-search-macos.zip
echo "$app"
