#!/usr/bin/env bash
# Wrap the SwiftPM binary into a minimal .app bundle. No Xcode needed.
# Usage: scripts/bundle-app.sh [debug|release]   (default: release)
set -euo pipefail
cd "$(dirname "$0")/.."
CONFIG="${1:-release}"
swift build -c "$CONFIG"
BIN="$(swift build -c "$CONFIG" --show-bin-path)/ColourmotionDMX"
APP=".build/ColourmotionDMX.app"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/ColourmotionDMX"
cat > "$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>ColourmotionDMX</string>
  <key>CFBundleDisplayName</key><string>ColourmotionDMX</string>
  <key>CFBundleIdentifier</key><string>nl.colourmotion.dmx</string>
  <key>CFBundleExecutable</key><string>ColourmotionDMX</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>0.1.0</string>
  <key>CFBundleVersion</key><string>1</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
# Ad-hoc signature so Gatekeeper lets it run locally.
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "Built $APP"
