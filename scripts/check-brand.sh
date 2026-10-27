#!/usr/bin/env bash
# BRD-01: no living file names the product by its old name.
# Not living (dated records keep the name they were written with): tests/*/runs/, DECISIONS.md,
# docs/research/, and IDEAS.md below its "## Ideas" heading. A line that keeps the old name on
# purpose carries the marker "brand: old-name".
# Usage: scripts/check-brand.sh   (from anywhere in the repo; CI runs it in the engine job)
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
OLD='colourmotiondmx'

hits=$(git grep --untracked -n -I -i "$OLD" -- . \
  ':(exclude)tests/*/runs/**' ':(exclude)DECISIONS.md' ':(exclude)docs/research/**' \
  ':(exclude)IDEAS.md' ':(exclude)scripts/check-brand.sh' | grep -v 'brand: old-name' || true)
# IDEAS.md: only the part above "## Ideas" is living.
ideas=$(awk -v old="$OLD" '/^## Ideas/{exit} tolower($0) ~ old && !/brand: old-name/ {print "IDEAS.md:" NR ":" $0}' IDEAS.md)
# File names count too (Sources/<target>/, the TouchOSC layout).
names=$(git ls-files | grep -i "$OLD" | grep -v -e '^tests/[^/]*/runs/' -e '^docs/research/' | sed 's/^/file name: /' || true)
hits=$(printf '%s\n%s\n%s\n' "$hits" "$ideas" "$names" | sed '/^$/d')

if [ -n "$hits" ]; then
  echo "check-brand: the old name in living files (BRD-01; say CMDMX, or mark the line 'brand: old-name'):"
  echo "$hits"
  exit 1
fi
echo "check-brand: ok"
