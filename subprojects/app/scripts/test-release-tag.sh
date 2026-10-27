#!/usr/bin/env bash
# REL-01: release-version.sh turns a v<MAJOR>.<MINOR>.<PATCH> tag into the version, refuses the rest.
# Usage: scripts/test-release-tag.sh   (no macOS needed; CI runs it in the engine job)
set -uo pipefail
cd "$(dirname "$0")"
fail=0
ok() {
  local got
  if ! got=$(./release-version.sh "$1" 2>/dev/null) || [ "$got" != "$2" ]; then
    echo "FAIL: '$1' should give '$2', got '${got:-<error>}'"; fail=1
  fi
}
bad() {
  local err
  if err=$(./release-version.sh "$1" 2>&1 >/dev/null); then
    echo "FAIL: '$1' should be refused"; fail=1
  elif [[ "$err" != *"$1"* ]]; then
    echo "FAIL: the refusal of '$1' does not name the tag: $err"; fail=1
  fi
}
ok v0.1.0 0.1.0
ok v12.34.567 12.34.567
for t in v1 v0.2 v0.2.0-rc1 0.2.0 vx.y.z v0.2.0.1 "v0.2. 0" ""; do bad "$t"; done
[ $fail = 0 ] && echo "release tags: ok"
exit $fail
