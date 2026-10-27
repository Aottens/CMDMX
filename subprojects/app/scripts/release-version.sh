#!/usr/bin/env bash
# REL-01: print the version of a release tag v<MAJOR>.<MINOR>.<PATCH>; refuse anything else.
# Usage: scripts/release-version.sh <tag>
tag="${1-}"
if [[ "$tag" =~ ^v([0-9]+\.[0-9]+\.[0-9]+)$ ]]; then
  echo "${BASH_REMATCH[1]}"
else
  echo "release-version: tag '$tag' is not v<MAJOR>.<MINOR>.<PATCH> (digits only)" >&2
  exit 1
fi
