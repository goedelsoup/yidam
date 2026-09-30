#!/usr/bin/env bash
# File the staged upgrade notes under the tag being cut.
#
#   scripts/file-upgrade-notes.sh <tag>
#   mise run file-upgrade-notes cli/v0.17.0
#
# A note is written as its own file under `changes/upgrading/`, one `### ` note per file, while
# the version it ships in is still unknown. This moves every one of them into
# `docs/upgrading.md` under `## <tag>` and deletes the files, staging both. The commit is left to
# the person cutting the release, and `release.sh` refuses the tag while any note is unfiled.
#
# Why files and not a section of the document: every pull request that wrote a note used to
# insert it at the same line — the top of `## Unreleased` — and two insertions at one line are a
# merge conflict. Separate files cannot collide. It also takes the choice of heading away from
# the author, which is how three notes came to be filed under a tag that had already shipped
# (#772).
#
# Order is newest first, by the issue number each file name starts with. Filing into a section
# that already exists — a note merged after the first filing but before the tag — puts the new
# notes at the top of it.
#
# Every problem is reported before anything is written, so a refusal leaves the tree untouched.

set -euo pipefail

TAG="${1:-}"
if [ -z "$TAG" ] || [ "$#" -ne 1 ]; then
  printf 'usage: scripts/file-upgrade-notes.sh <tag>   (e.g. cli/v0.17.0)\n' >&2
  exit 2
fi

cd "$(git rev-parse --show-toplevel)"

DOC="docs/upgrading.md"
DIR="changes/upgrading"
# The line new sections go under. A marker rather than "before the first `## `", because the
# document opens with `## ` sections of its own that are not releases.
ANCHOR='<!-- file-upgrade-notes: new release sections go below this line -->'

problems=()

case "$TAG" in
  *v[0-9]*.[0-9]*.[0-9]*) ;;
  *) problems+=("'$TAG' does not name a release tag (expected something like cli/v0.17.0)") ;;
esac

[ -f "$DOC" ] || problems+=("$DOC is missing")
if [ -f "$DOC" ] && ! grep -qxF "$ANCHOR" "$DOC"; then
  problems+=("$DOC has no line reading: $ANCHOR")
fi

notes=()
if [ -d "$DIR" ]; then
  while IFS= read -r name; do
    [ -n "$name" ] && notes+=("$name")
  done < <(find "$DIR" -maxdepth 1 -type f -name '*.md' ! -name README.md -exec basename {} \; \
             | sort -t- -k1,1nr -k2)
fi

for name in ${notes[@]+"${notes[@]}"}; do
  file="$DIR/$name"
  if ! [[ "$name" =~ ^[0-9]+-[a-z0-9-]+\.md$ ]]; then
    problems+=("$file: the name must be <issue>-<slug>.md; the issue number orders the notes")
  fi
  first=$(awk 'NF { print; exit }' "$file")
  if [[ "$first" != "### "* ]]; then
    problems+=("$file: the first line must be the note's '### ' heading")
  fi
  # A `#` or `##` heading would end the release section early: release.yml reads up to the next
  # `## `, so everything after it would reach no release.
  outer=$(awk '/^```/ { fence = !fence; next } !fence && /^##? / { print FNR; exit }' "$file")
  if [ -n "$outer" ]; then
    problems+=("$file:$outer: a '#' or '##' heading would split the release section")
  fi
done

if [ "${#problems[@]}" -gt 0 ]; then
  printf 'refused: %s\n' "${problems[@]}" >&2
  exit 1
fi

if [ "${#notes[@]}" -eq 0 ]; then
  printf 'nothing to file: %s holds no notes\n' "$DIR" >&2
  exit 0
fi

block=$(mktemp)
trap 'rm -f "$block" "$block.doc"' EXIT
for name in "${notes[@]}"; do
  printf '\n' >> "$block"
  # Trailing blank lines dropped, so the spacing between notes is this loop's and not the files'.
  awk '{ lines[NR] = $0 } NF { last = NR } END { for (i = 1; i <= last; i++) print lines[i] }' \
    "$DIR/$name" >> "$block"
done

heading="## $TAG"
if grep -qxF "$heading" "$DOC"; then
  # Into the existing section, above what is already filed there.
  awk -v heading="$heading" -v block="$block" '
    { print }
    $0 == heading { while ((getline line < block) > 0) print line }
  ' "$DOC" > "$block.doc"
else
  awk -v anchor="$ANCHOR" -v heading="$heading" -v block="$block" '
    { print }
    $0 == anchor { print ""; print heading; while ((getline line < block) > 0) print line }
  ' "$DOC" > "$block.doc"
fi
cat "$block.doc" > "$DOC"

for name in "${notes[@]}"; do
  rm -f "$DIR/$name"
done
git add -A -- "$DOC" "$DIR"

printf 'filed %d note(s) under %s in %s; review and commit it before cutting the tag\n' \
  "${#notes[@]}" "$heading" "$DOC" >&2
