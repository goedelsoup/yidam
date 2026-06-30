# Directory Conventions

Guidelines for what belongs in each top-level directory of an yidam-derived repository.

---

## `samudaya/`

A transient bootstrap influence layer. Present only before and during the bootstrap run;
consumed and committed away as part of the genesis event.

**What belongs here:** Markdown files with `kind: axiom | hint | constraint | augmentation`
frontmatter that seed or constrain the ontology-discovery dialogue. See
[samudaya/README.md](../../samudaya/README.md) for the full protocol.

**Lifecycle:** Placed by the repo author before invoking the bootstrap agent. Removed by
the bootstrap agent in the commit immediately following genesis. Presence after genesis is
an error state.

---

## `corpus/`

The corpus is the primary knowledge store — the body of nodes that constitute the domain graph.

**What belongs here:** Domain concepts, named relationships, artifacts, open questions, and
synthesis notes. Each file is one node. Content should be written to stand alone and be
traversed in any order.

**What does not belong here:** Implementation notes, agent prompts, skill definitions, code,
or anything that describes how the repo operates rather than what it knows.

**Node conventions:**

- One concept per file; one file per concept
- Filenames are kebab-case, descriptive, and stable — renaming a node severs edges, so choose
  well. Do not include dates in filenames; the git history has dates.
- Size: 2–10 sentences is often right. If a node grows beyond a screen, decompose it.
- Every node must have at least one outgoing edge. Orphan nodes do not belong in the corpus.
- If a concept is uncertain or under investigation, mark it: prefix the title with `?` or
  open a branch. Uncertainty is valid; unlabeled speculation is not.
- Types to distinguish in content (not in filename): concept, relation, artifact, open question.

**Node kinds — authored vs. generated:**

*Authored nodes* are written through deliberate knowledge work — by a human or agent
reasoning about the domain. They are stable, permanent, and not regenerable from any
source. Examples: concept definitions, synthesis notes, open questions, hypothesis assessments.

*Generated nodes* are produced by a pipeline from a primary source — extracted, computed,
or assembled automatically. They are regenerable if the pipeline is re-run against the
same source. Examples: structured data extracted from documents, computed scenarios,
compiled entity graphs.

Both are legitimate corpus nodes and are committed permanently. But their commit semantics
differ — generated node commits are **operational events** (name the pipeline and source),
while authored node commits are **epistemic events** (name what was understood and why).
Do not mix the two kinds in a single commit; the log must remain readable as a knowledge
record.

Validation also differs: authored nodes are checked for structure and link integrity;
generated nodes are validated against a schema and reconciled against their source.

**Growth:** The corpus grows through committed inquiry. New nodes emerge from gaps identified
during traversal or synthesis. Do not add nodes preemptively — add them when an edge needs
a target that does not yet exist.

---

## `catalog/`

The catalog tracks data sources, allowing corpus nodes to reference them with shallow edges
rather than embedding source metadata inline.

**What belongs here:** One file per data source — datasets, papers, APIs, databases, external
knowledge bases, tool outputs, or any external artifact the corpus draws on. A catalog node
describes the source, not the knowledge derived from it.

**Catalog node conventions:**

- Filename is a stable identifier for the source: author-year for papers (`pearl-2009.md`),
  slug for datasets and APIs (`world-bank-gdp.md`, `openai-embeddings-api.md`)
- Content: source name, type, location or access method, a one-sentence description of what
  it contains, and any access constraints
- Optional: a `used-by` list of corpus node links — makes reverse traversal explicit

**Relationship to `corpus/`:** Corpus nodes link to catalog nodes as edges. A corpus node on
a concept that draws on a source writes `[Pearl 2009](../catalog/pearl-2009.md)` rather than
embedding a full citation. The catalog node holds all the source metadata so corpus nodes
stay focused on knowledge, not provenance.

**What does not belong here:** Derived knowledge, synthesis, or analysis. If you find yourself
writing more than a few sentences of interpretation in a catalog node, that content belongs
in a corpus node that links to this one.

---

## `crates/`

Rust crates implementing the retrieval and traversal toolkit — the computational layer that
makes the knowledge graph queryable.

**What belongs here:** Crates implementing the domain computer — the retrieval, calculation,
and feature engineering capabilities that agents use to work with the corpus. Each crate
should have a clear, narrow scope aligned to one of the three capability types below.

**The three capability types:**

*Connectors* — External-facing adapters. A connector fetches data from an API, database,
or external source and returns a validated domain model. Connectors are async, can fail,
and are cached — results are stored locally and refreshed on a TTL or on demand. Connectors
must support an offline mode (falling back to committed fixtures) so tests and analysis
remain hermetic. Name connectors by what they fetch (`nwis`, `echo`, `census`).

*Calculators* — Internal, deterministic transforms. A calculator takes domain models as
input and returns domain models as output. No network, no filesystem — pure functions.
Calculators are the right home for domain-specific computation: hydrological balance,
statistical estimation, unit conversion, graph traversal. They are fully testable without
mocking. Name calculators by what they compute (`lowflow`, `curve-number`, `et`).

*Feature engineering* — Transforms domain data into representations for retrieval and
machine learning. Takes structured corpus data (nodes, edges, extracted values) and produces
embeddings, feature vectors, or derived signals. Feature engineering bridges the corpus and
the index layer (see below); it is distinct from calculators because its outputs are
optimized for retrieval quality, not domain correctness.

**The index layer:** A vector index (e.g., LanceDB) over corpus embeddings enables semantic
retrieval — finding relevant nodes by meaning rather than by path-following or keyword.
The index is not the corpus; it is a derived representation of it. Maintaining an accurate
index significantly reduces token consumption: agents retrieve only the nodes relevant to
a phase rather than loading the full corpus. Index maintenance belongs in the crates layer.

**Purpose:** The corpus is a knowledge store; crates give agents the ability to navigate it
efficiently. An agent orchestrates connectors, calculators, and the index — it does not
implement retrieval or calculation directly.

**Conventions:** Standard Rust crate layout. Each crate exposes a library interface;
binaries are secondary. Prefer composability over monolithic capability.

---

## `packages/`

Other-language packages in the same retrieval and traversal toolkit layer as `crates/`.

**What belongs here:** Python, TypeScript, or other runtime packages implementing any of the
three capability types — connectors, calculators, or feature engineering — in a language
better suited to the task than Rust.

**When to use packages/ over crates/:** Ecosystem access (ML frameworks, embedding model
SDKs, geospatial libraries, statistical packages) often determines the language. Prefer
Rust for performance-critical retrieval and index maintenance; prefer Python or TypeScript
for ML pipelines, embedding generation, and connector targets where the upstream SDK is
already Python-native.

**Relationship to `crates/`:** These occupy the same conceptual layer — they are part of the
domain computer. The connector/calculator/feature-engineering distinction applies equally
here. A Python package implementing a data-source connector follows the same contract as a
Rust connector: async, cached, offline-aware, returning validated domain models.

---

## `agents/`

Agent definitions for agents that operate in this repository.

**What belongs here:** Agent definitions (system prompts, role descriptions, capability
declarations) for named agents whose purpose is specific to this domain. Generic agents
inherited from yidam live in the prelude; domain-specific agents live here.

---

## `skills/`

Reusable capabilities available to agents in this repository.

**What belongs here:** Domain-specific skills — structured procedures agents can invoke
when working in this repo. Generic skills inherited from yidam live in the prelude;
skills that require knowledge of this domain's corpus or toolkit live here.

---

## `sangha/`

The collective resolution protocol. Encodes how multiple participants (agents and humans)
maintain individual positions and synthesize them into shared understanding.

**What belongs here:** Protocol documents only — not knowledge. `PROTOCOL.md` (resolution
algorithm), `resolutions/` (records of past resolution events), `electors.md` (recognized
participants). Knowledge lives in the corpus; sangha is the governance layer above it.

**Ref store:** Sangha's live state is in git refs, not in files. `refs/heads/ma/<elector>`
tracks each participant's working position; `refs/heads/rigpa/<evolution>` records settled
collective evolutions. See [sangha/README.md](../../sangha/README.md) and
[GRAPH.md](../GRAPH.md) for the full encoding model.

---

## `web/`

Web interface layer, if applicable.

**What belongs here:** A frontend or API surface for interacting with the domain computer —
browsing the corpus, issuing retrieval queries, visualizing the graph, or surfacing synthesis.
Optional; add only when direct programmatic access to the crates/packages layer is
insufficient for the intended use.
