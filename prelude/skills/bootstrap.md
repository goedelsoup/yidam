---
name: bootstrap
description: Initialize an yidam-derived repository from an empty or near-empty state
---

# Skill: bootstrap

Invoked when an agent enters a freshly cloned yidam repository with [BOOTSTRAP.md](../../BOOTSTRAP.md)
as its entry prompt. Produces a fully scaffolded, ontology-grounded, corpus-seeded repository
with a legible genesis commit.

## Steps

### 0. Read samudaya (if present)

Before anything else, check whether `samudaya/` exists. If it does:

- Read every file in it — excluding `samudaya/examples/`, which contains format
  templates for domain authors, not seeds. Note each file's `kind` frontmatter field.
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

Read these files — in order — before doing anything else. Skip domain implementations and
SDK source code; they are reference artifacts, not context for bootstrapping.

1. `prelude/IDENTITY.md` — what kind of knowledge artifact this repo is
2. `prelude/GRAPH.md` — the graph model: nodes, edges, commit types, branch semantics
3. `prelude/CONSTITUTION.md` — the governance rules that constrain what you may do
4. `prelude/HARNESS.md` — how scenarios and the judge rubric work
5. `prelude/PHASES.md` — the named phases of inquiry
6. `prelude/guidelines/agent-conduct.md` — specific conduct norms
7. `prelude/guidelines/directories.md` — where things live and what belongs in each
8. `prelude/skills/judge.md` — the judge's criteria; internalize so the genesis commit passes

After reading all eight, output a brief synthesis before opening the dialogue:

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor: [two or
> three bullet points from CONSTITUTION and agent-conduct]. Directory layout: [one sentence].

This output is a checkpoint — it proves processing, not just scanning, and lets the user
correct any misread before the dialogue begins.

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

### 3. Orient to the scaffold

The repository's directory structure is already in place — yidam's clone process creates all
directories with templated READMEs. Do not recreate or overwrite them.

Instead, read these four files to confirm the layout and understand what each directory is
prepared to receive:

- `agents/README.md`
- `catalog/README.md`
- `corpus/README.md`
- `skills/README.md`

If any directory is missing, recreate it with a minimal README stub. Otherwise, proceed.

### 4. Formalize the ontology

Render each node from the confirmed sketch as a domain class definition in `corpus/`:

```
corpus/<domain-class>.ont.yml
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

### 5. Identify connectors and calculators

Before seeding any objects, read the full set of `.ont.yml` class definitions and reason
about what the schema implies at the domain level. Then present a structured report to the
user for confirmation:

**Connector sources** — edge types between classes that are not yet explicit in the `.ont.yml`
files but are implied by the domain. For each:

| From | Relationship | To | Basis |
|------|--------------|----|-------|
| `class` | verb phrase | `class` | one line — why this edge exists |

**Calculators** — domain computations that follow naturally from the class structure. For each:

| Name | Computes | Reads | Returns |
|------|----------|-------|---------|
| `name` | what it derives | which classes/edges | what it produces |

Do not wire connectors or implement calculators yet. This report is a checkpoint — present
it and wait for the user to confirm, modify, or discard individual items before proceeding.
Only carry forward what the user approves.

### 6. Seed corpus objects

For each class defined in step 4, instantiate at least one concrete object as a corpus
markdown node:

```
corpus/<instance-name>.md
```

Each instance node should:

- Open with a one-sentence statement of what this specific thing is
- Link to its class definition (e.g., `[Ruling](ruling.ont.yml)`)
- Carry at least one outgoing edge to another node (instance or class definition)
- Be specific enough to be wrong — a vague placeholder is not an object

For domains with clear hierarchies, seed from the root down so that edge targets exist when
they are referenced. Prefer depth over breadth: a well-linked object with real content is
worth more than several shallow stubs.

### 7. Wire connectors and scaffold calculators

After all objects are seeded, read the full corpus — every `.ont.yml` class file and every
instance node. Then act on what the user approved in step 5:

**Connectors** — add the approved edges as markdown links in the relevant instance nodes.
A connector resolves a missing relationship between specific objects; it does not add new
content to nodes.

**Calculators** — for each approved calculator, write a stub in `skills/`:

```
skills/<calculator-name>.md
```

The stub should describe what it computes, which corpus nodes it reads, and what it returns.
Do not implement — define the interface. Calculators are proposals that agents and developers
can later implement.

Commit connectors as a single epistemic commit. Commit calculator stubs as a separate
operational commit.

### 8. Write the genesis commit

Commit all class definitions (`.ont.yml`), seed instances, and scaffolded skill stubs as a
single genesis commit. The message should name the domain, summarize the class schema, and
describe what seed objects were created and how they connect.

This commit is the first event in the knowledge graph. It should read like one.

If `samudaya/` was present, remove it in a separate commit immediately after. The message
should record what samudaya contained and what it influenced — which axioms became class
definitions, which constraints shaped the schema. Samudaya's content now lives only in
history. That is by design.

### 9. Report

Output a structured handoff with four sections:

**Ontology** — the class definitions written. One line per class; list the outgoing edges.

**Objects seeded** — the instance nodes created. One line each; note which class each
instantiates.

**Connectors and calculators** — edges wired and skill stubs scaffolded. One line each.

**Next steps** — three concrete, ordered actions:

1. **First catalog entry** — identify the most authoritative data source for this domain
   and add it to `catalog/` as the first provenance anchor. Name it specifically.
2. **First corpus expansion** — name the instance node most ready to grow and suggest the
   first sub-node or property to deepen it. This becomes the first epistemic commit after
   genesis.
3. **First agent** — describe the simplest agent immediately useful in this domain. One
   sentence on what it does and which corpus nodes it draws from.

Then ask:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps,
> and proposing new objects, connectors, or calculators until the corpus reaches a stable
> initial state. Reply **yes** to continue, **no** to stop here, or describe a specific
> area to focus on.
