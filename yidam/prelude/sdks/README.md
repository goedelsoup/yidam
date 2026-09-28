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
    Core.lean             ← type-theoretic corpus + resolution model
    lakefile.lean         ← the Lake package `lake build Yidam` compiles
    lean-toolchain        ← elan's pin, and the only one (see [tasks.verify])
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

A node is a YAML document — `.yidam/corpus/<class>/<name>.yml`. It is what the CLI walks,
what the reports count, and what all eighteen measured derived corpora commit. There is no
Markdown node type here any more; #714 retired one that three SDKs agreed about exactly and
no product called.

```
CorpusInstance
  class       : string?          — as the node declares it, never derived from its directory
  label       : string?
  description : string?          — the prose field the model has always named
  properties  : map?             — the fields the class declares, as the instance wrote them.
                                   Untyped here because the ontology is the type
  links       : CorpusLink[]?    — outbound edges only. `null` is "said nothing"; `[]` is
                                   "declared none", and they are different answers
  cites       : Citation[]?      — RFC-0019 claims on a dependency's nodes. Beside `links`
                                   and never inside it: a foreign node is not an edge target
  extra       : map              — every other top-level key, KEPT. Corpora coin prose keys
                                   (`summary`, `findings`), and dropping them made one
                                   corpus measure 21 lines where it had written 118

CorpusLink
  target       : string?
  relationship : string?
  claim_tag    : any?            — a standing, or a list of them. Carried, not interpreted
  source       : string?

Citation
  package, node, commit, tag, span : string?
```

Values inside `properties` and `extra` are the corpus's own. A YAML timestamp resolves to a
string carrying its source spelling; how any *other* non-string scalar resolves is outside
the contract, and `parity/README.md` records the measurements that put it there.

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


sourceClasses(classes) : Set<string>
                                   — free function, NOT a method: which classes are source
                                     classes is a property of the whole ontology, because an
                                     inbound relationship may be declared from either end.
                                     A class declaring NO edges is never one — it has said
                                     nothing. A self-edge does not make a class pointed at.

OntologyProperty
  name        : string
  type        : string             — string | text | date | number | ref | claim, or a type
                                     this corpus coined, which is carried through unconstrained
  description : string
  required    : boolean            — must every instance carry it? absent means false
  unit        : string             — what a number is written in; empty is dimensionless.
                                     A fact about the column, declared once, so an ordering
                                     never compares across units. Published as x-yidam-unit
  values      : string[]           — the closed set a string may hold; empty is unbounded.
                                     Declaring it closes it: the gate reports a value outside
                                     it, and the schema carries it as enum, a constraint

OntologyEdge
  relationship : string
  target       : string            — the class at the OTHER end, whichever end authors it
  direction    : string            — out | in
  description  : string
```

`compileClassSchema` turns one of these into a JSON Schema for its instances. It is
deliberately **no stricter than `yidam lint`**: declared properties are typed, and listed as
`required` for exactly those the class declared `required: true` — the same declaration
`missing-property` gates on, so the schema and the gate cannot come apart — the property bag
is closed only when the class declared any (matching `undeclared-property`, which does gate),
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
PhaseRow
  name        : string           — the phase name, humanized from the ref
  state       : string           — active | settled | position
  ref_name    : string           — the ref this row was derived from
  owner       : string           — who last committed on it
  started     : string           — the date of its first commit
  commits     : integer          — commits ahead of the baseline
```

This is what `yidam phases` emits, and no SDK implements it — the row is a CLI report shape,
carried here because the report contract is shared. Three things it deliberately does **not**
have:

- **No `kind`.** A phase's *type* — Investigation, Extraction, Synthesis, Assessment — is
  declared by the vendored kuten (`kuten/inquiry/kuten.yml`, `phases.types`) and described in
  [PHASES.md](../PHASES.md). Nothing in any repository records a type against a phase: across
  the eighteen corpora the layer was measured over there are 60 `phase/*` refs and 301
  `phase:` subjects, and not one of either encodes a type. The validator that would read one
  is filed and unbuilt.
- **No `branch`.** `state` is a lifecycle word derived from the ref namespace, not a branch
  name; `ref_name` carries the ref.
- **Not `ma/*`.** A phase lives on `phase/<name>`. `ma/<elector>` is a standing elector
  position, which never settles and is a different object — conflating the two is what had
  `yidam status` reporting 26 active phases in a repository holding one.

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

These functions form the **parity contract** — the operations all three SDKs must
implement identically, as verified by the fixture harness. `parity-check`'s `functions` loop
in `mise.toml` is the authoritative list; this section is the prose beside it, and it said
"eight" while naming eight of ten for as long as `is_recognized_verb` and
`compile_class_schema` were on the surface.

```
parse_instance(text: string) -> CorpusInstance
  Parse a corpus node from its YAML source text.
  Every top-level key the model does not name is kept in `extra` rather than dropped.
  Unreadable YAML is the empty instance, not an error: one bad file must not take a
  corpus's whole report down. No path argument — deriving a node's kind from its
  directory is the Markdown model's move, and RFC-0002 rejected it by name.

classify_commit(message: string) -> CommitEvent
  Parse a commit message into CommitEvent.
  Leading verb (before `:`) → CommitKind. Remainder → subject and optional context.
  Unknown verbs are treated as Epistemic.

parse_markers(text: string) -> Marker[]
  Every TEMPLATE marker and every REGEN block, in document order.
  A REGEN block may stand on its own lines or sit inside one: an open tag whose arrow
  is followed, on the same line, by the block's own close tag ends there, and a line
  may carry more than one. The rest of the line is prose and stays prose.
  scan_markers is this plus two more channels — each well-formed block's extent, and
  the blocks that are malformed. An extent is an offset, and an offset is not a parity
  contract: Rust counts bytes, JavaScript UTF-16 code units, Python code points. No
  fixture asserts one. See RFC-0043.

update_regen(text: string, command: string, new_content: string) -> string
  Replace the body of every REGEN block whose command is exactly `command` — exactly,
  so that a command does not claim a block whose command merely begins with it.
  Defined over scan_markers' extents rather than a search of its own, so the writer and
  the reader cannot disagree about where a block is or whether there is one.
  Leaves all other text — including other REGEN sections — unchanged exactly.
  A block written on one line is written back on one line, and one spread over several
  keeps its newlines. New content that will not fit on one line goes back in block form
  whichever the block was, which is what keeps the next call a no-op.
  Idempotent: calling it twice with the same new_content returns the same string.
  Empty new_content clears the body: no blank line between block-form markers, and
  nothing at all between the markers of an inline block.

find_reachable(edges: GraphEdge[], node_path: string) -> string[]
  All nodes reachable from node_path following directed edges (BFS).
  The start node is not included. Sorted by code point, which is not what
  JavaScript's default comparator does — see parity/README.md.

find_citations(edges: GraphEdge[], node_path: string) -> string[]
  All nodes with a directed edge pointing to node_path, sorted by code point
  and deduplicated.

is_recognized_verb(verb: string) -> bool
  Whether a leading commit verb is in the closed vocabulary — the epistemic and
  operational verb sets together. classify_commit treats anything else as Epistemic;
  this is the predicate that says whether it was recognised at all.

compile_class_schema(class: OntologyClass) -> JsonSchema
  Compile a parsed .ont.yml class definition into the JSON Schema its instances
  validate against. An empty `required` is omitted rather than written as [].
  Declared relationships are published as an x-yidam-edges annotation, not as a
  constraint on links[].relationship — see ontology.rs for why.
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
lemma UpdateRegenSpec(text: string, command: string, newContent: string)
  requires HasRegenFor(text, command)
  requires ContainsNo(newContent, RegenClose)
  ensures  result[..sp.body] == text[..sp.body]              // frame, before
  ensures  result[sp.body + |body|..] == text[sp.close..]    // frame, after
  ensures  RegenSpan(result, command) == Some(...)           // the section, exactly
  ensures  UpdateRegen(result, command, newContent) == result // idempotency
```
Everything outside the target REGEN section is byte-for-byte identical; the target section
gets exactly `new_content`, still bracketed by the same open tag and arrow; and running it
again with the same content changes nothing.

The precondition is not decoration: a caller who writes `<!-- /REGEN -->` into
`new_content` terminates the section early, and every clause above is false of that call.

A fourth clause — "no REGEN blocks are created or destroyed", over a count — used to be here
and is not, because it was false and because it was the weaker instrument. Byte-for-byte
equality says more about the blocks outside the section than a count of them can, and inside
the section the content is the caller's. `RegenBlockCountWasTheWrongInstrument` proves the
unconditional form false.

**`classify_commit` — totality and coverage**
```
function ClassifyCommit(message: string): CommitEvent
  ensures message != "" ==> result.kind == Epistemic || result.kind == Operational
```
Every non-empty commit message maps to exactly one kind. No partial function, no panics.

**`parse_markers` — no phantom markers**
```
function ParseFrom(lines: seq<string>, i: nat): seq<Marker>
  ensures forall m :: m in ParseFrom(lines, i) ==>
    exists k :: i <= k < |lines| && Opens(lines[k], m)
```
Every marker returned is one that some line of the source opens — the parser cannot invent
one. The postcondition rides on the scan itself, so every call discharges it.

The converse does **not** hold, and `ParseMarkersIsNotComplete` proves it: a REGEN block
missing its close tag swallows every marker below it, because the scan looking for
`<!-- /REGEN -->` runs to the end of the file and takes the rest of the document as that
block's content.

It no longer does so in silence. `scan_markers` (#524) returns the same marker sequence and,
beside it, the blocks whose extent it could not read the way they were meant:

```
Fault = OpenArrowMissing | CloseTagMissing | ClosedOnAnothersTag
MalformedBlock { command, line, fault, swallowed_lines, swallowed_markers }
```

`ClosedOnAnothersTag` is the one a real file carries. `CloseTagMissing` needs the damaged
block to be the *last* in the document; give it a sibling below and the scan runs past the
sibling's open tag, closes on the sibling's close tag, and returns one well-formed-looking
block with a marker quietly gone. `TheSwallowedBlockIsReported` and
`ABlockThatClosesOnAnothersTagIsReported` prove both of the model.

`parse_markers` is `scan_markers` without the other two channels and keeps its signature:
the marker sequence is the frozen contract and did not change. `yidam lint`'s
`malformed-regen-block` is the first consumer either has had in this repository, and
`update_regen` — rewritten over the extents in RFC-0043 — is the second.

Grounding is stated over *lines*, not raw substrings. The version that said a marker's
command appears in the source after `"<!-- REGEN: "` is false of the parser, which trims:
one extra space in the tag is enough. `TheSubstringFormOfGroundingIsFalse` is the witness.

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

### LEAN 4 (`spec/Core.lean`)

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
`ConstitutionBase` states Articles II, III, V and VI over the data each is about — the corpus
graph, the elector positions, the evolutions on record, the commit classifier. A domain
augmentation adds material: nodes to the corpus, evolutions to the record, obligations of its
own. `additive_augmentations_do_not_contradict` says that a base constitution plus a purely
additive augmentation that discharges its own obligations *is* a constitution, and that
nothing the base established is withdrawn — no node's claims modified, no morphism lost.

"Purely additive" is `AugmentsGraph`: every node `g` holds, `g'` holds unchanged.

One article is an obligation of the augmentation rather than an inheritance, and
`additivity_does_not_preserve_acyclicity` is why: adding nodes is exactly how a cycle
appears, so Article VI has to be re-established on the augmented corpus. The witness is two
nodes pointing at each other, added to a corpus that had none.

None of this was true before #499. The theorem's conclusion was `True`, its proof was
`trivial`, its three hypotheses were unused — Lean said so, on every build, in three
`unused variable` warnings that sat on a green run. `ConstitutionBase` was mostly `True`
placeholders for the same reason. The build is warning-free now, and that is the check: a
hypothesis that earns its place is one the proof cannot be completed without.

---

## Rust SDK (`rust/`)

*The reference implementation. The source of truth. Where the model lives in metal.*

**Crate**: `yidam-core` — published to crates.io, because the `yidam` CLI depends on it
and cannot publish until it is there. See VERSIONING.md, Layer 2.
**Additional binary**: `yidam` (the CLI — all REGEN subcommands, `mise run status`, etc.)

### API surface

```rust
// yidam_core::corpus
pub struct CorpusInstance { pub class, pub label, pub description, pub properties, pub links, pub cites, pub extra }
pub struct CorpusLink { pub target, pub relationship, pub claim_tag, pub source }
pub struct ExternalCitation { pub package, pub node, pub commit, pub tag, pub span }

pub fn parse_instance(text: &str) -> CorpusInstance
impl CorpusInstance { pub fn to_json(&self) -> serde_json::Value }

pub struct CorpusGraph { nodes: HashMap<PathBuf, CorpusNode>, ... }
pub fn load_corpus(root: &Path) -> Result<CorpusGraph>
pub fn broken_links(graph: &CorpusGraph) -> Vec<(PathBuf, Link)>
pub fn orphan_nodes(graph: &CorpusGraph) -> Vec<PathBuf>
pub fn open_questions(graph: &CorpusGraph) -> Vec<&CorpusNode>

// yidam_core::git
pub enum CommitKind { Epistemic, Operational }
pub struct CommitEvent { pub hash, pub kind, pub verb, pub subject, pub context }
pub fn classify_commit(message: &str) -> CommitEvent

pub fn active_phases(repo: &Repository) -> Result<Vec<PhaseRow>>  // would read phase/* refs
pub fn resolved_evolutions(repo: &Repository) -> Result<Vec<SanghaEvolution>>  // rigpa/*

// yidam_core::sangha
pub fn read_positions(repo: &Repository) -> Result<Vec<SanghaPosition>>
pub fn read_evolutions(repo: &Repository) -> Result<Vec<SanghaEvolution>>

// yidam_core::samudaya
pub fn load_seeds(dir: &Path) -> Result<Vec<SamudayaSeed>>

// yidam_core::markers
pub fn parse_markers(text: &str) -> Vec<Marker>
pub fn update_regen(text: &str, command: &str, new_content: &str) -> String

// yidam_core::prose — what a node says, given the keys its class declared
pub const ALWAYS: &str = "description";
pub fn of<'a>(inst: &'a CorpusInstance, keys: &[String], properties: &[String]) -> Vec<(String, &'a str)>
pub fn text(inst: &CorpusInstance, keys: &[String], properties: &[String]) -> String

// yidam_core::embed — what a node is embedded as
pub struct EmbedFields { pub prose_keys, pub prose_properties, pub retrievable_properties }
pub fn compose_embed_text(inst: &CorpusInstance, fields: &EmbedFields) -> String

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
export interface CorpusInstance { class, label, description, properties, links, cites, extra }
export interface CorpusLink { target, relationship, claim_tag, source }
export interface ExternalCitation { package, node, commit, tag, span }

export function parseInstance(text: string): CorpusInstance
export function instanceToJson(i: CorpusInstance): Record<string, unknown>

// @yidam/core/git
export type CommitKind = 'epistemic' | 'operational'
export interface CommitEvent { hash, kind, verb, subject, context }
export function classifyCommit(message: string): CommitEvent

// @yidam/core/markers
export type Marker = TemplateMarker | RegenMarker
export function parseMarkers(text: string): Marker[]
export function updateRegen(text: string, command: string, newContent: string): string
export function templateSections(text: string): TemplateMarker[]

// @yidam/core/prose — what a node says, given the keys its class declared
export const ALWAYS = 'description'
export function proseOf(inst: CorpusInstance, keys: string[], properties: string[]): [string, string][]
export function proseText(inst: CorpusInstance, keys: string[], properties: string[]): string

// @yidam/core/embed — what a node is embedded as
export interface EmbedFields { prose_keys, prose_properties, retrievable_properties }
export function composeEmbedText(inst: CorpusInstance, fields: EmbedFields): string

// @yidam/core/agent
// The distinctive TypeScript layer: semantic context assembly
export interface AgentContext {
  nodes: ScoredNode[]                // semantically retrieved, not path-followed
  openQuestions: CorpusNode[]        // nodes with open claims — natural agent entry points
  activePhases: PhaseRow[]           // in-progress phase/* branches
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

*The parity surface in Python, and the runner that holds an embedding runtime to the same
config the index was built under.*

**Package**: `yidam-core` — not published. Same reasoning as the TypeScript SDK above;
PyPI was considered and reversed. See VERSIONING.md, Layer 2.

**This section used to promise a machine-learning layer, and RFC-0007 retracted it (#1031).**
It typed `yidam_core.features`, `yidam_core.index` and `yidam_core.pipeline` — `embed_node`,
`embed_corpus`, `build_index`, `update_index`, `query`, `index_status`, `sync_index` — and
assigned Python ownership of embedding and index construction: *"the only place raw text →
vectors happens"*, *"Rust can query but Python builds"*. None of it was ever written, and a
promise in a README is what the first downstream consumer builds against: BOSC read this
section, found the hole, and wrote 285 lines of LanceDB and sentence-transformers itself —
including its own answer to *which fields of a node carry meaning*, which is the judgement the
two implementations then disagreed on.

What replaced the promise is smaller and is real. The half that had to be shared was never the
model, it was the **text** — and that is `compose_embed_text`, a parity function all three SDKs
implement identically (see [the parity surface](#the-parity-surface)). Building an index stays
in Rust, where the weights are: `yidam index-build`, `yidam index-verify`, `yidam retrieve`.
The retraction is measured rather than assumed — across 180 derived-repo sessions,
`index-build` was invoked 0 times and 0 of 16 corpora hold a local `.yidam/index/` — and
RFC-0007's open questions record what a future Python index layer would have to answer first,
so the day it is wanted the question is waiting rather than rediscovered.

### API surface

```python
# yidam_core.corpus — parity surface, same model as other SDKs
def parse_instance(text: str) -> CorpusInstance: ...
def instance_to_json(inst: CorpusInstance) -> dict: ...
# `CorpusInstance.cls`, because `class` is a Python keyword. The wire name is `class`.

# yidam_core.git
def classify_commit(commit_hash: str, message: str) -> CommitEvent: ...
def is_recognized_verb(word: str) -> bool: ...

# yidam_core.graph
def find_reachable(edges: list[GraphEdge], start: str) -> list[str]: ...
def find_citations(edges: list[GraphEdge], target: str) -> list[str]: ...

# yidam_core.markers
def parse_markers(text: str) -> MarkerScan: ...
def update_regen(text: str, command: str, new_content: str) -> str: ...

# yidam_core.ontology
def compile_class_schema(text: str) -> dict: ...

# yidam_core.uri
def parse_reference(text: str) -> Reference | None: ...
def render_reference(ref: Reference) -> str: ...
def reference_conforms(ref: Reference) -> bool: ...

# yidam_core.prose — what a node says, given the keys its class declared
ALWAYS = "description"
def of(inst, keys: list[str], properties: list[str]) -> list[tuple[str, str]]: ...
def text(inst, keys: list[str], properties: list[str]) -> str: ...

# yidam_core.embed — what a node is embedded as
@dataclass
class EmbedFields:
    prose_keys: list[str]
    prose_properties: list[str]
    retrievable_properties: list[str]

def compose_embed_text(inst, fields: EmbedFields) -> str: ...
```

### Key dependencies
- `pyyaml` — the one runtime dependency; the ontology compiler and the instance parser read YAML
- `tomllib` — parity fixture I/O (standard library since 3.11)
- `sentence-transformers` — the optional `embed` extra, scoped to
  `tests/parity/test_embed_config.py`. An embedding runtime is a *test subject* here, not a
  capability the package offers.

### What Python owns that others don't
Nothing, and that is the point of the retraction above. Python is a peer implementation of the
parity surface, held to the same fixtures as Rust and TypeScript. Its one asymmetry is the
`embed_config` runner: a Python embedding runtime is the one that can most easily be pointed at
weights other than the index's, so it is the one the fixture exists to catch — the
degrade-don't-re-embed doctrine, checked where it is most likely to be violated.

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
description = "Formal verification: Dafny specs and LEAN 4 proofs (needs lake on PATH via elan)."
# `dir`, not repo-relative paths: `lake build` resolves its lakefile from the working
# directory, so without this it looks for one at the repository root and there is none.
# The task stood here and in mise.toml for months with repo-relative paths and no `dir`,
# which is why it had never run once (#461).
dir = "prelude/sdks/spec"
tools = { "github:dafny-lang/dafny" = { version = "4.11.0", extract_all = true, bin_path = "dafny" } }
run = [
  "dafny verify graph.dfy",
  "dafny verify sangha.dfy",
  "lake build Yidam",   # LEAN 4 project named Yidam in spec/
]
```

Dafny is pinned on the task rather than in `[tools]`: it is a 200MB download for a task most
corpora will never invoke, and `mise install` runs everywhere. LEAN has no pin here at all —
`spec/lean-toolchain` is elan's own, and a second one would be a second thing to keep in step.

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
