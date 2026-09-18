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
# declares it reads, and it writes under $YIDAM_OUT. It never sees the working tree.

set -eu

# Byte ordering, so the row order is a property of the corpus and not of the locale the run
# happened to start in.
LC_ALL=C
export LC_ALL

out="$YIDAM_OUT/.yidam/computed"
mkdir -p "$out"

# The node files, in byte order, as arguments rather than on stdin: awk keys each record on
# FILENAME, and a list piped in would be read as the corpus rather than as the names of it.
# Built one line at a time so a path holding a space stays one path.
set --
while IFS= read -r f; do set -- "$@" "$f"; done <<LIST
$(find .yidam/corpus -name '*.yml' ! -name '*.ont.yml' | sort)
LIST

awk '
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

# ── read one node per file ───────────────────────────────────────────────────
FILENAME != seen {
  seen = FILENAME
  id = FILENAME
  sub(/^\.yidam\/corpus\//, "", id)
  sub(/\.yml$/, "", id)
  nodes[++n] = id
  here = id
  sub(/\/[^\/]*$/, "", here)   # the directory the file sits in, for relative targets
  dir[id] = here
  declared[id] = "unmarked"
  current = id
  pending = ""
}

/^  claim_tag:/ {
  v = $2
  gsub(/["\[\]]/, "", v)
  if (v in rank) declared[current] = v
  next
}

# A link is two lines: the target, then the relationship it is drawn under.
/^  - target:/ { pending = $3; next }

/^    relationship:/ {
  if (pending == "") next
  rel = $2
  target = pending
  pending = ""
  # `instance-of` points at the class definition, which carries no claim and is not part of
  # any supporting chain. Every other outgoing link is.
  if (rel == "instance-of") next
  add_edge(current, resolve(dir[current], target))
  next
}

# ── resolving a link target to a node id ─────────────────────────────────────
#
# Targets are relative to the file the link is written in, which is how the corpus is
# authored and how every other reader of it resolves them.
function resolve(base, t,   parts, i, k, stack, outp) {
  t = base "/" t
  k = split(t, parts, "/")
  outp = 0
  for (i = 1; i <= k; i++) {
    if (parts[i] == "." || parts[i] == "") continue
    if (parts[i] == "..") { if (outp > 0) outp--; continue }
    stack[++outp] = parts[i]
  }
  t = ""
  for (i = 1; i <= outp; i++) t = (i == 1 ? stack[i] : t "/" stack[i])
  sub(/\.yml$/, "", t)
  return t
}

# Every outgoing link that is not `instance-of`. Whether its target is part of the chain is
# decided at END and not here: a link may point at a node whose file has not been read yet,
# so a target is classified against the whole corpus or against nothing.
function add_edge(a, b) {
  edges[++e] = a SUBSEP b
}

# ── the fixed point, and the report ──────────────────────────────────────────
END {
  for (i = 1; i <= n; i++) tier[nodes[i]] = rank[declared[nodes[i]]]

  # A link whose target is not a corpus node in this corpus carries no claim and is not part
  # of any supporting chain — a class definition, a catalog anchor, or a target that is not
  # there at all. Counted rather than silently dropped: a chain rule that quietly ignored
  # part of the outgoing links of a node would compute a tier that travels further than it
  # should,
  # which is the one direction this vocabulary exists to prevent.
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
  printf "nodes:\n"
  for (i = 1; i <= n; i++) {
    id = nodes[i]
    printf "  - node: %s\n", id
    printf "    declared: %s\n", declared[id]
    printf "    travels_as: %s\n", name[tier[id]]
    printf "    downgraded: %s\n", (rank[declared[id]] > tier[id] ? "true" : "false")
    if (rank[declared[id]] > tier[id]) down++
  }
  printf "summary:\n"
  printf "  nodes: %d\n", n
  printf "  downgraded: %d\n", down + 0
  printf "  links_to_non_nodes: %d\n", off + 0
}
' "$@" > "$out/travel-tier.yml"
