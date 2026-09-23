#!/bin/sh
# disclosure-envelope — what this corpus could publish today, and what it could not.
#
# `guidelines/agent-conduct.md` states where each tier of claim may go:
#
#   [verified] may reach public material; [inference] reaches attributed memos and
#   backgrounders; [open] does not leave the repository.
#
# `travel-tier` computes the tier each node's assertions actually travel at, after the chain
# rule has had its way with the declared tags. This is the question an operator asks with that
# answer in hand and cannot ask without it: if we published this corpus tomorrow, what leaves?
#
# It is the second step of a pipeline and reads nothing but the first step's output. That is
# what makes it a demonstration of `after` rather than a second calculator that happens to be
# declared beside one: run on its own against a corpus where `travel-tier` has not run, its
# declared `reads` resolve to nothing and there is no input to compute over. The manifest says
# `after = ["travel-tier"]`, so the executor lands that commit first and this one reads it.
#
# It reproduces no observation and invents no value. Every number here is a partition of a
# file already committed, which is why it is recomputed rather than written once.

set -eu

# Byte ordering, so the member lists are a property of the corpus and not of the locale the
# run happened to start in.
LC_ALL=C
export LC_ALL

out="$YIDAM_OUT/.yidam/computed"
mkdir -p "$out"

awk '
# ── where each tier may go ───────────────────────────────────────────────────
#
# The three destinations are the guideline'"'"'s, not this script'"'"'s. `unmarked` is grouped with
# `open` for the reason `travel-tier` ranks it below: a node that has not declared its
# evidence standing has not earned a destination, and the floor is the only safe answer.
BEGIN {
  reach["verified"]  = "public material"
  reach["inference"] = "attributed memos and backgrounders"
  reach["open"]      = "this repository only"
  reach["unmarked"]  = "this repository only"
  order[1] = "verified"; order[2] = "inference"; order[3] = "open"; order[4] = "unmarked"
}

# ── read travel-tier'"'"'s output ────────────────────────────────────────────────
#
# Keyed on the record separator travel-tier emits rather than on line numbers: the file is a
# list of three-field records under `nodes:`, and a reader counting lines would break the day
# a field is added to it.
/^  - node:/      { node = $3; next }
/^    travels_as:/ {
  tier = $2
  count[tier]++
  members[tier] = members[tier] (members[tier] == "" ? "" : "\n") "      - " node
  total++
  next
}
/^    downgraded: true/ { downgraded++; next }

END {
  if (total == 0) {
    print "travel-tier computed no nodes, so there is no envelope to report" > "/dev/stderr"
    exit 1
  }

  printf "# Computed by the `disclosure-envelope` calculator and committed by `yidam run`.\n"
  printf "# Recomputed from `.yidam/computed/travel-tier.yml`; edit the corpus, not this file.\n"
  printf "method:\n"
  printf "  rule: |\n"
  printf "    Each node is placed at the tier `travel-tier` computed for it, and each tier\n"
  printf "    reaches exactly as far as `guidelines/agent-conduct.md` says it may. A node\n"
  printf "    whose chain downgraded it is placed where it travels, never where it was\n"
  printf "    declared — which is the whole of what the chain rule is for.\n"
  printf "  source: .yidam/computed/travel-tier.yml, computed by the `travel-tier` capability\n"
  printf "  excludes: |\n"
  printf "    Nothing. Every node travel-tier reported is placed, and `unmarked` shares\n"
  printf "    `open`'"'"'s destination rather than being dropped: a node that declared no\n"
  printf "    evidence standing has not earned one.\n"
  printf "tiers:\n"
  for (i = 1; i <= 4; i++) {
    t = order[i]
    printf "  - tier: %s\n", t
    printf "    reaches: %s\n", reach[t]
    printf "    nodes: %d\n", count[t] + 0
    if (count[t] > 0) {
      printf "    members:\n"
      printf "%s\n", members[t]
    }
  }
  leaves = count["verified"] + count["inference"]
  printf "summary:\n"
  printf "  nodes: %d\n", total
  printf "  leaves_the_repository: %d\n", leaves + 0
  printf "  repository_only: %d\n", total - leaves
  printf "  downgraded_before_placement: %d\n", downgraded + 0
}
' .yidam/computed/travel-tier.yml > "$out/disclosure-envelope.yml"
