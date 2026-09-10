# The web interface layer

The `web/` directory in derived repos is optional. It is added when direct programmatic
access to the domain computer is insufficient. It may serve:

- Corpus browsing
- Retrieval query issuance
- Graph visualization
- Synthesis surfacing
- Hypothesis exploration

Data source: corpus directly, or a bundled export feed with a versioned contract.

## Generated status fields (from CLI)

The corpus README template includes machine-regenerated sections:

**Corpus index** (`yidam corpus-index`): per-node table with filename, title, kind,
outgoing link count, incoming link count, line count, last commit date.

**Semantic index status** (`yidam index-status`): total nodes indexed, embedding model,
index freshness (last indexed commit vs HEAD), stale node count.

**Bundle status** (`yidam bundle-status`): bundle contract version, feed list, last export
timestamp, node counts per feed, deployment target, last deploy status.

**Repository status** (`yidam status`): corpus node count, open question count, catalog
source count, index freshness, last genesis commit date. Phase counts are deliberately not in
this block. A gated block holds only what every checkout of the commit agrees on, and a count of
branch refs is not that. `yidam status --format json` carries them for a client that wants them.

**Open questions** (`yidam open-questions`): all corpus nodes whose title begins with `?`
or whose content contains `[open]` claims.
