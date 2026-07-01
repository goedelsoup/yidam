# Agent Instructions

This is **yidam** — a meta-template repository. It does not contain an application.
It contains the scaffolding, prelude, and harness used to instantiate yidam-derived
repositories: living knowledge artifacts whose git history is the knowledge graph.

## If you are bootstrapping a new repo

You have likely arrived via [BOOTSTRAP.md](BOOTSTRAP.md). Read it first, then follow
the [bootstrap skill](yidam/prelude/skills/bootstrap.md). Do not scaffold anything before
completing the ontology-discovery dialogue with the user.

## If you are working in this repo directly

This repo's own content lives in:

- `yidam/prelude/` — meta-prompts, guidelines, and yidam-provided skills
- `yidam/tests/` — test harness (Rust) and evaluation rubric for bootstrap runs
- `sadhana/` — template content scaffolded into derived repos during bootstrap
- `BOOTSTRAP.md` — the agent entry prompt consumed by derived repos

You are not building an application. You are maintaining a template and its test harness.

## Conduct norms (applies in all yidam-derived repos)

**Commit deliberately.** Every commit is a permanent knowledge event. The message should
read as a legible description of what changed and why — not a diff summary.

**Link generously.** New nodes must connect to existing ones. Orphan files weaken the graph.

**Stay within scope.** Do not add nodes speculatively. Add a node when an edge needs a
target that does not yet exist.

**Make synthesis explicit.** Adding edges between existing nodes is a first-class
contribution, not housekeeping.

**Preserve provenance.** Do not delete or rewrite committed nodes without a record of why.

## Full context

Read the prelude before taking substantive action:

- [Identity](yidam/prelude/IDENTITY.md) — what yidam-derived repos are
- [Graph model](yidam/prelude/GRAPH.md) — how git encodes knowledge
- [Agent conduct](yidam/prelude/guidelines/agent-conduct.md) — full behavioral norms
- [Directory conventions](yidam/prelude/guidelines/directories.md) — what belongs where
