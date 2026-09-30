#!/bin/sh
# travel-tier — how far each corpus node's assertions may travel.
#
# `guidelines/agent-conduct.md` states the rule this computes and states that it must be
# computed:
#
#   A derived assertion travels only as far as the weakest claim beneath it. Its tier is the
#   minimum tag across the whole supporting chain, computed rather than declared. [verified]
#   may reach public material; [inference] reaches attributed memos and backgrounders; [open]
#   does not leave the repository. Declared tiers drift the moment a supporting node is
#   revised — computing it means a downgrade upstream propagates on the next build.
#
# A norm whose own words are "computed rather than declared" and which nothing computes is
# the shape this repository keeps finding. This is a calculator: its output is a fact about
# the corpus that changes whenever the corpus does, which is why it is recomputed and
# committed rather than written once.
#
# It reproduces no observation and invents no value. Every number here is derived from bytes
# already committed, which is what makes it safe to run over a corpus whose stations are
# illustrative.
#
# The contract it is invoked under: it stands in a tree holding exactly what the manifest
# declares it reads, it is handed the resolved corpus at $YIDAM_GRAPH, and it writes under
# $YIDAM_OUT. It never sees the working tree.
#
# ── what this script no longer does (#1080) ──────────────────────────────────
#
# It used to be 190 lines, and most of them were not the travel-tier rule. It parsed node
# YAML by regex on `/^  claim_tag:/` and `/^  - target:/`; it re-implemented link resolution
# as an awk `function resolve(base, t)`, which was a second answer to the question
# `corpus/edges.rs`'s `resolve_target` already answers and which nothing compared against it;
# and it opened with a `find | sort` and an `LC_ALL=C` so that the order awk saw files in was
# a property of the corpus rather than of the locale.
#
# All three are gone, because `yidam run` now hands the resolved corpus over: `$YIDAM_GRAPH`
# is the CLI's own parse, the CLI's own link resolution, and the CLI's own walk order, in
# tab-separated records. What is left below is the rule.

set -eu

out="$YIDAM_OUT/.yidam/computed"
mkdir -p "$out"

# Stated rather than left to `awk` reading an empty name. A step is handed no resolved corpus
# when its declared `reads` admit no node, and a calculator over the corpus that finds itself
# in that position has a manifest problem rather than an empty corpus.
if [ -z "${YIDAM_GRAPH:-}" ]; then
  echo "travel-tier: no resolved corpus. Does this step's \`reads\` cover .yidam/corpus?" >&2
  exit 1
fi

awk -F'\t' '
# ── the claim vocabulary, weakest first ──────────────────────────────────────
#
# `unmarked` ranks below `open` deliberately. A node that has not declared its evidence
# standing has not earned a stronger one, and a chain running through it may not travel
# further than one running through a node that says [open] out loud. The conservative
# direction is the only safe one here: this vocabulary exists to stop material travelling
# further than its evidence, so where the evidence is silent the answer is the floor.
BEGIN {
  rank["unmarked"]  = 0
  rank["open"]      = 1
  rank["inference"] = 2
  rank["verified"]  = 3
  name[0] = "unmarked"; name[1] = "open"; name[2] = "inference"; name[3] = "verified"
}

# ── the node id this corpus reports signals against ──────────────────────────
#
# The resolved corpus identifies a node by its repository-relative path, which is the
# spelling `reads` and the receipt use. A signal table names nodes in the reference grammar —
# `concept/base-flow-separation` — so the prefix and the extension come off here, once, and
# by string surgery rather than by a regex: `.yidam/corpus/` holds two dots and a pattern
# built from it would also match `Xyidam/corpus/`.
function id(p) {
  if (substr(p, 1, length(prefix)) == prefix) p = substr(p, length(prefix) + 1)
  if (substr(p, length(p) - 3) == ".yml") p = substr(p, 1, length(p) - 4)
  return p
}

$1 == "corpus_dir" { prefix = $2 "/"; next }

$1 == "node" {
  nodes[++n] = id($2)
  declared[id($2)] = "unmarked"
  next
}

$1 == "prop" && $3 == "claim_tag" {
  if ($5 in rank) declared[id($2)] = $5
  next
}

# `instance-of` points at the class definition, which carries no claim and is not part of any
# supporting chain. Every other outgoing link is. Whether the target is one of this corpus
# nodes is decided at END and not here: a link may point at a node whose record has not been
# read yet, so a target is classified against the whole corpus or against nothing.
$1 == "link" && $4 != "instance-of" {
  edges[++e] = id($2) SUBSEP id($6)
  next
}

# ── the fixed point, and the report ──────────────────────────────────────────
END {
  for (i = 1; i <= n; i++) tier[nodes[i]] = rank[declared[nodes[i]]]

  # A link whose target is not a corpus node in this corpus carries no claim and is not part
  # of any supporting chain — a class definition, a catalog anchor, or a target that is not
  # there at all. Counted rather than silently dropped: a chain rule that quietly ignored
  # part of the outgoing links of a node would compute a tier that travels further than it
  # should, which is the one direction this vocabulary exists to prevent.
  for (j = 1; j <= e; j++) {
    split(edges[j], ab, SUBSEP)
    if (!(ab[2] in tier)) off++
  }

  # Relaxation to a fixed point: a tier can only fall, and there are finitely many ranks, so
  # this terminates. It is written as a loop rather than a traversal because a corpus graph
  # may hold cycles and a recursive walk would not come back.
  changed = 1
  while (changed) {
    changed = 0
    for (j = 1; j <= e; j++) {
      split(edges[j], ab, SUBSEP)
      a = ab[1]; b = ab[2]
      if (!(b in tier)) continue
      if (tier[b] < tier[a]) { tier[a] = tier[b]; changed = 1 }
    }
  }

  printf "# Computed by the `travel-tier` calculator and committed by `yidam run`.\n"
  printf "# Recomputed from the corpus; edit the corpus, not this file.\n"
  # The version of the signal-table contract, not of this calculator. `yidam` reads the rows
  # under `signals:` and attaches them to the node each one names, and a file carrying no
  # version is listed and not read — which is how a calculator that has not adopted the
  # contract stays readable to a person and invisible to the embedder, rather than being
  # guessed at. See RFC-0026 §4.1.
  printf "format_version: 1\n"
  printf "method:\n"
  printf "  rule: |\n"
  printf "    A derived assertion travels only as far as the weakest claim beneath it. A\n"
  printf "    node'"'"'s travel tier is the minimum claim tag over the node itself and the\n"
  printf "    transitive closure of its outgoing links, computed rather than declared.\n"
  printf "  source: guidelines/agent-conduct.md, \"When claims leave the repository\"\n"
  printf "  order: [unmarked, open, inference, verified]\n"
  printf "  excludes: |\n"
  printf "    `instance-of` links, which point at a class definition rather than at an\n"
  printf "    assertion, and every link whose target is not a corpus node here — a catalog\n"
  printf "    anchor is a source and not a claim. Those are counted as\n"
  printf "    `links_to_non_nodes` rather than dropped in silence. A node declaring no\n"
  printf "    `claim_tag` is `unmarked`, which ranks below `open`: where the evidence is\n"
  printf "    silent the answer is the floor.\n"
  # `signals:` and not `nodes:`, because the key is the contract rather than a label. Every
  # row names a node in the reference grammar and every other field of it is a signal about
  # that node — which is what makes `travels_as` reach a search rather than only a reader.
  printf "signals:\n"
  for (i = 1; i <= n; i++) {
    nd = nodes[i]
    printf "  - node: %s\n", nd
    printf "    declared: %s\n", declared[nd]
    printf "    travels_as: %s\n", name[tier[nd]]
    printf "    downgraded: %s\n", (rank[declared[nd]] > tier[nd] ? "true" : "false")
    if (rank[declared[nd]] > tier[nd]) down++
  }
  printf "summary:\n"
  printf "  nodes: %d\n", n
  printf "  downgraded: %d\n", down + 0
  printf "  links_to_non_nodes: %d\n", off + 0
}
' "$YIDAM_GRAPH" > "$out/travel-tier.yml"
