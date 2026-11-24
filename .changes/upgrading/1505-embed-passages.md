### A long node is indexed as passages

**`index-build` now splits a long node's text into passages of at most 1,200 characters (#1505).**

Each passage is embedded on its own, so a phrase deep in a long node is findable again.
A node that fits in one passage is one row, exactly as before.
A search returns a node once, with the passage that scored best.
See [RFC-0060](rfcs/0060-embed-passages.md) for how the text is split.

**Rebuild the index with `yidam index-build`.**

The witness in `embed.config.json` now reads `format_version: "2"` and records its `passages`.
An index built before this keeps working, with every row read as passage 0.
`yidam doctor` reports such an index as stale and names `index-build`.
This release refuses an index whose `format_version` it does not know.

**Move the producer and every consumer of a shared remote index together.**

A rebuilt remote index keys a passage as `path#n`.
An older pin cannot resolve those keys, and several passages of one node can fill its results.
A consumer on this release refuses the older witness, so the mismatch is never silent here.

**`retrieve` gains `passage` (MCP contract 0.34.0).**

A vector result's `text` is the passage that scored, and `passage` numbers it from 0.
`k` counts nodes, not passages.
Keyword search and an index built before this answer 0.

**`index-status` gains `row_count`.**

`node_count` still counts distinct nodes, and `row_count` counts the passages they were split into.
It is `null` on an index built before this.

**`export-sqlite` writes one row per passage, with a `passage` column.**

A `vec0` query returns rows, so group by `path` to get one answer per node.
[Domain computers](domain-computer.md) shows the query.

**One new check, `node-too-many-passages`, warns on a node past 32 passages.**

That is about 38,000 characters of composed text, in any class.
The finding names the passage and character counts and suggests where the node splits.
This release is a minor bump.
