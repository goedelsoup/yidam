---
name: bootstrap
description: Initialize an yidam-derived repository from an empty or near-empty state
---

# Skill: bootstrap

Invoked when an agent enters a freshly cloned yidam repository with [BOOTSTRAP.md](../../../BOOTSTRAP.md)
as its entry prompt. Produces a fully scaffolded, ontology-grounded, corpus-seeded repository
with a legible genesis commit.

## Pre-flight

Before any step, run:

```
git log --oneline
```

If this returns no output, the repository has not been bootstrapped and you are in bootstrap
mode. Do not invoke any other skill, workflow, or tool until the genesis commit is written
in step 8. If the user's opening message contains domain context (a theme, subject, or
research direction), hold it as seed material for the ontology dialogue in step 2 — do not
route it to a research skill or workflow.

If the genesis commit already exists, do not re-run bootstrap.

If the repository has not been bootstrapped, immediately proceed to **Step 0** — do not read
BOOTSTRAP.md or any prelude file before checking samudaya.

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

Read these eight files — and **only** these eight files, in this exact order, using their
exact paths. Do **not** run `ls`, `find`, or any directory enumeration of `yidam/prelude/`
at any point during bootstrapping. Do not read any other file in `yidam/prelude/` (including
`SCRIPTURE.md` or any file surfaced by enumeration). Do not read
`yidam/prelude/skills/bootstrap.md` — it is the skill you are currently executing, not a
file to internalize here.

1. `yidam/prelude/IDENTITY.md` — what kind of knowledge artifact this repo is
2. `yidam/prelude/GRAPH.md` — the graph model: nodes, edges, commit types, branch semantics
3. `yidam/prelude/CONSTITUTION.md` — the governance rules that constrain what you may do
4. `yidam/prelude/HARNESS.md` — how scenarios and the judge rubric work
5. `yidam/prelude/PHASES.md` — the named phases of inquiry
6. `yidam/prelude/guidelines/agent-conduct.md` — specific conduct norms
7. `yidam/prelude/guidelines/directories.md` — where things live and what belongs in each
8. `yidam/prelude/skills/judge.md` — the judge's criteria; internalize so the genesis commit passes

After reading all eight, output the synthesis as a **standalone message** — do not append
questions or any other content to it. Wait for the user to acknowledge before opening the
Step 2 dialogue. This gives the user the opportunity to correct any misread before questions
begin.

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor: [two or
> three bullet points from CONSTITUTION and agent-conduct]. Directory layout: [one sentence].

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

After the user confirms, write the ontology decision record before proceeding to step 3:

```
.yidam/decisions/ontology.yml
```

```yaml
id: ontology
summary: <one line — the domain and how many classes were confirmed>
context: |
  <what the ontology discovery dialogue surfaced; key choices made>
decision: |
  <the confirmed class list and edges>
rationale: |
  <why these classes; what was considered and discarded>
```

### 3. Orient to and scaffold the derived-repo structure

The sadhana directory (`sadhana/`) holds the template content for this derived repo. In this
step, read the templates and create the derived-repo directory structure from them.

**First, read the sadhana templates:**

```
ls sadhana/
```

Then read each template file in `sadhana/`:

- `sadhana/agents/README.md`
- `sadhana/catalog/README.md`
- `sadhana/corpus/README.md`
- `sadhana/crates/README.md`
- `sadhana/docs/README.md`
- `sadhana/packages/README.md`
- `sadhana/sangha/README.md` (and PROTOCOL.md, electors.md, resolutions/ if present)
- `sadhana/skills/README.md`
- `sadhana/web/README.md`

**Then create the derived-repo structure:**

Top-level directories (created directly from sadhana templates):
```
agents/README.md
crates/README.md
docs/README.md
packages/README.md
web/README.md
```

`.yidam/` directories (created from sadhana templates):
```
.yidam/catalog/README.md
.yidam/corpus/README.md
.yidam/decisions/          ← new, empty; written to in steps 2 and 5
.yidam/sangha/             ← all files from sadhana/sangha/
.yidam/skills/README.md
```

Each README may contain a `<!-- TEMPLATE -->` comment block marking fields that need
domain-specific content. Fill every such block now, before proceeding. These are the only
edits made to the scaffolded content in this step — do not add or remove files beyond what
sadhana provides.

### 4. Formalize the ontology

Render each node from the confirmed sketch as a domain class definition in `.yidam/corpus/`:

```
.yidam/corpus/<domain-class>.ont.yml
```

Each file defines what that class of thing is — its properties and its edge participation.
One file per class; the filename matches the class name exactly.

```yaml
class: <name>
label: <Human-Readable Label>
description: <one sentence — what this class of thing is and why it is irreducible>
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

| Name | Computes | Reads | Returns |
|------|----------|-------|---------|
| `name` | what it derives | which classes/edges | what it produces |

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
rationale: |
  <any rationale provided; gaps or domain logic behind approvals>
```

### 6. Seed corpus objects

For each class defined in step 4, create a class directory and seed at least one concrete
instance within it:

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
description: <one sentence — what this specific thing is>
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

For domains with clear hierarchies, seed from the root down so that link targets exist when
they are referenced. Prefer depth over breadth: a well-linked instance with real content is
worth more than several shallow stubs.

**Opportunistic retrieval**: While seeding, watch for the demand threshold — five or more
instances that share a missing property attributable to a single approved connector source.
When that threshold is met, invoke the connector inline rather than deferring it: fetch the
missing data, populate the instances, and commit the result as part of the seed. Respect
rate limits: pause between requests; do not batch-hammer a source. Record what was fetched
in the commit message.

### 7. Wire implied edges and scaffold connectors and calculators

After all objects are seeded, read the full corpus — every `.ont.yml` class file, every
class directory, and every instance. Then act on what the user approved in step 5:

**Implied edges** — add each approved edge as an entry in the `links:` field of the
relevant instance `.yml` files. An implied edge resolves a missing relationship between
specific objects; it does not add new content to instances.

**Connectors** — for each approved connector not already invoked during seeding, scaffold
a crate stub in `crates/`:

```
crates/<connector-name>/
```

The stub should name the external source, describe what corpus classes it feeds, and define
the retrieval interface. Connectors invoked during seeding need no stub — their invocation
and the resulting epistemic commit are the record.

**Calculators** — for each approved calculator: if the seeded corpus contains enough
instances to produce a meaningful result, run it now and commit the output as an epistemic
commit. Otherwise write a stub in `.yidam/skills/`:

```
.yidam/skills/<calculator-name>.md
```

The stub should describe what it computes, which corpus nodes it reads, and what it returns.

Commit implied edges as a single epistemic commit. Commit any remaining connector and
calculator stubs together as a single operational commit.

### 8. Write the genesis commit and consume transient layers

**Genesis commit** — stage and commit all class definitions (`.ont.yml`), seed instances,
decision records, scaffolded skill stubs, and the `.yidam/` directory structure as a single
genesis commit. Do not include `sadhana/` or `samudaya/` in this commit.

The message should name the domain, summarize the class schema, and describe what seed
objects were created and how they connect. This commit is the first event in the knowledge
graph. It should read like one.

**Consume samudaya** — after the genesis commit is written, delete `samudaya/`:

First try the tracked path:

```
git rm -r samudaya/
git commit -m "consume(samudaya): ..."
```

If `git rm` fails because samudaya files were never staged (they are untracked), delete
the directory directly and record the event as an empty commit:

```
rm -rf samudaya/
git commit --allow-empty -m "consume(samudaya): ..."
```

Do not ask the user to run either command manually — the deletion is part of the bootstrap
protocol and must complete before step 9.

The deletion message should record what samudaya contained and what it influenced. If no
seeds were present (only `README.md` and `examples/`), say so explicitly: "no seeds present;
directory removed."

**Consume sadhana** — immediately after consuming samudaya, delete `sadhana/`:

```
git rm -r sadhana/
git commit -m "consume(sadhana): scaffold template consumed; derived structure in place"
```

If sadhana files were untracked:

```
rm -rf sadhana/
git commit --allow-empty -m "consume(sadhana): scaffold template consumed; derived structure in place"
```

### 9. Report

Do not begin this step until the genesis commit, the consume(samudaya) commit, and the
consume(sadhana) commit are all written. If any is unresolved, finish it before proceeding.

Output a structured handoff with four sections:

**Ontology** — the class definitions written. One line per class; list the outgoing edges.

**Objects seeded** — the instance nodes created. One line each; note which class each
instantiates.

**Implied edges, connectors, and calculators** — edges wired, crate stubs and skill stubs scaffolded. One line each.

**Next steps** — three concrete, ordered actions:

1. **First catalog entry** — identify the most authoritative data source for this domain
   and add it to `.yidam/catalog/` as the first provenance anchor. Name it specifically.
2. **First corpus expansion** — name the instance node most ready to grow and suggest the
   first sub-node or property to deepen it. This becomes the first epistemic commit after
   genesis.
3. **First agent** — describe the simplest agent immediately useful in this domain. One
   sentence on what it does and which corpus nodes it draws from.

Then ask:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps,
> and proposing new objects, implied edges, connectors, or calculators until the corpus
> reaches a stable initial state. Reply **yes** to continue, **no** to stop here, or
> describe a specific area to focus on.
