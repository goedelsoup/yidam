# Directory Conventions

Guidelines for what belongs in each directory of a yidam-derived repository.

After bootstrap, a derived repository has two tiers:

**Top-level** — domain work visible to collaborators and tooling:
- `agents/` — domain agent definitions
- `crates/` — Rust domain computer (connectors, calculators, index)
- `docs/` — repository documentation
- `packages/` — other-language packages in the same toolkit layer
- `web/` — optional web interface

**`.yidam/`** — yidam-managed infrastructure:
- `.yidam/catalog/` — provenance anchors for corpus knowledge
- `.yidam/corpus/` — the living knowledge graph
- `.yidam/decisions/` — structured records of choices made during this repo's life
- `.yidam/sangha/` — collective resolution protocol
- `.yidam/skills/` — domain-specific skills
- `.yidam/.vendor/` — inherited yidam prelude; not modified in derived repos

---

## `agents/`

Agent definitions for agents that operate in this repository.

**What belongs here:** Agent definitions (system prompts, role descriptions, capability
declarations) for named agents whose purpose is specific to this domain. Generic agents
inherited from yidam live in `.yidam/.vendor/prelude/`; domain-specific agents live here.

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
the index layer; it is distinct from calculators because its outputs are optimized for
retrieval quality, not domain correctness.

**The index layer:** A vector index (e.g., LanceDB) over corpus embeddings enables semantic
retrieval. The index is not the corpus; it is a derived representation of it. Maintaining an
accurate index significantly reduces token consumption: agents retrieve only the nodes
relevant to a phase rather than loading the full corpus.

**Conventions:** Standard Rust crate layout. Each crate exposes a library interface;
binaries are secondary. Prefer composability over monolithic capability.

---

## `docs/`

Documentation about this repository — its purpose, scope, domain conventions, and decisions
that shaped its structure.

**What belongs here:** Repository-level documentation written for contributors, agents, and
users of this domain. This is distinct from the corpus (which holds knowledge claims) and
the prelude (which holds yidam's model). Documentation here describes the *repository*,
not the domain.

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

---

## `web/`

Web interface layer, if applicable.

**What belongs here:** A frontend or API surface for interacting with the domain computer —
browsing the corpus, issuing retrieval queries, visualizing the graph, or surfacing synthesis.
Optional; add only when direct programmatic access to the crates/packages layer is
insufficient for the intended use.

---

## `.yidam/catalog/`

Tracks data sources, allowing corpus nodes to reference them with shallow edges rather than
embedding source metadata inline.

**What belongs here:** One file per data source — datasets, papers, APIs, databases, external
knowledge bases, tool outputs, or any external artifact the corpus draws on. A catalog node
describes the source, not the knowledge derived from it.

**Catalog node conventions:**

- Filename is a stable identifier for the source: author-year for papers (`pearl-2009.md`),
  slug for datasets and APIs (`world-bank-gdp.md`, `openai-embeddings-api.md`)
- Content: source name, type, location or access method, a one-sentence description of what
  it contains, and any access constraints
- Optional: a `used-by` list of corpus node links — makes reverse traversal explicit

**Relationship to `.yidam/corpus/`:** Corpus nodes link to catalog nodes as edges. A corpus
node on a concept that draws on a source writes `[Pearl 2009](../../catalog/pearl-2009.md)`
rather than embedding a full citation.

---

## `.yidam/corpus/`

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

**Node kinds — authored vs. generated:**

*Authored nodes* are written through deliberate knowledge work — by a human or agent
reasoning about the domain. They are stable, permanent, and not regenerable from any source.

*Generated nodes* are produced by a pipeline from a primary source — extracted, computed,
or assembled automatically. They are regenerable if the pipeline is re-run against the same
source.

Both are committed permanently. But their commit semantics differ — generated node commits
are **operational events**; authored node commits are **epistemic events**. Do not mix the
two kinds in a single commit; the log must remain readable as a knowledge record.

---

## `.yidam/decisions/`

Structured records of choices made during this repository's life — from the genesis
bootstrap onward.

**What belongs here:** One YAML file per decision. A decision is any choice that shaped the
repository's structure, ontology, or direction — confirmed ontology sketches, approved
implied edges, connector and calculator selections, governance resolutions.

**Format:**

```yaml
id: <slug>
summary: <one line — what was decided>
context: |
  <what the choice was about>
decision: |
  <what was chosen>
rationale: |
  <why this, not alternatives considered>
```

**Lifecycle:** Written during bootstrap for genesis-level choices; written by agents or the
sangha for subsequent choices. Decision files are permanent records — they are not updated
when a decision is superseded, but a new decision may reference a prior one by `id`.

---

## `.yidam/sangha/`

The collective resolution protocol. Encodes how multiple participants (agents and humans)
maintain individual positions and synthesize them into shared understanding.

**What belongs here:** Protocol documents only — not knowledge. `PROTOCOL.md` (resolution
algorithm), `resolutions/` (records of past resolution events), `electors.md` (recognized
participants). Knowledge lives in the corpus; sangha is the governance layer above it.

**Ref store:** Sangha's live state is in git refs, not in files. `refs/heads/ma/<elector>`
tracks each participant's working position; `refs/heads/rigpa/<evolution>` records settled
collective evolutions. See [GRAPH.md](../GRAPH.md) for the full encoding model.

---

## `.yidam/skills/`

Reusable capabilities available to agents in this repository.

**What belongs here:** Domain-specific skills — structured procedures agents can invoke
when working in this repo. Generic skills inherited from yidam live in `.yidam/.vendor/prelude/`;
skills that require knowledge of this domain's corpus or toolkit live here.

---

## `.yidam/.vendor/`

The inherited yidam prelude, moved here by the `vendor(yidam)` commit during bootstrap.

**What belongs here:** `prelude/` and nothing else. The vendor step moves `yidam/prelude/`
to `.yidam/.vendor/prelude/` and deletes the rest of the template.

**What deliberately does not belong here:** yidam's CLI source, its bootstrap test harness,
its design notes, and its docs site. None of them are readable, runnable, or updatable from
inside a derived repo. A vendored copy of the CLI is a fork that will never be rebuilt — the
`yidam` binary is installed from the pinned origin (`mise run yidam-build`), not compiled
from a snapshot. A vendored copy of the harness brings `HARNESS.md`, whose links point at
scenario files the derived repo does not have.

**Read-only.** Do not modify anything under `.yidam/.vendor/` in the course of domain work.
An edit here is silently discarded the next time the prelude is re-vendored, and until then
it is a local divergence nobody can see. A defect in the prelude is fixed upstream in yidam
and adopted by re-vendoring.

**Note:** Paths to inherited skills and agents use `.yidam/.vendor/prelude/` — for example,
the bootstrap skill lives at `.yidam/.vendor/prelude/skills/bootstrap.md` after genesis.

---

## `.yidam.toml` (repository root)

The provenance pin: which yidam this repository was derived from. Written by `yidam clone`
or `yidam overlay`, confirmed by the bootstrap vendor step, and updated by
`mise run yidam-vendor-update`.

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "4f2a…"      # the resolvable pin — what re-vendor and CI check out
template  = "v0.1.0"     # release tag at that commit, or "untagged"
committed = "2026-08-08" # that commit's date — how old this prelude is
```

`commit` is the field that does the work. `template` is a semantic version and is only
meaningful once the origin is tagged; a pin that records a version but no commit points at
nothing. `committed` is the *upstream* commit's date, not the date this repo last ran the
vendor step — it answers how old the prelude is, which is what staleness turns on. See
[VERSIONING.md](https://github.com/goedelsoup/yidam/blob/main/VERSIONING.md) for the three
release layers.

**Re-vendoring.** The prelude is not frozen at the repository's birth. Corrections made
upstream reach a derived repo when it re-vendors:

```
mise run yidam-vendor-status    # what you are pinned to, and what is newer
mise run yidam-vendor-update    # fetch, replace prelude/, re-pin .yidam.toml
```

The update replaces `.yidam/.vendor/prelude/` wholesale and rewrites `.yidam.toml`. It
touches nothing else — `corpus/`, `catalog/`, `decisions/`, `skills/`, `crates/`, and every
top-level file are domain-owned and are never overwritten by an update. Review the resulting
diff and commit it as its own event:

```
git commit -m "vendor(yidam): re-vendor prelude at <commit> — <what changed>"
```

Re-vendor deliberately, not reflexively. A prelude change can alter what the graph gate
accepts; adopting one is a decision worth its own commit and, if it changes conventions the
corpus depends on, its own record in `.yidam/decisions/`.

---

## `samudaya/` (transient — present only before and during bootstrap)

A transient bootstrap influence layer, consumed and committed away as part of the genesis
event. See [samudaya/README.md](../../../samudaya/README.md) for the full protocol.

**Presence after genesis is an error state.**

---

## `sadhana/` (transient — present only during bootstrap)

The scaffold template layer. Provides the initial content for each derived-repo directory.
Bootstrap reads these templates, creates the derived-repo structure from them, then deletes
this directory. Like samudaya, it does not survive genesis.

**Presence after genesis is an error state.**
