# yidam — Design Brief for a Design System

This document bundles the conceptual, structural, and interaction material needed to create a
design system for yidam and the family of yidam-derived applications. Read it in order;
later sections assume familiarity with earlier ones.

---

## Section 1 — What yidam is

### The scripture (foundational intent)

> What you commit to shapes you.

A yidam is not found. It is chosen — and in the choosing, it begins to choose back. The form
does not complete itself. It completes *through* being held.

---

An yidam-derived repository is a **living knowledge artifact** — a structured, evolving body
of knowledge maintained collaboratively by humans and agents through a git repository.

The repository's git history **is** the knowledge graph:

- Every **file** is a knowledge node
- Every **commit** is a knowledge event
- Every **markdown link** (`[label](path)`) is a directional edge
- Every **branch** is a parallel inquiry thread
- Every **merge** is a synthesis

This is not a software project. It is a research instrument.

### What it is not

- Not a software project (there may be software in it, but that is not its nature)
- Not a static document collection
- Not a scratchpad — every commit is a permanent node in the graph

### Two kinds of commits; no others

**Epistemic**: what the corpus knows changes. A change in understanding. The commit message
is testimony — not a changelog.

**Operational**: infrastructure, tooling, pipeline work. Legitimate provenance but not a
knowledge event.

---

## Section 2 — Vocabulary

These terms carry specific meaning throughout the system and should be treated as design
tokens in their own right.

| Term | Meaning |
|------|---------|
| **corpus** | The living knowledge graph — all domain nodes and their edges |
| **catalog** | Provenance layer — one node per external data source |
| **node** | A single file; one concept, relation, artifact, or open question |
| **edge** | A markdown link between two nodes — directional |
| **corpus node** | An authored or generated domain knowledge claim |
| **catalog node** | A source descriptor (dataset, paper, API) |
| **agent** | A participant (human or AI) who commits to the graph |
| **elector** | A recognized sangha participant; maintains a `ma/*` branch |
| **sangha** | The collective of all participants; the governance layer |
| **rigpa** | *Clear seeing* — a settled collective understanding; a named branch `rigpa/<evolution>` |
| **ma** | *Voice, position* — one elector's working branch `ma/<name>` |
| **samudaya** | *Arising* — pre-bootstrap seed material; consumed at genesis |
| **sadhana** | The scaffold template layer; also consumed at genesis |
| **genesis commit** | The first commit in a derived repo; names domain, seeds ontology |
| **phase** | A bounded unit of agent inquiry: Investigation, Extraction, Synthesis, or Assessment |
| **connector** | An external-facing async adapter that fetches data into the corpus |
| **calculator** | A pure, deterministic domain computation |
| **prelude** | Inherited yidam infrastructure: identity, graph model, constitution, conduct norms |
| **BFO** | Basic Formal Ontology — foundational alignment organized around the continuant/occurrent axis |
| **UFO** | Unified Foundational Ontology — foundational alignment organized around kinds, roles, and relators |
| **foundational type** | The BFO or UFO type assigned to an ontology class; encoded in `foundational_type:` in `.ont.yml` |

### Claim confidence markers

Inline tags that annotate epistemic status within a corpus node:

| Marker | Meaning |
|--------|---------|
| `[verified]` | Supported by a committed primary source |
| `[inference]` | A reasonable conclusion from verified facts; not directly witnessed |
| `[open]` | A live question; unknown, contested, or under investigation |

---

## Section 3 — Information architecture

### Top-level directories (in a derived repo)

| Directory | Role | Lifecycle |
|-----------|------|-----------|
| `agents/` | Domain agent definitions | Permanent |
| `crates/` | Rust domain computer — connectors, calculators, index | Permanent |
| `packages/` | Other-language packages (Python, TypeScript) | Permanent |
| `web/` | Web interface layer (optional) | Permanent |
| `docs/` | Repo-level documentation | Permanent |
| `.yidam/corpus/` | The knowledge graph nodes | Permanent |
| `.yidam/catalog/` | Data source provenance nodes | Permanent |
| `.yidam/decisions/` | Structured records of choices made at bootstrap and beyond | Permanent |
| `.yidam/sangha/` | Governance protocol and resolution records | Permanent |
| `.yidam/skills/` | Domain-specific reusable agent capabilities | Permanent |
| `.yidam/.vendor/` | Inherited yidam prelude; read-only in derived repos | Permanent |
| `samudaya/` | Pre-bootstrap seed layer | Consumed at genesis |
| `sadhana/` | Scaffold templates | Consumed at genesis |

### Corpus node structure

Each corpus node is a small, focused file:

- 2–10 sentences is typically right; 40 lines is the hard ceiling
- One concept per file; one file per concept
- Kebab-case, descriptive, stable filenames (renaming severs edges)
- Must have at least one outgoing link
- Uncertainty is valid if labeled: prefix title with `?` or open a branch

**Authored nodes** — written through deliberate knowledge work. Permanent, non-regenerable.

**Generated nodes** — produced by a pipeline from a primary source. Regenerable. Committed as
operational events, not epistemic events.

### Catalog node structure

- One file per data source
- Filename: `author-year.md` for papers, `slug.md` for datasets/APIs
- Content: source name, type, location, one-sentence description, access constraints
- Optional: `used-by` list of corpus node links for reverse traversal

### Ontology class definitions

During bootstrap, the schema layer is written to `.yidam/corpus/<class>.ont.yml`. If a
foundational ontology was chosen (BFO or UFO), each class carries a `foundational_type` field;
omit it entirely for "none" alignment:

```yaml
class: <name>
label: <Human-Readable Label>
foundational_type:           # omit if alignment is "none"
  ontology: bfo | ufo
  type: <bfo or ufo type value>
description: <one sentence>
properties:
  - name: <field>
    type: string | date | ref | text
    description: <one line>
edges:
  - relationship: <verb phrase>
    target: <class name>
    direction: out | in
    description: <one line>
```

**BFO type values** (partial): `material-entity`, `occurrent`, `process`, `quality`,
`disposition`, `role`, `function`, `site`

**UFO type values**: `kind`, `subkind`, `role`, `phase`, `relator`, `mode`, `quality`,
`event`, `situation`

### Corpus instance objects

```yaml
class: <class-name>
label: <Human-Readable Instance Name>
description: <one sentence>
properties:
  <field>: <value>
links:
  - target: ../<other-class>/<other-instance>.yml
    relationship: <verb phrase>
  - target: ../<class>.ont.yml
    relationship: instance-of
```

### Decision records

`.yidam/decisions/<slug>.yml`:

```yaml
id: <slug>
summary: <one line>
context: |
  <what the choice was about>
decision: |
  <what was chosen>
rationale: |
  <why this, not alternatives considered>
```

### Resolution records

`.yidam/sangha/resolutions/<evolution>.md`:

```markdown
---
evolution: <name>
date: <YYYY-MM-DD>
tips:
  - ma/<elector>@<short-hash>
---

## What was resolved
## What changed
## What remains open
```

---

## Section 4 — The git branch model

Two ref namespaces encode the collective knowledge protocol:

### `ma/<elector>` — individual positions

Each elector (human or agent) maintains one branch as their working position. Commits here
are free — no consensus required. Positions are expected to diverge.

### `rigpa/<evolution>` — settled evolutions

When the sangha synthesizes individual positions into shared understanding, a new
`rigpa/<evolution>` branch is created. Named for what it represents. This is a stable
checkpoint; elector branches diverge again from here.

**The semantic distinction is ontological, not procedural.** `ma/` is a voice moving
toward recognition. `rigpa/` is recognition.

### Phase types

| Phase type | What happens | Outputs |
|------------|-------------|---------|
| Investigation | Agent explores a question using connectors and calculators | New corpus nodes, catalog edges, open questions |
| Extraction | Structured data pulled from a primary source | Generated nodes linked to catalog entries |
| Synthesis | Existing nodes linked or merged across threads | New edge-bearing nodes, resolved tensions |
| Assessment | Competing hypotheses evaluated against evidence | Hypothesis nodes updated, questions narrowed |

---

## Section 5 — The bootstrap flow

This is the onboarding flow for a new derived repository. It is the most interaction-dense
surface in the system.

### Overview

1. **Check for samudaya** — read pre-placed seed files if present
2. **Internalize the prelude** — output a single confirmation message; wait for acknowledgment
3. **Ontology discovery dialogue** — iterative Q&A to surface the domain's core concepts
4. **Scaffold the structure** — create directory layout from templates
5. **Formalize the ontology** — write `.ont.yml` class definitions
6. **Identify implied edges, connectors, calculators** — present a structured report; await approval
7. **Seed corpus objects** — create class directories, README, ACTIONS, and instance `.yml` files
8. **Wire edges and scaffold stubs** — connect approved edges; stub approved connectors/calculators
9. **Write genesis commit and consume transient layers** — commit, then delete samudaya/ and sadhana/
10. **Report and offer continuation** — structured handoff with next steps

### Samudaya seed kinds

Pre-placed seed files shape the bootstrap before dialogue begins:

| Kind | Behavior |
|------|----------|
| `axiom` | Concept that must appear in the corpus; treated as pre-committed |
| `hint` | Candidate relationship to surface during discovery; may be discarded |
| `constraint` | Hard scope boundary; enforced during scaffolding |
| `augmentation` | Additional prelude content; constitutional augmentations persist permanently |

### Ontology discovery sketch format

The bootstrap confirms the ontology with the user in this format before writing any files:

**Nodes**

| Node | What it is |
|------|------------|
| `name` | one-line description |

**Edges**

```
source →[relationship]→ target
```

### Prelude internalized checkpoint

Before questions begin, the bootstrap outputs a standalone message:

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor:
> [two or three bullet points]. Directory layout: [one sentence].

### Genesis commit quality criteria

The genesis commit message must:
- Name the domain
- Describe the ontology (what the seed nodes are)
- Note at least one edge (relationship between nodes)

A boilerplate message or a list of filenames fails.

### Continuation offer

After the genesis commit, the bootstrap asks:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps, and
> proposing new objects, implied edges, connectors, or calculators until the corpus reaches a
> stable initial state. Reply **yes** to continue, **no** to stop here, or describe a
> specific area to focus on.

---

## Section 6 — The sangha resolution flow

### When to resolve

Not every divergence warrants resolution. Appropriate moments:

- A shared question has been sufficiently explored across ≥2 `ma/*` branches
- An axiom is contested and dependent nodes cannot be trusted until it is settled
- A new phase of inquiry requires a common baseline

### Resolution procedure

1. **Read** — read the current tip of each participating `ma/*` branch
2. **Synthesize** — produce a corpus representing collective understanding
3. **Open tensions** — any disagreement that cannot be resolved becomes an open-question node; not silently collapsed
4. **Commit** — create the `rigpa/<evolution>` branch with a message naming what was resolved, which `ma/*` tips were read, what changed, what remains open
5. **Record** — write a resolution file to `sangha/resolutions/<evolution>.md`

### Elector registration

A participant becomes a recognized elector by:

1. Opening a `ma/<name>` branch with at least one committed position
2. Having an existing elector add them to `electors.md` on their own `ma/*` branch
3. Including the registration in the first resolution they participate in

The first elector registers themselves.

---

## Section 7 — Constitutional governance

The constitution (invariant across all derived repos):

**Article I — Primacy of the Prelude**: The prelude cannot be overridden by resolution.

**Article II — Epistemic Equality**: No elector's position is privileged by identity, seniority, or model. Human and agent electors are equal.

**Article III — Provenance**: Resolution must preserve ancestry. Unresolved tensions become open-question nodes. `ma/*` branches are never rewritten after resolution.

**Article IV — Legibility**: A resolution that cannot be described legibly must not proceed.

**Article V — Scope Fidelity**: Resolution may only synthesize knowledge present in the participating positions. It may not introduce new claims.

**Article VI — Minimal Authority**: The sangha exercises the minimum authority needed for coherence. Positions that do not conflict are inherited as-is.

---

## Section 8 — The domain computer layer

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

---

## Section 9 — The web interface layer

The `web/` directory in derived repos is optional. It is added when direct programmatic
access to the domain computer is insufficient. It may serve:

- Corpus browsing
- Retrieval query issuance
- Graph visualization
- Synthesis surfacing
- Hypothesis exploration

Data source: corpus directly, or a bundled export feed with a versioned contract.

### Generated status fields (from CLI)

The corpus README template includes machine-regenerated sections:

**Corpus index** (`yidam corpus-index`): per-node table with filename, title, kind,
outgoing link count, incoming link count, line count, last commit date.

**Semantic index status** (`yidam index-status`): total nodes indexed, embedding model,
index freshness (last indexed commit vs HEAD), stale node count.

**Bundle status** (`yidam bundle-status`): bundle contract version, feed list, last export
timestamp, node counts per feed, deployment target, last deploy status.

**Repository status** (`yidam status`): corpus node count, open question count, catalog
source count, index freshness, active phase branches, last genesis commit date.

**Open questions** (`yidam open-questions`): all corpus nodes whose title begins with `?`
or whose content contains `[open]` claims.

---

## Section 10 — Quality rubric (automated + judge)

This is the evaluation framework for bootstrap runs. It defines the quality bar the design
system must help users achieve.

### Structural checks (pass/fail)

| ID | Check |
|----|-------|
| S1 | `corpus/` exists and contains ≥2 `.md` files |
| S2 | Each corpus node has ≥1 outgoing markdown link |
| S3 | No corpus node has zero incoming AND zero outgoing links (no orphans) |
| S4 | Exactly 1 git commit exists (the genesis commit) |
| S5 | The genesis commit message is ≥3 lines |
| S6 | `agents/`, `skills/`, and `catalog/` stub directories exist |
| S7 | No corpus node exceeds 40 lines |

### Quality checks (scored `pass` / `marginal` / `fail`)

| ID | Criterion |
|----|-----------|
| Q1 | Bootstrap asked ≥2 clarifying questions before scaffolding |
| Q2 | Corpus nodes are scoped to one concept each |
| Q3 | Corpus node content is substantive and domain-specific |
| Q4 | Edges reflect real conceptual relationships (not directory citations) |
| Q5 | Genesis commit message names domain, describes ontology, notes ≥1 edge |
| Q6 | Seed nodes are at a consistent level of abstraction |
| Q7 | Ontology matches the domain's stated `good_bootstrap_looks_like` |

### Regression thresholds

A run is a regression if:
- Any structural check changes from pass → fail
- Any quality criterion drops by ≥1 band (pass → marginal, or marginal → fail)
- The corpus node count decreases

---

## Section 11 — Conduct norms (agent + design behavior)

These norms govern agents in the graph. They also describe the design's posture — slow,
deliberate, provenance-preserving.

**Commit deliberately.** Every commit is permanent. Before committing: Is this complete?
Is the message legible as a graph event? Are new nodes linked?

**Link generously.** New nodes must reference existing ones. Orphan files weaken the graph.
When adding a file, ask: what does this connect to?

**Stay within scope.** Do not add nodes speculatively. Breadth is driven by need, not
completeness anxiety.

**Make synthesis explicit.** Adding edges between existing nodes is a first-class
contribution, not housekeeping.

**Preserve provenance.** Do not delete or rewrite committed nodes without a record of why.
If a node is superseded, mark it and link to its successor.

---

## Section 12 — Test harness and multi-agent architecture

Three agents participate in every bootstrap test run:

| Agent | Role | Model |
|-------|------|-------|
| **Bootstrap** | The thing under test — runs the bootstrap skill | Varies (test matrix dimension) |
| **Domain owner** | Simulates a human answering ontology questions | Fixed (Haiku — cheap, credible) |
| **Judge** | Reads repo state and scores against rubric | Fixed (Opus — stable scorer) |

The domain owner is intentionally constrained: seed concept hints are anchors, not definitions.

### Scenario schema

Scenarios drive test runs:

```yaml
id: <kebab-case>
domain: <one-line domain description>
central_question: <the question this repo exists to investigate>
seed_concepts:
  - name: <string>
    hint: <one-sentence anchor>
good_bootstrap_looks_like: <1–2 sentences describing a successful result>
```

### Snapshot path

```
tests/results/<id>/<model>/<YYYY-MM-DD>/
  structural.json
  quality.json
  snapshot.json
```

---

## Section 13 — Aesthetic and tonal direction

The naming draws from Tibetan Buddhist epistemology (*yidam*, *rigpa*, *ma*, *sangha*,
*samudaya*, *sadhana*) and from philosophy of knowledge (corpus, epistemic, provenance).
The register is serious, contemplative, precise.

Design implications:

- Restraint over ornamentation. Every element should have a reason.
- Slow over fast: the system values deliberateness, not velocity.
- Legibility of history: the graph's past is as important as its present state.
- Equality between participants: the UI should not privilege one elector's view over another.
- Honesty at the edges: uncertainty is labeled, not hidden. `[open]`, `[inference]`,
  `[verified]` are first-class visual states.
- Synthesis as a first-class act: edges and connections deserve the same visual weight as nodes.
