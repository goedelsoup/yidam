---
name: bootstrap
description: Initialize an yidam-derived repository from an empty or near-empty state
---

# Skill: bootstrap

Invoked when an agent enters an empty or near-empty repository with [BOOTSTRAP.md](../../BOOTSTRAP.md)
as its entry prompt. Produces a fully scaffolded, yidam-derived repository with a seeded
corpus and a legible genesis commit.

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

Read all files under `prelude/` as foundational context. They establish the identity, graph
model, and conduct norms that govern everything that follows. This step is not optional.

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

The ontology sketch is the blueprint for everything that follows. The genesis commit should
be a faithful rendering of it — not a guess at what might be useful.

### 3. Scaffold the structure

With the ontology confirmed, create the following directories and stub files if they do not
exist. See [directory conventions](../guidelines/directories.md) for what belongs in each.

- `agents/` — agent definitions specific to this repo
- `catalog/` — data source registry; shallow refs for corpus edges
- `corpus/` — knowledge nodes; the primary body of domain content
- `crates/` — Rust crates implementing the retrieval and traversal toolkit
- `packages/` — other-language packages in the same toolkit layer
- `skills/` — skills available to agents in this repo
- `web/` — web interface layer, if applicable

Stubs should be minimal — a one-line header and a single orienting sentence. Do not
pre-populate with placeholder content.

### 4. Seed the corpus

Render the confirmed ontology sketch as corpus nodes. Each node should be a small, focused
markdown file — one concept per file. Each node should:

- Have a clear, specific title matching the concept name from the ontology
- Contain 2–4 sentences establishing what it is and why it is irreducible here
- Reference at least one related node (an edge) — use the ontology sketch as the edge map

Prefer depth over breadth. Two well-linked nodes are better than five orphans.

### 5. Write the genesis commit

Commit all scaffolding and seed nodes as a single genesis commit. The commit message should
name the domain, describe the ontology sketch, and note what the initial edges are.
This message is the first event in the knowledge graph; it should read like one.

If `samudaya/` was present, commit its removal immediately after the genesis commit as a
separate consumption event. The message should record what samudaya contained and what it
influenced (e.g., which axioms became seed nodes, which constraints shaped the scaffold).
Samudaya's content is now preserved only in git history — that is by design.

### 6. Report

After committing, summarize:

- The ontology: what the seed nodes are and how they relate
- What the structure is ready for
- Suggested next inquiry threads — what the graph most wants to grow toward
