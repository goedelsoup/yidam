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
