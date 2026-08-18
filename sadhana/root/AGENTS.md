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
- [Constitution](.yidam/.vendor/prelude/CONSTITUTION.md) — binding on every resolution event
  *(collective governance only; dormant in a single-elector repository)*

Files under `.yidam/.vendor/` are read-only. A defect in the prelude is fixed by re-vendoring
against a newer yidam release (`mise run yidam-vendor-update`), never by editing in place —
an edit there is silently discarded on the next update.

## Conduct norms

**Commit deliberately.** Every commit is a permanent knowledge event. The message should read
as a legible description of what changed and why — not a diff summary. Keep epistemic commits
(understanding added or revised) distinct from operational commits (extraction, refresh,
regeneration) — and the distinction is carried by the **closed verb vocabulary** in
[GRAPH.md](.yidam/.vendor/prelude/GRAPH.md), not by style alone. Every subject begins
`<verb>: `, the verb stands alone with no `(scope)` suffix, and `yidam lint --commits`
reports anything outside the list. A merge deserves a written subject too; git's default
names the ref and not what joining it meant.

**Link generously.** New nodes must connect to existing ones. Orphan files weaken the graph.

**Stay within scope.** Do not add nodes speculatively. Add a node when an edge needs a target
that does not yet exist.

**Make synthesis explicit.** Adding edges between existing nodes is a first-class
contribution, not housekeeping.

**Preserve provenance.** Do not delete or rewrite committed nodes without a record of why.

**Tag your claims.** Non-obvious claims carry `[verified]`, `[inference]`, or `[open]`.
Untagged inference is the problem the tags exist to prevent.

<!-- TEMPLATE
Delete this whole section in a single-elector repository. Keep and fill it if governance is
collective — name the electors and say where positions go.
-->
## Governance

This repository is `governance: collective`. Electors are listed in
[`.yidam/sangha/electors.md`](.yidam/sangha/electors.md); the resolution algorithm is in
[`.yidam/sangha/PROTOCOL.md`](.yidam/sangha/PROTOCOL.md). Commit your working position to
your own `ma/<elector>` branch. Do not synthesize another elector's position into yours
outside a resolution event — Article V limits resolution to synthesis of what electors
actually hold.

Before a resolution, **write your position down** in
[`.yidam/sangha/positions/`](.yidam/sangha/positions/). The branch tip records which nodes
you hold; it does not record why, what you conceded, or which of your own earlier grounds
you are withdrawing — and that is what the resolution turns on. After the resolution, take
the new baseline by merge, not rebase: `adopt: the baseline after <evolution>`.

## The gate

`mise run graph-check` and `mise run graph-lint` are what CI runs. A commit that breaks an
edge, orphans a node, or leaves a REGEN block stale will fail there. Run them before
committing rather than after.

`yidam lint` gates against `.yidam/lint-baseline.yml`, not against zero. It asks whether
*this change* made the corpus less clean, because a gate that fails on inherited debt gets
switched off and stays off. Two things fail it: an error-severity violation that is not in
the baseline, and a baseline entry that no longer occurs. The second is not a bug — a
baseline permitted to be wrong drifts, and one that over-lists silently re-permits whatever
it over-lists. Fix the corpus, then `mise run graph-lint-bless` and commit the diff.

`mise run graph-lint-explain` prints each check's rationale. Read it before deciding a
check is wrong.

## Validation while you type

```
mise run schema           # emit .yidam/schemas/*.json
yidam schema --settings   # the editor mapping to paste into .vscode/settings.json
```

Read by `yaml-language-server` (the Red Hat YAML extension in VS Code; available to Neovim
and Helix over LSP). Editors using none of these still get the check from `yidam lint`.

<!-- TEMPLATE
If this domain adds gates of its own — a corpus audit, a series reconciliation, a fixture
check — name them here with the command that runs them.
-->
