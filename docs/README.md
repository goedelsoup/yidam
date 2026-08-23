# docs

*Documentation for the yidam template itself — how it works, how to use it, and the decisions
that shaped it.*

This directory is part of the yidam template and does **not** get copied into derived
repositories. It describes yidam, not any domain being bootstrapped. For the documentation
scaffold that derived repos receive, see [sadhana/docs/](../sadhana/docs/README.md).

## Contents

| Document | Topic |
|---|---|
| [what-yidam-is.md](what-yidam-is.md) | The scripture, the knowledge graph model, the two commit kinds |
| [vocabulary.md](vocabulary.md) | All system terms defined; claim confidence markers |
| [information-architecture.md](information-architecture.md) | Directory layout, node structure, instance and decision schemas |
| [git-branch-model.md](git-branch-model.md) | `ma/<elector>` positions, `rigpa/<evolution>` evolutions, phase types |
| [bootstrap-flow.md](bootstrap-flow.md) | The ten-step onboarding sequence with quality criteria |
| [sangha-resolution-flow.md](sangha-resolution-flow.md) | When and how to resolve; elector registration |
| [constitutional-governance.md](constitutional-governance.md) | The six articles of the invariant constitution |
| [domain-computer.md](domain-computer.md) | Connectors, calculators, feature engineering, vector index |
| [web-interface.md](web-interface.md) | Optional data export, bundle contracts, CLI-generated status fields |
| [sharing-derivations.md](sharing-derivations.md) | Publishing a `.yiz` bundle and consuming one; what a cross-corpus citation is and is not |
| [quality-rubric.md](quality-rubric.md) | Structural checks, scored quality criteria, regression thresholds |
| [conduct-norms.md](conduct-norms.md) | Deliberate commits, generous linking, provenance preservation |
| [test-harness.md](test-harness.md) | Bootstrap/domain-owner/judge triad, scenario schema, snapshot path |
| [post-genesis-measurement.md](post-genesis-measurement.md) | What three derived repositories say about corpus health over time, and why reachability is a per-class property |
| [aesthetic-direction.md](aesthetic-direction.md) | Naming register and design implications |
| [rfcs/](rfcs/README.md) | Design documents under review — the downstream-integration-contract set (RFC-0001…0007) |

## What belongs here

- Design documents for yidam features or architectural decisions
- ADRs (architecture decision records) for template-level choices
- Notes on the relationship between yidam layers (prelude, sadhana, samudaya, tools)
- Anything describing the template's own structure, not any derived repo's domain

What does **not** belong here: corpus nodes, domain knowledge, derived-repo documentation.
Those live in `corpus/`, the domain's `docs/`, or `sadhana/docs/` respectively.
