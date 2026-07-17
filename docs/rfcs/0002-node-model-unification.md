# RFC-0002 — Node-model unification

- **Status:** Draft
- **Track:** I2
- **Relates to:** RFC-0001 (report contract), RFC-0005 (MCP tool contract), RFC-0006 (correctness reconciliation)
- **Versioning layers touched:** template / SDK+parity / bootstrap protocol
- **Downstream reference case:** Project BOSC (watermark-directory)

## Summary

yidam carries two disjoint node models with no bridge between them. The SDK/parity model is
Markdown (`parse_node` → title, path-derived kind, prose claims, markdown links); the CLI/report
model is YAML instances (`class`/`label`/`description`/`links[{target, relationship}]`, class from
the parent directory). They share no fields and no file format, and no code converts one into the
other. Every product a consumer integrates — `graph-check`, `lint`, `corpus-index`,
`open-questions`, the MCP server, the index — runs on the YAML model, while the parity surface and
the bootstrap quality rubric certify the Markdown model. So "parity" guarantees three SDKs agree on
a shape the tool's own outputs never touch. This RFC is the root of the set: RFC-0001 cannot freeze
report fixtures, and RFC-0005 cannot fix `get_node`, until we decide which model is canonical.

## Problem

**Model A — Markdown (SDK + parity).** `parse_node(path, text)` at `sdks/rust/src/corpus.rs:157-179`
(Python peer `python/yidam_core/corpus.py:88-108`) produces a `CorpusNode { path, title, kind,
claims, links }` (`corpus.rs:54-61`):

- `title` is the first `# ` H1 line (`corpus.rs:158-162`).
- `kind` is derived from the path prefix — `corpus/` → `Concept`, `decisions/` → `Decision`, else
  `Authored` (`corpus.rs:164-170`). `NodeKind::Generated` is declared (`corpus.rs:40`) but
  `parse_node` never emits it.
- `claims` are prose lines carrying a trailing `[verified]` / `[inferred]` / `[open]` marker, else
  `Implicit` (`extract_claims`, `corpus.rs:116-155`).
- `links` are inline `[label](target#anchor)` markdown links (`extract_links`, `corpus.rs:63-109`),
  with `#anchor` split off the destination (`corpus.rs:81-90`).

**Model B — YAML instance (CLI + reports).** An instance is a `.yml` file inside a class directory,
parsed to `CorpusInstance { class, label, description, links: [{ target, relationship }] }`
(`cli/src/parse.rs:19-32`). Discovery is `walk_corpus_instances` (`walk.rs:24-41`): `*.yml` at depth
≥ 2 under `.yidam/corpus/`, excluding `*.ont.yml`; the `.ont.yml` schema files sit at depth 1
(`walk.rs:44-59`); an instance's class is its parent directory name (used at `corpus.rs:121-129`).
The parser has no `deny_unknown_fields`, so `properties:` and any other keys are silently dropped.
The on-disk shape is specced in `docs/information-architecture.md:74-88`.

**They do not meet.** Feed a `.yml` instance to `parse_node`: it finds no `# ` H1 (title empty),
finds no `[label](target)` markdown links (`links` empty), and reads the YAML body lines as
`Implicit` prose claims — garbage. Feed a Markdown node to `graph-check`: `serde_yaml::from_str`
falls back to `CorpusInstance::default()` (`corpus.rs:82`), every field `None`, and the report flags
it `missing 'class:'`, `missing 'label:'`, and `orphan node` (`corpus.rs:85-100`). The two parsers
each produce nonsense on the other's input. The split runs all the way up: the quality rubric checks
`.md` files and "≥1 outgoing **markdown** link" (`quality-rubric.md:11-16`, S1/S2/S3/S7) — the
Markdown model — while the reports that keep a live corpus honest run entirely on Model B. Even the
information-architecture doc is internally split, describing Markdown corpus nodes at
`information-architecture.md:22-35` and YAML instances at `74-88` as if both were the graph.

**Downstream consequence.** A consumer that projects an external corpus into yidam nodes inherits
neither parser it needs and neither report. BOSC re-implemented both from scratch in Python —
`watermark.site.corpus_mirror` (~948 lines) walks and parses the YAML instance form *and*
re-derives every `graph-check`/`open-questions` rule, because no SDK offers the instance parser and
no library offers the reports. The re-derivation has already drifted: BOSC keys "open" off a
structured `claim_tag == open` field and never writes `?`/`[open]` into an instance's label or body,
so the real `yidam open-questions` — which tests `label.starts_with('?') || has_open_claim(text)`
(`corpus.rs:44`) — would under-report open questions on BOSC's own mirror. The replica and the tool
it claims parity with already disagree on the same corpus.

**The tag split.** The SDK reads `[inferred]` (`corpus.rs:137`, `corpus.py:79`) and maps it to
`EvidenceTag::Inference`; `cli/src/diff.rs:209` and every BOSC artifact write `[inference]`. The two
node models therefore also disagree on the marker vocabulary. Resolving the model boundary here
forces a decision on the canonical spelling; the details are carried in RFC-0006.

## Proposal

Decide the canonical on-disk node model, then make *that* model the parity surface. Three options:

- **(A) Canonicalize the YAML instance model.** Declare `.yidam/corpus/<class>/<name>.yml` (Model B)
  the graph's on-disk form. Add a `parse_instance` function to all three SDKs, returning
  `CorpusInstance { class, label, description, properties, links: [{ target, relationship }] }`, and
  put it on the parity surface. Reframe the Markdown `parse_node` explicitly as a *prose-ingestion
  projection*: `project_markdown(path, text) -> CorpusInstance`, mapping H1 → `label`, path prefix →
  `class`, and `[label](target#anchor)` → `links[]` (relationship defaulting to a named constant,
  anchors dropped or folded into `target`). Markdown authoring stays a supported input; it is no
  longer a second graph model, just a spec'd doorway into the canonical one.
- **(B) Keep both models, spec the projection.** Leave Model A and Model B where they are, but make
  `project_markdown → CorpusInstance` a first-class, parity-tested function with a golden fixture, so
  the relationship between the two is *defined* rather than implicit. This is (A) minus the decision
  about which side is canonical; the reports keep reading Model B, and the SDKs keep parsing Model A,
  but the bridge is now tested.
- **(C) Collapse to one Markdown-with-frontmatter model.** Give every node a `---` frontmatter block
  carrying `class` / `label` / typed `links[{target, relationship}]`, and migrate the four reports
  off `walk_corpus_instances` onto a frontmatter walk. One file format, one parser. This is the
  cleanest end state and the largest migration: every existing `.yml` instance and every `.ont.yml`
  schema in every derived repo is rewritten, and `graph-check`'s filesystem link resolution
  (`corpus.rs:107`) is reworked.

**Recommendation: (A).** The reports, MCP server, and index already run on Model B; the honest fix
is to certify the model the products use, not the one they ignore. (A) subsumes (B) — the
`project_markdown` bridge it specs *is* option (B)'s projection — while additionally settling that
YAML instances are the graph and Markdown is an ingestion path. That matches how a projecting
consumer like BOSC actually operates (it never authors Markdown corpus nodes; it emits instances)
and lets BOSC delete its re-implemented instance parser in favor of the SDK one.

**What each option costs:**

- **(A):** add + parity-test `parse_instance` in three SDKs (Rust already has the parser at
  `parse.rs:19-32`; Python/TS write it fresh); reframe `parse_node` as `project_markdown`; rewrite
  bootstrap rubric S1/S2 (`quality-rubric.md`) to check instance files and instance `links` rather
  than `.md` files and markdown links; BOSC drops its instance-walk re-implementation.
- **(B):** add + parity-test one `project_markdown` function; no rubric change, no report change, but
  the two-model split is made permanent by contract.
- **(C):** rewrite every instance and schema file in every derived repo; rework `graph-check` link
  resolution and the `.ont.yml` discovery split (`walk.rs:44-59`); largest blast radius, deferred.

The incompatibility specifics all resolve under (A): `kind` derivation moves from path prefix
(`corpus.rs:164-170`) to the explicit `class:` field, with `project_markdown` supplying the
path-based default for ingested Markdown; prose `[tag]` claims become an ingestion concern of
`project_markdown`, not a property of the canonical node; `[label](target#anchor)` links project to
`{target, relationship}` (anchor handling is an open question below); and the `[inferred]`/
`[inference]` split is pinned to one spelling (RFC-0006) at the projection boundary.

## Migration & compatibility

This is a template-layer and SDK+parity-layer change, with a bootstrap-protocol touch:

- **SDK + parity (`prelude/sdks/parity/VERSION`):** bump `0.3.0 → 0.4.0` to add `parse_instance` (and,
  under (A), `project_markdown`) to the parity surface. Ship the fixture directory and real
  Python/TS implementations, not just a directory stub — the `mise run parity` gate currently only
  checks that a fixture *directory* exists per name (see RFC-0006), so a missing impl would pass CI
  silently.
- **Template (monorepo tag `v{x.y.z}`):** minor bump for the reframed
  `docs/information-architecture.md` (collapse the two "corpus node" sections into one canonical
  instance model plus an ingestion note) and any `.ont.yml`/instance schema clarification.
- **Bootstrap protocol (`PROTOCOL_VERSION`):** bump for the rubric rewrite. S1 ("`corpus/` contains
  ≥2 `.md` files") and S2 ("≥1 outgoing markdown link") must be restated against instance files and
  instance `links`; S3 (orphans) and S7 (40-line ceiling) carry over unchanged.
- **Derived-repo migration (BOSC):** under (A), BOSC replaces its hand-rolled instance walk/parse in
  `corpus_mirror.py` with the SDK `parse_instance`, and adopts the shared open-question predicate
  (RFC-0001) — which retires the live `claim_tag == open` vs `[open]` divergence. No corpus files
  move; BOSC already writes Model B on disk.

**RFC-0001 is blocked on this.** Report golden fixtures cannot be frozen until the node model the
reports parse is settled — freezing `graph-check` output over Model B while parity still certifies
Model A would re-encode the split into the contract. RFC-0001 should be authored against the model
this RFC accepts.

## Alternatives considered

- **Dual-but-bridged, permanently (option B as the end state).** The two models may genuinely serve
  two audiences: humans authoring prose Markdown, and pipelines emitting machine YAML instances (the
  `Authored` vs never-produced `Generated` split at `corpus.rs:39-40` hints at this intent). If
  human Markdown authoring is a first-class workflow yidam wants to keep, (B) is defensible — but
  only with the projection made a tested contract, so the bridge stops being implicit. The
  recommendation folds this in: under (A) the projection still exists and is spec'd; what changes is
  that the graph has a single canonical form the reports and SDKs both name.
- **Collapse to Markdown-with-frontmatter (C).** Rejected for now on migration cost, not on merit;
  worth revisiting if a future template version already rewrites every instance for another reason.

The graph functions cut across all three options. `find_reachable` and `find_citations`
(`sdks/rust/src/graph.rs:11-41`) operate on `GraphEdge { from, to }` — bare path pairs, model-blind.
They need an *edge extractor* to feed them: from Model A that is `extract_links` (target strings);
from Model B it is `links[].target`. Whichever model wins, the graph functions survive unchanged;
only the adapter that builds the edge list moves. Notably, neither of the four reports calls these
functions today — `graph-check` resolves links itself against the filesystem (`corpus.rs:107`) — so
`find_reachable`/`find_citations` are already orphaned parity functions (they are also Rust-only; see
RFC-0006). Unifying the node model is the precondition for giving them a single, tested edge source.

## Open questions

- **Two audiences, or one?** Is human Markdown authoring a workflow yidam commits to keeping, or is
  every node ultimately an instance and Markdown merely a convenience input? The answer decides
  whether `project_markdown` is a permanent parity function (A/B) or a migration-only tool (C).
- **`Generated` nodes.** `NodeKind::Generated` is declared and never produced (`corpus.rs:40`).
  Should generated/projected nodes be instance-only by construction, making the projection the sole
  path to a `Generated` node?
- **Anchor semantics.** Markdown links carry `#anchor` (`corpus.rs:81-90`); YAML instance links have
  no anchor field (`parse.rs:28-32`). Does `project_markdown` drop anchors, or must the instance link
  schema grow one?
- **`properties:` on the parity surface.** The IA doc's instance schema has a free-form `properties:`
  map (`information-architecture.md:81`) that the Rust parser drops entirely. Should `parse_instance`
  surface and type it, or keep it opaque?
- **Tag spelling.** Fix `[inferred]` vs `[inference]` here at the projection boundary, or defer the
  whole tag reconciliation to RFC-0006? (Recommend: pin the spelling in the projection spec, keep the
  broader reconciliation in RFC-0006.)
