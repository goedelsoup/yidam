# RFCs

*Design documents for yidam architectural decisions. Each RFC is a proposal held open for
review before it becomes behavior — testimony about a change, not a changelog of one.*

These RFCs describe the yidam **template and tooling**, not any derived repo's domain. They
follow the register of the rest of [`docs/`](../README.md): precise, evidenced, and honest
about what is not yet settled.

## Why this set exists

yidam's promise is that a derived repository can inherit a research instrument — the node
model, the reports that keep it honest, the index that makes it retrievable, the MCP surface
that lets an agent work it — and stay in parity with upstream as both evolve. In practice the
first real downstream consumer, **Project BOSC** (`watermark-directory`), could not inherit any
of that. It re-implemented ~1,600 lines of yidam in Python: the four reports, the MCP server,
and the vector index. It did so because everything a consumer actually integrates lives
*above* the parity line and is siloed per language — the reports are Rust-CLI-only, the node
model the reports run on is parsed by no SDK, the MCP surface exists in three mutually
incompatible forms, and the Python index layer is documented but unbuilt. The `cli_ref` pin
that was supposed to make drift visible is enforced by nothing, and a divergence already
exists in the wild.

The through-line of every RFC below: **yidam's parity surface certifies the wrong things
match.** It guarantees three SDKs parse *Markdown* identically while the products a consumer
depends on drift freely. These RFCs move the contract boundary to where integration actually
happens, so a downstream repo can *conform to* and *verify against* yidam instead of
re-deriving it.

## Index

| RFC | Track | Title | Status |
|---|---|---|---|
| [0001](0001-report-contract.md) | I1 | The report contract — reports as versioned rules with golden fixtures | Draft |
| [0002](0002-node-model-unification.md) | I2 | Node-model unification — one graph model the reports and SDKs share | Draft |
| [0003](0003-feature-gated-reports-binary.md) | I3 | Feature-gated builds and a publishable reports-only binary | Draft |
| [0004](0004-drift-detection.md) | I4 | Drift detection — making `.yidam.toml` enforceable (`yidam sync`) | Draft |
| [0005](0005-mcp-tool-contract.md) | I5 | One MCP tool contract across the Rust CLI, TS, and Python servers | Draft |
| [0006](0006-correctness-reconciliation.md) | I6 | Correctness reconciliation — runtime-verifiable embeds + internal inconsistencies | Draft |
| [0007](0007-python-index-layer.md) | I7 | The Python SDK index/feature layer — building what the README already promises | Draft |
| [0008](0008-emergent-claims.md) | G1 | Emergent claims and the scope of synthesis — ratifying the strict reading of Article V | Draft |
| [0009](0009-resolution-executor.md) | G2 | Resolution execution authority and the `synthesized-by` record | Draft |
| [0010](0010-evolution-lineage.md) | G3 | Evolution lineage — forking, parentage, and explicit baselines | Draft |
| [0011](0011-partial-sangha.md) | G4 | Partial-sangha resolutions and participant-scoped binding | Draft |
| [0012](0012-elector-attestation.md) | G5 | Elector identity and attestation — model/version/config + commit signing | Draft |
| [0013](0013-node-model-close.md) | I8 | Closing RFC-0002 — the node-model open questions | Draft |
| [0014](0014-node-rename.md) | I9 | Node rename as a sanctioned operation — dangling-edge gate + atomic `yidam rename` | Draft |
| [0015](0015-epistemic-log.md) | I10 | An epistemic-only history view (`yidam log --epistemic`) | Draft |
| [0016](0016-editor-surface.md) | I11 | An editor surface for derived repositories (`yidam` for VS Code) | Draft |
| [0017](0017-assertion-surface.md) | I12 | Serving assertions, not documents (`claims` and the practice tools) | Draft |
| [0018](0018-query-surface.md) | I13 | The query surface — typed traversal bounded by the ontology (`yidam query`) | Draft |
| [0019](0019-external-citation.md) | I14 | Citing a corpus you cannot revise (`cites:`) | Draft |
| [0020](0020-proposal-surface.md) | I15 | Proposing what a finding already says (`yidam propose`) | Draft |
| [0021](0021-diff-alignment.md) | I16 | Code that names what the ontology has not (`yidam check-diff`) | Draft |
| [0022](0022-semantic-alignment.md) | I17 | What a tool may say about code it cannot read (`check-diff`, Phase B) | Draft |

## Reading order

- Start with **0002** (the root: two disjoint node models with no bridge) and **0001** (the
  highest-leverage fix: make the reports a tested contract).
- **0003** removes the reason a consumer re-implements at all (a light, publishable binary);
  **0004** turns the version pin into an enforced check on top of it.
- **0005**, **0006**, **0007** reconcile the surfaces that already diverged (MCP, embeddings,
  the empty Python layer).

## Status legend

`Draft` — under review, not accepted. `Accepted` — agreed, implementation may begin.
`Implemented` — landed and referenced by a released layer. `Superseded` — replaced by a
later RFC (named in its header). `Rejected` — considered and declined, with the reason kept.

## RFC template

Every RFC in this directory follows this shape:

```markdown
# RFC-000X — <Title>

- **Status:** Draft
- **Track:** I<n>
- **Relates to:** RFC-000Y, RFC-000Z
- **Versioning layers touched:** template / SDK+parity / bootstrap protocol
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary
One paragraph: what changes and why it matters to a downstream consumer.

## Problem
The current shape, with concrete `file:line` evidence from the yidam tree and from the
BOSC consumer. State the drift or friction precisely.

## Proposal
The concrete design — schemas, command surface, fixture format, type changes.

## Migration & compatibility
Which versioning layer bumps; how existing derived repos and BOSC adopt; what breaks.

## Alternatives considered
The options weighed and why this one.

## Open questions
What is genuinely unresolved and needs a decision.
```
