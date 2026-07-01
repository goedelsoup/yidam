# Knowledge Graph Model

This repository's knowledge graph lives in git. No external store is required.

## Encoding

| Git primitive | Knowledge meaning |
|---|---|
| File | Knowledge node — a concept, document, artifact, or relation |
| Commit | Knowledge event — an addition, revision, or synthesis |
| Commit message | Event description — what changed and why |
| Link (`[label](path)`) | Edge — an explicit relationship from one node to another |
| Branch | Parallel inquiry thread — speculative or in-progress |
| Merge commit | Synthesis — two threads of knowledge joined |
| Tag | Stable checkpoint — a named state of the graph |
| `refs/heads/rigpa/<evolution>` | Settled evolution — a named, stable collective understanding |
| `refs/heads/ma/<elector>` | Elector position — one participant's current working knowledge |

## Nodes

Files are nodes. Their content is the node's value; their name and path are the node's
identity within the graph.

Nodes should be **small and focused** — one concept, one artifact, one relationship.
Large files are a sign that decomposition is needed.

Node types are distinguished by directory, not by filename:

| Directory | Node type | What it represents |
|---|---|---|
| `corpus/` | Knowledge node | A concept, relation, artifact, or open question in the domain |
| `catalog/` | Source node | A data source — dataset, paper, API, external knowledge base |

Corpus nodes represent derived knowledge; catalog nodes represent its provenance. An edge
from a corpus node to a catalog node reads as "this concept draws on this source." Catalog
nodes do not contain derived knowledge — only enough to locate and characterize the source.

## Edges

Edges are explicit markdown references: `[label](path)`. An agent reading the graph can
follow edges to traverse related knowledge.

Edges are **directional** — the file containing the reference is the source node, the
referenced file is the target. Bidirectional relationships require a reference in both files.

## Commits as events

Not all commits carry the same kind of meaning. Two types coexist in every yidam-derived
repository:

**Epistemic commits** add or revise understanding. They are the primary knowledge events of
the graph: authored nodes, synthesis, assessment, open questions resolved or opened. Write
these in the active voice of inquiry:
> `establish: confounding variable framework — links to identification and intervention`
> `revise: identification conditions — updated after reviewing Pearl 2009`

**Operational commits** advance the corpus through pipeline work: data extraction, connector
refreshes, bundle generation, catalog reconciliation. They are legitimate provenance records
but are not epistemic events. Write these by naming the pipeline step and its output:
> `extract: NPDES permit fields for site X — 14 structured values from document Y`
> `refresh: ECHO inventory — 3 new dischargers added since last pull`

Both types appear in the git log and both are part of the graph's history. Keeping them
visually distinct — by message style — preserves the log's readability as a knowledge record.

Every commit of either type should answer:
- What changed?
- Why? (What prompted this — what question, what source, what finding?)
- What does it connect to?

Commit messages are part of the graph. They record the provenance of every node.

## Branches as inquiry

Open a branch to explore a speculative direction. The branch represents an inquiry thread
that may or may not be merged into the main graph. Merging is synthesis; abandoning a branch
is a deliberate choice to exclude that thread. Both are valid knowledge acts.

## Collective resolution

When multiple participants maintain the graph, two ref namespaces encode their relationship:

`refs/heads/ma/<elector>` branches are individual positions — each elector (human or agent)
commits their working understanding here freely, without requiring consensus. Positions are
expected to diverge.

`refs/heads/rigpa/<evolution>` branches are settled evolutions — points where individual
positions have been synthesized into shared understanding. A resolution event reads all
`ma/*` tips, identifies agreement and tension, and produces a new rigpa branch as a named
collective baseline. Elector branches diverge again from there.

See [sangha/](../sangha/README.md) for the full resolution protocol.
