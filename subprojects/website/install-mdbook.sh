#!/usr/bin/env bash
# Installs the pinned mdBook (v0.5.4, linux x86_64) for CI (WEB-30). Locally: `brew install mdbook` (same version).
set -euo pipefail
v=v0.5.4
sha=3f28de05dafca9d0f2eab99c662116b0e37b89b1d96a08f8f430b9eeae958cd7
dir=${RUNNER_TEMP:-/tmp}/mdbook
mkdir -p "$dir"
curl -fsSL -o "$dir/mdbook.tar.gz" "https://github.com/rust-lang/mdBook/releases/download/$v/mdbook-$v-x86_64-unknown-linux-gnu.tar.gz"
echo "$sha  $dir/mdbook.tar.gz" | sha256sum -c -
tar -xzf "$dir/mdbook.tar.gz" -C "$dir"
rm "$dir/mdbook.tar.gz"
if [ -n "${GITHUB_PATH:-}" ]; then echo "$dir" >> "$GITHUB_PATH"; fi
