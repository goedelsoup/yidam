# yidam

*Archetype.* The durable infrastructure layer carried by every derived repository.

`yidam/` is the meta-template engine: the prelude an agent reads before acting, the tools
it invokes to query the corpus, and the tests that verify bootstrap correctness. It is copied
from this template repository into every derived repo — by `mise run clone` for new repos,
by `mise run overlay` for existing ones — and updated in place when the template evolves.

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
| `tools/yidam/` | The `yidam` CLI — corpus analysis and index tool |
| `tests/` | Bootstrap test harness and rubric |

### prelude/

The prelude is structured as a curriculum. An agent reads it in order before any other action:

- **[SCRIPTURE.md](prelude/SCRIPTURE.md)** — narrative orientation; read first
- **[IDENTITY.md](prelude/IDENTITY.md)** — what this kind of repository is and how to inhabit it
- **[GRAPH.md](prelude/GRAPH.md)** — how git history encodes knowledge
- **[CONSTITUTION.md](prelude/CONSTITUTION.md)** — invariant constraints governing sangha resolutions
- **[PHASES.md](prelude/PHASES.md)** — how post-genesis inquiry is structured into named units of work
- **[guidelines/](prelude/guidelines/)** — behavioral norms and directory conventions for agents operating in the graph
- **[skills/](prelude/skills/)** — capabilities provided by yidam: bootstrap, judge, and domain-specific extensions
- **[sdks/](prelude/sdks/)** — programmable bindings to the prelude model in Rust, TypeScript, and Python; cross-language parity harness; formal specifications

### tools/yidam/

The `yidam` binary is the derived repo's operational tool. It reads `.yidam/corpus/` and
produces outputs used by `mise run regen`:

- Corpus index tables and semantic index status
- Open-question lists and graph integrity checks (`yidam graph-check`)
- Skills, agents, crates, and packages index tables
- Catalog audit reports and web bundle status
- Decisions log from `.yidam/decisions/`
- `.yiz` bundles — gzipped tar archives containing the full corpus, rendered indexes, skills, decisions, and a manifest

Build once with `mise run yidam-build`; the binary installs to `~/.cargo/bin/yidam`.

### tests/

The Rust test harness runs bootstrap scenarios against the judge rubric in `tests/rubric.md`.
Each scenario in `tests/scenarios/` describes an agent run; the harness evaluates the outcome
against rubric criteria. See [HARNESS.md](tests/HARNESS.md) for authoring scenarios.

## Lifecycle

1. `mise run clone <dir>` copies the full template (including `yidam/`) into a new repo and
   inits a fresh git repo.
2. `mise run overlay <dir>` copies only `yidam/` infrastructure into an existing git repo,
   leaving its content untouched.
3. Bootstrap reads `yidam/prelude/` before beginning the ontology dialogue.
4. `mise run yidam-build` compiles and installs the `yidam` binary from `tools/yidam/`.
5. `mise run regen` runs all REGEN passes, populating README indexes with live corpus data.
6. When the yidam template updates, re-run `mise run overlay` to sync `yidam/` without
   touching domain content.

## What yidam is not

- A knowledge graph: domain content lives in `corpus/`, `agents/`, `skills/`, and `catalog/`
- A fixed schema enforced at parse time: the prelude is read by agents, not validated by a parser
- A runtime dependency at inference time: the yidam CLI is a development tool, not part of any deployed artifact
