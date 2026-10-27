#!/usr/bin/env bash
# Wrap the SwiftPM binary into a minimal .app bundle. No Xcode needed.
# Usage: scripts/bundle-app.sh [debug|release] [version] [build]   (default: release 0.0.0 dev, REL-03)
# UNIVERSAL=1 builds arm64 + x86_64 (make-dmg.sh sets it).
set -euo pipefail
cd "$(dirname "$0")/.."
CONFIG="${1:-release}" VERSION="${2:-0.0.0}" BUILD="${3:-dev}"
ARCHS=(); [ "${UNIVERSAL:-}" = 1 ] && ARCHS=(--arch arm64 --arch x86_64)
swift build -c "$CONFIG" ${ARCHS[@]+"${ARCHS[@]}"}
BIN="$(swift build -c "$CONFIG" ${ARCHS[@]+"${ARCHS[@]}"} --show-bin-path)/ColourmotionDMX"
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
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$BUILD</string>
  <key>LSMinimumSystemVersion</key><string>14.0</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSPrincipalClass</key><string>NSApplication</string>
</dict></plist>
PLIST
# Ad-hoc signature so Gatekeeper lets it run locally.
codesign --force --sign - "$APP" >/dev/null 2>&1 || true
echo "Built $APP"
