#!/usr/bin/env bash
# Run `yidam bench` against `examples/streamflow` with a real vector index.
#
# The example is copied to a scratch directory and `git init`-ed rather than run in place,
# for the reason `example_corpus.rs` gives: `repo_root()` resolves through
# `git rev-parse --show-toplevel`, so a binary run inside `examples/streamflow/` finds
# *this* repository, which has no `.yidam/` and would fail for an unrelated reason.
#
# `embed` and `index-build` are not optional here. `bench` refuses to measure when
# retrieval would be keyword search, which is the whole point of it — so a run without an
# index is a failed job rather than a degraded number.
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
bin="$root/yidam/cli/target/debug/yidam"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

git -C "$root" ls-files examples/streamflow | while read -r tracked; do
  dest="$work/${tracked#examples/streamflow/}"
  mkdir -p "$(dirname "$dest")"
  cp "$root/$tracked" "$dest"
done

cd "$work"
git init -q
git config user.email bench@yidam.test
git config user.name Bench
git add -A
git commit -qm "chore: genesis — bench"

"$bin" embed
"$bin" index-build
"$bin" bench

# The anchored query, against a real index (#263).
#
# This is the only place the vector anchor is exercised at all. PR CI never compiles
# `--features index`, and every test and golden fixture in the repository runs on the keyword
# fallback — so an anchor that resolved to nothing, or resolved through a path the corpus
# loader spells differently from the one `embed` records, would ship green through all of
# them. The two checks below are the two ways that can be wrong: it degraded, or it landed
# nowhere.
echo
echo "--- anchored query, with the index built above ---"
anchored="$("$bin" query 'concept~"rapid sub-daily variation below a dam" <-exhibits- reach')"
echo "$anchored"
case "$anchored" in
  *"anchored on nothing"*)
    echo "the anchor resolved to no entry node against an indexed corpus" >&2
    exit 1
    ;;
esac
case "$anchored" in
  *"— semantic search"*) ;;
  *)
    echo "the anchor did not use the index it was just handed — see the reason above" >&2
    exit 1
    ;;
esac
