# The domain computer layer

Connectors and calculators are the computational substrate agents use during phases.

### Connectors

- External-facing async adapters
- Fetch data from APIs, databases, external sources
- May fail; results cached locally and refreshed on TTL or on demand
- Must support offline mode (falling back to committed fixtures)
- Named by what they fetch: `nwis`, `echo`, `census`

**Opportunistic retrieval threshold**: when 5+ instances share a missing property from a
single connector source, invoke the connector inline rather than deferring.

### Calculators

- Pure, deterministic transforms
- No network, no filesystem; same input always produces same output
- Named by what they compute: `lowflow`, `curve-number`, `et`
- The right home for domain-specific math

### Feature engineering

- Transforms corpus data into embeddings and feature vectors
- Bridges corpus and the semantic index (e.g., LanceDB)
- Distinct from calculators: outputs optimized for retrieval quality, not domain correctness

### The index layer

A vector index over corpus embeddings enables semantic retrieval. The index is not the
corpus — it is a derived representation. Maintaining a fresh index reduces token consumption
by letting agents retrieve only relevant nodes rather than loading the full corpus.

### Embedding reproducibility

Every consumer that embeds a query against the index — the CLI, the browser agent, the MCP
server — must produce vectors that live in the same space as the stored ones. A silent
pooling, normalization, or weights-precision mismatch between runtimes degrades retrieval
without any error signal: cosine scores stay plausible, results are just quietly worse.

The contract lives in `embed.config.json`, written next to `meta.json` by
`yidam index-build` and carried into every index-bearing export:

```json
{
  "format_version": "1",
  "model_id": "Xenova/all-MiniLM-L6-v2",
  "embedding_dim": 384,
  "model_file": "onnx/model_quantized.onnx",
  "pooling": "mean",
  "normalize": true,
  "fastembed_model_enum": "AllMiniLML6V2Q"
}
```

`model_file` matters as much as `model_id`: quantized and fp32 exports of the same model
drift ~1e-2 per element, far beyond retrieval-safe tolerance. transformers.js consumers map
it to a `dtype` (`model_quantized.onnx` → `q8`).

The contract is enforced by a cross-runtime parity fixture
(`prelude/sdks/parity/fixtures/embed_config/`) asserting that fastembed (Rust),
transformers.js (TypeScript), and sentence-transformers (Python) embed the same sentence to
matching vectors. Run it with `mise run embed-parity` (downloads model weights on first
run; it is not part of the default `parity` task). sentence-transformers cannot load the
quantized ONNX export, so its looser bound is declared in the fixture's `[known_delta]`
section rather than silently widened.

A consumer that cannot satisfy the contract — the model or weights file is unavailable in
its runtime — must degrade to keyword search, not embed with different settings.

### The web agent

`yidam export --format web [--out <dir>] [--webllm-model <id>]` produces a static,
offline-capable browser agent (default output: `.yidam/web/`): `index.html`, assets, a
`web.config.json` pinning model IDs and CDN URLs, and the `.yiz` bundle itself. Serve the
directory (auto-loads the bundle) or open `index.html` directly and drop the bundle onto
the page. Query embedding follows `embed.config.json` (transformers.js, matching dtype);
retrieval is in-memory cosine over the bundled Arrow index, degrading to keyword search —
labeled, never silent — when no index exists. When WebGPU is available, a chat panel
offers RAG-grounded generation via WebLLM after explicit download consent; without WebGPU
the panel states the limitation plainly. The UI contract lives in
`yidam/design/ui_kits/web-agent/DESIGN.md`.

### Ontology interchange

`yidam export --format rdf [--rdf-format turtle|jsonld]` serializes the corpus as RDF —
Turtle (`corpus.ttl`) and JSON-LD (`corpus.jsonld`), both by default, carrying identical
triple sets. Classes become `owl:Class` (with `skos:exactMatch` when the `.ont.yml`
declares a `bfo_anchor:` URI), instances are typed individuals at
`yidam://corpus/<class>/<name>`, links are `yidam:linksTo` triples (named relationships
become `rdfs:subPropertyOf yidam:linksTo` properties), and the `owl:Ontology` header
carries provenance (`prov:generatedAtTime`, commit, genesis date). Example SPARQL:

```sparql
PREFIX yidam: <https://yidam.dev/ontology#>
SELECT ?node ?label WHERE { ?node a yidam:concept ; rdfs:label ?label . }
```

`yidam export --format graphml` serializes the link graph as GraphML (`corpus.graphml`)
for Gephi, Cytoscape, and yEd: one node per instance (stable `<class>/<name>` ids, class
and description attributes) and one directed edge per resolved link with the relationship
as its `type`. Dangling links are warned and skipped, never fatal.

### The portable vector DB

`yidam export --format sqlite [--out corpus.db]` writes the vector index as a single-file
SQLite database using the [sqlite-vec](https://github.com/asg017/sqlite-vec) extension: a
`corpus_vec` vec0 virtual table (path, class, label, text, embedding) and a `corpus_meta`
table carrying the embedding contract (model, dim, pooling, normalize — from
`embed.config.json`, never hardcoded). Vectors are copied from the already-built Arrow
index; nothing is re-embedded. Requires an index (`yidam embed && yidam index-build`).

Query from Python:

```python
import sqlite3, struct
import sqlite_vec  # pip install sqlite-vec

conn = sqlite3.connect("corpus.db")
conn.enable_load_extension(True)
sqlite_vec.load(conn)

model_id, dim, pooling, normalize = conn.execute(
    "SELECT model_id, embedding_dim, pooling, normalize FROM corpus_meta").fetchone()
# Embed the query with the SAME settings (see "Embedding reproducibility" above),
# e.g. via sentence-transformers, then:
blob = struct.pack(f"{dim}f", *query_vector)
rows = conn.execute(
    "SELECT label, path, distance FROM corpus_vec "
    "WHERE embedding MATCH ? ORDER BY distance LIMIT 5", (blob,)).fetchall()
```

Query from JS (WASM SQLite in the browser or Node):

```js
// npm install sqlite-vec  (loadable into better-sqlite3, node:sqlite, or wa-sqlite)
import * as sqliteVec from "sqlite-vec";
import Database from "better-sqlite3";

const db = new Database("corpus.db");
sqliteVec.load(db);
const rows = db.prepare(
  `SELECT label, path, distance FROM corpus_vec
   WHERE embedding MATCH ? ORDER BY distance LIMIT 5`
).all(new Float32Array(queryVector));
```

### The llms.txt context pack

`yidam export --format llms [--out llms.txt] [--token-budget <tokens>]` flattens the corpus
into a single plaintext file for dropping into any LLM's context window — the zero-dependency,
maximum-reach export. Each node becomes a short named section (`## <class>/<name>`, label,
description, `[[link]]` targets) under a provenance header (domain, generation date, commit,
node count).

With `--token-budget` the output is capped at approximately `budget × 4` characters
(1 token ≈ 4 chars — deliberately an approximation, not a tokenizer). A budget degrades
**coverage before membership**: descriptions are dropped first, leaving each node as its
label and its `[[link]]` targets, so the shape of the graph survives a budget that its prose
cannot. Only when even the labels do not fit are nodes dropped, and then round-robin across
classes — every class places its first node before any class places its second — so a small
budget yields a spread of the ontology rather than a prefix of whichever class sorts first.

Prose is spent in priority order (open-question nodes first, then by outgoing link count
descending); the node at the boundary keeps whatever description fits, cut at `[truncated]`.

Whatever the budget still cost is named at the foot of the file:

```
# Omitted: 22 nodes (concept: 22)
# Elided: 177 descriptions (label and links kept)
```

so header count always equals sections emitted plus omitted, and a budgeted pack is never
mistakable for the whole corpus. The header reads `Nodes: 177 of 199` whenever it is a slice,
and `yidam export` reports what it wrote rather than what it was given.

### The MCP server

`yidam serve --mcp` exposes the domain computer to any MCP-capable agent over stdio.
Register it in the consuming project's `.mcp.json`:

```json
{
  "mcpServers": {
    "yidam": {
      "command": "yidam",
      "args": ["serve", "--mcp"],
      "cwd": "/path/to/derived-repo"
    }
  }
}
```

**Resources** — `yidam://graph/summary` (classes, node count, open questions),
`yidam://corpus/<class>` (class listing), `yidam://corpus/<class>/<name>` (one instance),
`yidam://skills/<name>`, and `yidam://decisions/<name>`.

**Tools** — the list is frozen in
[`prelude/sdks/parity/mcp/tools.json`](../yidam/prelude/sdks/parity/mcp/tools.json) and
described in [the MCP server guide](mcp-server.md#3-the-tools-and-when-an-agent-should-reach-for-each);
it is not restated here, because the copy that used to be had already lost `list_nodes`.
Briefly: retrieval and node reads, the graph walk — undirected, and typed — the corpus's
assertions at claim granularity, a context pack for one goal with an account of what did not
fit in it, and the practice — the commit vocabulary, the evidence tags, and what a class
licenses — as calls rather than as prose to hold.

All reads come from the already-built corpus and index on disk — no live git operations.
Without a vector index, `retrieve` degrades to keyword search and marks responses with
`"degraded": true`; if HEAD has advanced past the indexed commit, the server warns on
startup (stderr) but keeps serving the stale index. Run `yidam index-build` to refresh.
