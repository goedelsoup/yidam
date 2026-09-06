# RFC-0006 — Correctness reconciliation

- **Status:** Implemented
- **Track:** I6
- **Relates to:** RFC-0001, RFC-0002, RFC-0007
- **Versioning layers touched:** SDK+parity (primary); template (docs, embed contract)
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

This RFC is a bundle of five concrete, mostly-cheap correctness fixes plus one new
safeguard. None is a redesign; each closes a place where two parts of yidam that are
*supposed* to agree silently do not — a query provider that has drifted out of the index's
vector space with no error, an evidence marker the SDKs read one way and every real corpus
writes another, an open-question rule copied four times, and a parity gate that certifies
functions no downstream SDK implements. These are the RFC set's through-line — the parity
surface certifies the wrong things match — reduced to small, individually-fixable
discrepancies, grouped because they share a theme (drift CI cannot see) and because fixing
them piecemeal would churn the same files. Each sub-proposal carries its own problem and
proposal; a combined migration, alternatives, and open-questions section closes the RFC.

---

## 1. A runtime-verifiable embed contract

**Problem.** `embed.config.json` (`yidam/cli/src/embed_config.rs:18-64`) pins everything a
consumer needs to embed a query into the *same* space as the stored index: `model_id`,
`embedding_dim`, `model_file` (the quantized ONNX weights — `embed_config.rs:32`), `pooling`,
and `normalize`.
Doctrine is explicit that a consumer who cannot load those exact weights must **degrade to
keyword search, not re-embed with different settings** (`docs/domain-computer.md:69-70`;
`embed_config.rs:15-17`).

The gap: nothing lets a consumer *detect* that it is off-contract. The config pins the
settings but carries no witness a consumer can reproduce. Cross-runtime agreement is proven
only at CI time, in a parity fixture (`prelude/sdks/parity/fixtures/embed_config/sentence-a.toml`)
that embeds the probe `"knowledge graph traversal"` and compares the first eight normalized
dimensions within `tolerance = 1e-5`, with a looser `[known_delta.sentence-transformers]`
bound of `1e-2` for the fp32 runtime that cannot load the quantized weights. That fixture
never travels with an index, so a consumer holding only an index directory has no probe to
check itself against.

BOSC is the live instance. It embeds with `sentence-transformers/all-MiniLM-L6-v2` in **fp32**
(`watermark.retrieval.get_provider`) while yidam's space is Xenova quantized ONNX — the exact
`[known_delta.sentence-transformers]` case. The result is a different vector space and
silently degraded cosine scores that still look plausible, and nothing in the running system
surfaces it.

**Proposal.** Carry the parity fixture's witness into the contract, and give consumers a
command to check against it.

Extend `EmbedConfig` with an optional `verification` block, written by `yidam index-build`
from the same reference embedding the parity fixture uses:

```json
"verification": {
  "probe": "knowledge graph traversal",
  "prefix_dims": 8,
  "prefix": [0.0008519883, 0.023440583, -0.016719049, "…"],
  "tolerance": 1e-5,
  "known_delta": { "sentence-transformers": { "tolerance": 1e-2 } }
}
```

Add `yidam index-verify --index <dir> [--provider <cmd>]` and an SDK function
`verify_embed_provider(config, provider)`. Given an index directory and a query provider, it
asserts `embedding_dim`, `normalize`, and `pooling` match, and that the provider's embedding
of `probe` agrees with `prefix` within `tolerance` — or within a declared
`known_delta.<runtime>` bound, reusing the `YIDAM_EMBED_PARITY`/`[known_delta]` convention
already documented in `prelude/sdks/parity/README.md:65-78`. A clean match exits 0; a match
only under a `known_delta` bound exits 0 but prints the measured drift and runtime name — what
BOSC would get, turning its silent fp32 degradation into a labelled, expected one. A hard
mismatch (wrong dim, wrong normalization, or drift beyond every declared bound) fails loudly
and instructs the consumer to degrade to keyword search. Space-drift becomes a checkable error
at index-load time instead of an invisible quality regression.

---

## 2. Unify the evidence-tag vocabulary

**Problem.** The SDK claim extractors read the marker `[inferred]`: Rust
`prelude/sdks/rust/src/corpus.rs:137`, TypeScript `prelude/sdks/typescript/src/corpus.ts:55-56`,
Python `prelude/sdks/python/yidam_core/corpus.py:79-80`. All three agree with each other and
all three map that marker to an enum variant already spelled **`Inference`**
(`corpus.ts:1`, `corpus.py:10`). Meanwhile the CLI's own diff renderer
(`yidam/cli/src/cmd/diff.rs:208`) and the *entire BOSC corpus* write `[inference]`.

Because the SDKs match one spelling and the type is named after the other, the split is
invisible to the tooling that should catch it: the parity fixtures also write `[inferred]`
(`prelude/sdks/parity/fixtures/extract_claims/tagged-and-implicit.toml:7`,
`prelude/sdks/parity/fixtures/parse_node/multiple-claims.toml:10`), so cross-language parity
passes as a tautology while agreeing on a marker no real corpus uses. The consequence is not
cosmetic: a corpus line ending ` [inference]` does not match `strip_suffix(" [inferred]")` and
falls through to `Implicit` — the evidence tag is silently dropped on ingest.

**Proposal.** Canonicalize on `[inference]`. It already matches the enum variant name,
`diff.rs`, and the downstream corpus; only the SDK parsers' recognized string has to move.
During the transition the parsers accept both spellings and normalize to
`EvidenceTag::Inference`; a follow-up bump removes `[inferred]` recognition. Add a parity
fixture whose `[input]` uses `[inference]` and
whose `[expected]` tag is `Inference` — the fixture that would have caught the split, and that
fails today across all three SDKs until they are migrated together. Because claim extraction is
frozen by RFC-0001/RFC-0002, this marker change is a parity-surface contract change and moves
under those RFCs' fixture discipline.

---

## 3. Deduplicate the open-question predicate

**Problem.** "Is this node an open question?" is expressed in two independent embodiments plus
repeated inline copies. `has_open_claim(text)` (`yidam/cli/src/cmd/mod.rs:48-50`) covers only the
`[open]` half; the full rule `label.starts_with('?') || has_open_claim(text)` is written out
at each call site — `cmd/corpus.rs:44` (the `open-questions` report), `cmd/status.rs:26`
(`status`), and `cmd/export_llms.rs:18-19` (`export --format llms` ordering). The MCP server
carries a *separate* full copy, `is_open_question(node)`
(`yidam/cli/src/cmd/serve/resources.rs:13-15`), consumed by the `open_questions` tool
(`serve/tools.rs:273-287`). Four surfaces, two spellings of the same predicate, free to drift.

This is the exact predicate RFC-0001 freezes as part of the `open-questions` report contract,
so it must have one definition to freeze. The stakes are visible downstream: BOSC keys "open"
off a structured `claim_tag == open` field that never puts `?` or `[open]` into the label or
body, so a real `yidam open-questions` run over BOSC's own mirror would *under-report* — the
replica and the tool it claims parity with disagree on the same corpus. A single named
predicate is the precondition for reconciling that (RFC-0002).

**Proposal.** Collapse to one function, `fn is_open_question(label: &str, text: &str) -> bool`,
in a shared module, and route all four call sites through it. Delete `has_open_claim` and the
duplicate `serve::resources::is_open_question` in favour of it. This is a pure refactor with no
behavioral change today; its value is that RFC-0001 can then freeze exactly one rule and the
four surfaces can no longer diverge.

---

## 4. Make the parity gate real

> **Landed in #530, by a different mechanism than proposed below.** `find_reachable` and
> `find_citations` now exist in `graph.ts` and `graph.py` and are read by both runners. The
> gate is a static check rather than the emitted manifest this section describes:
> `yidam/cli/tests/parity_implementations.rs` walks the tree for definitions and each SDK's
> `tests/` for fixture-loader calls, discovering the function list from `parity-check`'s own
> `functions` loop. A manifest would have each runner report on itself; walking the sources
> asks the same question of a runner that reports nothing. The problem statement below is
> preserved as written.

**Problem.** `find_reachable` and `find_citations` are on the eight-function parity surface
(`prelude/sdks/parity/README.md:9-19`) but exist **only in the Rust SDK**
(`prelude/sdks/rust/src/graph.rs:11-41`). There is no `graph.py` or `graph.ts`, and neither
the TypeScript nor the Python SDK exports either function. Yet CI is green, because the gate
does not check what it appears to.

`mise run parity` runs `parity-check` (`mise.toml:125-137`), which iterates a hard-coded list
of the eight names and asserts only that `fixtures/<fn>/*.toml` is non-empty — a *fixture
directory* existence check. The fixture directories for `find_reachable` and `find_citations`
exist, so the check passes. The task then runs each SDK's own test command
(`mise.toml:139-147`): `cargo test -- parity`, `npm test -- parity`, `pytest tests/parity/`.
Each runner executes only the parity tests that happen to exist, so a function with no
implementation and no test in a given SDK is simply never exercised — it passes vacuously. The
gate certifies "a fixture file exists," not "all three SDKs implement and pass this function."

**Proposal.** The parity runner must assert that every one of the eight surface functions is
*exercised by a passing test in all three SDKs*, not merely that a fixture directory exists.
Concretely: each SDK's parity runner emits the set of surface-function names it actually ran
(e.g. a JSON manifest of `{function → fixtures-consumed}`), and `parity-check` reconciles those
three sets against the canonical surface list from `prelude/sdks/parity/VERSION`'s companion
manifest, failing if any (function, SDK) pair is missing. A function on the surface with no
test in a given SDK is then a hard CI failure.

Making the gate real reddens CI, so this sub-proposal also implements
`find_reachable`/`find_citations` in `graph.py` and `graph.ts`, mirroring the Rust reference
(`graph.rs:11-41`): BFS over directed `GraphEdge`s excluding the start node (sorted result),
and sorted-deduplicated incoming sources respectively. The Python build-out lands under
RFC-0007; this RFC supplies the two functions and the gate that would have flagged them.

---

## 5. Fix documentation and resolution drift

**Problem.** Two smaller inconsistencies, both about a claimed shape not matching the real one.

- **Count drift.** `VERSIONING.md:53-56` states "The six parity functions (`parse_node`,
  `extract_claims`, `extract_links`, `classify_commit`, `parse_markers`, `update_regen`)" —
  omitting `find_reachable` and `find_citations` entirely. The parity README says eight
  (`prelude/sdks/parity/README.md:2`, table at lines 9-19), and `mise.toml:128` iterates eight.
  A reader following `VERSIONING.md` would not know two surface functions exist — the same two
  that sub-proposal 4 shows are unimplemented.
- **Link-resolution drift.** graph-check decides a link is broken by a *filesystem-relative*
  existence test, `dir.join(target).exists()` (`yidam/cli/src/cmd/corpus.rs:107-108`).
  Everywhere else, links resolve to *node ids* via `resolve_link_target(source_class, target)`
  (`yidam/cli/src/model.rs:310-328`), which folds `.`/`..`, strips `.yml`, and yields a
  `<class>/<name>` id. The two can disagree on what a link points at — graph-check reports on
  the disk path while the node/report model reasons in id space.

**Proposal.** Correct `VERSIONING.md:53-56` to name all eight functions and match the "eight"
wording of the READMEs (a documentation-only patch). For link resolution, route graph-check's
broken-link check through `resolve_link_target` so "broken link" means "resolves to no known
node id," consistent with the rest of the model — or, if the filesystem-relative check is
intentional (e.g. to catch links to files outside the corpus), document the divergence at both
call sites and in the report contract (RFC-0001) so it is a decision, not an accident.

---

## Migration & compatibility

Most of this RFC is non-breaking. Sub-proposals 3 (predicate dedupe) and 5 (docs + resolution
alignment) are internal refactors or documentation with no observable output change (a
resolution fix may reclassify a handful of edge-case links; note it in the report contract).
Sub-proposal 1 adds an *optional* `verification` block — consumers ignore unknown fields per
`embed_config.rs:20-21`, and an index without the block simply cannot be `index-verify`-d
(fail-open to a warning). Sub-proposal 4 reddens CI for exactly the two SDKs that never
implemented the functions; that is the point, and it lands with the implementations.

The sensitive one is sub-proposal 2. Renaming the recognized marker from `[inferred]` to
`[inference]` is a **parity-surface contract change** — a major bump under `VERSIONING.md`
Layer 2 (`prelude/sdks/parity/VERSION`, all three SDKs in one PR). The transition is staged:
(a) parsers accept **both** spellings, normalizing to `EvidenceTag::Inference`, and the new
`[inference]` fixture lands green; (b) once downstream corpora and fixtures are confirmed on
`[inference]`, a later bump drops `[inferred]` recognition. Repos on `[inferred]` keep parsing
through window (a); BOSC, `diff.rs`, and the corpus are already on `[inference]` and gain
correctly-tagged claims the moment the parsers accept it.

## Alternatives considered

- **Ship the embed probe as a sidecar fixture rather than in `embed.config.json`.** Keeps the
  config lean, but the point is that a consumer holding only an index directory can
  self-verify; a sidecar that may not travel with the index reintroduces the gap. The block is
  optional, so lean configs remain possible.
- **Canonicalize on `[inferred]` instead of `[inference]`.** Touches fewer SDK lines, but would
  force a rename of `diff.rs:208` and the entire BOSC corpus, contradict the enum name
  (`Inference`), and move the burden onto real data rather than three parser call sites. The
  open question below records the tension.
- **Leave the parity gate as fixture-existence.** The status quo, which demonstrably let two
  unimplemented functions sit on the surface. A gate that does not check implementations is
  worse than none: it reads as coverage.
- **A single mega-fix.** Rejected in favour of five separable sub-proposals reviewable and
  revertable independently; only sub-proposal 4's two halves must land together.

## Open questions

- **Embed-checksum tolerance.** The parity fixture uses `1e-5` for quantized-native runtimes
  and `1e-2` for fp32 (`fixtures/embed_config/sentence-a.toml`). Should `index-verify` reuse
  those verbatim, or is a load-time check (`embedding_dim` + `normalize` + a coarser probe
  bound) enough for the common case, reserving tight tolerances for `embed-parity`?
- **Marker canonicalization.** Canonicalize on `[inference]` (matching BOSC, `diff.rs:208`, and
  the `Inference` enum, accepting a parity major bump), or hold at `[inferred]` and migrate the
  corpus? This RFC recommends `[inference]`; the decision belongs with RFC-0001/RFC-0002, which
  own claim extraction.
- **Parity coverage manifest shape.** Should each SDK runner emit a machine-readable
  function→fixture manifest for `parity-check` to reconcile, or should the check invoke a known
  per-SDK entrypoint per function? The former detects silent test-skips; the latter is simpler
  but couples the gate to each SDK's test layout.
- **graph-check resolution.** Is the filesystem-relative broken-link check
  (`corpus.rs:107-108`) an intentional divergence from `resolve_link_target` (catching
  out-of-corpus targets) or an accident to unify? If intentional, state it in the report
  contract (RFC-0001) rather than leave it inferred from the code.
