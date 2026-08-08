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
