# docs

*Documentation for the yidam template itself — how it works, how to use it, and the decisions
that shaped it.*

This directory is part of the yidam template and does **not** get copied into derived
repositories. It describes yidam, not any domain being bootstrapped. For the documentation
scaffold that derived repos receive, see [sadhana/docs/](../sadhana/docs/README.md).

Everything below is published at **[goedelsoup.github.io/yidam](https://goedelsoup.github.io/yidam/)**,
rendered from these files on every push to `main`. This page is the exception: the site's
sidebar is its version of the contents table, so publishing it would ship a second copy.

## Contents

### Start here

| Document | Topic |
|---|---|
| [what-yidam-is.md](what-yidam-is.md) | The graph model, the two commit kinds, what a corpus is made of, and what the discipline buys |
| [quickstart.md](quickstart.md) | Install, read a worked corpus, break its gate and repair it, then bootstrap your own |
| [installation.md](installation.md) | Every install channel, the cargo-feature matrix, verifying, upgrading |

### Using yidam

| Document | Topic |
|---|---|
| [cli-reference.md](cli-reference.md) | Every command, grouped as `--help` groups them; the query language; which commands rewrite files |
| [configuration.md](configuration.md) | `.yidam.toml`, `.yidam/config.toml`, the lint baseline, `tonpa.toml`, env vars, editor settings |
| [editor-setup.md](editor-setup.md) | `serve --lsp` for Neovim and Helix; installing and resolving the VS Code extension |
| [mcp-server.md](mcp-server.md) | Connecting an agent: which build carries `serve --mcp`, client configuration, which tool to reach for, and what `degraded` and `origin` mean |
| [sharing-derivations.md](sharing-derivations.md) | Publishing a `.yiz` bundle and consuming one; what a cross-corpus citation is and is not |
| [troubleshooting.md](troubleshooting.md) | `yidam doctor`'s ten checks, and the failure modes that actually recur |

### The model

| Document | Topic |
|---|---|
| [vocabulary.md](vocabulary.md) | All system terms defined; claim confidence markers |
| [information-architecture.md](information-architecture.md) | Directory layout, node structure, instance and decision schemas |
| [git-branch-model.md](git-branch-model.md) | `ma/<elector>` positions, `rigpa/<evolution>` evolutions, phase types |
| [bootstrap-flow.md](bootstrap-flow.md) | The ten-step onboarding sequence with quality criteria |
| [domain-computer.md](domain-computer.md) | Connectors, calculators, feature engineering, vector index |
| [web-interface.md](web-interface.md) | Optional data export, bundle contracts, CLI-generated status fields |
| [../examples/](../examples/README.md) | Worked corpora, gated by this repository's CI — what a good corpus looks like |
| [walkthroughs/](walkthroughs/property-research.md) | One domain per page: the ontology dialogue that produced it, and the question a folder cannot answer |

### Governance and quality

| Document | Topic |
|---|---|
| [sangha-resolution-flow.md](sangha-resolution-flow.md) | When and how to resolve; elector registration |
| [constitutional-governance.md](constitutional-governance.md) | The six articles of the invariant constitution |
| [conduct-norms.md](conduct-norms.md) | Deliberate commits, generous linking, provenance preservation |
| [quality-rubric.md](quality-rubric.md) | Structural checks, scored quality criteria, regression thresholds |
| [test-harness.md](test-harness.md) | Bootstrap/domain-owner/judge triad, scenario schema, snapshot path |
| [post-genesis-measurement.md](post-genesis-measurement.md) | What three derived repositories say about corpus health over time, and why reachability is a per-class property |

### The project

| Document | Topic |
|---|---|
| [contributing.md](contributing.md) | Setup, the gates, what a change needs, where a change goes, proposing a design |
| [versioning.md](versioning.md) | The four independent release trains and how a derived repo pins them |
| [aesthetic-direction.md](aesthetic-direction.md) | Naming register and design implications |
| [rfcs/](rfcs/README.md) | Design documents under review |

## What belongs here

- Design documents for yidam features or architectural decisions
- ADRs (architecture decision records) for template-level choices
- Notes on the relationship between yidam layers (prelude, sadhana, samudaya, tools)
- Anything describing the template's own structure, not any derived repo's domain

What does **not** belong here: corpus nodes, domain knowledge, derived-repo documentation.
Those live in `corpus/`, the domain's `docs/`, or `sadhana/docs/` respectively.
