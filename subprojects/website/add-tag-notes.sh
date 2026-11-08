#!/usr/bin/env bash
# WEB-37: add a release-notes entry (from the annotated tag message) for the latest released tag and
# every older v* tag until one the notes already have. Usage: add-tag-notes.sh <notes file> <latest tag>
set -euo pipefail
export LC_ALL=C
notes=$1 latest=$2
new=$(mktemp); trap 'rm -f "$new" "$new.out"' EXIT
seen=0
for tag in $(git tag -l 'v*' --sort=-v:refname); do
  [ "$seen" = 1 ] || { [ "$tag" = "$latest" ] && seen=1 || continue; }
  v=${tag#v}
  grep -qxF "## $v" "$notes" && break
  date=$(git for-each-ref --format='%(creatordate:format:%e %B %Y)' "refs/tags/$tag" | sed 's/^ *//')
  msg=$(git for-each-ref --format='%(contents)' "refs/tags/$tag" | sed "1s/^CMDMX $v: //;1s/^v$v: //")
  printf '## %s\n\nPublished %s as "CMDMX %s".\n[Release on GitHub](https://github.com/Aottens/CMDMX/releases/tag/%s)\n\n%s\n\n' \
    "$v" "$date" "$v" "$tag" "$msg" >> "$new"
done
[ -s "$new" ] || exit 0
{ head -2 "$notes"; cat "$new"; tail -n +3 "$notes"; } > "$new.out"
cat "$new.out" > "$notes"
