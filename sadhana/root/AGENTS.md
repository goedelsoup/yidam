# Agent Instructions

<!-- TEMPLATE
Replace this block with two or three sentences naming the domain and the central question,
so an agent arriving cold knows what it is working on before it reads anything else.
-->

This repository is a living knowledge artifact. Its git history is the knowledge graph:
files are nodes, commits are knowledge events, links are edges.

## Before taking substantive action

Read the vendored prelude. It is the model this repository runs on, and it is not
negotiable from inside the repo:

- [Identity](.yidam/.vendor/prelude/IDENTITY.md) — what this kind of repository is
- [Graph model](.yidam/.vendor/prelude/GRAPH.md) — how git encodes knowledge
- [Agent conduct](.yidam/.vendor/prelude/guidelines/agent-conduct.md) — behavioral norms,
  including the `[verified]` / `[inference]` / `[open]` claim tags
- [Directory conventions](.yidam/.vendor/prelude/guidelines/directories.md) — what belongs where
- [Phases](.yidam/.vendor/prelude/PHASES.md) — how a unit of inquiry is bounded and committed

Files under `.yidam/.vendor/` are read-only. A defect in the prelude is fixed by re-vendoring
against a newer yidam release (`mise run yidam-vendor-update`), never by editing in place —
an edit there is silently discarded on the next update.

## Conduct norms

**Commit deliberately.** Every commit is a permanent knowledge event. The message should read
as a legible description of what changed and why — not a diff summary. Keep epistemic commits
(understanding added or revised) distinct in style from operational commits (extraction,
refresh, regeneration).

**Link generously.** New nodes must connect to existing ones. Orphan files weaken the graph.

**Stay within scope.** Do not add nodes speculatively. Add a node when an edge needs a target
that does not yet exist.

**Make synthesis explicit.** Adding edges between existing nodes is a first-class
contribution, not housekeeping.

**Preserve provenance.** Do not delete or rewrite committed nodes without a record of why.

**Tag your claims.** Non-obvious claims carry `[verified]`, `[inference]`, or `[open]`.
Untagged inference is the problem the tags exist to prevent.

## The gate

`mise run graph-check` and `yidam lint` are what CI runs. A commit that breaks an edge,
orphans a node, or leaves a REGEN block stale will fail there. Run them before committing
rather than after.

<!-- TEMPLATE
If this domain adds gates of its own — a corpus audit, a series reconciliation, a fixture
check — name them here with the command that runs them.
-->
