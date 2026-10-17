#!/usr/bin/env bash
# Fetch the QLC+ fixture library at a pinned commit into vendor/qlcplus (LIB-13).
# The files are Apache-2.0 (QLC+ project); the licence and notice are copied alongside.
# Usage: scripts/fetch-qlcplus.sh            (re-run to refresh; never needed at runtime)
set -euo pipefail
cd "$(dirname "$0")/.."
COMMIT=3d832cec0042ad937d0da0346e7b953f5952cec7
DEST=vendor/qlcplus
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
curl -sfL "https://codeload.github.com/mcallegari/qlcplus/tar.gz/$COMMIT" -o "$TMP/qlcplus.tar.gz"
mkdir "$TMP/x"
tar -xzf "$TMP/qlcplus.tar.gz" -C "$TMP/x" --strip-components=1 \
  "qlcplus-$COMMIT/resources/fixtures" "qlcplus-$COMMIT/COPYING"
rm -rf "$DEST"
mkdir -p "$DEST"
cp -R "$TMP/x/resources/fixtures/." "$DEST/fixtures/"
cp "$TMP/x/COPYING" "$DEST/LICENSE"
cat > "$DEST/NOTICE" <<NOTICE
Fixture definitions from Q Light Controller Plus (https://github.com/mcallegari/qlcplus),
commit $COMMIT, licensed under the Apache License 2.0 (see LICENSE).
Authors are named in each file's <Creator> element.
NOTICE
echo "$COMMIT" > "$DEST/COMMIT"
echo "QLC+ fixtures at $COMMIT: $(find "$DEST/fixtures" -name '*.qxf' | wc -l | tr -d ' ') files in $DEST"
