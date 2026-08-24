# Prelude SDKs

Three language bindings for the yidam model, a shared formal specification, and a
cross-language parity harness. Together they make the prelude programmable — turning its
concepts into first-class types and operations that agents, connectors, calculators, and
web interfaces can depend on without re-deriving them from the prose.

---

## Why three languages

The yidam model spans terrain that no single runtime dominates.

**Rust** is where the model is *defined*. Graph traversal, git integration, index
maintenance, and the `yidam` CLI binary all live here. It is the reference
implementation — when the other two SDKs disagree with it, Rust wins.

**TypeScript** is where agents *work*. Agent context assembly, MCP tool definitions,
streaming corpus queries, and web bundle feeds require the TypeScript runtime ecosystem.
This SDK is the bridge between the prelude model and the LLM orchestration layer.

**Python** is where the corpus *becomes representable*. Embedding generation, feature
engineering, index construction, and statistical analysis over structured extractions are
Python-native by ecosystem gravity. This SDK owns the pipeline from corpus nodes to vectors.

These are not transliterations of each other. The type model and parity surface are shared;
everything else is language-native in idiom, dependency, and audience.

---

## Directory structure

```
prelude/sdks/
  README.md               ← this document
  spec/                   ← formal specifications (Dafny, LEAN 4)
    graph.dfy             ← corpus graph invariants, marker update correctness
    sangha.dfy            ← resolution soundness (Article V proof)
    core.lean             ← type-theoretic corpus + resolution model
  parity/                 ← cross-language parity fixtures and runner
    fixtures/             ← TOML files: input text → expected canonical output
    README.md             ← parity contract and how to add cases
  rust/                   ← Rust SDK (crate: yidam-core)
  typescript/             ← TypeScript SDK (package: @yidam/core)
  python/                 ← Python SDK (package: yidam-core)
```

---

## The canonical type model

These types are the shared vocabulary. Each SDK expresses them in its own idiom (enums,
discriminated unions, dataclasses), but the semantics are the same. Parity tests verify
that parsers and serializers produce identical values across all three.

### Corpus types

```
CorpusNode
  path        : string           — relative path from repo root
  title       : string           — first H1 heading in the file
  kind        : NodeKind         — Concept | Decision | Authored | Generated
                                   Concept: path under corpus/; Decision: path under
                                   decisions/; Authored: any other path; Generated is
                                   reserved for derived artifacts
  claims      : Claim[]
  links       : Link[]           — outbound edges only

Claim
  text        : string           — the claim sentence(s), stripped of the tag
  tag         : EvidenceTag      — Verified | Inference | Open | Implicit
  span        : Range            — byte offset in source (for round-trip fidelity)

EvidenceTag = Verified | Inference | Open | Implicit
  — Implicit: an untagged claim in a direct transcription node; valid but not explicitly marked

Link
  label       : string           — the visible link text
  target      : string           — the href (relative path or URL)
  anchor      : string?          — fragment identifier, if present
```

### Ontology types

The typed accessor for a class definition. A consumer reads this rather than reaching into
`.ont.yml` and guessing at field names — which is how a mirror of a schema drifts from the
schema.

```
OntologyClass
  name        : string             — the class; the filename's stem when the file does not say
  label       : string
  description : string
  properties  : OntologyProperty[]
  edges       : OntologyEdge[]

  isSourceClass() : bool           — declares edges, and none of them inbound. A class that
                                     declares NO edges is not one: it has said nothing.

OntologyProperty
  name        : string
  type        : string             — string | text | date | ref | claim, or a type this
                                     corpus coined, which is carried through unconstrained
  description : string

OntologyEdge
  relationship : string
  target       : string            — the class at the OTHER end, whichever end authors it
  direction    : string            — out | in
  description  : string
```

`compileClassSchema` turns one of these into a JSON Schema for its instances. It is
deliberately **no stricter than `yidam lint`**: declared properties are typed but never
`required` (because `missing-property` reports and does not gate), the property bag is
closed only when the class declared any (matching `undeclared-property`, which does gate),
and `links[].relationship` is not constrained at all — the gate licenses a relationship only
for edges landing on another instance, and JSON Schema cannot resolve a path. The declared
relationships are published as `x-yidam-edges` for completion instead of as a rule.

A consumer that rejected what the gate accepts would fail somebody's build on a file that
looked fine everywhere else. That is the failure RFC-0002 documents at the node-model layer
and RFC-0005 at the MCP layer; this is the same shape one level down, which is why the rule
about silence lives in the compiler and not in each of the three consumers.

### Git types

```
CommitEvent
  hash        : string
  kind        : CommitKind       — Epistemic | Operational
  verb        : string           — the leading word before the colon
  subject     : string           — the main clause after the colon
  context     : string?          — the em-dash tail (links to, updated after, etc.)

CommitKind = Epistemic | Operational
  — Epistemic verbs: establish, revise, link, synthesize, assess, open, close, retract
  — Operational verbs: extract, refresh, compute, index, bundle, reconcile, build, fix, regen
  — Unknown verb → Epistemic by default; operational is the marked case
```

### Sangha types

```
SanghaPosition
  elector     : string           — the elector name (from ma/<elector> ref)
  branch      : string           — full ref: refs/heads/ma/<elector>
  tip_hash    : string           — current HEAD of that branch

SanghaEvolution
  name        : string           — the evolution name (from rigpa/<evolution> ref)
  branch      : string           — full ref: refs/heads/rigpa/<evolution>
  tip_hash    : string
```

### Phase types

```
Phase
  name        : string           — short declarative label
  kind        : PhaseKind        — Investigation | Extraction | Synthesis | Assessment
  branch      : string           — the ma/* branch this phase lives on

PhaseKind = Investigation | Extraction | Synthesis | Assessment
```

### Samudaya types

```
SamudayaSeed
  path        : string           — file path within samudaya/
  kind        : SeedKind         — Axiom | Hint | Constraint | Augmentation
  constitutional : bool          — only meaningful for Augmentation kind
  content     : string           — raw file body (after frontmatter)

SeedKind = Axiom | Hint | Constraint | Augmentation
```

### Marker types

```
Marker
  kind        : MarkerKind       — Template | Regen
  instruction : string?          — Template: the instruction text inside the comment
  command     : string?          — Regen: the subcommand name (e.g., "yidam corpus-index")
  content     : string?          — Regen: current content between the open and close tags
  span        : Range            — byte range of the entire marker block in source
```

### Index types

```
IndexStatus
  backend     : string           — e.g., "lancedb"
  model       : string           — embedding model identifier
  indexed     : usize            — node count in the index
  stale       : usize            — nodes committed since last index update
  p50         : Duration         — median retrieval latency (last benchmark)
  p95         : Duration

ScoredNode
  node        : CorpusNode
  score       : f32              — similarity score from the vector query
```

---

## The parity surface

These eight functions form the **parity contract** — the operations all three SDKs must
implement identically, as verified by the fixture harness.

```
parse_node(text: string) -> CorpusNode
  Parse a corpus node from its markdown source text.
  Extracts title (first H1), kind (from directory context or explicit marker),
  claims (with evidence tags), and outbound links.

extract_claims(text: string) -> Claim[]
  Find all [verified], [inference], and [open] tags in prose.
  Each claim is the surrounding sentence(s), the tag stripped from the text, and the byte span.
  Untagged sentences in direct-transcription nodes are Implicit; in all others they are unmarked.

extract_links(text: string) -> Link[]
  Find all markdown links [label](target) and [label](target#anchor).
  Skip image links. Skip reference-style links for now.

classify_commit(message: string) -> CommitEvent
  Parse a commit message into CommitEvent.
  Leading verb (before `:`) → CommitKind. Remainder → subject and optional context.
  Unknown verbs are treated as Epistemic.

parse_markers(text: string) -> Marker[]
  Find all <!-- TEMPLATE ... --> and <!-- REGEN: cmd --> ... <!-- /REGEN --> blocks.
  Return them in document order with their spans.

update_regen(text: string, command: string, new_content: string) -> string
  Replace the content between <!-- REGEN: command --> and <!-- /REGEN --> with new_content.
  Leaves all other text — including other REGEN sections — unchanged exactly.
  Idempotent: calling it twice with the same new_content returns the same string.
  Empty new_content clears the body with no blank line left between the markers.

find_reachable(edges: GraphEdge[], node_path: string) -> string[]
  All nodes reachable from node_path following directed edges (BFS), sorted.

find_citations(edges: GraphEdge[], node_path: string) -> string[]
  All nodes with a directed edge pointing to node_path, sorted.
```

Every parity fixture is a TOML file pairing one of these functions with a representative
input and its canonical output. The parity runner calls each SDK's implementation against
every fixture and diffs the results. Rust is the reference; TS and Python must match it.

---

## Formal specification

Not everything should be tested empirically. Some invariants are worth proving.

### Dafny (`spec/graph.dfy`, `spec/sangha.dfy`)

Dafny targets the *operational correctness* of the parity surface — the properties you
can state as method postconditions and verify automatically.

**`update_regen` — the content preservation theorem**
```
method UpdateRegen(text: string, command: string, new_content: string)
    returns (result: string)
  ensures TextOutsideRegen(result, command) == TextOutsideRegen(text, command)
  ensures ContentBetweenRegen(result, command) == new_content
  ensures CountRegen(result) == CountRegen(text)
```
Informally: everything outside the target REGEN section is byte-for-byte identical;
the target section gets exactly `new_content`; no REGEN blocks are created or destroyed.

**`classify_commit` — totality and coverage**
```
function ClassifyCommit(message: string): CommitEvent
  ensures message != "" ==> result.kind == Epistemic || result.kind == Operational
```
Every non-empty commit message maps to exactly one kind. No partial function, no panics.

**`parse_markers` — no phantom markers**
```
method ParseMarkers(text: string) returns (markers: seq<Marker>)
  ensures forall m in markers :: IsValidMarker(text, m)
  ensures forall span in ValidMarkerSpans(text) :: exists m in markers :: m.span == span
```
Every marker found is real; every real marker in the text is found.

**Sangha Article V — resolution scope fidelity**
```
method Resolve(positions: seq<Position>) returns (evolution: Evolution)
  ensures forall claim in evolution.claims ::
    exists p in positions :: claim in p.claims
```
No claim appears in the resolution output unless at least one elector position held it.
This is the formal statement of "resolution is synthesis, not generation."

**Sangha Article III — provenance completeness**
```
method Resolve(positions: seq<Position>) returns (evolution: Evolution)
  ensures forall p in positions :: p.tip_hash in evolution.commit_message
  ensures forall tension in UnresolvedTensions(positions) ::
    exists node in evolution.open_questions :: tension.description == node.title
```

### LEAN 4 (`spec/core.lean`)

LEAN 4 targets the *mathematical structure* of the model — the deeper invariants that
Dafny's imperative style doesn't reach well.

**The corpus graph as a category**
Nodes are objects. Links are morphisms. The composition law holds when the graph is acyclic
(DAG structure). This gives us a principled notion of "path from A to B through the corpus"
and a basis for reasoning about reachability, communities, and cuts.

**Sangha positions as a partial order**
Each elector's `ma/*` branch is a position. Positions form a poset under semantic
entailment — if position P holds claim C and position Q also holds claim C, P ≤ Q on that
claim's axis. Rigpa synthesis is the *join* of positions in this poset, restricted to
claims present in at least one elector (Article V as a monotone join).

**Constitutional non-contradiction**
Articles I–VI can be expressed as axioms in a type theory. Domain articles added by
samudaya augmentations are additional axioms. The consistency check is: do the domain axioms
derive `False` when combined with Articles I–VI? Provably not, if domain articles are
purely additive (which is the constraint the bootstrap agent enforces).

LEAN 4 proofs here are more aspirational than the Dafny specs — they're the mathematical
skeleton of "the model is coherent" rather than "the implementation is correct." Both matter.

---

## Rust SDK (`rust/`)

*The reference implementation. The source of truth. Where the model lives in metal.*

**Crate**: `yidam-core` — published to crates.io, because the `yidam` CLI depends on it
and cannot publish until it is there. See VERSIONING.md, Layer 2.
**Additional binary**: `yidam` (the CLI — all REGEN subcommands, `mise run status`, etc.)

### API surface

```rust
// yidam_core::corpus
pub struct CorpusNode { pub path, pub title, pub kind, pub claims, pub links }
pub struct Claim { pub text, pub tag, pub span }
pub enum EvidenceTag { Verified, Inference, Open, Implicit }
pub struct Link { pub label, pub target, pub anchor }

pub fn parse_node(text: &str) -> Result<CorpusNode>
pub fn extract_claims(text: &str) -> Vec<Claim>
pub fn extract_links(text: &str) -> Vec<Link>

pub struct CorpusGraph { nodes: HashMap<PathBuf, CorpusNode>, ... }
pub fn load_corpus(root: &Path) -> Result<CorpusGraph>
pub fn broken_links(graph: &CorpusGraph) -> Vec<(PathBuf, Link)>
pub fn orphan_nodes(graph: &CorpusGraph) -> Vec<PathBuf>
pub fn open_questions(graph: &CorpusGraph) -> Vec<&CorpusNode>

// yidam_core::git
pub enum CommitKind { Epistemic, Operational }
pub struct CommitEvent { pub hash, pub kind, pub verb, pub subject, pub context }
pub fn classify_commit(message: &str) -> CommitEvent

pub fn active_phases(repo: &Repository) -> Result<Vec<Phase>>   // reads ma/* refs
pub fn resolved_evolutions(repo: &Repository) -> Result<Vec<SanghaEvolution>>  // rigpa/*

// yidam_core::sangha
pub fn read_positions(repo: &Repository) -> Result<Vec<SanghaPosition>>
pub fn read_evolutions(repo: &Repository) -> Result<Vec<SanghaEvolution>>

// yidam_core::samudaya
pub fn load_seeds(dir: &Path) -> Result<Vec<SamudayaSeed>>

// yidam_core::markers
pub fn parse_markers(text: &str) -> Vec<Marker>
pub fn update_regen(text: &str, command: &str, new_content: &str) -> String

// yidam_core::index
pub struct IndexStatus { pub backend, pub model, pub indexed, pub stale, pub p50, pub p95 }
pub struct ScoredNode { pub node: CorpusNode, pub score: f32 }
pub fn index_status(index_path: &Path) -> Result<IndexStatus>
pub fn semantic_query(index_path: &Path, query: &str, k: usize) -> Result<Vec<ScoredNode>>
```

### Key dependencies
- `git2` — repo introspection, ref reading, commit walking
- `lancedb` — vector index operations (Rust client; async)
- `pulldown-cmark` — markdown parsing for link and heading extraction
- `serde` / `toml` — samudaya frontmatter, parity fixture I/O

### What Rust owns that others don't
- The `yidam` binary and all REGEN subcommands
- Graph integrity checks (`broken_links`, `orphan_nodes`)
- Commit history walking (epistemic/operational classification over git log)
- Index maintenance (the update path from new nodes to LanceDB vectors)
- The canonical parsers — other SDKs may call the binary or FFI, or independently
  implement and verify against parity fixtures

---

## TypeScript SDK (`typescript/`)

*The agent integration layer. Where the corpus meets the context window.*

**Package**: `@yidam/core` — not published. The parity harness in this repository is the
only consumer; npm was considered and reversed. See VERSIONING.md, Layer 2.

The TypeScript SDK is not a port of the Rust SDK. It is the interface between the
prelude model and the world of LLM agents, MCP servers, streaming API calls, and web feeds.
Its primary job is **context assembly** — taking a semantic query and returning a
well-structured agent context without loading the entire corpus into memory.

### API surface

```typescript
// @yidam/core/corpus
export interface CorpusNode { path, title, kind, claims, links }
export interface Claim { text, tag, span }
export type EvidenceTag = 'verified' | 'inference' | 'open' | 'implicit'
export interface Link { label, target, anchor }

export function parseNode(text: string): CorpusNode
export function extractClaims(text: string): Claim[]
export function extractLinks(text: string): Link[]

// @yidam/core/git
export type CommitKind = 'epistemic' | 'operational'
export interface CommitEvent { hash, kind, verb, subject, context }
export function classifyCommit(message: string): CommitEvent

// @yidam/core/markers
export type Marker = TemplateMarker | RegenMarker
export function parseMarkers(text: string): Marker[]
export function updateRegen(text: string, command: string, newContent: string): string
export function templateSections(text: string): TemplateMarker[]

// @yidam/core/agent
// The distinctive TypeScript layer: semantic context assembly
export interface AgentContext {
  nodes: ScoredNode[]                // semantically retrieved, not path-followed
  openQuestions: CorpusNode[]        // nodes with open claims — natural agent entry points
  activePhases: Phase[]              // in-progress ma/* branches
  tokenEstimate: number              // approximate context consumption
}

export async function assembleContext(
  query: string,
  options: {
    indexPath: string,
    k?: number,           // default 12
    includePhases?: boolean,
    repo?: string,
  }
): Promise<AgentContext>

// Streaming variant for progressive context loading
export function streamContext(
  query: string,
  options: ContextOptions
): AsyncIterable<ScoredNode>

// MCP tool definitions — call this to register yidam tools in an MCP server
export function yidamMcpTools(options: McpOptions): McpTool[]
// The tool list is NOT restated here. It is frozen once, in
// `parity/mcp/tools.json`, and every server reads it from there.
//
// This line used to name `semantic_query, open_questions, corpus_node,
// phase_status, sangha_positions` — a list that shared exactly one name with the
// Rust server's, because each was frozen where it was written: one in a README,
// one in a test. A third implementation drifted further still. The contract is
// the fix, and a README that repeats it is the bug coming back.

// @yidam/core/bundle
// Web bundle feed generation (backing web/ REGEN sections)
export interface BundleFeed<T> {
  version: string
  exported_at: string
  nodes: T[]
}
export function exportFeed<T>(
  graph: CorpusNode[],
  schema: FeedSchema<T>,
  options: ExportOptions
): BundleFeed<T>
```

### Key dependencies
- `@anthropic-ai/sdk` — LLM API calls, streaming
- `@modelcontextprotocol/sdk` — MCP server/tool definitions
- `simple-git` — lightweight git operations (ref reading, commit log)
- `lancedb` (npm) — vector index queries (TypeScript client)
- `gray-matter` — YAML frontmatter parsing (samudaya, agent definitions)

### What TypeScript owns that others don't
- `assembleContext` — the primary agent loop entry point
- MCP tool definitions and server wiring
- Streaming corpus query (progressive loading)
- Web bundle export feeds (the contract between corpus and web layer)
- React hooks for web layer (optional, in a separate `@yidam/react` package if needed)

---

## Python SDK (`python/`)

*The machine learning layer. Where nodes become vectors and corpus becomes a searchable space.*

**Package**: `yidam-core` — not published. Same reasoning as the TypeScript SDK above;
PyPI was considered and reversed. See VERSIONING.md, Layer 2.

Python is where the corpus is transformed. This SDK does not merely provide bindings to the
Rust model — it is the primary implementation of the feature engineering pipeline and the
index construction layer. LanceDB is easier to operate from Python; embedding models live
here by ecosystem gravity.

### API surface

```python
# yidam_core.corpus — parity surface, same model as other SDKs
def parse_node(text: str) -> CorpusNode: ...
def extract_claims(text: str) -> list[Claim]: ...
def extract_links(text: str) -> list[Link]: ...

# yidam_core.git
def classify_commit(message: str) -> CommitEvent: ...

# yidam_core.markers
def parse_markers(text: str) -> list[Marker]: ...
def update_regen(text: str, command: str, new_content: str) -> str: ...

# yidam_core.features — the Python-native layer
def embed_node(
    node: CorpusNode,
    model: str,               # e.g. "sentence-transformers/all-MiniLM-L6-v2"
    include_claims: bool = True,
) -> np.ndarray: ...

def embed_corpus(
    graph: list[CorpusNode],
    model: str,
    batch_size: int = 64,
) -> EmbeddingSet: ...

class EmbeddingSet:
    nodes: list[CorpusNode]
    vectors: np.ndarray       # shape: (n_nodes, embedding_dim)
    model: str
    embedded_at: datetime

# yidam_core.index — LanceDB integration
def build_index(
    embeddings: EmbeddingSet,
    index_path: str,
) -> None: ...

def update_index(
    new_nodes: list[CorpusNode],
    index_path: str,
    model: str,
) -> IndexUpdateResult: ...

def query(
    client: lancedb.LanceTable,
    query: str,
    model: str,
    k: int = 12,
) -> list[ScoredNode]: ...

def index_status(index_path: str) -> IndexStatus: ...

# yidam_core.pipeline — batch corpus operations
# Higher-level: coordinate embed + index + status in one pass
def sync_index(
    corpus_root: str,
    index_path: str,
    model: str,
    force: bool = False,
) -> SyncResult: ...
```

### Key dependencies
- `sentence-transformers` — local embedding generation (no API call required)
- `lancedb` — embedded vector database (same db, Python client)
- `numpy` — vector operations
- `marko` or `mistletoe` — markdown parsing (for parity surface)
- `tomli` / `tomllib` — parity fixture I/O

### What Python owns that others don't
- `embed_node` / `embed_corpus` — the only place raw text → vectors happens
- `build_index` / `update_index` — index lifecycle; Rust can query but Python builds
- `sync_index` — the high-level pipeline: detect stale nodes, embed, update, report
- Statistical analysis over extracted values (assessment phases, hypothesis testing)
- `scikit-learn` integration for structured feature vectors from calculated domain data

---

## Parity testing (`parity/`)

The parity harness is the immune system of the SDK layer. When implementations diverge,
agents and tools that depend on a specific SDK will silently get different answers.
The harness makes that divergence visible before it matters.

### Fixture format

Each fixture is a TOML file with a function name, an input, and the expected canonical output:

```toml
# parity/fixtures/parse_markers_regen_basic.toml
function = "parse_markers"
input = """
# Some document

<!-- REGEN: yidam corpus-index
Fields: filename, title.
-->
_placeholder_
<!-- /REGEN -->
"""

[[expected]]
kind = "regen"
command = "yidam corpus-index"
content = "_placeholder_"
```

Fixtures cover:
- **Parse node**: heading extraction, multiline claims, claim tags, outbound links
- **Extract claims**: all three tags, implicit claims, inline tags in list items
- **Extract links**: plain links, anchored links, nested in prose, in tables
- **Classify commit**: epistemic verbs, operational verbs, unknown verbs, em-dash tails
- **Parse markers**: TEMPLATE blocks, REGEN blocks, adjacent blocks, nested-comment edge cases
- **Update regen**: basic replacement, idempotency, multiple REGEN sections coexisting

### Parity runner

```
mise run parity
```

Runs all three SDKs' parity test suites against the shared fixture directory.
Each SDK reads every fixture and produces its output in a canonical JSON form.
The runner diffs all three against Rust's output. Any divergence is a parity failure.

The fixture directory is the single source of truth; adding a fixture adds a test
to all three SDKs simultaneously.

---

## Mise tasks (additions to root `mise.toml`)

```toml
[tasks.parity]
description = "Run cross-language parity tests across all three SDKs."
run = [
  "cargo test --manifest-path prelude/sdks/rust/Cargo.toml -- parity",
  "npx vitest run prelude/sdks/typescript/",
  "python -m pytest prelude/sdks/python/tests/parity/",
]

[tasks.verify]
description = "Type-check formal specs (Dafny and LEAN 4)."
run = [
  "dafny verify prelude/sdks/spec/graph.dfy",
  "dafny verify prelude/sdks/spec/sangha.dfy",
  "lake build Yidam",   # LEAN 4 project named Yidam in spec/
]
```

These join `harness-test` and `ci` as repo-level tasks. `ci` eventually includes `parity`.

---

## Design constraints

A few things the SDKs must not do:

**Do not load the full corpus into memory to answer a query.** That is what the index layer
exists to prevent. Context assembly starts with semantic retrieval, not a full directory walk.

**Do not reimplement git operations in the TypeScript or Python SDKs.** Ref reading and
commit classification can call the Rust binary (`yidam ...`) or use a thin client.
Only Rust has `git2` as a first-class dependency.

**The Rust SDK is always the reference.** If the TypeScript or Python implementation
diverges from a parity fixture, the fix is in those SDKs — not in the fixture and not
in the Rust implementation unless the Rust implementation is demonstrably wrong. Changes
to the parity surface require updating all three SDKs simultaneously.

**Parity fixtures are immutable once merged.** Changing a fixture's expected output is
a breaking change to the cross-SDK contract. Additions are always safe; mutations are PRs.
