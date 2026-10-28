#!/usr/bin/env bash
# WEB-02, 03, 11, 13, 20, 21: runs build.sh against a fake `gh` and checks _site/.
# Usage: subprojects/website/test-build.sh   (needs jq; CI runs it in the website job)
set -uo pipefail
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT
fail=0
no() { echo "FAIL: $*"; fail=1; }

# A fake gh: prints $FAKE_GH_JSON, or fails with $FAKE_GH_ERR like the real one.
mkdir -p "$tmp/bin"
cat > "$tmp/bin/gh" <<'EOF'
#!/usr/bin/env bash
[ "$1 $2" = "release view" ] || { echo "fake gh: unexpected '$*'" >&2; exit 2; }
if [ -n "${FAKE_GH_ERR:-}" ]; then echo "$FAKE_GH_ERR" >&2; exit 1; fi
printf '%s\n' "$FAKE_GH_JSON"
EOF
chmod +x "$tmp/bin/gh"

release='{"assets":[{"name":"CMDMX-0.2.0.dmg","size":23792628,"digest":"sha256:2c45851c41f840560896f0216db23496d604b260486a1e8dd394eaf5d71dfcad","url":"https://github.com/Aottens/CMDMX/releases/download/v0.2.0/CMDMX-0.2.0.dmg"}],"publishedAt":"2026-11-01T20:46:11Z","tagName":"v0.2.0","url":"https://github.com/Aottens/CMDMX/releases/tag/v0.2.0"}'

# build <site copy> [env...]: runs build.sh in a copy of the site with the fake gh first on PATH.
build() {
  local dir=$1; shift
  env PATH="$tmp/bin:$PATH" FAKE_GH_JSON="" FAKE_GH_ERR="" "$@" "$dir/build.sh" > "$tmp/out.log" 2>&1
}
copy() { rm -rf "$tmp/$1"; mkdir -p "$tmp/$1"; cp -R "$here/." "$tmp/$1/"; rm -rf "$tmp/$1/_site"; }
has() { grep -qF -- "$2" "$1" || no "$3: '$2' not in $(basename "$1")"; }

[ -x "$here/build.sh" ] || { echo "FAIL: build.sh missing or not executable"; exit 1; }

# The builds run in copies, so screenshots must come from the repo, not from a path inside the copy.
export CMDMX_ROOT="$root"

# WEB-11, 13: a release fills the download block.
copy a
if build "$tmp/a" FAKE_GH_JSON="$release"; then
  page="$tmp/a/_site/index.html"
  has "$page" "0.2.0" "WEB-11 version"
  has "$page" "2026-11-01" "WEB-11 date"
  has "$page" "22.7 MB" "WEB-11 size"
  has "$page" "2c45851c41f840560896f0216db23496d604b260486a1e8dd394eaf5d71dfcad" "WEB-11 SHA-256"
  has "$page" 'href="https://github.com/Aottens/CMDMX/releases/download/v0.2.0/CMDMX-0.2.0.dmg"' "WEB-11 button"
  has "$page" 'href="https://github.com/Aottens/CMDMX/releases/tag/v0.2.0"' "WEB-11 release notes"
  has "$page" "macOS 14 or later, Apple Silicon and Intel" "WEB-11 requirements"
  has "$page" "Open Anyway" "WEB-12 install steps"
  has "$page" "CMDMX by Colourmotion" "WEB-10 footer"
  has "$page" "$(date +%Y)" "WEB-10 year"
  has "$page" 'href="https://github.com/Aottens/CMDMX/issues"' "WEB-10 issues link"
  has "$page" "all rights reserved" "WEB-14 licence line"
  grep -q "No release yet" "$page" && no "WEB-11: 'No release yet' shown although there is a release"
  grep -q "{{" "$page" && no "WEB-13: '{{' left in index.html"
  # WEB-02: nothing loaded from other hosts; outbound links only to GitHub.
  if grep -oE '(src|href|srcset)="https?://[^"]*"' "$tmp/a/_site"/*.html | grep -vE '="https://github\.com/'; then
    no "WEB-02: a link or resource outside github.com (above)"
  fi
  grep -qiE 'url\(["'\'']?https?:|@import' "$tmp/a/_site"/*.css && no "WEB-02: style.css loads from elsewhere"
  grep -qi '<script' "$page" && no "WEB-03: the landing page has a script"
  # WEB-03: alt on every image, dark/light, phone width, visible focus.
  grep -oE '<img[^>]*>' "$page" | grep -v 'alt="[^"]' && no "WEB-03: an <img> without alt text (above)"
  grep -q '<h1' "$page" || no "WEB-03: no <h1>"
  grep -q 'name="viewport"' "$page" || no "WEB-03: no viewport meta"
  grep -q 'prefers-color-scheme' "$tmp/a/_site/style.css" || no "WEB-03: no dark/light in style.css"
  grep -q 'focus-visible' "$tmp/a/_site/style.css" || no "WEB-03: no visible focus style"
  # Every local file the page names is in the site (lockup, screenshots, favicon).
  for f in $(grep -oE '(src|href|srcset)="[^"#:]+"' "$page" | sed -E 's/^[a-z]+="//; s/"$//; s/ .*//'); do
    [ -e "$tmp/a/_site/$f" ] || no "WEB-10: page links to '$f', which is not in _site"
  done
  grep -q 'src="brand/' "$page" || grep -q 'srcset="brand/' "$page" || no "WEB-10: the lockup does not come from brand/"
  ls "$tmp/a/_site"/*/*.png >/dev/null 2>&1 || no "WEB-10: no screenshots in _site"
  [ -e "$tmp/a/_site/CNAME" ] && no "WEB-20: CNAME in _site without subprojects/website/CNAME"
else
  no "WEB-11: build with a release failed:"; cat "$tmp/out.log"
fi

# WEB-11: no release is not an error.
copy b
if build "$tmp/b" FAKE_GH_ERR="release not found"; then
  has "$tmp/b/_site/index.html" "No release yet" "WEB-11 no release"
  has "$tmp/b/_site/index.html" 'href="https://github.com/Aottens/CMDMX"' "WEB-11 no release links the repo"
  grep -q "{{" "$tmp/b/_site/index.html" && no "WEB-13: '{{' left without a release"
else
  no "WEB-11: build without a release failed:"; cat "$tmp/out.log"
fi

# Any other gh failure (auth, network) fails the build: never publish "No release yet" by mistake.
copy c
build "$tmp/c" FAKE_GH_ERR="HTTP 401: Bad credentials" && no "WEB-13: build passed although gh failed"

# WEB-13: a placeholder build.sh does not know fails the build.
copy d
# HOME is always set: only build.sh's own fields are filled, never any environment variable.
echo '<p>{{HOME}}</p>' >> "$tmp/d/index.html"
build "$tmp/d" FAKE_GH_JSON="$release" && no "WEB-13: build passed with '{{' left in the output"

# WEB-20: the custom domain is one file; when it is there it lands in the site.
copy e
echo "cmdmx.example.org" > "$tmp/e/CNAME"
if build "$tmp/e" FAKE_GH_JSON="$release"; then
  [ "$(cat "$tmp/e/_site/CNAME" 2>/dev/null)" = "cmdmx.example.org" ] || no "WEB-20: CNAME not copied into _site"
else
  no "WEB-20: build with CNAME failed:"; cat "$tmp/out.log"
fi

# WEB-20, 21: Pages from the Actions artifact, least permissions, called after every release.
wf="$root/.github/workflows/website.yml"
if [ -f "$wf" ]; then
  for s in "actions/upload-pages-artifact" "actions/deploy-pages" "workflow_call" "workflow_dispatch" \
           "subprojects/website/**" "pages: write" "id-token: write" "contents: read"; do
    has "$wf" "$s" "WEB-20/21 website.yml"
  done
  grep -q "contents: write" "$wf" && no "WEB-20: website.yml asks for contents: write"
else
  no "WEB-20: .github/workflows/website.yml missing"
fi
has "$root/.github/workflows/release.yml" "uses: ./.github/workflows/website.yml" "WEB-21 release.yml"
has "$root/.github/workflows/ci.yml" "subprojects/website/test-build.sh" "WEB-22 ci.yml"

[ $fail = 0 ] && echo "website build: ok"
exit $fail
