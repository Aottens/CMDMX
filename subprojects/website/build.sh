#!/usr/bin/env bash
# Builds _site/ next to this script (WEB-13). Needs gh (logged in or GH_TOKEN) and jq.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=${CMDMX_ROOT:-$here/../..}
site=$here/_site
rm -rf "$site"; mkdir -p "$site/screenshots"
cp "$here/index.html" "$here/style.css" "$site/"
cp -R "$here/brand" "$site/brand"
cp "$root"/docs/screenshots/*.png "$site/screenshots/"
if [ -f "$here/CNAME" ]; then cp "$here/CNAME" "$site/CNAME"; fi

err=$(mktemp); trap 'rm -f "$err"' EXIT
if json=$(gh release view --json tagName,publishedAt,assets,url 2>"$err"); then
  dmg=$(jq -c '[.assets[] | select(.name | endswith(".dmg"))][0] // empty' <<<"$json")
  [ -n "$dmg" ] || { echo "site: release has no .dmg asset" >&2; exit 1; }
  sha=$(jq -r '(.digest // "") | sub("^sha256:";"")' <<<"$dmg")
  [ -n "$sha" ] || { echo "site: .dmg asset has no digest" >&2; exit 1; }
  VERSION=$(jq -r '.tagName | ltrimstr("v")' <<<"$json")
  DATE=$(jq -r '.publishedAt[0:10]' <<<"$json")
  SIZE=$(jq -r '(.size / 1048576 * 10 | round / 10 | tostring) + " MB"' <<<"$dmg")
  DMG_URL=$(jq -r .url <<<"$dmg")
  NOTES_URL=$(jq -r .url <<<"$json")
  SHA256=$sha
  export VERSION DATE SIZE SHA256 DMG_URL NOTES_URL
  keep=release drop=norelease label="v$VERSION"
elif grep -q "release not found" "$err"; then
  keep=norelease drop=release label="no release"
else
  cat "$err" >&2; exit 1
fi
YEAR=$(date +%Y); export YEAR

# awk: drop the unused block, strip marker lines, fill the known {{X}} from the environment
# (values are never used as regex); any other {{X}} stays and fails the check below.
awk -v keep="$keep" -v drop="$drop" -v known=" VERSION DATE SIZE SHA256 DMG_URL NOTES_URL YEAR " '
  index($0, "<!-- " drop " -->") {skip=1; next}
  index($0, "<!-- /" drop " -->") {skip=0; next}
  index($0, "<!-- " keep " -->") || index($0, "<!-- /" keep " -->") {next}
  skip {next}
  {
    out=""; s=$0
    while (match(s, /\{\{[A-Z0-9_]+\}\}/)) {
      k=substr(s, RSTART+2, RLENGTH-4)
      v=(index(known, " " k " ") && k in ENVIRON) ? ENVIRON[k] : substr(s, RSTART, RLENGTH)
      out=out substr(s, 1, RSTART-1) v
      s=substr(s, RSTART+RLENGTH)
    }
    print out s
  }' "$here/index.html" > "$site/index.html"

if grep -q '{{' "$site/index.html"; then
  echo "site: unresolved {{ placeholder in index.html" >&2; grep -n '{{' "$site/index.html" >&2; exit 1
fi
echo "site: _site ($label)"
