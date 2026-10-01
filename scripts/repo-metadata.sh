#!/usr/bin/env bash
# Make GitHub's About box say what `.github/repository.json` says.
#
#   scripts/repo-metadata.sh [--check]
#   mise run repo-metadata [--check]
#
# The description, homepage and topics are how a repository is found by someone who does not
# already have its link. Every distribution channel here was built with a release gate, and these
# three fields were not artifacts, so no gate ever asked for them: the repository had a published
# docs site, an npm scope, a Homebrew tap and a plugin marketplace, and no topics and no homepage
# (#949). Holding them in a file makes a change to them a diff someone reviews; this applies it.
#
# `--check` changes nothing and exits nonzero when the live repository differs from the file.
# Without it, the file is applied: description and homepage set, missing topics added, and topics
# the file does not name removed, so the file is the whole list rather than a floor.
#
# It needs `gh` authenticated with admin on the repository to apply, and read access to check.

set -euo pipefail

root=$(cd "$(dirname "$0")/.." && pwd)
file="$root/.github/repository.json"
repo=goedelsoup/yidam

check=0
case "${1:-}" in
  "") ;;
  --check) check=1 ;;
  *) echo "usage: $0 [--check]" >&2; exit 2 ;;
esac

want_description=$(jq -r '.description' "$file")
want_homepage=$(jq -r '.homepage' "$file")
want_topics=$(jq -r '.topics[]' "$file" | sort)

live=$(gh repo view "$repo" --json description,homepageUrl,repositoryTopics)
have_description=$(jq -r '.description // ""' <<<"$live")
have_homepage=$(jq -r '.homepageUrl // ""' <<<"$live")
have_topics=$(jq -r '(.repositoryTopics // [])[].name' <<<"$live" | sort)

missing=$(comm -23 <(echo "$want_topics") <(echo "$have_topics") | sed '/^$/d')
extra=$(comm -13 <(echo "$want_topics") <(echo "$have_topics") | sed '/^$/d')

drift=0
if [ "$want_description" != "$have_description" ]; then
  echo "description: \"$have_description\" -> \"$want_description\""
  drift=1
fi
if [ "$want_homepage" != "$have_homepage" ]; then
  echo "homepage: \"$have_homepage\" -> \"$want_homepage\""
  drift=1
fi
for t in $missing; do echo "topic +$t"; drift=1; done
for t in $extra; do echo "topic -$t"; drift=1; done

if [ "$drift" = 0 ]; then
  echo "$repo matches .github/repository.json"
  exit 0
fi

if [ "$check" = 1 ]; then
  echo "$repo differs from .github/repository.json; run \`mise run repo-metadata\` to apply it" >&2
  exit 1
fi

args=(--description "$want_description" --homepage "$want_homepage")
for t in $missing; do args+=(--add-topic "$t"); done
for t in $extra; do args+=(--remove-topic "$t"); done
gh repo edit "$repo" "${args[@]}"
echo "applied .github/repository.json to $repo"
