# RFC-0013 — Closing RFC-0002: the node-model open questions

- **Status:** Draft
- **Track:** I8
- **Relates to:** RFC-0002 (resolves its open questions), RFC-0001 (blocked on this), RFC-0006 (tag
  reconciliation)
- **Versioning layers touched:** SDK+parity / template / bootstrap protocol
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

RFC-0002 recommended option **(A)** — the YAML instance is the canonical graph, Markdown is an
ingestion projection — but parked five open questions, and RFC-0001 cannot freeze report fixtures
until they close ([`0002:139-142`](0002-node-model-unification.md#L139-L142)). This RFC decides all
five: **one canonical model** (instances; Markdown is a doorway, not a second graph);
`project_markdown` runs at **authoring time** (one on-disk form — YAML — produced from ephemeral
prose); **generated nodes are instance-only**; **anchors are dropped** at the projection boundary,
not grown into the schema; **`properties:` is surfaced and typed with deny-unknown**; and the
**`[inferred]`/`[inference]` spelling is pinned** at the projection. Two of these ratify calls already
made; three take a position so the draft has something to disagree with.

## Problem

RFC-0002's five open questions ([`0002:165-181`](0002-node-model-unification.md#L165-L181)) each block
a downstream decision:

1. **Two audiences, or one?** Is human Markdown authoring a workflow yidam commits to keeping, or is
   every node an instance with Markdown a convenience input?
2. **`Generated` nodes.** `NodeKind::Generated` is declared and never produced (`corpus.rs:40`);
   should generated nodes be instance-only?
3. **Anchor semantics.** Markdown links carry `#anchor` (`corpus.rs:81-90`); instance links have no
   anchor field. Drop, or grow the schema?
4. **`properties:` on the parity surface.** The Rust parser drops the free-form `properties:` map
   silently ([`0002:41`](0002-node-model-unification.md#L41)); surface and type it, or keep it opaque?
5. **Tag spelling.** `[inferred]` (SDK) vs `[inference]` (conduct norms + CLI) — pin at the projection
   or defer entirely to RFC-0006?

Until these are answered, "parity" keeps certifying a Markdown model the products never touch
([`0002:16-19`](0002-node-model-unification.md#L16-L19)), and BOSC keeps re-deriving an instance
parser no SDK offers.

## Proposal

### One model, Markdown is a doorway *(decides Q1)*

Ratify option (A) as a decision: every node is a `CorpusInstance` on disk; Markdown authoring is a
supported **ingestion path** (`project_markdown(path, text) → CorpusInstance`), not a co-equal graph
model. "Two audiences" collapses to **one graph, two input styles.** The parity surface certifies the
instance the products use.

### Authoring-time projection, one on-disk form *(decides the timing sub-question)*

`project_markdown` runs when a node is **authored**: the canonical YAML instance is what lands in the
commit; the Markdown prose is *ephemeral input*, not a second on-disk artifact. This is explicitly
**not** option-B-with-better-paperwork (two on-disk forms reconciled at read time) — there is one
on-disk form. The elector's raw prose is not carried into the permanent record as a parallel shadow
of the instance; its substance lives in the instance's `description`. A repo that genuinely wants the
raw prose preserved commits it as its own authored node, not as a hidden twin of the instance.
*(Position, not a prior call — the softest of the three; see open questions.)*

### Generated nodes are instance-only *(decides Q2)*

The only path to a `Generated` node is projection/pipeline emission into an instance. There is no
Markdown `Generated` node. This matches how a projecting consumer actually operates — BOSC emits
instances, never authors Markdown corpus nodes ([`0002:97-99`](0002-node-model-unification.md#L97-L99)).

### Anchors dropped at the boundary *(decides Q3)*

`project_markdown` drops `#anchor`; the instance link schema does **not** grow an anchor field. A node
is atomic — "one concept per file; one file per concept," decompose past a screen
([`directories.md:150-151`](../../yidam/prelude/guidelines/directories.md#L150-L151)) — so an
intra-node anchor is a Markdown-document affordance with nothing to bind to in a node graph. A link
that needs to target a sub-part is a signal to make that sub-part its own node.
*(Position — the one most likely to draw pushback; flagged below.)*

### `properties:` surfaced, typed, deny-unknown *(ratifies the standing call)*

`parse_instance` surfaces and types `properties:`, and the parser sets `deny_unknown_fields` so an
unspecced key is an **error**, not a silent drop. This applies the silent-loss ethic the whole RFC
set is built on — the same ethic that flags "silently dropped" as the cardinal sin
([`0002:41`](0002-node-model-unification.md#L41), and RFC-0001/0005/0007 throughout). "Settled in
direction, unratified" becomes ratified: surface and deny-unknown.

### Tag spelling pinned *(ratifies the standing call)*

Pin **`[inference]`** as the canonical on-disk spelling — it is the conduct-norm and CLI spelling
already ([`agent-conduct.md:42`](../../yidam/prelude/guidelines/agent-conduct.md#L42), `diff.rs:209`).
`project_markdown` maps the SDK's legacy `[inferred]` → `[inference]` on ingest; the SDK's
`EvidenceTag::Inference` reads the canonical spelling. The broader marker reconciliation stays in
RFC-0006, per RFC-0002's recommendation ([`0002:179-181`](0002-node-model-unification.md#L179-L181)).

## Migration & compatibility

- **SDK + parity (`parity/VERSION` 0.3.0 → 0.4.0).** Add `parse_instance` and `project_markdown` to
  the parity surface with real Python/TS implementations and fixtures (not directory stubs — the gate
  only checks a fixture directory exists, [`0002:124-127`](0002-node-model-unification.md#L124-L127)).
  `parse_instance` sets `deny_unknown_fields` and types `properties:`.
- **Template (minor).** Collapse `information-architecture.md`'s two "corpus node" sections into one
  canonical instance model plus an ingestion note.
- **Bootstrap protocol.** Rewrite rubric S1/S2 to check instance files and instance `links` rather
  than `.md` files and markdown links ([`0002:131-133`](0002-node-model-unification.md#L131-L133)).
- **Unblocks RFC-0001**, which can now freeze report fixtures against the settled model.
- **BOSC** drops its hand-rolled instance walk/parse for the SDK's `parse_instance`; its artifacts
  already write `[inference]`, so the pinned spelling costs it nothing.

## Alternatives considered

- **Keep two on-disk forms (option B).** Rejected: it is the split with paperwork. A tested projection
  is necessary either way; making YAML canonical is the part that ends the drift.
- **Read-time projection.** Rejected: it implies either two on-disk forms or re-projecting Markdown on
  every read — cost and ambiguity for no gain over authoring-time.
- **Grow an anchor field on instance links.** Rejected: it imports a document affordance into an
  atomic-node graph; decomposition is the node-native answer.
- **Keep `properties:` opaque.** Rejected: silent loss, the exact failure this set exists to end.

## Open questions

- **Prose fidelity (the soft call).** Is `description` a sufficient home for authored prose, or do some
  repos need the raw prose as a first-class artifact? If the latter, is that just "commit it as its own
  authored node," or does the instance schema need a `source_prose` field? This is where I most expect
  disagreement.
- **deny-unknown migration cost.** Turning unknown keys into errors may break derived repos relying on
  free-form `properties:`. Does `parse_instance` need a one-release warn-then-deny grace period?
- **Anchor escape hatch.** If dropping anchors proves too strict for a real corpus, the fallback is a
  named-node decomposition — but is there a case where an anchor genuinely cannot be a node?
