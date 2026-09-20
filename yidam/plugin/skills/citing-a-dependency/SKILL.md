---
name: citing-a-dependency
description: Use before writing a `cites:` in a yidam repository — a repository with a .yidam/ directory. Into an installed dependency, whether the citation will hold cannot be answered by reading, and the yidam MCP server answers it before the citation is written; into a node in this corpus, `cites:` with no `package:` is the form and `yidam lint` decides it. Triggers on "cites", "cite a dependency", "cite a node", "external citation", "local citation", "tonpa", "span", "pin", "foreign node".
---

# Citing a dependency

A `cites:` records that this corpus read a specific node in another corpus, at a specific
pin, and observed a specific standing there. Four checks decide whether that record holds,
and **none of them can be answered by reading**: `retrieve` reaches a dependency, `get_node`
reads a node out of one, `query --across` walks them, and not one of them says whether
leaning on what it returned would stand. The package may be installed at a different pin than
the one you are about to write, and a `span:` is a claim about text that no read-tool checks.

**Call `check_citation`** before you write it. `package` is required; `node`, `commit`, `tag`
and `span` are optional and each one you supply is one more check that can actually run.

It is total and never errors — a citation that will not hold is a verdict in the payload.

## Reading the answer

- `holds` — the verdict. `findings` carries the check ids the gate would report, each with
  that check's own message, and **the message names the repair**.
- `external-citation-unresolved` (error) — the node is not in that corpus at that pin. This is
  also what you get for a call with no `node`, which is the honest answer rather than a schema
  complaint.
- `external-citation-span-drift` (error) — your `span:` is not verbatim in the cited node.
- `external-citation-pin-moved` (warn) — the pin you recorded is not the pin installed.
- `external-citation-unpinned` (info) — nothing records which commit was read.
- `dependencies` — what is actually installed and at what pin each. This is the value a
  correct `commit:` must hold, and it is reachable no other way on this surface.

## Citing a node in **this** corpus is the other half, and needs no tool

Leave `package:` off and the citation names a node here — RFC-0034. There is no dependency, no
pin and nothing to ask a server about: the node is in the tree you are editing, so the only way
to get it wrong is not to read it.

```yaml
cites:
  - node: reach/tailwater      # <class>/<name> in this corpus
    tag: inference             # the standing THIS corpus holds that span at
    span: >-
      Discharge below the dam tracks the release schedule within a day
```

Four checks, and the third is the one with no external counterpart:

- `local-citation-unresolved` (error) — no `node:`, or this corpus holds no such node.
- `local-citation-span-drift` (error) — no `span:`, or the span is not in that node.
- `local-citation-tag-drift` (error) — the span is there and the claim governing it carries a
  different standing than the one you declared. Across a boundary a tag is the producer's and
  cannot be checked; here the producer is you.
- `local-citation-untagged` (info) — a span and no standing.

Citing an `[open]` span is allowed. Calling it `[verified]` is not.

## The tag does not transfer

The standing you record is the producer's, observed at that pin. It is a fact about what they
said, not a warrant you inherit — a `cites:` into a dependency does not discharge
`verified-unsourced` on your own claim. See the `tagging-a-claim` skill.

## If the tool is not there

`check_citation` is in the `dependencies` capability. A server that cannot resolve the
dependency set declines it at connect time rather than guessing, and refuses the call with
`capability-not-supported`. Do not fall back to writing the citation unchecked; run
`yidam lint` in the repository instead, which is the same predicate.

## Where the reasoning is

`.yidam/.vendor/prelude/guidelines/directories.md` for what `catalog/` and `cites:` are for,
and the corpus's own `.yidam/tonpa.toml` for what it has installed.
