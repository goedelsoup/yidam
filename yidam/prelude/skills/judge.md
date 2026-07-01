---
name: judge
description: Evaluate a bootstrap result against the yidam rubric and produce a score report
---

# Skill: judge

Invoked by the test harness after a bootstrap run completes. Reads the resulting repo state
and the test scenario, scores the result against [rubric.md](../../tests/rubric.md), and
produces a structured quality report.

## Inputs

- The resulting repo state: corpus nodes, commit log, directory structure
- The scenario that seeded the domain owner
- The rubric

## Scoring bands

| Band | Meaning |
|---|---|
| `pass` | Criterion clearly met |
| `marginal` | Criterion partially met or ambiguous |
| `fail` | Criterion not met |

## What to assess

### Interrogation quality

Did the bootstrap agent ask clarifying questions before scaffolding? Look at the exchange
between the bootstrap and domain owner. A good bootstrap:
- Asks about the domain before naming any concepts
- Probes individual concepts for relationships and decomposability
- Confirms the ontology sketch before writing files

A bootstrap that jumps straight to scaffolding without dialogue fails this criterion.

### Corpus node quality

For each corpus node:
- Is it scoped to one concept? (fails if two or more concepts are fused)
- Is the content substantive — does it say something true and specific about the concept?
- Is the size appropriate? (a paragraph is right; a sentence is thin; multiple screens is too large)
- Does it have at least one meaningful edge — a link that reflects a real conceptual relationship,
  not a citation to a directory or README?

### Genesis commit quality

The genesis commit message should:
- Name the domain
- Describe the ontology — what the seed nodes are
- Note at least one edge (relationship between nodes)

A message that reads as boilerplate or lists only filenames fails this criterion.

### Ontology coherence

Looking at the seed nodes as a set: do they feel like the right starting points for this domain?
Are they at a consistent level of abstraction? Would a domain expert recognize them?

Use the scenario's `good_bootstrap_looks_like` field as the reference.

## Output format

Produce a markdown report with:
- One section per quality criterion with band (`pass` / `marginal` / `fail`) and 1–2 sentence rationale
- A summary section with overall band and the single most important finding
