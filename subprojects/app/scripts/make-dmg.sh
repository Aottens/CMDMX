#!/usr/bin/env bash
# REL-07: build the release DMG (universal app + daemon, QLC+ fixtures, ad-hoc signed).
# Usage: scripts/make-dmg.sh <version> <build>   -> .build/ColourmotionDMX-<version>.dmg
# Needs subprojects/engine/vendor/qlcplus (run subprojects/engine/scripts/fetch-qlcplus.sh first).
set -euo pipefail
cd "$(dirname "$0")/.."
VERSION="$1" BUILD="$2"
ENGINE=../engine
VENDOR="$ENGINE/vendor/qlcplus"
[ -d "$VENDOR/fixtures" ] || { echo "make-dmg: $VENDOR missing: run subprojects/engine/scripts/fetch-qlcplus.sh first" >&2; exit 1; }

TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
(cd "$ENGINE" && rustup target add "${TARGETS[@]}")
for t in "${TARGETS[@]}"; do
  (cd "$ENGINE" && cargo build --release -p cmdmx-daemon --target "$t")
done
DAEMON=.build/cmdmx-daemon-universal
lipo -create -output "$DAEMON" "$ENGINE/target/${TARGETS[0]}/release/cmdmx-daemon" "$ENGINE/target/${TARGETS[1]}/release/cmdmx-daemon"

UNIVERSAL=1 scripts/bundle-app.sh release "$VERSION" "$BUILD"
APP=.build/ColourmotionDMX.app
cp "$DAEMON" "$APP/Contents/MacOS/cmdmx-daemon"
mkdir -p "$APP/Contents/Resources/qlcplus"
cp -R "$VENDOR/fixtures" "$VENDOR/LICENSE" "$VENDOR/NOTICE" "$VENDOR/COMMIT" "$APP/Contents/Resources/qlcplus/"
# REL-05: daemon first, then the app.
codesign --force --sign - "$APP/Contents/MacOS/cmdmx-daemon"
codesign --force --sign - "$APP"
codesign --verify --deep --strict "$APP"

STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG=".build/ColourmotionDMX-$VERSION.dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "ColourmotionDMX $VERSION" -srcfolder "$STAGE" -format UDZO "$DMG"
echo "Built $DMG"
