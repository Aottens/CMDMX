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

err=$(mktemp); mtmp=$(mktemp -d); trap 'rm -rf "$err" "$mtmp"' EXIT
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

# The manual (WEB-23, 30..34): built from a copy so the banner and version can be filled in.
# MANUAL_FROM_TAG=1 (set by website.yml when manual/ comes from the release tag) drops the preview banner.
m=$mtmp/manual
cp -R "$here/manual" "$m"; mkdir -p "$m/theme"
cp "$here/brand/favicon.svg" "$m/theme/favicon.svg"
if [ "${MANUAL_FROM_TAG:-}" = 1 ]; then
  sed -i.bak '/<!-- preview -->/,/<!-- \/preview -->/d' "$m/theme/header.hbs"
  [ -n "${VERSION:-}" ] && mv_text="CMDMX $VERSION" || mv_text="the first release of CMDMX (preview)"
elif [ -n "${VERSION:-}" ]; then
  mv_text="the next release of CMDMX (preview; the latest release is CMDMX $VERSION)"
else
  mv_text="the first release of CMDMX (preview)"
fi
sed -i.bak -E '/<!-- \/?preview -->/d' "$m/theme/header.hbs"
for f in "$m"/src/*.md; do
  MV=$mv_text awk '{ while ((i = index($0, "{{MANUAL_VERSION}}")) > 0) $0 = substr($0, 1, i-1) ENVIRON["MV"] substr($0, i+18); print }' "$f" > "$f.new" && mv "$f.new" "$f"
done
if grep -n '{{' "$m"/src/*.md >&2; then echo "site: unresolved {{ placeholder in the manual" >&2; exit 1; fi
rm -f "$m/theme/header.hbs.bak"
mdbook build -d "$site/manual" "$m" >/dev/null
rm -f "$site/manual/404.html"   # root-absolute links; Pages serves its own 404

# WEB-34: every local href/src in the manual resolves to a file in the site.
bad=0
while IFS= read -r page; do
  dir=$(dirname "$page")
  while IFS= read -r link; do
    case $link in http:*|https:*|mailto:*|\#*|data:*|javascript:*|"") continue ;; esac
    link=${link%%#*}; link=${link%%\?*}; [ -n "$link" ] || continue
    case $link in /*) echo "site: root-absolute link '$link' in ${page#"$site"/}" >&2; bad=1; continue ;; esac
    t=$dir/$link
    if [ -d "$t" ]; then t=$t/index.html; fi
    [ -e "$t" ] || { echo "site: broken link '$link' in ${page#"$site"/}" >&2; bad=1; }
  done < <(grep -oE '(href|src)="[^"]*"' "$page" | sed -E 's/^[a-z]+="//; s/"$//')
done < <(find "$site/manual" -name '*.html')
[ $bad = 0 ] || exit 1
echo "site: manual ok"
