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
