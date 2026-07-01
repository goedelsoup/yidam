# Agent Conduct

Guidelines for agents operating in yidam-derived repositories.

## Commit deliberately

Every commit is a permanent node in the knowledge graph. Before committing:
- Is this a complete, coherent knowledge event?
- Is the commit message legible as a graph event description?
- Are new nodes linked to existing ones?

Do not commit partial work or exploratory scratch to main. Use branches for open-ended
exploration; commit to main when knowledge is settled.

## Link generously

New nodes should reference existing related nodes. Orphan nodes — files with no incoming
or outgoing edges — weaken the graph. When adding a file, ask: what does this connect to?

## Stay within scope

The corpus grows through sustained, directed inquiry. Do not add nodes speculatively or
outside the domain of the current repository. Breadth should be driven by need, not by
completeness anxiety.

## Make synthesis explicit

When two ideas relate, say so in the files — and in the commit. A synthesis commit that
adds edges between existing nodes is a first-class knowledge contribution, not housekeeping.

## Preserve provenance

Do not delete or rewrite committed nodes without a record of why. If a node is superseded,
mark it as such and link to its successor. The graph's history is part of its value.

## Mark claim confidence

Corpus nodes often contain claims at different levels of certainty. Tag them inline so
readers and agents can assess the node's reliability without reading sources:

- `[verified]` — supported by a committed primary source linked from this node or its catalog entry
- `[inference]` — a reasonable conclusion drawn from verified facts; not directly witnessed
- `[open]` — a live question; the answer is unknown, contested, or under investigation

**Rules:**
- Untagged claims are only implicitly verified if the node is a direct transcription of a
  primary source. In all other cases, tag every non-obvious claim.
- `[inference]` is not a weakness — it is honest. Untagged inference is the problem.
- `[open]` claims do not need to be resolved before committing. An open question is a valid
  and permanent knowledge contribution.
- A synthesis node will typically contain all three: verified facts it draws on, inferences
  it makes, and open questions it generates. This is expected and good.
