# Prelude SDKs

Three language bindings for the yidam model, a shared formal specification, and a
cross-language parity harness. Each binding reads the same corpus node, the same ontology
class, the same commit message and the same marker block, and answers the same way.

This page names only what the SDKs have. Every module, function and dependency below is in
the source tree or in a manifest. The function table in
[the parity surface](#the-parity-surface) is held to `mise.toml` and to
`parity/fixtures/` by a test, so it cannot list a function the harness does not run.

---

## Three SDKs, one surface

The SDKs are peers. Each implements the same parity surface and passes the same fixtures.
None of them reads a git repository, queries an index, assembles an agent context or serves
MCP. Those live in the `yidam` CLI (`yidam/cli`), which depends on the Rust crate.

**Rust** is the reference. The `yidam` CLI imports `yidam-core` for its parsers. When another
SDK disagrees with it on a fixture, the other SDK is wrong.

**TypeScript** (`@yidam/core`) and **Python** (`yidam-core`) are independent implementations
of the same functions. They exist so that a consumer in either language reads a node, a
marker block or a reference the way the CLI does, with no subprocess. Neither is published.
The parity harness in this repository is the only consumer of the TypeScript package, and
the Python package is installed from this tree. See `VERSIONING.md`, Layer 2.

The type model and the parity surface are shared. Names follow each language: snake_case in
Rust and Python, camelCase in TypeScript, and `cls` for `class` in Python.

---

## Directory structure

```
yidam/sdks/
  README.md               ← this document
  spec/                   ← formal specifications (Dafny, LEAN 4)
    graph.dfy             ← marker and commit models, update_regen correctness
    sangha.dfy            ← resolution soundness (Articles III and V)
    Core.lean             ← type-theoretic corpus + resolution model
    lakefile.lean         ← the Lake package `lake build Yidam` compiles
    lean-toolchain        ← elan's pin, and the only one
  parity/                 ← cross-language fixtures
    fixtures/             ← one directory per function, one TOML file per case
    mcp/                  ← the MCP tool contract (see parity/mcp/README.md)
    VERSION               ← the parity surface version
    README.md             ← the parity contract and how to add cases
  conformance/            ← what a producer that never builds yidam writes against
  rust/                   ← Rust SDK (crate: yidam-core)
  typescript/             ← TypeScript SDK (package: @yidam/core)
  python/                 ← Python SDK (package: yidam-core)
```

---

## Modules

Each SDK exports the same eight modules.

| Module | What it holds |
|---|---|
| `corpus` | `CorpusInstance`, `CorpusLink`, `ExternalCitation`, `Refusal`, `NotRefusal`; `parse_instance` and the JSON form |
| `embed` | `EmbedFields`; the text a node is embedded as, and its split into passages |
| `git` | `CommitKind`, `CommitEvent`; commit classification and the verb vocabulary |
| `graph` | `GraphEdge`; reachability and citation over a list of edges |
| `markers` | `Marker`, `MalformedBlock`, `Fault`, `RegenSpan`, `Scan`; marker parsing and `update_regen` |
| `ontology` | `OntologyClass`, `OntologyProperty`, `OntologyEdge`, `ClassParseError`; `parse_class`, `source_classes`, `compile_class_schema` |
| `prose` | `ALWAYS`; what a node says, given the keys its class declared |
| `uri` | `Kind`, `Reference`; the reference grammar of RFC-0032 |

The TypeScript `exports` map lists these eight as `@yidam/core/<module>`, and `.` re-exports
all of them. The Rust crate declares them as `pub mod` in `src/lib.rs`, and the Python
package imports them in `yidam_core/__init__.py`.

---

## The canonical type model

These types are the shared vocabulary. Each SDK expresses them in its own idiom (structs,
interfaces, dataclasses), but the semantics are the same. The parity fixtures verify that
parsers and serializers produce identical values across all three.

### Corpus types

A node is a YAML document, `.yidam/corpus/<class>/<name>.yml`. It is what the CLI walks and
what the reports count. There is no Markdown node type: #714 retired one that three SDKs
agreed about exactly and no product called.

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
  references  : string[]?        — references to things that are not nodes, as strings in the
                                   grammar of `parse_reference`. An unreadable entry is kept
  refuses     : Refusal[]?       — RFC-0045: what the node's prose declines to conclude,
                                   each {span, inference?} quoted from that prose
  not_refusals: NotRefusal[]?    — RFC-0057: each paragraph a person ruled is not a
                                   refusal, as {span}, so a proposer does not flag it again
  extra       : map              — every other top-level key, KEPT. Corpora coin prose keys
                                   (`summary`, `findings`), and dropping them made one
                                   corpus measure 21 lines where it had written 118

CorpusLink
  target       : string?
  relationship : string?
  claim_tag    : any?            — a standing, or a list of them. Carried, not interpreted
  source       : string?

Citation (ExternalCitation)
  package, node, commit, tag, span : string?
```

Values inside `properties` and `extra` are the corpus's own. A YAML timestamp resolves to a
string carrying its source spelling. How any other non-string scalar resolves is outside the
contract, and `parity/README.md` records the measurements that put it there.

### Ontology types

The typed accessor for a class definition. A consumer reads this rather than reaching into
`.ont.yml` and guessing at field names, which is how a mirror of a schema drifts from the
schema.

```
OntologyClass
  name        : string             — the class; the filename's stem when the file does not say
  label       : string
  description : string
  properties  : OntologyProperty[]
  edges       : OntologyEdge[]

source_classes(classes) : set of class names
                                   — a free function, not a method: which classes are source
                                     classes is a property of the whole ontology, because an
                                     inbound relationship may be declared from either end.
                                     A class declaring NO edges is never one — it has said
                                     nothing. A self-edge does not make a class pointed at.

OntologyProperty
  name        : string
  type        : string             — string | text | date | number | ref | claim | quotation
                                     | list, or a type this corpus coined, which is carried
                                     through unconstrained. A quotation is {of, span, sha256?},
                                     or a non-empty list of them. A list is a non-empty array
  description : string
  required    : boolean            — must every instance carry it? absent means false
  unit        : string             — what a number is written in; empty is dimensionless.
                                     A fact about the column, declared once, so an ordering
                                     never compares across units. Published as x-yidam-unit
  values      : string[]           — the closed set a string may hold; empty is unbounded.
                                     Declaring it closes it: the gate reports a value outside
                                     it, and the schema carries it as enum, a constraint
  items       : string             — on a list: the scalar type every item has, one of
                                     string | text | ref | date | number | claim. The
                                     list's values and unit apply to each item
  fields      : OntologyProperty[] — on a list: the record every item is, closed against
                                     keys it does not name. Exclusive with items; both, or
                                     a field typed list or quotation, compiles to true

OntologyEdge
  relationship : string
  target       : string            — the class at the OTHER end, whichever end authors it
  direction    : string            — out | in
  description  : string
```

`parse_class(name, content)` reads one `.ont.yml` into an `OntologyClass`. `name` is the
file's stem, and it names the class: a `class:` field inside the document is not read. A
document that does not parse, or whose top level is not a mapping, is refused with a
`ClassParseError` rather than read as an empty class. An empty document declares nothing.
A top-level key of the wrong shape reads as absent, and the rest of the class is kept.
`compile_class_schema` turns one into a JSON Schema for its instances. It is deliberately
**no stricter than `yidam lint`**: declared properties are typed, and listed as `required`
for exactly those the class declared `required: true` — the same declaration
`missing-property` gates on, so the schema and the gate cannot come apart. The property bag
is closed only when the class declared any, matching `undeclared-property`, which does gate.
`links[].relationship` is not constrained at all: the gate licenses a relationship only for
edges landing on another instance, and JSON Schema cannot resolve a path. The declared
relationships are published as `x-yidam-edges` for completion instead of as a rule.

A consumer that rejected what the gate accepts would fail somebody's build on a file that
looked fine everywhere else. That is the failure RFC-0002 documents at the node-model layer
and RFC-0005 at the MCP layer. The rule about silence lives in the compiler and not in each
of the three consumers.

### Commit types

```
CommitEvent
  hash        : string
  kind        : CommitKind       — Epistemic | Operational
  verb        : string           — the text before the first ": " in the first line
  subject     : string           — the text after it
  context     : string?          — always null today; no SDK splits a tail off the subject
```

`classify_commit` is total. A verb in the operational set is `Operational`. Every other verb,
and a message with no `: `, is `Epistemic`. The operational verbs are the
`OPERATIONAL_VERBS` constant in each SDK, and the recognised epistemic verbs are
`EPISTEMIC_VERBS`. Both match the two verb sets in `graph.dfy` today, and nothing checks
that. `is_recognized_verb` says whether a verb is
in either set, which is the question `classify_commit` does not ask.

### Marker types

```
Marker  =  Template { instruction }  |  Regen { command, content }

Scan
  markers   : Marker[]             — what parse_markers returns
  malformed : MalformedBlock[]     — blocks whose extent the scan could not read as meant
  regen     : RegenSpan[]          — where each well-formed block sits

MalformedBlock { command, line, fault, swallowed_lines, swallowed_markers }
Fault = OpenArrowMissing | CloseTagMissing | ClosedOnAnothersTag
```

`RegenSpan` carries offsets into the text: bytes in Rust, UTF-16 code units in TypeScript,
code points in Python. No fixture asserts an offset. See RFC-0043.

### Graph and reference types

```
GraphEdge { from, to }           — a directed edge between two node paths

Reference
  corpus   : string?
  kind     : Kind                — node | crate | catalog | skill | decision | issue
  path     : string              — `<class>/<name>` for a node, one segment for the rest
  rev      : string?
  fragment : string?
```

`Reference` is RFC-0032's `yidam://<corpus>/<kind>/<path>[@<rev>][#<property>]`.

---

## The parity surface

These functions are the parity contract: the operations all three SDKs implement
identically, as verified by the fixture harness. The list is the `functions=` line of the
`parity-check` task in `mise.toml`. `parity/fixtures/<function>/` holds the cases for each.
The gate `yidam/cli/tests/gates/parity_implementations.rs` fails when this table, that line
and that directory disagree. Signatures are in Rust form.

| Function | Signature and contract |
|---|---|
| `parse_instance` | `(text: &str) -> CorpusInstance`. Parse a corpus node from its YAML source text. Every top-level key the model does not name is kept in `extra`. Unreadable YAML is the empty instance, not an error, so one bad file cannot take a corpus's report down. There is no path argument: deriving a node's kind from its directory is the Markdown model's move, and RFC-0002 rejected it by name. |
| `classify_commit` | `(hash: &str, message: &str) -> CommitEvent`. Split the first line of a message at `: ` into verb and subject, and set the kind from the verb. Unknown verbs are `Epistemic`. |
| `is_recognized_verb` | `(verb: &str) -> bool`. Whether a leading commit verb is in the closed vocabulary, the operational and epistemic sets together. |
| `parse_markers` | `(text: &str) -> Vec<Marker>`. Every TEMPLATE marker and every REGEN block, in document order. A REGEN block may stand on its own lines or sit inside one. `scan_markers` is this plus the extent of each well-formed block and the list of malformed ones. |
| `update_regen` | `(text: &str, command: &str, new_content: &str) -> String`. Replace the body of every REGEN block whose command is exactly `command`. It is defined over `scan_markers`' extents, so the writer and the reader cannot disagree about where a block is. All other text is unchanged. It is idempotent. Empty content clears the body. |
| `find_reachable` | `(edges: &[GraphEdge], node_path: &str) -> Vec<String>`. All nodes reachable from `node_path` along directed edges, breadth first. The start node is not included. Sorted by code point. |
| `find_citations` | `(edges: &[GraphEdge], node_path: &str) -> Vec<String>`. All nodes with a directed edge pointing to `node_path`, sorted by code point and deduplicated. |
| `compile_class_schema` | `(class: &OntologyClass) -> Value`. Compile a parsed class definition into the JSON Schema its instances validate against. An empty `required` is omitted. |
| `parse_reference` | `(input: &str) -> Option<Reference>`. Parse the reference grammar, its relative forms and its legacy spellings. Total: `None` only for input that names no thing at all. |
| `render_reference` | `(r: &Reference) -> String`. Render a `Reference` back to a string. The only place an identifier is built. |
| `reference_conforms` | `(r: &Reference) -> bool`. Whether every segment is a slug, so it needs no escaping in any rendering, and whether its fragment names something for its kind. |
| `compose_embed_text` | `(inst: &CorpusInstance, fields: &EmbedFields) -> String`. Compose the one string a node is embedded as, from the node and the three field lists its class declared: `prose_keys`, `prose_properties`, `retrievable_properties`. |
| `split_passages` | `(label: &str, text: &str, max_chars: usize) -> Vec<String>`. Split a composed embed text into passages no longer than a character budget, each led by the node's label (RFC-0060). `PASSAGE_MAX_CHARS` is the shipped budget. |

Each fixture is a TOML file pairing one of these functions with an input and its canonical
output. Each SDK's test suite loads every fixture directory and compares its own answer.
Rust is the reference; TypeScript and Python must match it.

The `embed` and `prose` modules also export `of` (`embedOf` and `proseOf` in TypeScript) and
`text` (`proseText`). They are the parts `compose_embed_text` is built from. They are not
parity functions, and `compose_embed_text` is what the fixtures grade.

---

## Formal specification

Some invariants are worth proving rather than testing. `mise run verify` checks them. It
needs Dafny and `lake` (through elan) on the path.

### Dafny (`spec/graph.dfy`, `spec/sangha.dfy`)

`graph.dfy` models `scan_markers`, `parse_markers`, `update_regen` and the commit
classifier, and proves properties of the models. Dafny cannot see the Rust, so what is proved
is the model. The parity fixtures check the three runtimes against each other, and the model
is short and transcribed so a reader can compare it to `markers.rs`.

- **`UpdateRegenSpec`.** Everything outside the bodies of the blocks named `command` is
  unchanged. Every such block holds exactly `new_content`. The scan of the result reads the
  same blocks in the same order. A second call with the same content changes nothing. It
  requires that `new_content` does not contain the close tag, because one that does ends the
  section early. `ContentThatSpellsATagIsNotABlock` is the document that separates a count of
  open tags from a count of blocks the scan reads.
  `ClearingASectionLeavesNoBlankLine` pins the empty-content case.
- **`ParseMarkersSound`.** Every marker `parse_markers` returns is opened by some line of the
  source. The parser cannot invent one.
- **`ParseMarkersIsNotComplete`.** The converse is false. A REGEN block missing its close tag
  swallows every marker below it. `scan_markers` reports such a block in `malformed`, and
  `TheSwallowedBlockIsReported` and `ABlockThatClosesOnAnothersTagIsReported` prove the report
  of the model. `parse_markers` keeps its signature: the marker sequence is the frozen
  contract.
- **`ClassifyCommitTotal`.** Every verb maps to `Epistemic` or `Operational`. Unknown verbs
  map to `Epistemic` (`EpistemicIsDefault`).
- **Line reading.** The scan reads a line at a time. `ACloseTagMustStandAlone` and
  `AnOpenTagMidSentenceIsProse` pin where a block can start and end, and
  `TheSubstringFormOfGroundingIsFalse` records that a marker's command is read after trimming,
  not as a raw substring.

`sangha.dfy` models resolution. `UnionSynthesisArticleV` proves that no claim appears in the
resolution unless an elector position held it. `UnionSynthesisArticleIII` proves that every
position's tip hash is among the resolution's sources and that every tension is carried as an
open question. The sangha types exist in the specification only. No SDK implements them.

### LEAN 4 (`spec/Core.lean`)

LEAN targets the structure of the model.

- **The corpus graph as a category.** Nodes are objects and links are morphisms.
  `corpus_id` and `corpus_comp` give identity and composition over `Reachable`.
  `dag_no_nontrivial_cycle` holds when the graph is acyclic.
- **Positions as a partial order.** `PositionLE` is reflexive, transitive and antisymmetric
  on claims. `union_satisfies_article_v` and `union_satisfies_article_iii` state the two
  resolution articles for the union synthesis.
- **Classification is total.** `classification_is_total` says a classifier into the
  two-constructor `CommitKind` has no third outcome.
- **Augmentations are additive.** `additive_augmentations_do_not_contradict` says a base
  constitution plus a purely additive augmentation that discharges its own obligations is
  still a constitution, and nothing the base established is withdrawn.
  `additivity_does_not_preserve_acyclicity` is why one article is an obligation of the
  augmentation: adding nodes is how a cycle appears.

---

## Rust SDK (`rust/`)

The reference implementation.

**Crate**: `yidam-core`, published to crates.io because the `yidam` CLI depends on it and
cannot publish until it is there. See `VERSIONING.md`, Layer 2. The crate is a library
(`src/lib.rs`). The `yidam` binary is built from `yidam/cli`.

### API surface

```rust
// yidam_core::corpus
pub struct CorpusInstance { pub class, pub label, pub description, pub properties, pub links,
                            pub cites, pub references, pub refuses, pub not_refusals, pub extra }
pub struct CorpusLink { pub target, pub relationship, pub claim_tag, pub source }
pub struct ExternalCitation { pub package, pub node, pub commit, pub tag, pub span }
pub struct Refusal { pub span, pub inference }
pub struct NotRefusal { pub span }
pub fn parse_instance(text: &str) -> CorpusInstance
impl CorpusInstance { pub fn to_json(&self) -> serde_json::Value }

// yidam_core::git
pub enum CommitKind { Epistemic, Operational }
pub struct CommitEvent { pub hash, pub kind, pub verb, pub subject, pub context }
pub const OPERATIONAL_VERBS: &[&str]
pub const EPISTEMIC_VERBS: &[&str]
pub fn classify_commit(hash: &str, message: &str) -> CommitEvent
pub fn is_recognized_verb(verb: &str) -> bool

// yidam_core::graph
pub struct GraphEdge { pub from, pub to }
pub fn find_reachable(edges: &[GraphEdge], node_path: &str) -> Vec<String>
pub fn find_citations(edges: &[GraphEdge], node_path: &str) -> Vec<String>

// yidam_core::markers
pub enum Marker { Template { instruction }, Regen { command, content } }
pub enum Fault { OpenArrowMissing, CloseTagMissing, ClosedOnAnothersTag }
pub struct MalformedBlock { pub command, pub line, pub fault, pub swallowed_lines, pub swallowed_markers }
pub struct RegenSpan { pub command, pub open, pub body, pub close, pub inline }
pub struct Scan { pub markers, pub malformed, pub regen }
pub fn scan_markers(text: &str) -> Scan
pub fn parse_markers(text: &str) -> Vec<Marker>
pub fn update_regen(text: &str, command: &str, new_content: &str) -> String

// yidam_core::ontology
pub struct OntologyClass { pub name, pub label, pub description, pub properties, pub edges }
pub struct OntologyProperty { pub name, pub property_type, pub description, pub required,
                              pub unit, pub values, pub items, pub fields }
pub struct OntologyEdge { pub relationship, pub target, pub direction, pub description }
pub struct ClassParseError  // .reason() -> &str
pub const LIST_ITEM_TYPES: [&str; 6]
pub const CLAIM_TOKENS: [&str; 6]
pub fn parse_class(name: &str, content: &str) -> Result<OntologyClass, ClassParseError>
pub fn source_classes(classes: &[OntologyClass]) -> BTreeSet<String>
pub fn compile_class_schema(class: &OntologyClass) -> Value

// yidam_core::prose — what a node says, given the keys its class declared
pub const ALWAYS: &str = "description"
pub fn of<'a>(inst: &'a CorpusInstance, keys: &[String], properties: &[String]) -> Vec<(String, &'a str)>
pub fn text(inst: &CorpusInstance, keys: &[String], properties: &[String]) -> String

// yidam_core::embed — what a node is embedded as
pub struct EmbedFields { pub prose_keys, pub prose_properties, pub retrievable_properties }
pub const PASSAGE_MAX_CHARS: usize = 1200
pub fn of<'a>(inst: &'a CorpusInstance, fields: &EmbedFields) -> Vec<(String, &'a str)>
pub fn compose_embed_text(inst: &CorpusInstance, fields: &EmbedFields) -> String
pub fn split_passages(label: &str, text: &str, max_chars: usize) -> Vec<String>

// yidam_core::uri
pub enum Kind { Node, Crate, Catalog, Skill, Decision, Issue }
pub struct Reference { pub corpus, pub kind, pub path, pub rev, pub fragment }
pub fn parse_reference(input: &str) -> Option<Reference>
pub fn render_reference(r: &Reference) -> String
pub fn reference_conforms(r: &Reference) -> bool
pub fn fragment_conforms(kind: Kind, fragment: &str) -> bool
pub fn is_slug(s: &str) -> bool
```

### Dependencies

- `serde` — derive for the model types
- `serde_yaml` — the crate `serde_yaml_ng`, imported under the old name. `Cargo.toml`
  aliases it because the original is archived (#931). Its parse path is byte-identical to
  `0.9.34`, which keeps the scalar resolution the fixtures pin
- `serde_json` — the JSON forms and the compiled schema
- `toml` — parity fixture I/O

There is no git, index or markdown dependency.

---

## TypeScript SDK (`typescript/`)

**Package**: `@yidam/core`. Not published. The parity harness in this repository is the only
consumer. npm was considered and reversed. See `VERSIONING.md`, Layer 2.

The package ships TypeScript source. `package.json` maps each module to its `.ts` file under
`exports`, and `.` to `src/index.ts`, which re-exports all eight.

### API surface

```typescript
// @yidam/core/corpus
export interface CorpusInstance { class, label, description, properties, links, cites,
                                  references, refuses, not_refusals, extra }
export interface CorpusLink { target, relationship, claim_tag, source }
export interface ExternalCitation { package, node, commit, tag, span }
export interface Refusal { span, inference }
export interface NotRefusal { span }
export function emptyInstance(): CorpusInstance
export function parseInstance(text: string): CorpusInstance
export function instanceToJson(i: CorpusInstance): Record<string, unknown>

// @yidam/core/git
export type CommitKind = 'Epistemic' | 'Operational'
export interface CommitEvent { hash, kind, verb, subject, context }
export const OPERATIONAL_VERBS: Set<string>
export const EPISTEMIC_VERBS: Set<string>
export function classifyCommit(hash: string, message: string): CommitEvent
export function isRecognizedVerb(verb: string): boolean

// @yidam/core/graph
export interface GraphEdge { from, to }
export function findReachable(edges: GraphEdge[], nodePath: string): string[]
export function findCitations(edges: GraphEdge[], nodePath: string): string[]

// @yidam/core/markers
export type Marker = TemplateMarker | RegenMarker
export type Fault = 'OpenArrowMissing' | 'CloseTagMissing' | 'ClosedOnAnothersTag'
export interface MalformedBlock { command, line, fault, swallowedLines, swallowedMarkers }
export interface RegenSpan { command, open, body, close, inline }
export interface Scan { markers, malformed, regen }
export function scanMarkers(text: string): Scan
export function parseMarkers(text: string): Marker[]
export function updateRegen(text: string, command: string, newContent: string): string

// @yidam/core/ontology
export interface OntologyClass { name, label, description, properties, edges }
export interface OntologyProperty { name, type, description, required, unit, values, items, fields }
export interface OntologyEdge { relationship, target, direction, description }
export const LIST_ITEM_TYPES, CLAIM_TOKENS
export class ClassParseError extends Error
export function parseClass(name: string, content: string): OntologyClass  // throws ClassParseError
export function sourceClasses(classes: OntologyClass[]): Set<string>
export function compileClassSchema(cls: OntologyClass): Record<string, unknown>

// @yidam/core/prose — what a node says, given the keys its class declared
export const ALWAYS = 'description'
export function proseOf(inst: CorpusInstance, keys: string[], properties: string[]): [string, string][]
export function proseText(inst: CorpusInstance, keys: string[], properties: string[]): string

// @yidam/core/embed — what a node is embedded as
export interface EmbedFields { prose_keys, prose_properties, retrievable_properties }
export const PASSAGE_MAX_CHARS = 1200
export function embedOf(inst: CorpusInstance, fields: EmbedFields): [string, string][]
export function composeEmbedText(inst: CorpusInstance, fields: EmbedFields): string
export function splitPassages(label: string, text: string, maxChars: number): string[]

// @yidam/core/uri
export type Kind = 'node' | 'crate' | 'catalog' | 'skill' | 'decision' | 'issue'
export interface Reference { corpus, kind, path, rev, fragment }
export function parseReference(input: string): Reference | null
export function renderReference(r: Reference): string
export function referenceConforms(r: Reference): boolean
export function fragmentConforms(kind: Kind, fragment: string): boolean
export function isSlug(s: string): boolean
export function kindFromWord(word: string): Kind | null
export function pathArity(kind: Kind): number
export function segmentsOf(r: Reference): string[]
```

### Dependencies

- `yaml` — the one runtime dependency; reads node and ontology YAML
- `vitest` — dev; the test runner
- `smol-toml` — dev; parity fixture I/O
- `@huggingface/transformers` — dev; the TypeScript runtime of the `embed_config` fixture,
  run only with `YIDAM_EMBED_PARITY=1`

---

## Python SDK (`python/`)

**Package**: `yidam-core`. Not published, for the reason given above. It needs Python 3.11 or
later.

### API surface

```python
# yidam_core.corpus
def parse_instance(text: str) -> CorpusInstance: ...
def instance_to_json(inst: CorpusInstance) -> dict: ...
# `CorpusInstance.cls`, because `class` is a Python keyword. The wire name is `class`.
# Also CorpusLink, ExternalCitation, Refusal and NotRefusal dataclasses.

# yidam_core.git
def classify_commit(hash: str, message: str) -> CommitEvent: ...
def is_recognized_verb(verb: str) -> bool: ...
# Also CommitKind, CommitEvent, OPERATIONAL_VERBS and EPISTEMIC_VERBS.

# yidam_core.graph
def find_reachable(edges: list[GraphEdge], node_path: str) -> list[str]: ...
def find_citations(edges: list[GraphEdge], node_path: str) -> list[str]: ...

# yidam_core.markers
def scan_markers(text: str) -> Scan: ...
def parse_markers(text: str) -> list[Marker]: ...
def update_regen(text: str, command: str, new_content: str) -> str: ...
# Also TemplateMarker, RegenMarker, Fault, MalformedBlock, RegenSpan and Scan.

# yidam_core.ontology
def parse_class(name: str, content: str) -> OntologyClass: ...  # raises ClassParseError
def source_classes(classes: list[OntologyClass]) -> set[str]: ...
def compile_class_schema(cls: OntologyClass) -> dict: ...
# Also OntologyClass, OntologyProperty, OntologyEdge, ClassParseError (a ValueError),
# LIST_ITEM_TYPES and CLAIM_TOKENS.

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

PASSAGE_MAX_CHARS = 1200
def of(inst, fields: EmbedFields) -> list[tuple[str, str]]: ...
def compose_embed_text(inst, fields: EmbedFields) -> str: ...
def split_passages(label: str, text: str, max_chars: int) -> list[str]: ...

# yidam_core.uri
def parse_reference(text: str) -> Reference | None: ...
def render_reference(r: Reference) -> str: ...
def reference_conforms(r: Reference) -> bool: ...
def fragment_conforms(kind: Kind, fragment: str) -> bool: ...
def is_slug(s: str) -> bool: ...
def kind_from_word(word: str) -> Kind | None: ...
# Also the Kind enum and the frozen Reference dataclass.
```

### Dependencies

- `pyyaml` — the one runtime dependency; reads node and ontology YAML
- `pytest` — the `dev` extra; the test runner
- `sentence-transformers` — the `embed` extra, used only by
  `tests/parity/test_embed_config.py`. An embedding runtime is a test subject here, not a
  capability the package offers
- `tomllib` — the standard library, since 3.11; parity fixture I/O

### Embedding and indexing

RFC-0007 retracted a Python machine-learning layer (#1031). None of it was written: no
`embed_node`, `build_index` or `query`. What the SDKs share is the text a node is embedded
as, and that is `compose_embed_text`. Building and querying an index stays in the CLI:
`yidam index-build`, `yidam index-verify` and `yidam retrieve`.

---

## Parity testing (`parity/`)

The harness makes divergence between the SDKs visible before it matters. When
implementations diverge, tools that depend on one SDK silently get a different answer from
tools that depend on another.

### Fixture format

Each fixture is a TOML file under `parity/fixtures/<function>/`, with the function name, a
description, an input and the expected canonical output:

```toml
# parity/fixtures/parse_markers/multiple-regen.toml
function = "parse_markers"
description = "File with two REGEN sections; both are parsed independently"

[input]
content = """
## Status

<!-- REGEN: yidam status
Fields: node count, open questions.
-->
_Run `yidam status` to populate._
<!-- /REGEN -->

## Index

<!-- REGEN: yidam corpus-index
Fields: node titles and paths.
-->
_Run `yidam corpus-index` to populate._
<!-- /REGEN -->
"""

[[expected]]
kind = "Regen"
command = "yidam status"
content = "_Run `yidam status` to populate._"

[[expected]]
kind = "Regen"
command = "yidam corpus-index"
content = "_Run `yidam corpus-index` to populate._"
```

`parity/README.md` states the contract for each function, what is out of contract, and how
to add a case.

### Running it

```
mise run parity-check
mise run parity
```

`parity-check` checks that every function in its `functions=` line has a fixture, that every
fixture directory is a function or a declared exception, and that every exception has a
section in `parity/README.md`. `yidam/cli/tests/gates/parity_implementations.rs` checks that
every SDK defines every function and loads every fixture directory, and that this page's
table matches the same list.

`parity` runs each SDK's whole test suite against the shared fixtures. The task is defined in
`mise.toml`, and so is `verify`.

---

## Design constraints

**The Rust SDK is the reference.** If a TypeScript or Python implementation diverges from a
fixture, the fix is in that SDK, not in the fixture and not in the Rust implementation,
unless the Rust implementation is demonstrably wrong. A change to the parity surface updates
all three SDKs in the same change and bumps `parity/VERSION`.

**Parity fixtures are immutable once merged.** Changing a fixture's expected output is a
breaking change to the cross-SDK contract. Additions are always safe; mutations are
reviewed.

**A function lives in the SDKs and nowhere else.** The CLI calls `yidam-core` for every
parity function. A fourth copy is a fourth answer, and the gate in
`parity_implementations.rs` fails on one.

**No SDK reads git or an index.** Those are CLI concerns. A consumer that needs them calls
the `yidam` binary.
