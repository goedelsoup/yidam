#!/bin/sh
# Which release do this tag's notes start from?
#
# Usage: <releases JSON on stdin> previous-release-tag.sh <tag>
#   e.g. gh api "repos/$R/releases?per_page=100" | previous-release-tag.sh cli/v0.9.0
#
# Prints the previous release tag *of the same layer*, or nothing when this is the
# first release of one. Nothing is a legitimate answer and the caller must omit
# `previous_tag_name` rather than send an empty one.
#
# ── why this is not GitHub's job ─────────────────────────────────────────────
#
# `releases/generate-notes` picks a base itself when `previous_tag_name` is
# omitted, and on `cli/v0.9.0` it picked `cli/v0.7.0` — reaching past `cli/v0.8.0`,
# which exists, is a release, is an annotated tag on main, and is an ancestor of
# the tag being cut. The published notes listed 37 pull requests where 16 belonged
# to the release; everything from #497 on had already shipped in 0.8.0 (#555).
#
# The same call answers `cli/v0.8.0` correctly, then and now. So the heuristic is
# not uniformly wrong — it is right for one release and wrong for the next with no
# visible difference between them, which is the whole argument for not asking it.
# Nothing here explains GitHub's choice, and this script does not need to.
#
# ── one namespace, four layers ───────────────────────────────────────────────
#
# `VERSIONING.md` gives every layer its own tag prefix and they interleave by date
# — `sdk/rust/v0.4.0` and `v0.3.0` both sit between `cli/v0.8.0` and `cli/v0.9.0`.
# Any question meaning "the previous release" has to mean "of this layer", which is
# the correction three other call sites in this repository have already had to make.
set -eu

tag=${1:?usage: previous-release-tag.sh <tag>, releases JSON on stdin}

# `cli/v0.9.0` -> `cli/v`; `sdk/rust/v0.4.0` -> `sdk/rust/v`; `v0.3.0` -> `v`.
# Everything up to the first digit, which is where the version starts in all five
# of this repository's prefixes. A layer whose name contained a digit would need a
# different rule; none does, and `every_layer_versioning_md_names_is_one_the_script_can_release`
# is what would notice a new one.
prefix=${tag%%[0-9]*}
[ -n "$prefix" ] || { echo "cannot read a layer prefix from '$tag'" >&2; exit 1; }

# Ordered by version, not by when the release was created.
#
# "Newest-first, take the first row that is not me" was the first version of this
# and it is wrong in a way that passes for `cli/v0.9.0`: it answers correctly only
# while the tag being cut is the newest release of its layer. Asked about
# `cli/v0.8.0` it returned `cli/v0.9.0` — a *later* release — and asked about
# `editor/v0.1.0` it returned `editor/v0.2.0`. Found by running it over every layer
# rather than over the one case in front of me.
#
# The release flow only ever cuts the newest, so that bug had no reachable symptom
# today. It would have one the first time somebody re-cuts an old tag or backfills
# a release, and it would look like the defect this script exists to fix.
#
# So: the greatest version of this layer that is strictly below the tag's own.
# `sort -V` orders versions rather than strings, which is what keeps `0.10.0` above
# `0.9.0`.
version=${tag#"$prefix"}

PREFIX="$prefix" TAG="$tag" jq -r '
  .[] | select(.tag_name | startswith($ENV.PREFIX)) | select(.tag_name != $ENV.TAG)
  | .tag_name
' | sed "s|^$prefix||" | {
    # The layer's released versions, plus this one, in version order. The entry
    # before this one is the answer; nothing before it means this is the layer's
    # first release, and the caller must then omit `previous_tag_name` rather than
    # send an empty string.
    { cat; printf '%s\n' "$version"; } | sort -V -u | awk -v v="$version" '
      $0 == v { print prev; exit }
      { prev = $0 }
    '
  } | sed "s|^\(..*\)$|$prefix\1|"
