# RFC-0007 — The Python SDK index/feature layer

- **Status:** Implemented
- **Track:** I7
- **Relates to:** RFC-0002 (node-model unification), RFC-0005 (MCP tool contract), RFC-0006 (correctness reconciliation), RFC-0003 (feature-gated builds)
- **Versioning layers touched:** SDK + parity (`yidam-core` minor bump; the `embed_config` parity runner)
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

`yidam-core`'s Python README promises a machine-learning layer — `features`, `index`,
`pipeline` — and calls Python "where nodes become vectors" (`sdks/README.md:456`). None of it
exists. The package exports `corpus, git, markers` and nothing else (`__init__.py:1-3`), with
`dependencies = []` (`pyproject.toml:10`). Because the specced surface is vapor, the first
consumer built its own: BOSC's `watermark.site.yidam_index` is a 285-line LanceDB +
sentence-transformers index that re-derives `embed_node`'s text assembly, `build_index`, and
`query`, and had to invent a reconciliation story with its own `/ask` embeddings from
scratch. This RFC proposes implementing the specced surface — honoring RFC-0006's embed
reproducibility contract so a Python-built index is interchangeable with the Rust one — and
confronts the one place they cannot be: yidam's canonical weights are quantized ONNX, which
sentence-transformers cannot load.

## Problem

**The spec is complete; the code is empty.** `sdks/README.md:480-527` fully types
`embed_node(node, model, include_claims)`, `embed_corpus(graph, model, batch_size)`,
`EmbeddingSet`, `build_index`, `update_index`, `query`, `index_status`, and `sync_index`. It
even assigns ownership: "`embed_node` / `embed_corpus` — the only place raw text → vectors
happens" and "Rust can query but Python builds" (`sdks/README.md:537-542`). Yet
`yidam_core/__init__.py` exports only the three parity modules, and `pyproject.toml` declares
zero runtime dependencies. There is no `features.py`, no `index.py`, no `pipeline.py`. A
downstream author who reads the README and then imports the package finds a hole exactly where
the README is most confident.

**The concrete cost is a 285-line re-implementation.** BOSC needed a semantic index over its
corpus mirror to serve `serve --mcp` (RFC-0005). With nothing to inherit, it wrote
`watermark.site.yidam_index`:

- `node_text` (`yidam_index.py:93-99`) is a hand-rolled `embed_node` — it assembles
  `label · description · node_class · <salient meta>`, and `_meta_bits`
  (`yidam_index.py:54-90`) curates *which* fields carry meaning (kind, roles, relationship,
  hypothesis, tags, aliases) and which are noise to drop (`site`, `scope`, `lei`, `uei`). This
  is precisely the "how do you turn a node into a string" judgment `embed_node` was specced to
  own.
- `YidamVectorIndex.build` / `.query` (`yidam_index.py:169-232`) re-derive `build_index` and
  `query` over LanceDB, cosine metric, batched embedding.
- The whole reconciliation narrative in the module docstring (`yidam_index.py:9-24`) — a table
  of three vector surfaces that must share *how* they embed but not *what* they index — is work
  the SDK forced onto the consumer because it shipped no opinion about embedding at all.

**Two design constraints bound any implementation.** First, `sdks/README.md:626-627`: "Do not
load the full corpus into memory to answer a query. That is what the index layer exists to
prevent." Second, the embed-reproducibility doctrine (`embed_config.rs:15-17`): a consumer that
cannot embed with the index's exact settings "must degrade to keyword search, not embed with
different settings." An empty Python layer satisfies neither — and BOSC, absent the contract,
silently violated the second (see the reconciliation section).

## Proposal

Implement `yidam_core.features`, `yidam_core.index`, and `yidam_core.pipeline` to write the
**same durable artifacts** the Rust path writes, so the two are interchangeable.

**`embed_node` / `embed_corpus` — one canonical text assembly.** The Rust reference composes a
node's embed text in `embed.rs:20-43` (`compose_text`): `label`, then `description`, then
`Related: <link-target file-stems>.`, with class kept as a *separate column*, not in the
embedded text. BOSC's `node_text` (`yidam_index.py:93-99`) instead folds class *into* the text
and uses curated meta bits rather than link stems. These are two different embed-text functions
over the same corpus; even with identical weights they would produce different vectors. So the
first deliverable is a single canonical assembler — mirroring BOSC's proven shape (label ·
description · class · salient-meta, dropping structural provenance) — surfaced as a parity
function so Rust `compose_text` and Python `embed_node` are held to a shared TOML fixture (per
RFC-0001's report-fixture discipline and RFC-0006's parity harness). `include_claims`
(`sdks/README.md:484`) presupposes the SDK's *Markdown* `CorpusNode` (title/claims/links),
while both real embedders run on the *YAML instance* model (label/description/links); which
node type `embed_node` accepts is inseparable from **RFC-0002** and must be settled there
first — this RFC assumes the unified node.

**`build_index` / `update_index` / `query` / `index_status` over LanceDB.** `build_index` must
write the Rust path's artifact set, not a private one. The Rust `index_build`
(`index_build.rs:44-206`) creates a LanceDB table named `corpus` (`index_build.rs:19`) with
schema `(path, class, label, text, vector<FixedSizeList<Float32>>)`, then exports three durable
files: `index/corpus.arrow` (Arrow IPC, `index_build.rs:166`), `index/meta.json` (model, dim,
node_count, indexed_commit, `index_build.rs:185-188`), and `index/embed.config.json`
(`index_build.rs:196-199`). BOSC writes *none* of these — it uses a table named `yidam_nodes`
(`yidam_index.py:46`), a richer schema, and no sidecars. The Python `build_index` must emit the
canonical table name + core columns (additive columns like `uri`/`claim_tag` are fine) **and**
all three sidecars, so an index built by `yidam_core.index.build_index` is byte-compatible with
one built by `yidam index-build` — both consumable by `serve --mcp` (RFC-0005) and by the
`export_web` / `export_sqlite` exporters, which copy vectors and never re-embed. `query` reads
`embed.config.json` to embed the query with the index's settings; `index_status` returns
`meta.json` plus a staleness verdict against the corpus HEAD commit.

**`sync_index` — the stale-detection pipeline.** `sync_index(corpus_root, index_path, model,
force)` (`sdks/README.md:520-527`) is the high-level pass the README reserves for Python:
compare `meta.json.indexed_commit` against the corpus's current commit and per-node content
hashes, embed only the drifted nodes, `update_index`, and return a `SyncResult`
(added/updated/removed/unchanged counts). BOSC's index has no incremental path — it always
rebuilds whole (`yidam_index.py:169-174`) because its mirror is small; `sync_index` is the
generalization it skipped.

**The dependency story.** These modules move `yidam-core` off `dependencies = []`, but the
parity primitives must stay installable with zero native deps. Keep the split the package
*already* uses for `sentence-transformers` (`pyproject.toml:14-16`) and widen it into an
`index` extra:

```toml
[project.optional-dependencies]
index = ["sentence-transformers>=3", "lancedb>=0.13", "numpy>=1.26"]
```

`import yidam_core.corpus` stays dependency-free; `import yidam_core.index` raises a crisp
"install `yidam-core[index]`" if the extra is absent. This preserves the README's own layering
— corpus/git/markers are the parity core, features/index/pipeline are the ML layer
(`sdks/README.md:454-463`).

## The reconciliation subtlety

yidam's canonical index is fastembed with **quantized** ONNX weights: `embed.config.json`
records `model_file: onnx/model_quantized.onnx` (`embed_config.rs:28-32`), and that field is
load-bearing — "quantized and fp32 exports of the same model differ by ~1e-3 per element, far
beyond retrieval-safe tolerance." sentence-transformers loads fp32 and **cannot** load the
Xenova quantized weights. So a Python-built index is *not* automatically in the Rust index's
vector space. This is not hypothetical: the parity suite already anticipates it — `parity/README.md:76-78`
says "a runtime that cannot load the exact weights in `input.model_file` declares its measured
drift in a `[known_delta.<runtime>]` section." BOSC embeds with `sentence-transformers/all-MiniLM-L6-v2`
fp32 via `get_provider` (`yidam_index.py:41,262`) and never reconciled against the quantized
space — a silent violation of the degrade-not-re-embed doctrine that nothing surfaced.

This RFC does not paper over it. Two honest paths, and the layer must pick one *per build*, not
silently:

1. **Target the same weights.** Have `embed_node` load the quantized ONNX (via
   `optimum`/`onnxruntime`, not fp32 sentence-transformers) so the Python index lands in the
   Rust space and `embed.config.json` matches byte-for-byte. Then a Python and a Rust index are
   truly interchangeable.
2. **Declare a distinct space.** If the Python build uses fp32, it writes an `embed.config.json`
   whose `model_file`/`fastembed_model_enum` reflect that, and RFC-0006's `index-verify`
   *surfaces* the mismatch instead of letting a query silently retrieve against the wrong space.
   The Python parity runner (`python/tests/parity/test_embed_config.py`, `parity/README.md:74`)
   records the measured fp32-vs-quantized drift as its `[known_delta.python]` tolerance.

Either way the invariant holds: **no index claims compatibility it does not have.** `query`
refuses to run a query embedded under one `embed.config.json` against a table built under
another.

## Migration & compatibility

This is an additive SDK-layer change: `yidam-core` gains three modules and an `index` extra —
existing importers of `corpus`/`git`/`markers` are untouched, so a minor bump (0.1 → 0.2).
No template or bootstrap version moves.

BOSC migrates `watermark.site.yidam_index` onto `yidam_core.index` by deletion, not rewrite:
`node_text` → the canonical `embed_node`; `YidamVectorIndex.build`/`.query` → `build_index` /
`query`; `build_yidam_index` (`yidam_index.py:245-272`) becomes a thin call into
`sync_index`. What BOSC **keeps** is its integration-specific reconciliation — the three-surface
table (`yidam_index.py:9-24`) that keeps this index distinct from `data/cache/lancedb/` and the
`/ask` feed. The SDK owns *how* to embed and index; BOSC keeps owning *what* it points at
`get_provider`. If BOSC adopts path (1) above it gains Rust-index interchange for free; if it
stays on fp32 (path 2) it inherits the `index-verify` signal it never had, closing the live
silent-drift gap.

## Alternatives considered

- **Python shells out to the Rust binary for embeddings (FFI or subprocess).** Honors "Rust is
  always the reference" (`sdks/README.md:633`) and sidesteps the quantized-weights problem
  entirely — one embedder, one space. But it makes `yidam-core[index]` depend on a built,
  unpublished Rust binary (RFC-0003), which defeats "no API call required, embeddings live in
  Python" (`sdks/README.md:531,537-542`). Reasonable as a *fallback* embedder, not the default.
- **Python never builds indexes; only queries them.** fastembed is Rust-native and quantization
  is a Rust strength; let Rust own `build_index` and give Python only `query` + `index_status`.
  This contradicts the explicit spec ("Rust can query but Python builds",
  `sdks/README.md:539`) but is the most drift-safe option — worth weighing against how badly
  Python-native building is actually wanted.
- **Trim the README instead of implementing it.** Delete `sdks/README.md:480-542` and admit the
  ML layer is out of scope. Cheapest, and honest about current reality — but it strands every
  future consumer in BOSC's position of re-deriving the layer, which is the cost this RFC set
  exists to eliminate.

## Open questions

- Should the canonical embed-text assembler be a **ninth parity function** (held to a fixture
  alongside `parse_node` et al.), or a lower-tier convention? It is the exact seam where Rust
  `compose_text` and BOSC `node_text` already diverged, which argues for parity.
- Path (1) vs (2) of the reconciliation: is loading quantized ONNX from Python (optimum) cheap
  enough to make same-space the default, or is fp32 + declared `[known_delta]` the pragmatic
  answer? This depends on RFC-0006's `index-verify` landing first.
- `embed_node` takes a `CorpusNode` in the spec but both real embedders run on the YAML instance
  model — which node type it accepts cannot be decided here; it is downstream of **RFC-0002**.
- Does `update_index` need true incremental LanceDB upserts, or is BOSC's rebuild-whole posture
  (`yidam_index.py:169-174`) the right default for corpora this size, with incremental as an
  opt-in?
