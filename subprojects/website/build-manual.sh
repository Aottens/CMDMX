#!/usr/bin/env bash
# Builds the manual (WEB-23, 30..34) into <out dir>; used by build.sh (site) and make-dmg.sh (REL-29).
# Usage: build-manual.sh <out dir> <version text> [--release] [--bundle]
#   --release  drops the preview banner (the manual comes from a release tag).
#   --bundle   stands alone inside the app: own brand/favicon.svg, the mark links the website.
# Needs mdbook. In the site build `../brand` must already exist next to <out dir>.
set -euo pipefail
here=$(cd "$(dirname "$0")" && pwd)
out=$1 mv_text=$2; shift 2
release=0 bundle=0
for a in "$@"; do
  case $a in --release) release=1 ;; --bundle) bundle=1 ;; *) echo "build-manual: unknown option $a" >&2; exit 1 ;; esac
done
# mdbook resolves a relative -d against the book, so make it absolute (its parent may not exist yet).
mkdir -p "$(dirname "$out")"; out=$(cd "$(dirname "$out")" && pwd)/$(basename "$out")

mtmp=$(mktemp -d); trap 'rm -rf "$mtmp"' EXIT
# Built from a copy so the banner and version can be filled in.
m=$mtmp/manual
cp -R "$here/manual" "$m"; mkdir -p "$m/theme"
cp "$here/brand/favicon.svg" "$m/theme/favicon.svg"
[ $release = 1 ] && sed -i.bak '/<!-- preview -->/,/<!-- \/preview -->/d' "$m/theme/header.hbs"
sed -i.bak -E '/<!-- \/?preview -->/d' "$m/theme/header.hbs"
if [ $bundle = 1 ]; then
  sed -i.bak -e 's#{{ path_to_root }}\.\./brand/#{{ path_to_root }}brand/#g' \
    -e 's#{{ path_to_root }}\.\./"#https://aottens.github.io/CMDMX/"#g' "$m/theme/header.hbs"
fi
for f in "$m"/src/*.md; do
  MV=$mv_text awk '{ while ((i = index($0, "{{MANUAL_VERSION}}")) > 0) $0 = substr($0, 1, i-1) ENVIRON["MV"] substr($0, i+18); print }' "$f" > "$f.new" && mv "$f.new" "$f"
done
if grep -n '{{' "$m"/src/*.md >&2; then echo "site: unresolved {{ placeholder in the manual" >&2; exit 1; fi
rm -f "$m/theme/header.hbs.bak"
mdbook build -d "$out" "$m" >/dev/null
rm -f "$out/404.html"   # root-absolute links; Pages serves its own 404
if [ $bundle = 1 ]; then mkdir -p "$out/brand"; cp "$here/brand/favicon.svg" "$out/brand/favicon.svg"; fi

# WEB-34: every local href/src in the manual resolves to a file next to it.
bad=0
while IFS= read -r page; do
  dir=$(dirname "$page")
  while IFS= read -r link; do
    case $link in http:*|https:*|mailto:*|\#*|data:*|javascript:*|"") continue ;; esac
    link=${link%%#*}; link=${link%%\?*}; [ -n "$link" ] || continue
    case $link in /*) echo "site: root-absolute link '$link' in ${page#"$out"/}" >&2; bad=1; continue ;; esac
    t=$dir/$link
    if [ -d "$t" ]; then t=$t/index.html; fi
    [ -e "$t" ] || { echo "site: broken link '$link' in ${page#"$out"/}" >&2; bad=1; }
  done < <(grep -oE '(href|src)="[^"]*"' "$page" | sed -E 's/^[a-z]+="//; s/"$//')
done < <(find "$out" -name '*.html')
[ $bad = 0 ] || exit 1
echo "site: manual ok"
