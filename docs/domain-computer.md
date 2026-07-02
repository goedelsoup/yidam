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
