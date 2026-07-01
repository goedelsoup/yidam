# Ohio Statutory Law — Bootstrap Script

Derived from yidam-proto-004a (score: ~7/10). Domain: the Ohio Revised Code — how
statutes, definitions, court decisions, and legal precedent form a knowledge graph for
legal research.

---

## Initial User Message

> The Ohio Revised Code. I want a corpus for researching Ohio statutory law — how statutes
> are structured, how definitions within them propagate, and how court decisions interpret
> and extend them. Start with criminal law (Title 29) but the structure should generalize.

---

## Prelude Internalized Checkpoint

> **Prelude internalized.** Graph model: files are nodes, markdown links are directed edges,
> git history is the knowledge record. Key constraints: every node must link to at least
> one other; commit messages describe what was understood; no orphan nodes. Directory
> layout: class definitions in `.yidam/corpus/<class>.ont.yml`; instances in
> `.yidam/corpus/<class>/<instance>.yml`.

---

## Ontology Discovery Dialogue

**Bootstrap:** Before scaffolding anything, I want to understand the knowledge structure
you're after. The ORC is a large, hierarchical document — but you've said the interesting
thing is how definitions *propagate* and how courts *interpret*. That suggests the graph
isn't the statute tree itself but something richer.

A few questions:

1. Is a *statute section* (e.g., § 2901.01) a node, or is it a *definition within a
   section* that matters? Some sections contain dozens of definitions.
2. When a court decision interprets a term, does the decision become a node that *modifies*
   the definition, or does it become a node in its own right with edges to what it touches?
3. Are you interested in the *hierarchy* of the ORC (Title → Chapter → Section) as
   structure, or as incidental context for the definitions and cases?

**User:** Definitions within sections are nodes. Court decisions are their own nodes —
they modify or extend definitions. The hierarchy matters for navigation but isn't the
primary structure.

**Bootstrap:** That clarifies a lot. So the core graph has:
- **Definitions** that live inside sections
- **Sections** that contain and group definitions
- **Chapters** and **Titles** for navigational hierarchy
- **Court decisions** that interpret, extend, or overturn definitions

One more question: does *precedent* between court decisions matter? If Decision A relies
on Decision B's interpretation, is that edge worth capturing?

**User:** Yes — the precedent chain is exactly what's interesting in criminal law.

**Bootstrap:** And one structural question: are there concepts that appear *across* the
ORC — definitional constants like "serious physical harm" or "purpose" — that need to be
represented differently from section-local definitions?

**User:** Yes. Cross-chapter definitions are first-class. They're often the most contested.

---

## Confirmed Sketch

**Nodes**

| Node | What it is |
|------|------------|
| `statute-section` | A numbered section of the ORC (e.g., § 2901.01) — the primary organizational unit |
| `definition` | A named term defined within a section, potentially reused across chapters |
| `chapter` | A grouping of related sections within a Title (e.g., Chapter 2901 — General Provisions) |
| `court-decision` | A published ruling by an Ohio court or the Supreme Court of Ohio |
| `legal-concept` | A cross-chapter concept not defined in a single section — "mens rea," "harm," "purpose" |
| `precedent-chain` | A sequence of decisions in which each relies on or modifies the prior interpretation |

**Edges**

```
statute-section →[defines]→ definition
statute-section →[contained-in]→ chapter
definition →[referenced-in]→ statute-section
definition →[cross-references]→ definition
court-decision →[interprets]→ definition
court-decision →[relies-on]→ court-decision
court-decision →[overturns]→ court-decision
legal-concept →[instantiated-by]→ definition
precedent-chain →[includes]→ court-decision
```
