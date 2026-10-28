#!/usr/bin/env bash
# REL-07: build the release DMG (universal app + daemon, QLC+ fixtures).
# Signing (REL-05, 17): ad hoc, or Developer ID when CMDMX_SIGN_IDENTITY is set (hardened runtime).
# Notarising (REL-19): also needs CMDMX_NOTARY_KEY (.p8 path), CMDMX_NOTARY_KEY_ID, CMDMX_NOTARY_ISSUER;
# the app is notarised and stapled, then the DMG built from it is notarised and stapled too.
# Usage: scripts/make-dmg.sh <version> <build>   -> .build/CMDMX-<version>.dmg
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
mkdir -p .build
DAEMON=.build/cmdmx-daemon-universal
lipo -create -output "$DAEMON" "$ENGINE/target/${TARGETS[0]}/release/cmdmx-daemon" "$ENGINE/target/${TARGETS[1]}/release/cmdmx-daemon"

UNIVERSAL=1 scripts/bundle-app.sh release "$VERSION" "$BUILD"
APP=.build/CMDMX.app
cp "$DAEMON" "$APP/Contents/MacOS/cmdmx-daemon"
mkdir -p "$APP/Contents/Resources/qlcplus"
cp -R "$VENDOR/fixtures" "$VENDOR/LICENSE" "$VENDOR/NOTICE" "$VENDOR/COMMIT" "$APP/Contents/Resources/qlcplus/"
IDENTITY="${CMDMX_SIGN_IDENTITY:-}"
NOTARISE=
[ -n "$IDENTITY" ] && [ -n "${CMDMX_NOTARY_KEY:-}" ] && [ -n "${CMDMX_NOTARY_KEY_ID:-}" ] && [ -n "${CMDMX_NOTARY_ISSUER:-}" ] && NOTARISE=1
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

# REL-19: submit a file to Apple and wait; anything but Accepted prints the log command and fails.
notarise() {
  local auth=(--key "$CMDMX_NOTARY_KEY" --key-id "$CMDMX_NOTARY_KEY_ID" --issuer "$CMDMX_NOTARY_ISSUER")
  local out id status
  out=$(xcrun notarytool submit "$1" --wait --output-format json "${auth[@]}") || true
  id=$(printf '%s' "$out" | plutil -extract id raw -o - - 2>/dev/null || true)
  status=$(printf '%s' "$out" | plutil -extract status raw -o - - 2>/dev/null || true)
  echo "make-dmg: notarisation of $(basename "$1"): ${status:-no answer} ($id)"
  [ "$status" = Accepted ] || { echo "make-dmg: not accepted; see: xcrun notarytool log $id --key $CMDMX_NOTARY_KEY --key-id $CMDMX_NOTARY_KEY_ID --issuer $CMDMX_NOTARY_ISSUER" >&2; exit 1; }
}

if [ -z "$IDENTITY" ]; then
  # REL-05: daemon first, then the app.
  codesign --force --sign - "$APP/Contents/MacOS/cmdmx-daemon"
  codesign --force --sign - "$APP"
else
  # REL-17: Developer ID, hardened runtime, no entitlements, no --deep.
  for f in "$APP/Contents/MacOS/cmdmx-daemon" "$APP"; do
    codesign --force --timestamp --options runtime --sign "$IDENTITY" "$f"
  done
fi
codesign --verify --deep --strict "$APP"
if [ -n "$NOTARISE" ]; then
  ditto -c -k --keepParent "$APP" "$STAGE/CMDMX.zip"
  notarise "$STAGE/CMDMX.zip"
  rm "$STAGE/CMDMX.zip"
  xcrun stapler staple "$APP"
fi

cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
DMG=".build/CMDMX-$VERSION.dmg"
rm -f "$DMG"
hdiutil create -quiet -volname "CMDMX $VERSION" -srcfolder "$STAGE" -format UDZO "$DMG"
if [ -n "$IDENTITY" ]; then
  codesign --timestamp --sign "$IDENTITY" "$DMG"
  codesign --verify "$DMG"
fi
if [ -n "$NOTARISE" ]; then
  notarise "$DMG"
  xcrun stapler staple "$DMG"
fi
echo "Built $DMG"
