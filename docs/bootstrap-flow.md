# The bootstrap flow

This is the onboarding flow for a new derived repository. It is the most interaction-dense
surface in the system.

## Overview

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

## Samudaya seed kinds

Pre-placed seed files shape the bootstrap before dialogue begins:

| Kind | Behavior |
|------|----------|
| `axiom` | Concept that must appear in the corpus; treated as pre-committed |
| `hint` | Candidate relationship to surface during discovery; may be discarded |
| `constraint` | Hard scope boundary; enforced during scaffolding |
| `augmentation` | Additional prelude content; constitutional augmentations persist permanently |

## Ontology discovery sketch format

The bootstrap confirms the ontology with the user in this format before writing any files:

**Nodes**

| Node | What it is |
|------|------------|
| `name` | one-line description |

**Edges**

```
source →[relationship]→ target
```

## Prelude internalized checkpoint

Before questions begin, the bootstrap outputs a standalone message:

> **Prelude internalized.** Graph model: [one sentence]. Key constraints I'll honor:
> [two or three bullet points]. Directory layout: [one sentence].

## Genesis commit quality criteria

The genesis commit message must:
- Name the domain
- Describe the ontology (what the seed nodes are)
- Note at least one edge (relationship between nodes)

A boilerplate message or a list of filenames fails.

## Continuation offer

After the genesis commit, the bootstrap asks:

> **Continue?** I can enter a seed/scaffold loop — reading the corpus, identifying gaps, and
> proposing new objects, implied edges, connectors, or calculators until the corpus reaches a
> stable initial state. Reply **yes** to continue, **no** to stop here, or describe a
> specific area to focus on.
