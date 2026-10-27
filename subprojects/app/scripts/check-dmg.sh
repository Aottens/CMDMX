#!/usr/bin/env bash
# REL-08: check a release DMG (layout REL-06, versions REL-02, universal binaries REL-04,
# signature REL-05, the bundled daemon runs REL-10). Unmounts in every case.
# Usage: scripts/check-dmg.sh <dmg> <version>
set -euo pipefail
DMG="$1" VERSION="$2"
fail() { echo "check-dmg: $*" >&2; exit 1; }
[ -f "$DMG" ] || fail "no file $DMG"
[ "$(basename "$DMG")" = "ColourmotionDMX-$VERSION.dmg" ] || fail "name is $(basename "$DMG")"
MNT=$(mktemp -d)
TMP=$(mktemp -d)
trap 'hdiutil detach -quiet "$MNT" 2>/dev/null || true; rm -rf "$MNT" "$TMP"' EXIT
hdiutil attach -quiet -readonly -nobrowse -mountpoint "$MNT" "$DMG"

[ "$(ls -A "$MNT" | grep -v '^\.' | sort | tr '\n' ' ')" = "Applications ColourmotionDMX.app " ] \
  || fail "DMG holds: $(ls -A "$MNT" | tr '\n' ' ')"
[ "$(readlink "$MNT/Applications")" = "/Applications" ] || fail "Applications is not a link to /Applications"
APP="$MNT/ColourmotionDMX.app"
PLIST="$APP/Contents/Info.plist"
short=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleShortVersionString' "$PLIST")
[ "$short" = "$VERSION" ] || fail "CFBundleShortVersionString is $short"
build=$(/usr/libexec/PlistBuddy -c 'Print :CFBundleVersion' "$PLIST")
[ -n "$build" ] && [ "$build" != dev ] || fail "CFBundleVersion is '$build'"
for bin in ColourmotionDMX cmdmx-daemon; do
  archs=$(lipo -archs "$APP/Contents/MacOS/$bin")
  [[ " $archs " == *" arm64 "* && " $archs " == *" x86_64 "* ]] || fail "$bin is $archs, not universal"
done
codesign --verify --deep --strict "$APP" || fail "codesign --verify failed"
find "$APP/Contents/Resources/qlcplus/fixtures" -name '*.qxf' | grep -q . || fail "no QLC+ fixtures"
for f in LICENSE NOTICE COMMIT; do [ -f "$APP/Contents/Resources/qlcplus/$f" ] || fail "no qlcplus/$f"; done

PORT=$(python3 -c 'import socket; s=socket.socket(); s.bind(("127.0.0.1",0)); print(s.getsockname()[1])')
HOME="$TMP" "$APP/Contents/MacOS/cmdmx-daemon" \
  --qlcplus-dir "$APP/Contents/Resources/qlcplus/fixtures" \
  --no-artnet --no-sacn --no-serial --ipc-port "$PORT" --show-dir "$TMP/Shows" --ticks 20 \
  >"$TMP/daemon.log" 2>&1 || { cat "$TMP/daemon.log" >&2; fail "bundled daemon failed"; }
echo "check-dmg: $(basename "$DMG") ok"
