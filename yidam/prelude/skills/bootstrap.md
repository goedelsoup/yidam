---
name: bootstrap
description: Initialize yidam in a repository — empty, near-empty, or an existing codebase
---

# Skill: bootstrap

Invoked when an agent enters a yidam repository with [BOOTSTRAP.md](https://github.com/goedelsoup/yidam/blob/main/BOOTSTRAP.md)
as its entry prompt. Produces a fully scaffolded, ontology-grounded, corpus-seeded repository
with a legible genesis commit. Works in two modes:

- **Empty-repo mode** — the repo has no commits. Steps proceed in full.
- **Existing-repo mode** — the repo has commits but `.yidam/` is absent. Yidam is applied
  as a knowledge-graph overlay on the existing structure. Step 1.5 explores what is already
  present before the ontology dialogue; steps 3, 6, and 8 adapt accordingly.

## Pre-flight

Before any step, run:

```
git log --oneline
ls .yidam/ 2>/dev/null && echo EXISTS
```

Determine the mode:

- **No commits** → empty-repo mode. Proceed to Step 0. The only file you should have read
  is `BOOTSTRAP.md`. Do not read any other file before completing Step 0.
- **Commits exist, `.yidam/` absent** → existing-repo mode. Proceed to Step 0, then follow
  the existing-repo variants noted in steps 1.5, 3, 6, and 8.
- **`.yidam/` present** → already bootstrapped. Do not re-run.

In either active mode, do not invoke any other skill, workflow, or tool until the genesis
commit is written in step 8. If the user's opening message contains domain context, hold it
as seed material for the ontology dialogue in step 2.

## Steps

### 0. Read samudaya (if present)

Before anything else, check whether `samudaya/` exists. If it does, list its contents first:

```
ls samudaya/
```

If only `README.md` and `examples/` are present, there are no seeds — skip to step 1.

Otherwise, read every file present — excluding `samudaya/examples/` and `samudaya/README.md`,
which are not seeds. Note each seed file's `kind` frontmatter field.
- **`axiom`** files: treat these concepts as pre-committed — they must appear in the corpus.
  Hold them in working memory as required nodes going into the ontology-discovery dialogue.
- **`hint`** files: treat these as candidate relationships or directions to surface during
  discovery. They are not guaranteed — if the user's answers don't support them, discard.
- **`constraint`** files: enforce these as hard boundaries during scaffolding. Do not deviate
  from them without surfacing the constraint and asking explicitly.
- **`augmentation`** files: examine whether the content is a constitutional extension or a
  general guideline. Constitutional extensions (domain-specific articles that add to
  [CONSTITUTION.md](../CONSTITUTION.md)) must be committed into the derived repo permanently
  — append them to the repo's copy of the constitution as part of the genesis scaffolding.
  General guideline augmentations are treated as additional prelude for this run only and do
  not persist after samudaya is consumed.

Samudaya does not replace the dialogue. It seeds it.

### 1. Internalize the prelude

Read these six files — and **only** these six files, in this exact order, using their
exact paths. Do **not** run `ls`, `find`, or any directory enumeration of `yidam/prelude/`
at any point during bootstrapping. Do not read any other file in `yidam/prelude/` (including
`SCRIPTURE.md` or any file surfaced by enumeration). Do not read
`yidam/prelude/skills/bootstrap.md` — it is the skill you are currently executing, not a
file to internalize here.

1. `yidam/prelude/IDENTITY.md` — what kind of knowledge artifact this repo is
2. `yidam/prelude/GRAPH.md` — the graph model: nodes, edges, commit types, branch semantics
3. `yidam/prelude/CONSTITUTION.md` — the governance rules that constrain what you may do
4. `yidam/prelude/PHASES.md` — the named phases of inquiry
5. `yidam/prelude/guidelines/agent-conduct.md` — specific conduct norms
6. `yidam/prelude/guidelines/directories.md` — where things live and what belongs in each

`yidam/tests/` is deliberately absent from this list, and absent from the repository you are
working in. It holds how the yidam template tests itself — the harness, the rubric, the
judge's criteria, and each scenario's reference description of a good result. None of it
teaches you anything about the domain you are bootstrapping, and the criteria you would be
scored against are not criteria you should be optimizing toward: an agent that has read
"seed nodes at a consistent level of abstraction" will assert consistency, which is not the
same as achieving it. What you need in order to do the work well is in these six files and
in the steps below.

After reading all six, output the synthesis as a **standalone message** — do not append
questions or any other content to it. Wait for the user to acknowledge before opening the
Step 2 dialogue. This gives the user the opportunity to correct any misread before questions
begin.

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor: [two or
> three bullet points from CONSTITUTION and agent-conduct]. Directory layout: [one sentence].

### 1.5. Explore the existing repository — existing-repo mode only

Skip this step in empty-repo mode.

Before opening the ontology dialogue, read the existing repository to ground the class sketch
in what is actually present. The goal is a one-paragraph inventory, not a full audit.

1. `ls -la` — note top-level directories. Flag which ones match sadhana's expected layout
   (`agents/`, `crates/`, `docs/`, `packages/`, `web/`) — step 3 will skip creating those.
2. Read `README.md` if present. Extract the domain framing in one sentence.
3. Identify the primary artifact type and read enough to understand the structure:
   - **Code repo**: find `Cargo.toml`, `go.mod`, `package.json`, or similar; skim one or two
     key source files to understand what the repo does.
   - **Data / research repo**: look for notebooks, dataset directories, experiment configs.
   - **Documentation / knowledge repo**: scan the docs structure and any existing index files.
4. Note any existing `sadhana/`, `samudaya/`, or `yidam/` directories — if absent, the
   transient-layer consume steps in step 8 do not apply.

Output a one-paragraph inventory before opening the step 2 dialogue. The ontology classes
should reflect the existing artifact structure where natural — a Rust repo might yield `Crate`,
`Module`, and `Trait`; a data repo might yield `Dataset`, `Experiment`, and `Model`.

### 2. Discover the ontology

Do not scaffold anything yet. Instead, open a dialogue to discover the core ontology of this
repository. This is an iterative loop — ask, receive, refine — until you have a stable sketch.

Begin with orienting questions:

- What is the domain or subject of this repository?
- What is the central question or problem it exists to investigate?
- What are the first 3–5 concepts that feel irreducible to this domain?

For each concept surfaced, probe further:
- What does this relate to? What does it depend on? What does it oppose?
- Is it atomic, or does it decompose?
- What would a node for this concept need to say?

Continue until you can draw a coherent sketch of the initial graph: a small set of named
nodes and the edges between them. Confirm this sketch with the user before proceeding.

Present the sketch in this format:

**Nodes**

| Node | What it is |
|------|------------|
| `name` | one-line description |

**Edges**

```
source →[relationship]→ target
```

One row per node; one line per edge. No prose — the format is the signal that the sketch is
ready to confirm.

After the user confirms the sketch, present the foundational ontology alignment choice before
writing any files. Describe each option concretely using 2–3 nodes from the confirmed sketch
as examples:

**BFO (Basic Formal Ontology)** — organizes entities along one axis: do they *persist through
time* or *unfold through time*?  Things that exist at a moment and have no temporal parts are
**continuants** (material entities, qualities, dispositions, sites). Things that happen over
an interval and have temporal parts are **occurrents** (processes, events, process boundaries).
Each class gets a `foundational_type:` field in its `.ont.yml`, with `ontology: bfo`. Best fit
for scientific, empirical, and
physical-process domains where the object/event distinction carries analytical weight (e.g.,
distinguishing a machine from the machining process it performs).

**UFO (Unified Foundational Ontology)** — organizes entities around rigidity and relationality.
A **Kind** is what something necessarily is (if it stops being one it ceases to exist as that
thing). A **Role** is what something contingently plays in a relational context (the same entity
may play different roles in different relationships). A **Relator** is a first-class node that
mediates a relationship with its own identity and properties — rather than a bare edge, a
relator carries the history and terms of the connection. Each class gets a `foundational_type:`
field, with `ontology: ufo`.
Best fit for institutional, enterprise, and process-modeling domains where the same entity plays
different roles and where relationships themselves carry meaning worth querying.

**None** — no foundational alignment. Classes are typed by domain convention only. Choose this
if foundational ontology alignment is not a goal of the corpus, or if you want to commit later.

Ask the user to choose one. Then ask:

> **How many seed instances should the initial corpus contain?** [default: 13]

The user may give a number or press enter to accept the default. Record it as `corpus_depth`
in the decision record — step 6 distributes instances across classes to reach this target.

Finally, ask the governance question:

> **Who will maintain this repository — one elector, or several?** [default: one]
>
> **One** — you (with agents acting on your behalf) are the sole elector. Phases run on
> `phase/<name>` branches off the baseline. This is the common case.
>
> **Several** — multiple humans or independently-directed agents hold positions that are
> expected to diverge and must be reconciled. This activates the sangha: each elector keeps
> a `ma/<elector>` branch, and resolution events synthesize them into `rigpa/<evolution>`
> baselines under the constitution.

Record the answer as `governance: single-elector | collective`. Do not choose `collective`
because it sounds more capable — it is a real protocol with real overhead, and a repository
that adopts it and never runs a resolution has paid for machinery it does not use. If the
user is unsure, take the default; a single-elector repo can adopt the sangha later by
scaffolding `.yidam/sangha/` when a second elector actually appears.

Then write the ontology decision record, including the chosen alignment, corpus depth, and
governance mode, before proceeding to step 3:

```
.yidam/decisions/ontology.yml
```

```yaml
id: ontology
summary: <one line — the domain, class count, and chosen foundational alignment>
corpus_depth: 13              # target instance count for initial seeding; user-configurable
governance: single-elector    # single-elector | collective
context: |
  <what the ontology discovery dialogue surfaced; key choices made; examples used to explain
  the alignment options>
decision: |
  <the confirmed class list and edges; the chosen foundational ontology (bfo | ufo | none);
  the governance mode and what the user said about who maintains this>
rationale: |
  <why these classes; what was considered and discarded; why this alignment was chosen>
```

### 3. Orient to and scaffold the derived-repo structure

The sadhana directory (`sadhana/`) holds the template content for this derived repo. In this
step, read the templates and create the derived-repo directory structure from them.

**First, read the sadhana templates:**

```
ls sadhana/
```

Then read each template file in `sadhana/`:

- `sadhana/catalog/README.md`
- `sadhana/corpus/README.md`
- `sadhana/crates/README.md`
- `sadhana/skills/README.md`
- `sadhana/web/README.md`
- `sadhana/root/README.md`, `sadhana/root/AGENTS.md`, `sadhana/root/CLAUDE.md`, `sadhana/root/mise.toml`,
  `sadhana/root/gitattributes`, `sadhana/root/gitignore`
- `sadhana/github/workflows/ci.yml`, `sadhana/github/workflows/release.yml`
- `sadhana/sangha/README.md` (and PROTOCOL.md, electors.md, resolutions/, positions/) —
  **only if `governance: collective`**; skip these five reads entirely in single-elector mode

`sadhana/agents/`, `sadhana/packages/`, and `sadhana/docs/` are deliberately not read here.
They are templates for directories created on first use, not at genesis — see below.

**Then create the derived-repo structure:**

Top-level directories (created directly from sadhana templates):
```
crates/README.md
web/README.md
```

`.yidam/` directories (created from sadhana templates):
```
.yidam/catalog/README.md
.yidam/corpus/README.md
.yidam/decisions/          ← new, empty; written to in steps 2 and 5
.yidam/skills/README.md
```

**Create on first use, not now:** `agents/`, `packages/`, and `docs/`. Their sadhana
templates exist and are the right content — but scaffold them the day something goes in
them, not at genesis. An empty directory with a README explaining what it would contain is
indistinguishable from an abandoned one, which is the argument for deferral — not the count
of what arrived. Measured across fifteen derived repositories, `packages/` stayed empty in
14 of 15, but `agents/` received 11 domain agents across 4 repositories and `docs/` received
53 files across 6: the deferral does not mean these directories go unused, only that they
are created the day a repeatable need for them emerges rather than speculatively at genesis
— the same argument `sadhana/skills/README.md` already makes for skills ("Add skills when a
repeatable procedure emerges from inquiry — not preemptively"). Note them in step 9 instead,
so the user knows they exist as conventions. The `yidam` CLI treats all three as optional —
`agents-index` and `packages-index` are no-ops when the directory is absent.

**`.yidam/sangha/` — only if `governance: collective`.** Read the governance mode recorded
in `.yidam/decisions/ontology.yml` in step 2:

- **`single-elector`** — do not create `.yidam/sangha/`. Do not copy `sadhana/sangha/`.
  The constitution is vendored with the rest of the prelude and lies dormant; it governs
  resolution events, and there will be none.
- **`collective`** — create `.yidam/sangha/` with all files from `sadhana/sangha/`, and fill
  `electors.md` with the participants the user named.

Repository-root files. `sadhana/root/` and `sadhana/github/` are not directory mirrors —
each file installs to a specific path, **overwriting yidam's own copy**:

```
sadhana/root/README.md            → README.md            (overwrites yidam's)
sadhana/root/AGENTS.md            → AGENTS.md            (overwrites yidam's)
sadhana/root/CLAUDE.md            → .claude/CLAUDE.md    (overwrites yidam's)
sadhana/root/mise.toml            → mise.toml            (overwrites yidam's)
sadhana/root/gitattributes        → .gitattributes       (overwrites yidam's)
sadhana/root/gitignore            → .gitignore           (overwrites yidam's)
sadhana/github/workflows/ci.yml   → .github/workflows/ci.yml  (overwrites yidam's)
sadhana/github/workflows/release.yml → .github/workflows/release.yml (overwrites yidam's)
```

Yidam's copies of these eight files describe yidam — its harness, its CLI workspace, its
bootstrap-mode entry check. Left in place they are wrong the moment genesis is written, and
yidam's `ci.yml` is worse than wrong: it builds `yidam/cli` and `yidam/tests/harness`, paths
that step 8 removes, so it goes green having compiled nothing. Yidam's `release.yml` is
wrong in a louder way: it publishes the yidam CLI's binaries on a `cli/v*` tag, from a
repository that has no CLI to publish. Overwrite all eight now. Do not merge yidam's content
into them.

`gitattributes` and `gitignore` are spelled without their dots for the same reason `root/`
and `github/` are: `ls sadhana/` is a step in this skill and a dotfile would not appear in
it. `.gitattributes` arrives holding only comments — the rule about connector fixtures and
line endings, which costs nothing until the first connector lands and is unrecoverable
advice afterwards.

`.gitignore` is the one of the eight most easily mistaken for generic, and it is not.
Yidam's own ignores `.local/` — where *its* binary installs — and a path under
`yidam/tests/`, which the vendor step in step 8 deletes; the rule outlives the directory it
names by the length of the repository's life. What this file needs instead is organized
around a hazard a derived repository has and yidam does not: both this skill and
`PROTOCOL.md` prescribe `git add -A`, so anything that appears in the working tree without
somebody putting it there is one prescribed command away from the corpus.

Each README may contain a `<!-- TEMPLATE -->` comment block marking fields that need
domain-specific content. Fill every such block now, before proceeding. These are the only
edits made to the scaffolded content in this step — do not add or remove files beyond what
sadhana provides.

**Existing-repo mode**: if `sadhana/` is absent, skip the template reads and directory
creation for top-level dirs — they either already exist or are not applicable to this repo.
Create only the `.yidam/` subdirectories that are missing. Do not overwrite any existing
file; if a target path already exists with content, leave it and note the conflict.

### 4. Formalize the ontology

Render each node from the confirmed sketch as a domain class definition in `.yidam/corpus/`:

```
.yidam/corpus/<domain-class>.ont.yml
```

Each file defines what that class of thing is — its properties and its edge participation.
One file per class; the filename matches the class name exactly.

One class per file, and one *concept* per class. Two ideas fused into a single class —
`site-and-region`, `event-or-interval` — cannot be linked to separately afterwards, and the
edge that wanted only one of them has nowhere to land. If a class name needs an "and" or an
"or", it is two classes.

```yaml
class: <name>
label: <Human-Readable Label>
foundational_type:           # omit this field entirely if alignment is "none"
  ontology: bfo | ufo
  type: <value>              # BFO: continuant | occurrent | quality | disposition | role | ...
                             # UFO: kind | subkind | role | phase | relator | mode | quality | event | situation
  iri: <url>                 # optional — the IRI that type has in that ontology, e.g.
                             #   BFO: http://purl.obolibrary.org/obo/BFO_0000002
                             #   UFO: https://purl.org/nemo/gufo#Relator
                             # `export-rdf` emits it as skos:exactMatch. Omit it if you have
                             # not looked it up; the alignment still exports without it.
description: |
  <one sentence — what this class of thing is and why it is irreducible>
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

These files are the schema layer of the corpus. They define what kinds of things exist, not
specific instances. Every class in the confirmed sketch gets a file. Do not add classes not
in the confirmed sketch.

### 5. Identify implied edges, connectors, and calculators

Definitions for this step — three distinct concepts:

- **Implied edge** — an edge type between two classes that is warranted by the domain but
  not yet declared in any `.ont.yml` file. An implied edge is a *proposal*, not an edge.
  It becomes a `links:` entry in instance files only after the user approves it and it is
  wired in step 7. Do not add anything to any file in this step.
- **Connector** — a retrieval bridge to an external data source (see `directories.md`:
  `crates/` holds connectors). A connector is identified here as a *proposal*. Approved
  connectors are invoked opportunistically during seeding (step 6) when demand is clear;
  any that were not invoked during seeding are stubbed in step 7. Do not create any files
  in this step.
- **Calculator** — a domain computation that derives a value or relationship from corpus
  data. A calculator is identified here as a *proposal*. Approved calculators are run
  during step 7 if enough seeded instances exist to make the result meaningful; otherwise
  they are stubbed. Do not create any files in this step.

Before seeding any objects, read the full set of `.ont.yml` class definitions and reason
about what the schema implies at the domain level. Then present a structured report to the
user for confirmation:

**Implied edges** — edge types warranted by the domain but absent from the current `.ont.yml` files:

| From | Relationship | To | Basis |
|------|--------------|----|-------|
| `class` | verb phrase | `class` | one line — why this edge is implied |

**Connectors** — external data sources that could feed this corpus, and the crate adapter each would require:

| Name | Source | Feeds | Notes |
|------|--------|-------|-------|
| `name` | external system or dataset | which classes | one line |

**Calculators** — domain computations that follow naturally from the class structure:

| Name | Computes | Reads | Returns | Prelude domain |
|------|----------|-------|---------|----------------|
| `name` | what it derives | which classes/edges | what it produces | `<domain>`, or — |

Before proposing a calculator, list `yidam/prelude/domains/` and see whether one of them
already computes what it needs. That layer holds small pure functions — means and variances,
centrality, entropy, geodesic distance — implemented identically in Rust, TypeScript and
Python and pinned to each other by shared fixtures. The last column names the domain a
calculator would draw on, or `—` if none fits.

This is the only point in the bootstrap where that layer is visible, and the selection has a
consequence in step 8: **only the domains named here are vendored.** A repository that names
none gets no `domains/` directory, which is the right outcome — fourteen of the fifteen are
wrong for any given corpus, and a library nothing can build is indistinguishable from an
abandoned one. Naming a domain is cheap and reversible; carrying all fifteen is neither.

Do not name a domain because it sounds adjacent. The question is whether a calculator in this
table would call a function in it.

This step produces a report only. Do not modify any file. Wait for the user to confirm,
modify, or discard individual items. Only what the user approves is carried into step 7.

After the user confirms, write the proposals decision record:

```
.yidam/decisions/proposals.yml
```

```yaml
id: proposals
summary: <one line — what was approved>
context: |
  <the full set of proposals presented>
decision: |
  <what the user approved, modified, or discarded — item by item>
prelude_domains: []          # domains selected for vendoring in step 8; [] is the common case
rationale: |
  <any rationale provided; gaps or domain logic behind approvals; for each domain named,
  which calculator would call into it>
```

### 6. Seed corpus objects

Read `corpus_depth` from `.yidam/decisions/ontology.yml` (default 13 if absent). This is
the target total instance count for the genesis corpus. Create a class directory for each
class, then distribute instances across classes to reach the target:

```
.yidam/corpus/<class>/README.md       — describes the class in prose; links to the .ont.yml
.yidam/corpus/<class>/ACTIONS.md      — operations, queries, and skills applicable to this class
.yidam/corpus/<class>/<instance>.yml  — a concrete object of this class
```

**`README.md`** — one paragraph describing what this class of thing is in the context of
this domain. Link to the class definition (`../<class>.ont.yml`). Written for a domain
contributor, not a schema reader.

**`ACTIONS.md`** — a list of operations meaningful for this class: queries that retrieve
instances, transitions an instance can undergo, skills that act on it, or calculators that
derive from it. Stub entries are fine; this file grows over time.

**`<instance>.yml`** — a concrete object. Structure:

```yaml
class: <class-name>
label: <Human-Readable Instance Name>
description: |
  <one or more sentences — what this specific thing is>
properties:
  <field>: <value>
links:
  - target: ../<other-class>/<other-instance>.yml
    relationship: <verb phrase>
  - target: ../<class>.ont.yml
    relationship: instance-of
```

Each instance must carry at least one outgoing link to another node. Be specific enough to
be wrong — a vague placeholder is not an object.

An edge is a claim that two things are related, and the `relationship` says how. A link to a
README, to a directory, or to the class definition alone is a citation rather than a
relationship: it satisfies the count and adds no knowledge. The `instance-of` link to
`../<class>.ont.yml` is structural and does not discharge this — every instance needs at
least one edge to another *instance*.

Keep the seed set at one level of abstraction. A corpus whose nodes are three fields and one
named specimen reads as two corpora, and the edges between the levels carry the confusion
rather than resolving it.

**Distribution** — allocate instances across classes to hit `corpus_depth` total, with a
minimum of 1 per class **where the sources support one**. Give more instances to hub classes
(those with the most edge participation) and fewer to peripheral classes. Seed from root
nodes down so link targets exist when referenced. Prefer depth over breadth: a well-linked
instance with real content is worth more than several shallow stubs.

**When the material for a class does not exist, leave the class empty.** It keeps its
`.ont.yml`, its directory, its README and its ACTIONS file, and it holds no instance nodes.
Do not invent one to satisfy the minimum, and do not stop and wait — ask for the material
once, and if it is not forthcoming, seed what the sources support and record the shortfall.

The pressure to fabricate is strongest here and the reason is worth stating, because the
`corpus_depth` you are short of is a number a user picked and the empty directory looks like
a failure to meet it. A fabricated instance would not look like a placeholder. It would be
well-formed, correctly typed, correctly linked, and it would pass every check this repository
runs — `graph-check` reads structure and `edge-target-class` asks whether an edge landed on
the right class, and neither asks whether a claim is true. It would sit among the sourced
nodes and be lent credibility by every one of them.

The asymmetry decides it. An empty class is a gap visible to everyone who opens the
directory, it costs nothing but the seeding work to close, and it is closed correctly the
first time someone supplies the real material. A fabricated instance has to be *found* before
it can be removed, and until it is found the corpus asserts it.

`corpus_depth` was chosen in step 2, before anyone knew what the sources covered. It is a
target and not a quota, and it is not revised here — leave it as written, and let the record
below say what was actually seeded and why the two numbers differ.

**Record the shortfall.** If the seeded count is short of `corpus_depth`, or any class is
empty, write this before moving on:

```
.yidam/decisions/seed-scope.yml
```

```yaml
id: seed-scope
summary: <one line — N instances seeded against a target of M; which classes are empty>
context: |
  <what material was available; what was asked for and not supplied; which classes each
  source was sufficient to seed>
decision: |
  <the seeded count and the classes that hold no instances; that nothing was fabricated to
  reach the target — name what was not invented, so the record is falsifiable>
rationale: |
  <why an empty class was preferred to a plausible one here; what the gap costs downstream —
  in particular, name any calculator approved in step 5 that now has nothing to read>
```

The last clause is the one that is easy to leave out and matters most. A calculator whose
inputs are all in the empty classes is a stub for a reason that has nothing to do with the
calculator, and step 7 will not be able to tell the difference.

**Existing-repo mode**: instances may represent existing repository artifacts directly. Add
a `source_path:` property pointing to the relevant existing file or directory, and link to
it as an evidence anchor. For example, a `Crate` instance for a Rust workspace member would
carry `source_path: crates/my-crate/Cargo.toml`. This is how the corpus models what already
exists rather than only what the bootstrap creates.

**Opportunistic retrieval**: While seeding, watch for the demand threshold — five or more
instances that share a missing property attributable to a single approved connector source.
When that threshold is met, invoke the connector inline rather than deferring it: fetch the
missing data, populate the instances, and commit the result as part of the seed with
`extract:` — structured data pulled from a primary source. Respect rate limits: pause
between requests; do not batch-hammer a source. Record what was fetched in the commit
message.

### 7. Wire implied edges and scaffold connectors and calculators

After all objects are seeded, read the full corpus — every `.ont.yml` class file, every
class directory, and every instance. Then act on what the user approved in step 5:

**Implied edges** — add each approved edge as an entry in the `links:` field of the
relevant instance `.yml` files. An implied edge resolves a missing relationship between
specific objects; it does not add new content to instances.

Add only the edges you can defend. This step reads the whole corpus at once and every pair of
instances looks like it could be related, which is the condition under which a plausible
relationship gets written as a settled one. An edge is a claim — see
[agent-conduct](../guidelines/agent-conduct.md), "An edge is a claim" — and the cost is
asymmetric: a missing edge is a gap somebody finds and fills, while a wrong edge is something
the corpus now asserts, made credible by every correct edge around it.

For each edge before you write it: could you say, in one sentence, why this relationship holds
in this domain? If yes, write it, and put that sentence in the node body. If the honest answer
is that these two things are associated but you could not say how, use the weakest relationship
that is true rather than the most interesting one that might be. If you cannot do either, do
not write the edge — and if it was approved in step 5, say so in the report rather than
quietly dropping it.

**Connectors** — for each approved connector not already invoked during seeding, scaffold
a crate stub in `crates/`:

```
crates/<connector-name>/
```

The stub should name the external source, describe what corpus classes it feeds, and define
the retrieval interface. Connectors invoked during seeding need no stub — their invocation
and the resulting epistemic commit are the record.

**Calculators** — for each approved calculator: if the seeded corpus contains enough
instances to produce a meaningful result, run it now and commit the output with `compute:`
— a calculator run and its output committed. Otherwise write a stub in `.yidam/skills/`:

```
.yidam/skills/<calculator-name>.md
```

The stub should describe what it computes, which corpus nodes it reads, and what it returns.

**Do not commit anything in this step.** The implied edges are an `establish:` — understanding
the ontology entailed and nobody had written down — and the remaining stubs are an
`implement:`, because a stub is structure and not a finding. Both are written in step 8,
after the genesis commit, for the reason a root commit cannot have a parent. Step 8 states
the whole sequence in one place; this step's job is to leave the working tree in the state
those two commits describe.

### 8. Write the genesis commit and consume transient layers

**The commit sequence.** Everything a bootstrap writes, in order. This block is the whole
list — no step writes a commit that is not here, and the harness reads these verbs to decide
whether a history is a bootstrap's:

```
genesis     the root commit — schema, instances, decision records, .yidam/ structure
establish   the implied edges wired in step 7 — omit if none were approved
implement   the connector and calculator stubs from step 7 — omit if none remained
consume     samudaya
consume     sadhana
vendor      the prelude, into .yidam/.vendor/
regen       the generated blocks, from step 8.5
```

Two of the seven are conditional and the rest are not. `establish:` and `implement:` are
skipped when step 5 approved nothing of that kind — that is a corpus with no implied edges
and no stubs, not a deviation.

`establish:` and `implement:` come *after* `genesis:` and not before, which is the opposite
of the order their steps appear in. A root commit has no parent; there is nowhere to put
them. Step 7 does the work and step 8 records it.

**Genesis commit** — stage and commit all class definitions (`.ont.yml`), seed instances,
decision records, and the `.yidam/` directory structure as a single genesis commit. Do not
include `sadhana/` or `samudaya/` in this commit, and do not include the step 7 stubs — they
are the `implement:` commit below.

The message should name the domain, summarize the class schema, and describe what seed
objects were created and how they connect — naming at least one specific relationship, not
just that relationships exist. This commit is the first event in the knowledge graph. It
should read like one: a list of filenames is a diff summary, and a paragraph that would fit
any domain is boilerplate. Neither is testimony about what the corpus now knows.

In existing-repo mode, open the message with `overlay:` instead of `genesis:` and note the
pre-existing commit count: `overlay: <domain> — yidam applied to N-commit repository; M
classes; K instances seeded`. This marks the graph's origin without misrepresenting the repo
history.

**`establish:` and `implement:`** — write them now, in that order, if step 7 produced
anything for them. The `establish:` message says which edges were wired and why each holds;
the `implement:` message names each stub and what it is a stub *for*. If an edge approved in
step 5 was not written because it could not be defended, say so here rather than letting it
disappear.

**Consume samudaya** — after those commits are written, delete `samudaya/`. Skip if
`samudaya/` does not exist (typical in existing-repo mode).

First try the tracked path:

```
git rm -r samudaya/
git commit -m "consume: samudaya — ..."
```

If `git rm` fails because samudaya files were never staged (they are untracked), delete
the directory directly and record the event as an empty commit:

```
rm -rf samudaya/
git commit --allow-empty -m "consume: samudaya — ..."
```

Do not ask the user to run either command manually — the deletion is part of the bootstrap
protocol and must complete before step 9.

The deletion message should record what samudaya contained and what it influenced. If no
seeds were present (only `README.md` and `examples/`), say so explicitly: "no seeds present;
directory removed."

**Consume sadhana** — immediately after consuming samudaya, delete `sadhana/`. Skip if
`sadhana/` does not exist (typical in existing-repo mode).

```
git rm -r sadhana/
git commit -m "consume: sadhana — scaffold template consumed; derived structure in place"
```

If sadhana files were untracked:

```
rm -rf sadhana/
git commit --allow-empty -m "consume: sadhana — scaffold template consumed; derived structure in place"
```

**Vendor the prelude** — immediately after consuming sadhana, move the inherited prelude
into the `.yidam/` infrastructure namespace and delete the rest of the template. Skip if
`yidam/` does not exist (typical in existing-repo mode; the vendor step only applies when
bootstrapping from the yidam template).

**Vendor exactly one directory.** `yidam/prelude/` is what a derived repo inherits. Everything
else under `yidam/` is yidam's own machinery — the CLI source, the bootstrap test harness, the
design notes, the docs site — and none of it is readable, runnable, or updatable from inside a
derived repo. Carrying it produces a stale fork of the CLI that will never be rebuilt and a
`HARNESS.md` whose links point at scenarios the repo does not have.

Because `yidam/` was not staged in the genesis commit (it is untracked), use filesystem
operations and stage the result directly:

```
mkdir -p .yidam/.vendor
mv yidam/prelude .yidam/.vendor/prelude
rm -rf yidam/
```

**Then drop the domain libraries this corpus did not ask for.** `prelude/domains/` is fifteen
domain libraries in three languages each — around 320 of the roughly 540 files just moved, and
the majority of the bytes. Read `prelude_domains` out of `.yidam/decisions/proposals.yml`
(step 5) and keep only what it names:

```
cd .yidam/.vendor/prelude/domains
ls -d */ | grep -vE '^(README.md|parity|<selected>)/' | xargs rm -rf
cd -
```

If `prelude_domains` is empty — the common case — remove the whole directory:

```
rm -rf .yidam/.vendor/prelude/domains
```

The same argument as the paragraph above, applied one level down. A derived repository has no
task that builds these, no workspace that includes them, and no CI job that runs them; the
`domain-parity` gate that keeps them honest is yidam's and does not travel. Fifteen unbuildable
libraries is the stale-fork outcome arriving through the one directory the vendor step allows.
`prelude/sdks/` stays whole — the prelude's own README and `agent-conduct.md` link into it, so
it is read from inside a derived repository even though it is not built there.

Keep `domains/README.md` when any domain is kept: it is the index that says what the layer is
and how a domain is wired into `crates/Cargo.toml` when the domain computer exists.

**Then delete the template's own top-level files.** These describe yidam, not this repository.
`README.md`, `AGENTS.md`, `.claude/CLAUDE.md`, `mise.toml`, `.gitattributes`, `.gitignore`,
`.github/workflows/ci.yml`, and `.github/workflows/release.yml` were already overwritten in
step 3; what remains is:

```
rm -f BOOTSTRAP.md VERSIONING.md
```

`BOOTSTRAP.md` is the entry prompt for a repo that has not been bootstrapped — this one now
has. `VERSIONING.md` documents how yidam releases its own three layers. Keep `LICENSE` and
`mise.yidam.toml`: the first is generic and the second is the inherited task layer that
`mise.toml` includes. `.gitignore` and `.gitattributes` are already this repository's own —
step 3 overwrote both from `sadhana/root/`.

**Confirm the provenance pin.** `.yidam.toml` records which yidam this repo came from; `yidam
clone` and `yidam overlay` write it. Check that it exists and carries a real commit:

```
cat .yidam.toml
```

If the file is missing — the template was copied by hand rather than by `yidam clone` — write
it now, with `commit = "unknown"` if the source SHA is genuinely unavailable. Do not guess a
commit. An honest `unknown` can be repaired by hand; a wrong SHA silently upgrades the repo
against the wrong baseline.

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "<40-char sha, or unknown>"
template  = "untagged"
committed = "<YYYY-MM-DD — the date of that commit>"
```

Then commit the whole vendor step as one event:

```
git add -A
git commit -m "vendor: yidam prelude into .yidam/.vendor/; template files removed"
```

Do not ask the user to run any of this manually — the vendor step is part of the bootstrap
protocol and must complete before step 9.

### 8.5. Run the gate this repository will be gated by

Everything up to here was written and none of it has been checked. The repository now has a
CI workflow, a local gate, and a CLI it can install — and no step has run any of them. That
is not a gap in coverage; it is the difference between a repository that works and one that
merely exists, and it is answerable in four commands.

**Install the binary this repository pins.** Nothing before this point needed the CLI, so it
is not there yet:

```
mise run yidam-build
```

**Then run the gate, in this order.** Each answers a different question and the order is the
order a failure is cheapest to fix in:

```
mise run graph-check          # is the graph well-formed
yidam regen                   # refresh every generated block
yidam lint --init-baseline    # record what the corpus starts with
yidam lint                    # and read what it says
```

`yidam regen` is the one that is easy to skip and cannot be. The scaffold installed in step 3
carries `<!-- REGEN: ... -->` markers in seven files, and **every one of them is stale on
arrival** — they are generated from a corpus that did not exist when the template was
written. A bare scaffold with no nodes at all reports ten stale blocks. `.github/workflows/ci.yml`
runs `yidam regen --check`, so until this command has been run and its output committed, the
repository's first push fails on generated content nobody wrote.

Commit the refreshed blocks and the baseline together:

```
git add -A
git commit -m "regen: REGEN blocks populated on the first run of the gate"
```

`regen:` is the operational verb for exactly this — generated content refreshed, no
understanding changed. Keep it out of the genesis commit: genesis is testimony about what the
corpus knows, and a regenerated index table is not testimony.

**If `graph-check` or `lint` reports anything, fix it now.** These are findings about work
that was written minutes ago by the agent reading this, which is the cheapest they will ever
be to act on. A `catalog-uncited` or a `missing-property` at this point is a step-4 or step-6
mistake still warm; the same finding six months from now is archaeology. Fix and amend the
commit it belongs to, or write a `fix:` commit if the genesis commit has already been pushed.

Do not ask the user to run any of this manually. A bootstrap that hands over a repository
whose gate it has never run has not finished; it has stopped.

### 9. Report

Do not begin this step until the genesis commit, both `consume:` commits, the `vendor:`
commit, and the step 8.5 gate run are all done. If any is unresolved, finish it before
proceeding. Step 9 opens by stating the gate result — a handoff that says the repository is
ready is a claim, and this is the one place it can be checked.

Output a structured handoff with seven sections:

**Gate** — one line: the result of the step 8.5 run. Name the commands, say whether each
passed, and name any finding left open and why. "Green as of `<sha>`" is checkable; "the
repository is ready" is not.

**Ontology** — the class definitions written. One line per class; list the outgoing edges.

**Objects seeded** — the instance nodes created. One line each; note which class each
instantiates.

**Classes seeded and classes empty** — every class in one of two lists, with the instance
count for the seeded ones. If any class is empty, name `.yidam/decisions/seed-scope.yml` and
say in one line what material would close it. A scaffold waiting for material and a corpus
that is finished look identical in the four sections around this one.

**Implied edges, connectors, and calculators** — edges wired, crate stubs and skill stubs scaffolded. One line each.

**Conventions not yet scaffolded** — one line each, so the user knows these exist without
finding an empty directory and guessing:

- `agents/` — domain agent definitions. Create it when you write the first agent.
- `packages/` — non-Rust toolkit code (Python/TypeScript connectors, ML pipelines). Create
  it when a capability genuinely belongs outside `crates/`.
- `docs/` — documentation about the repository, as distinct from the corpus's knowledge.
  Create it when there is something to say that is not a corpus node.
- `.yidam/sangha/` — collective resolution. State the governance mode chosen in step 2. In
  single-elector mode, say that phases run on `phase/<name>` branches and that the sangha
  can be adopted later if a second elector appears.
- `prelude/domains/` — shared pure-function libraries. Name the domains vendored in step 8,
  or say that none were and that the layer exists: the fifteen are listed in the yidam
  repository, and one can be vendored later by re-running `mise run yidam-vendor-update`
  after adding it to `prelude_domains`. A reader who never hears of the layer will write the
  calculator by hand.

**Next steps** — three concrete, ordered actions:

1. **First catalog entry** — identify the most authoritative data source for this domain
   and add it to `.yidam/catalog/` as the first provenance anchor. Name it specifically.
2. **First corpus expansion** — name the instance node most ready to grow and suggest the
   first sub-node or property to deepen it. This becomes the first `establish:` commit
   after genesis.
3. **First agent** — describe the simplest agent immediately useful in this domain. One
   sentence on what it does and which corpus nodes it draws from. Creating `agents/` with
   that one definition is the action; do not create the directory to hold nothing.

Then ask:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps,
> and proposing new objects, implied edges, connectors, or calculators until the corpus
> reaches a stable initial state. Reply **yes** to continue, **no** to stop here, or
> describe a specific area to focus on.
