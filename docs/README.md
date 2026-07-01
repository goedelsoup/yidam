# docs

*Documentation for the yidam template itself — how it works, how to use it, and the decisions
that shaped it.*

This directory is part of the yidam template and does **not** get copied into derived
repositories. It describes yidam, not any domain being bootstrapped. For the documentation
scaffold that derived repos receive, see [sadhana/docs/](../sadhana/docs/README.md).

## Contents

### [design-brief.md](design-brief.md)

The primary reference document for the yidam system. Thirteen sections covering:

| Section | Topic |
|---|---|
| 1 | What yidam is — the scripture, the knowledge graph model, the two commit kinds |
| 2 | Vocabulary — all system terms defined as design tokens |
| 3 | Information architecture — directory layout, node structure, instance and decision schemas |
| 4 | The git branch model — `ma/<elector>` positions and `rigpa/<evolution>` evolutions |
| 5 | The bootstrap flow — the ten-step onboarding sequence with quality criteria |
| 6 | The sangha resolution flow — when and how to resolve, elector registration |
| 7 | Constitutional governance — the six articles of the invariant constitution |
| 8 | The domain computer layer — connectors, calculators, feature engineering, vector index |
| 9 | The web interface layer — optional data export, bundle contracts, CLI-generated status |
| 10 | Quality rubric — structural checks, scored quality criteria, regression thresholds |
| 11 | Conduct norms — deliberate commits, generous linking, provenance preservation |
| 12 | Test harness and multi-agent architecture — bootstrap/domain-owner/judge triad, scenario schema |
| 13 | Aesthetic and tonal direction — naming register, design implications |

Read the design brief before extending yidam or authoring bootstrap scenarios.

## What belongs here

- Design documents for yidam features or architectural decisions
- ADRs (architecture decision records) for template-level choices
- Notes on the relationship between yidam layers (prelude, sadhana, samudaya, tools)
- Anything describing the template's own structure, not any derived repo's domain

What does **not** belong here: corpus nodes, domain knowledge, derived-repo documentation.
Those live in `corpus/`, the domain's `docs/`, or `sadhana/docs/` respectively.
