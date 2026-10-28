#!/usr/bin/env bash
# REL-08: check a release DMG (layout REL-06, versions REL-02, universal binaries REL-04,
# signature REL-05, the bundled daemon runs REL-10, Local Network text REL-18). Unmounts in every case.
# --notarised adds REL-21: stapled tickets, Gatekeeper accepts, hardened runtime, Developer ID.
# Usage: scripts/check-dmg.sh <dmg> <version> [--notarised]
set -euo pipefail
DMG="$1" VERSION="$2" NOTARISED="${3:-}"
[ -z "$NOTARISED" ] || [ "$NOTARISED" = --notarised ] || { echo "check-dmg: unknown option $NOTARISED" >&2; exit 1; }
fail() { echo "check-dmg: $*" >&2; exit 1; }
[ -f "$DMG" ] || fail "no file $DMG"
[ "$(basename "$DMG")" = "CMDMX-$VERSION.dmg" ] || fail "name is $(basename "$DMG")"
MNT=$(mktemp -d)
TMP=$(mktemp -d)
trap 'hdiutil detach -quiet "$MNT" 2>/dev/null || true; rm -rf "$MNT" "$TMP"' EXIT
hdiutil attach -quiet -readonly -nobrowse -mountpoint "$MNT" "$DMG"

[ "$(ls -A "$MNT" | grep -v '^\.' | sort | tr '\n' ' ')" = "Applications CMDMX.app " ] \
  || fail "DMG holds: $(ls -A "$MNT" | tr '\n' ' ')"
[ "$(readlink "$MNT/Applications")" = "/Applications" ] || fail "Applications is not a link to /Applications"
APP="$MNT/CMDMX.app"
PLIST="$APP/Contents/Info.plist"
short=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$PLIST")
[ "$short" = "$VERSION" ] || fail "CFBundleShortVersionString is $short"
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$PLIST")
[ -n "$build" ] && [ "$build" != dev ] || fail "CFBundleVersion is '$build'"
/usr/libexec/PlistBuddy -c 'Print :NSLocalNetworkUsageDescription' "$PLIST" >/dev/null 2>&1 \
  || fail "Info.plist has no NSLocalNetworkUsageDescription (REL-18)"
for bin in CMDMX cmdmx-daemon; do
  archs=$(lipo -archs "$APP/Contents/MacOS/$bin")
  [[ " $archs " == *" arm64 "* && " $archs " == *" x86_64 "* ]] || fail "$bin is $archs, not universal"
done
codesign --verify --deep --strict "$APP" || fail "codesign --verify failed"
[ -n "$(find "$APP/Contents/Resources/qlcplus/fixtures" -name '*.qxf' -print -quit)" ] || fail "no QLC+ fixtures"
for f in LICENSE NOTICE COMMIT; do [ -f "$APP/Contents/Resources/qlcplus/$f" ] || fail "no qlcplus/$f"; done

PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
HOME="$TMP" "$APP/Contents/MacOS/cmdmx-daemon" \
  --qlcplus-dir "$APP/Contents/Resources/qlcplus/fixtures" \
  --no-artnet --no-sacn --no-serial --ipc-port "$PORT" --show-dir "$TMP/Shows" --ticks 20 \
  >"$TMP/daemon.log" 2>&1 || { cat "$TMP/daemon.log" >&2; fail "bundled daemon failed"; }

if [ "$NOTARISED" = --notarised ]; then
  xcrun stapler validate "$DMG" >/dev/null || fail "no stapled ticket on the DMG (REL-21)"
  xcrun stapler validate "$APP" >/dev/null || fail "no stapled ticket on CMDMX.app (REL-21)"
  out=$(spctl --assess --type open --context context:primary-signature -v "$DMG" 2>&1) || fail "spctl rejects the DMG: $out"
  [[ "$out" == *accepted* && "$out" == *"source=Notarized Developer ID"* ]] || fail "spctl on the DMG: $out"
  out=$(spctl --assess --type execute -v "$APP" 2>&1) || fail "spctl rejects CMDMX.app: $out"
  [[ "$out" == *accepted* && "$out" == *"source=Notarized Developer ID"* ]] || fail "spctl on CMDMX.app: $out"
  for bin in CMDMX cmdmx-daemon; do
    info=$(codesign -dv "$APP/Contents/MacOS/$bin" 2>&1)
    [[ "$info" == *"flags="*runtime* ]] || fail "$bin has no hardened runtime (REL-18)"
    [[ "$info" == *"Authority=Developer ID Application"* ]] || fail "$bin is not signed with a Developer ID (REL-17)"
  done
fi
echo "check-dmg: $(basename "$DMG") ok${NOTARISED:+ (notarised)}"
