#!/usr/bin/env bash
# Installs the pinned mdBook (v0.5.4; Linux x86_64, macOS arm64 or x86_64) for CI (WEB-30) and the release job (REL-29).
# Locally: `brew install mdbook` (same version).
set -euo pipefail
v=v0.5.4
case "$(uname -s)/$(uname -m)" in
  Linux/x86_64) t=x86_64-unknown-linux-gnu sha=3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7 ;;
  Darwin/arm64) t=aarch64-apple-darwin sha=03e8a6d8b13a2971e0b3280affd03b388373c1485e26f73407c3a76b0b1838df ;;
  Darwin/x86_64) t=x86_64-apple-darwin sha=a47d7bf0d5d670cff9ee6cce95537cbeb62dc10704d9e7131ffbd13e2b59a5de ;;
  *) echo "install-mdbook: no pinned build for $(uname -s)/$(uname -m)" >&2; exit 1 ;;
esac
dir=${RUNNER_TEMP:-/tmp}/mdbook
mkdir -p "$dir"
curl -fsSL -o "$dir/mdbook.tar.gz" "https://github.com/rust-lang/mdBook/releases/download/$v/mdbook-$v-$t.tar.gz"
echo "$sha  $dir/mdbook.tar.gz" | shasum -a 256 -c -
tar -xzf "$dir/mdbook.tar.gz" -C "$dir"
rm "$dir/mdbook.tar.gz"
if [ -n "${GITHUB_PATH:-}" ]; then echo "$dir" >> "$GITHUB_PATH"; fi
