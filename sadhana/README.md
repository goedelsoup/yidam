# sadhana

*Practice.* The scaffold that gives the derived repository its shape.

`sadhana/` is the directory-structure layer of the yidam template. Its subdirectories mirror
the top-level layout of every derived repo — `agents/`, `catalog/`, `corpus/`, `crates/`,
`docs/`, `packages/`, `sangha/`, `skills/`, `web/` — each seeded with a README carrying
placeholder TEMPLATE comments and REGEN markers.

Two subdirectories are not directory mirrors and install to a renamed path:

| Sadhana path | Installs to | Why it exists |
|---|---|---|
| `root/` | repository root | The derived repo's own `README.md`, `AGENTS.md`, `mise.toml`, and `.claude/CLAUDE.md`. Yidam's copies of these describe *yidam* and must not survive genesis. |
| `github/` | `.github/` | The derived repo's CI. Yidam's own workflow builds paths (`yidam/cli`, `yidam/tests/harness`) that the `vendor(yidam)` step removes, so inheriting it yields a build that compiles nothing. |

`root/` and `github/` are spelled without a leading dot deliberately: `ls sadhana/` is a step
in the bootstrap skill, and a dotfile directory would not appear in it.

## Purpose

Where [samudaya](../samudaya/) provides domain influence, sadhana provides structural
influence: the directories, README scaffolds, and REGEN stubs that the bootstrap agent
commits as the skeleton of the new repository.

Sadhana is not destroyed at genesis. Its files become the derived repo's own content —
the TEMPLATE comments are replaced by domain-specific text, and the REGEN markers are
populated by the `yidam` CLI as the corpus grows. Sadhana's shape persists as the
architecture of the derived repo.

## Contents

Each subdirectory in `sadhana/` corresponds to a top-level directory in derived repos:

| Directory | Role in the derived repo |
|---|---|
| `agents/` | Domain-specific agent definitions |
| `catalog/` | Source catalog: references, datasets, primary texts |
| `corpus/` | Knowledge graph: concepts, relations, artifacts, open questions |
| `crates/` | Domain computer: Rust workspace (calculators, connectors, index) |
| `docs/` | Prose documentation: field notes, memos, synthesis reports |
| `packages/` | ML pipelines and embedding model packages (Python/uv) |
| `sangha/` | Governance: sangha protocol, elector list, resolution records |
| `skills/` | Domain-specific skills extending the prelude |
| `web/` | Data export: web-facing JSON feeds, bundle contracts |
| `root/` | Repository-root files: `README.md`, `AGENTS.md`, `mise.toml`, `.claude/CLAUDE.md` |
| `github/` | `.github/` — the derived repo's CI workflow |

Each README carries `<!-- TEMPLATE -->` comments guiding the bootstrap agent on what
domain-specific text to write, and `<!-- REGEN: yidam <subcommand> -->` markers that
the `yidam` CLI populates with computed indexes.

## Lifecycle

1. Bootstrap reads `sadhana/` to understand the expected directory structure.
2. Bootstrap copies each subdirectory into the derived repo root, then replaces TEMPLATE
   comment bodies with domain-specific content and commits the result.
3. After genesis, `sadhana/` is no longer referenced as a template source — its files
   are now the derived repo's own content, diverging as the domain evolves.
4. In this (yidam template) repository, `sadhana/` is updated when the scaffold structure
   changes. Derived repos receive updates only by re-running `mise run overlay` or manually
   merging changes from the template.

## What sadhana is not

- A replacement for domain authorship: TEMPLATE markers show where to write, not what to write
- A persistent dependency: once bootstrap copies and fills the scaffolds, the derived repo owns those files
- A schema enforced by any tool: the REGEN markers are filled by `yidam` CLI but the structure is read by an agent
