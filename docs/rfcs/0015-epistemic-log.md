# RFC-0015 — An epistemic-only history view (`yidam log --epistemic`)

- **Status:** Draft
- **Track:** I10
- **Relates to:** `classify_commit` (parity surface), RFC-0001 (report/JSON conventions), RFC-0003
  (light binary), `prelude/GRAPH.md`, `prelude/SCRIPTURE.md`
- **Versioning layers touched:** tooling (`yidam` CLI) — no parity or protocol change
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

yidam already has a parity-certified commit classifier (`classify_commit` → epistemic / operational)
and a scripture that rests on the commit message being **testimony**. But the classifier's only
consumer is `backfill`, and a reader who wants to *see* the testimony has to eyeball a git log where
operational regeneration diffs dominate by volume. `yidam log --epistemic` is a filtered view over the
existing primitive — roughly a day of work — that makes the system's central promise visible instead
of merely asserted. It is a cheap win that expresses the philosophy, worth shipping ahead of the hard
decisions that merely *enable* it.

## Problem

The epistemic/operational split is foundational, not incidental. `GRAPH.md` devotes a section to it
([`GRAPH.md:47-64`](../../yidam/prelude/GRAPH.md#L47-L64)), and the scripture is explicit: "The commit
message is **testimony** — not a changelog, but a record of a change in understanding"
([`SCRIPTURE.md:19`](../../yidam/prelude/SCRIPTURE.md#L19)); "Two kinds of events. No others."
([`:17`](../../yidam/prelude/SCRIPTURE.md#L17)).

The classifier is built and certified. `classify_commit` is one of the six parity functions
([`VERSIONING.md:53-54`](../../VERSIONING.md#L53-L54)), implemented in all three SDKs
(`rust/src/git.rs`, `python/yidam_core/git.py`, TS) with fixtures for `epistemic-add`,
`epistemic-refine`, `epistemic-link`, `operational-build`, `operational-fix`, `operational-regen`.

**But nothing exposes it as a view.** Its sole consumer is `yidam backfill`, which classifies commits
only to write decision records from epistemic ones and skip operational ones
([`backfill.rs:81-122`](../../yidam/cli/src/cmd/backfill.rs#L81-L122)). There is no `log`-style
surface. So the moment a corpus does real pipeline work — extraction, connector refreshes, bundle
regeneration, all legitimately operational
([`GRAPH.md:57-62`](../../yidam/prelude/GRAPH.md#L57-L62)) — the testimony is buried under
infrastructure churn in `git log`, with a certified classifier sitting one command away from surfacing
it. The system's central artifact is the one thing it cannot show you cleanly.

## Proposal

Add `yidam log`:

```
yidam log [--epistemic | --operational] [--format text|json] [<range>]
```

- Runs `classify_commit` over each commit in `<range>` (default: current ref history) and filters.
- `--epistemic` shows only epistemic events — the testimony. `--operational` the inverse. No filter
  shows both, each line tagged `[E]` / `[O]`.
- Default `text` output is testimony-first and legible — `<short-hash>  [E]  <subject>`; `--format
  json` emits structured records for tooling, consistent with the report-JSON convention the set uses
  elsewhere (RFC-0001, RFC-0004).

No new classification logic — it consumes the parity-certified surface, so the CLI view and the SDK
agree on what "epistemic" means by construction. It composes with the existing `yidam diff <range>`
and is the natural "testimony view" a downstream browser (web export) or MCP surface can later expose
without re-deriving the classifier (which is exactly what BOSC did).

**Why this, now.** It is a day of work on a primitive already built and tested, and it turns the
epistemic/operational distinction from an *asserted* principle into a *usable* one. The rest of this
RFC set makes hard decisions that *enable* the philosophy (the node model, the contract boundary);
this one makes a piece of the philosophy immediately *visible* at almost no cost. Ship the cheap wins
that express the system's promise ahead of the expensive ones that merely support it.

## Migration & compatibility

Tooling only — a new `yidam log` subcommand. No parity-surface change (it *consumes* the surface), no
template or protocol change. Light-binary compatible (RFC-0003): classification needs no
`fastembed`/`lancedb`, so the view ships in the reports-only binary a Node/Python consumer can install.
BOSC gets it for free once on the CLI; it has already re-derived `classify_commit`, so this replaces a
re-implementation with the shared surface.

## Alternatives considered

- **`git log --grep` on message-style conventions.** Rejected: it re-derives the classifier by
  fragile prefix-matching, when a certified `classify_commit` is the source of truth. The whole set's
  thesis is *stop re-deriving what a tested surface already decides.*
- **Fold the filter into `yidam diff` only.** Rejected: `diff` is range/content-oriented; a *log* is
  the native home for an event-stream view. `diff` may share the classifier, but the testimony view
  wants its own surface.
- **A web-only testimony view first.** Rejected as the first cut: the CLI is cheaper, composes with
  existing commands, and the web/MCP surfaces can consume it later rather than each re-deriving it.

## Open questions

- **Default filter.** Should `yidam log` default to `--epistemic` (testimony-first, matching the
  philosophy) or show both tagged? Lean: show both by default, testimony a discoverable flag — but a
  principled case exists for testimony-as-default. Worth a call.
- **Lineage awareness.** Purely a commit filter, or should it follow `supersedes`/rigpa lineage
  (RFC-0010)? Lean: commit filter now; lineage-aware view is a later, larger feature.
- **Multi-ref scope.** Current ref + range (like `git log`), or span `ma/*` and `rigpa/*` together?
  Lean: current ref + explicit range, leaving cross-ref views to a dedicated command.
