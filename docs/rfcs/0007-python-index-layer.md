# RFC-0007 — The Python SDK index/feature layer

- **Status:** Implemented
- **Track:** I7
- **Relates to:** RFC-0002 (node-model unification), RFC-0005 (MCP tool contract), RFC-0006 (correctness reconciliation), RFC-0003 (feature-gated builds)
- **Versioning layers touched:** SDK + parity (`yidam-core` minor bump; the `embed_config` and `compose_embed_text` parity runners)
- **Downstream reference case:** Project BOSC (watermark-directory)

> **Status 2026-09-26 (#1031): two asks of three ship, the third is retracted, and this now
> reads `Implemented` on an amended scope.** The scope is amended in this block and nowhere
> else — the sections below are the document as filed, and rewriting them would erase what was
> actually proposed.
>
> **Ask 1, the runtime-verifiable embed contract, shipped and is good.**
> [`embed_config.rs`](../../yidam/cli/src/embed_config.rs) and `yidam index-verify`
> ([`cmd/index_verify.rs`](../../yidam/cli/src/cmd/index_verify.rs)) exist, the `verification`
> block exists, and the silent-drift failure [#129](https://github.com/goedelsoup/yidam/issues/129)
> described is now detectable — #536 later exercised exactly that path.
>
> **Ask 2, one canonical text assembly, shipped — in a different shape than the Proposal
> names.** It is `compose_embed_text`, a parity function in all three SDKs held to twelve
> fixtures in `prelude/sdks/parity/fixtures/compose_embed_text/`. Two departures from the
> Proposal, both deliberate:
>
> - **It is not "mirroring BOSC's proven shape (label · description · class · salient-meta)".**
>   The class stays a *column*, as the Rust reference always had it: folded into the text it
>   puts one identical phrase in every vector of a class, which pulls the class toward a point
>   and distinguishes no member of it from any other.
> - **`_meta_bits` is answered by the ontology, not by a curated list.** The judgement BOSC was
>   making by hand — *which fields carry meaning* — is now two declarations a class writes.
>   `prose: true` (#746) says which keys and properties a node's substance is in, and
>   `retrievable: true` (#717) says which identifiers and codes belong in an embedding without
>   being prose: a gage's `parameter: "00060"` is what a query is typed in and is not something
>   `node-too-long` should count. `compose_embed_text` takes both lists and unions them. So the
>   answer to "which fields" is per corpus and written down in that corpus, rather than curated
>   once in whichever consumer got there first.
>
> **It is one assembler and not the whole embedder.** Composing the text is the half that needs
> no model to check, and it is the half the two implementations disagreed on. Turning that text
> into a vector stays where the weights are.
>
> **Ask 3, `build_index` / `update_index` / `query` / `index_status` in Python, is retracted —
> not deferred.** It was filed to spare the next consumer a 285-line re-implementation, and the
> measurement since says the next consumer is not arriving: across **180 derived-repo sessions**
> and **62,048** `yidam`-bearing shell invocations, `index-build` was run **0** times and
> `retrieve` **0** times, and **0 of 16** corpora hold a local `.yidam/index/`. A second
> implementation of an index nobody builds is not a cost avoided, it is a second thing to keep
> in step with `embed.config.json`. The reconciliation subtlety below is unresolved because
> nothing now depends on resolving it; it is written down so the day a Python index is wanted
> the question is waiting rather than rediscovered. See **Open questions** for each.
>
> **`sdks/README.md` was the other half of this change, and is trimmed with it.** It typed the
> full `features` / `index` / `pipeline` surface under "the machine learning layer" and assigned
> Python ownership of it — "`embed_node` / `embed_corpus` — the only place raw text → vectors
> happens", "Rust can query but Python builds". Under `Accepted` that read as a plan; retracting
> ask 3 without trimming it would leave it a claim. The line citations in the Summary and
> Problem below point into the untrimmed README and are as-of-filing.
>
> **This header has been wrong once before, in the other direction.** It read `Implemented` on
> the strength of ask 1 alone until #718 corrected it to `Accepted` on 2026-09-17. That
> correction is why this one names each ask and its evidence rather than moving a word: a
> status is a claim about three deliverables, and a document that states it without stating
> them can be wrong for a year with every gate green.

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

Answered 2026-09-26 (#1031). Each is kept with its answer rather than deleted: a question that
turns out not to matter is worth the same sentence as one that does, and "not pursued, because
—" is the part a later reader cannot reconstruct.

- **Should the canonical embed-text assembler be a parity function, or a lower-tier
  convention?** *A parity function.* The argument in the question was the right one — it is the
  exact seam where two implementations already diverged — and a convention is what the seam
  already had. It is `compose_embed_text`, in the table in
  [`parity/README.md`](../../yidam/prelude/sdks/parity/README.md), implemented in all three
  SDKs and held to twelve fixtures.

  The question said "**ninth** parity function", and that count was stale when it was written.
  The table is the count now, deliberately: a number beside a list is a second copy of the
  list's length with nothing keeping it honest.

  One boundary the fixture draws that the question did not anticipate: *resolving* which fields
  a class declared is not in the surface. Walking `<class>.ont.yml` and `universal.yml` is a
  question about a corpus on disk and the CLI answers it; what all three SDKs must agree on is
  what a node composes to once it is answered.

- **Path (1) vs (2) of the reconciliation — quantized ONNX from Python, or fp32 with a declared
  `[known_delta]`?** *Not pursued, because ask 3 is retracted and nothing now builds an index
  from Python.* The section stands as written: the constraint is real, the two paths are still
  the two paths, and `index-verify` — which the question said this depended on — landed and is
  what surfaces a mismatch. The invariant it protects is unchanged and is enforced:
  **no index claims compatibility it does not have.** The day a second embedder exists, this is
  the first question it has to answer, and it is answerable then with a measurement rather than
  now with a guess.

- **`embed_node` takes a `CorpusNode` in the spec but both real embedders run on the YAML
  instance model.** *Settled, as the question said it would be: downstream of RFC-0002, which
  landed.* There is one node model, and `compose_embed_text` takes a `CorpusInstance`.
  `include_claims` went with the Markdown model and has no successor — what a node says is the
  declared prose, and whether a claim is in it is the class's `prose:` declaration to make.

- **Does `update_index` need true incremental LanceDB upserts, or is rebuild-whole the right
  default?** *Not pursued, with ask 3.* Rebuild-whole is what `yidam index-build` does and, at
  0 invocations across the measured population, no corpus has yet made the cost of it visible.
  The question to ask first is not incremental-versus-whole but whether an index is being built
  at all.
