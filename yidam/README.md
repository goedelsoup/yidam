# yidam

*Archetype.* The durable infrastructure layer carried by every derived repository.

`yidam/` is the meta-template engine: the prelude an agent reads before acting, the tools
it invokes to query the corpus, and the tests that verify bootstrap correctness. `yidam clone`
carries it into a new repository and `yidam overlay` onto an existing one; what *survives* in
the derived repo is the prelude alone, at `.yidam/.vendor/prelude/`, re-vendored by
`mise run yidam-vendor-update` when the template evolves.

## Purpose

Every derived repo needs a shared cognitive substrate — a common model of what the knowledge
graph is, how phases work, what the graph invariants are, and how to read the corpus. `yidam/`
carries that substrate. It is infrastructure, not domain content: it does not know about any
particular field, corpus, or ontology. The domain lives in `corpus/`, `agents/`, and `skills/`;
`yidam/` provides the tools and language for building it.

## Contents

| Subdirectory | Contents |
|---|---|
| `prelude/` | Foundational texts and behavioral norms the agent reads before acting |
| `cli/` | The `yidam` CLI — corpus analysis, linting, indexing, export, MCP and LSP servers |
| `editors/` | The editor surfaces: `serve --lsp` for any LSP client, and the VS Code extension |
| `tests/` | Bootstrap test harness and rubric |
| `design/` | Design system and UI kits for the web surfaces |
| `web/docs/` | The Astro/Starlight docs site, rendering the template's `docs/` |

Only `prelude/` is vendored. `clone` copies the whole directory, and then the bootstrap's
vendor step moves `prelude/` to `.yidam/.vendor/prelude/` and deletes the rest — because
none of the rest is readable, runnable, or updatable from inside a derived repo. Carrying the
CLI source there would produce a fork that is never rebuilt.

### prelude/

The prelude is structured as a curriculum. An agent reads it in order before any other action:

- **[SCRIPTURE.md](prelude/SCRIPTURE.md)** — narrative orientation; read first
- **[IDENTITY.md](prelude/IDENTITY.md)** — what this kind of repository is and how to inhabit it
- **[GRAPH.md](prelude/GRAPH.md)** — how git history encodes knowledge
- **[CONSTITUTION.md](prelude/CONSTITUTION.md)** — invariant constraints governing sangha resolutions
- **[PHASES.md](prelude/PHASES.md)** — how post-genesis inquiry is structured into named units of work
- **[guidelines/](prelude/guidelines/)** — behavioral norms and directory conventions for agents operating in the graph
- **[skills/](prelude/skills/)** — capabilities provided by yidam: bootstrap, and domain-specific extensions. The judge is not here: it scores yidam's own harness runs and would otherwise be vendored into every derived repo, so it lives beside the rubric at [tests/judge.md](tests/judge.md)
- **[sdks/](prelude/sdks/)** — programmable bindings to the prelude model in Rust, TypeScript, and Python; cross-language parity harness; formal specifications

### cli/

The `yidam` binary is the derived repo's operational tool. It reads `.yidam/corpus/` and
produces:

- Corpus index tables and semantic index status
- Open-question lists and graph integrity checks (`yidam graph-check`)
- Skills, agents, crates, and packages index tables — the outputs `mise run regen` writes
- Catalog audit reports and web bundle status
- Decisions log from `.yidam/decisions/`
- Typed traversals over the resolved graph (`graph`, `query`, `pack`, `neighbors`)
- MCP and LSP service over stdio (`serve --mcp`, `serve --lsp`)
- `.yiz` bundles — gzipped tar archives containing the full corpus, rendered indexes, skills, decisions, and a manifest

In *this* repository, `mise run yidam-build` compiles it `--features full` and installs it to
`.local/bin/yidam`, which `mise.toml` puts first on `PATH` — deliberately per-repository, so
building here cannot clobber a derived repo's pinned binary. A derived repo builds its own to
`.yidam/bin`. To just *use* the CLI, install a release instead; see the
[repository README](../README.md#getting-started).

### editors/

`yidam serve --lsp` for any LSP-capable editor, and the VS Code extension. Both render
verdicts the CLI computes and neither re-derives them — see
[editors/README.md](editors/README.md).

### tests/

The Rust test harness runs bootstrap scenarios against the judge rubric in `tests/rubric.md`.
Each scenario in `tests/scenarios/` describes an agent run; the harness evaluates the outcome
against rubric criteria. See [HARNESS.md](tests/HARNESS.md) for authoring scenarios.

## Lifecycle

1. `yidam clone <dir>` copies the template (minus `docs/` and `examples/`) into a new repo and
   inits a fresh git repo.
2. `yidam overlay <dir>` adds the infrastructure — `yidam/`, `sadhana/`, `BOOTSTRAP.md`,
   `mise.yidam.toml`, and the `.yidam.toml` pin — to an existing git repo, leaving its own
   content untouched.
3. Bootstrap reads `yidam/prelude/` before beginning the ontology dialogue.
4. After genesis, the bootstrap's vendor step moves `yidam/prelude/` to
   `.yidam/.vendor/prelude/` and removes the rest of `yidam/`.
5. `mise run regen` runs all REGEN passes, populating README indexes with live corpus data.
6. When the yidam template updates, `mise run yidam-vendor-update` re-vendors the prelude and
   re-pins `.yidam.toml`, without touching domain content.

## What yidam is not

- A knowledge graph: domain content lives in `corpus/`, `agents/`, `skills/`, and `catalog/`
- A fixed schema enforced at parse time: the prelude is read by agents, not validated by a parser
- A runtime dependency at inference time: the yidam CLI is a development tool, not part of any deployed artifact
