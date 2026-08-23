# yidam

**A template and toolchain for living knowledge artifacts — git repositories whose history
*is* a knowledge graph.**

Most knowledge systems keep the graph in a database and the prose in files, then spend their
lives keeping the two in agreement. yidam removes the database. In a yidam-derived repository
the graph is the repository:

| Git | Graph |
|---|---|
| a file | a node — one concept, relation, artifact, or open question |
| a markdown link `[label](path)` | a directional edge |
| a commit | a knowledge event |
| a branch | a parallel inquiry thread |
| a merge | a synthesis |

Provenance, attribution, review, blame, and time travel come for free, because git already
does all of them. What yidam adds is the model that makes a repository legible as *knowledge*
rather than as code — and the tooling to query, lint, index, and export it.

**This repository is the template, not an instance of it.** It holds no domain knowledge.
Bootstrapping it produces a repository that does.

## Two commit kinds, and no others

An **epistemic** commit means what the corpus knows has changed; its message is testimony,
not a changelog. An **operational** commit means the pipeline advanced — an extraction ran,
an index rebuilt — legitimate provenance, but not a knowledge event.

The distinction is carried by a closed vocabulary of leading verbs (`establish:`, `revise:`,
`open:`, `close:`, `synthesize:` … versus `extract:`, `refresh:`, `index:`, `regen:` …), which
`yidam lint --commits` enforces. See [prelude/GRAPH.md](yidam/prelude/GRAPH.md) for the full
list and the reasoning behind closing it.

## Getting started

> **In a hurry?** [docs/quickstart.md](docs/quickstart.md) goes from no toolchain to a
> bootstrapped repository whose gate you have watched pass, fail, and pass again — by way of
> a worked corpus in [examples/streamflow/](examples/streamflow/). About twenty minutes.

Get the CLI. No toolchain required — the default build ships as a binary:

```sh
curl -fsSL https://raw.githubusercontent.com/goedelsoup/yidam/main/install.sh | sh
```

It resolves the latest release for your platform, verifies the checksum, and installs to
`~/.local/bin` (override with `YIDAM_BIN_DIR`). If no checksum tool is present it declines
the download rather than installing something it could not verify.

On a Mac or a Linux box with Homebrew, the tap serves the same binary and keeps it upgradable
with everything else:

```sh
brew install goedelsoup/tap/yidam
```

The formula is rendered from the release's own checksums by the release workflow, so the tap
cannot lag behind a published version.

With cargo already on hand, [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall)
fetches the same artifact:

```sh
cargo binstall yidam
```

Or from source, if you would rather. Same light default build, and it needs only a Rust
toolchain — no protoc, no system C library, no ML runtime. `--locked` builds from the lock file the
tag committed, which is what makes it the same binary the release was built from; without it
cargo re-resolves every dependency:

```sh
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.2.1 --locked yidam
```

Either way, `yidam --version` should answer, naming the build and the features it carries.

`--features full` adds `index-build`, `serve --mcp`, and the sqlite/rdf exports — and with
them protoc 31, an ONNX runtime, and a system C toolchain. That is the maintainer's build,
described under [Working on yidam](#working-on-yidam) below. It is **not** what deriving a
repository needs: `clone`, `overlay` and `tonpa` are all in the light set, as is every report
and gate.

Then create a derived repository, or overlay the infrastructure onto one that already exists:

```sh
yidam clone ../my-domain          # new repo, fresh git history
yidam overlay ../existing-repo    # existing repo, content untouched
```

Neither command produces knowledge. They produce a repository ready to be bootstrapped: point
an agent at its `BOOTSTRAP.md`, which routes to the bootstrap skill. The skill runs an
ontology-discovery dialogue with you *before* scaffolding anything, and only then writes the
genesis commit — a faithful rendering of the ontology you confirmed together.

To seed that dialogue with prior commitments, drop files into `samudaya/` before the agent
arrives (see [samudaya/README.md](samudaya/README.md)).

## Layout

| Path | Layer |
|---|---|
| [`yidam/prelude/`](yidam/prelude/) | The inherited model: scripture, identity, graph model, constitution, phases, conduct guidelines, skills, SDKs |
| [`yidam/cli/`](yidam/cli/) | The `yidam` binary — corpus analysis, linting, indexing, export, MCP server |
| [`yidam/tests/`](yidam/tests/) | Bootstrap test harness — scenarios scored against a judge [rubric](yidam/tests/rubric.md) |
| [`yidam/design/`](yidam/design/) | Design system and UI kits for the web surfaces |
| [`yidam/web/docs/`](yidam/web/docs/) | Astro/Starlight docs site, rendering `docs/` |
| [`sadhana/`](sadhana/) | The scaffold copied into derived repos — directory shape, README stubs, root files, CI |
| [`samudaya/`](samudaya/) | Seed layer — axioms, hints, constraints, augmentations; consumed at genesis |
| [`packages/web/`](packages/web/) | Browser shell over an exported bundle — embeddings and vector search in WASM |
| [`docs/`](docs/README.md) | Documentation for yidam itself: design docs, RFCs, vocabulary |
| [`examples/`](examples/README.md) | Worked corpora for reading — not copied into a derived repository |
| [`BOOTSTRAP.md`](BOOTSTRAP.md) | The agent entry prompt a derived repo is bootstrapped from |
| [`mise.yidam.toml`](mise.yidam.toml) | The inherited task layer derived repos include |

Three layers meet in a derived repo and each has a different lifetime: `yidam/` is vendored
and re-vendored as the template evolves; `sadhana/` becomes the repo's own content at genesis
and diverges from there; `samudaya/` is consumed and deleted, surviving only in history.

## The CLI

```
yidam doctor              is this setup sound? pin, PATH, prelude age, index, REGEN — read-only
yidam status              repo overview: nodes, open questions, catalog, index freshness, phases
yidam graph-check         orphans, broken links, missing labels — the gate CI runs
yidam lint --commits      corpus quality checks against a baseline ratchet, plus the commit vocabulary
yidam diff main..HEAD     node and edge changes between two refs
yidam phases              active inquiry branches
yidam embed               extract embedding text from corpus instances
yidam index-build         build the LanceDB vector index
yidam serve --mcp         serve the domain computer to MCP-capable agents over stdio
yidam export --format …   bundle · web · rdf · graphml · sqlite · llms
yidam tonpa add …         manage bundle dependencies on other derived repos
```

`yidam --help` lists all of them under these same groups, and marks with `*` the ones that
rewrite files in the repository they are run against — ten of them do, and that was
previously visible only in each command's long help, where you had to already suspect it to
go looking.

Index subcommands (`corpus-index`, `skills-index`, `catalog-audit`, …) back the
`<!-- REGEN: yidam <subcommand> -->` markers embedded in README files. `mise run regen`
refreshes them all in one pass; in derived repos a stale REGEN block is a failing build.

`yidam doctor` is the one to reach for when something is off and you do not yet know what.
It answers, in one screen, the questions that were previously spread across a stderr
warning, a CI step, and three reports: whether this is a derived repository at all, whether
the running binary is the one `.yidam.toml` pins, whether `.yidam/bin` is ahead on PATH, how
old the vendored prelude is, whether the index and the REGEN blocks are current, and what
this binary was compiled with. It writes nothing and does no network, so it is safe against
a checkout you only mean to inspect — which the rest of the reports are not. It exits
nonzero on what is wrong now; warnings (no index, an old pin) gate only under `--strict`.

The binary is partitioned by cargo feature so the common case stays cheap to install:

| Feature | Adds | Cost |
|---|---|---|
| `reports` *(default)* | Every pure-Rust command — reports, gates, export to graphml/llms, clone, overlay | None. No protoc, no ML runtime |
| `tonpa` *(default)* | Bundle dependency manager — `tonpa add`, `verify`, `update` | reqwest (rustls) + tokio. Vendored C, no system library |
| `index` | `index-build`, `serve --mcp` | fastembed (ONNX) + LanceDB; needs protoc 31 at build time |
| `export-sqlite` | `export --format sqlite` | Bundled SQLite + sqlite-vec, compiled from C |
| `export-graph` | `export --format rdf` | Pure Rust |
| `full` | All of the above | |

`tonpa` is in the default set even though it costs an HTTP stack, because it is the only
feature whose absence broke an instruction rather than removing a capability: without it
`yidam tonpa add …` answered `unrecognized subcommand`, and inside a script with output
redirected that is indistinguishable from success.

MSRV is Rust 1.85, deliberately below the 1.88 toolchain this repo pins, so derived repos on
older toolchains can still install the CLI.

## Prelude SDKs, parity, and specs

The prelude model is not only prose. [`yidam/prelude/sdks/`](yidam/prelude/sdks/) implements
it three times — Rust (the reference), TypeScript, and Python — and holds all three to the
same TOML fixtures. The CLI consumes the Rust implementation as `yidam-core` rather than
re-implementing the parse-and-classify surface, so drift between the tool and the model
becomes a test failure instead of a mystery.

```sh
mise run parity          # cross-language parity for the SDK functions
mise run domain-parity   # same discipline for the domain calculators in prelude/domains/
mise run embed-parity    # embedding reproducibility across fastembed / transformers.js / sentence-transformers
mise run verify          # Dafny specs and Lean 4 proofs (dafny + lake installed separately)
```

## Working on yidam

```sh
mise install             # provision toolchains (rust, protoc, python, uv, node)
mise tasks               # everything available
mise run yidam-build     # install the full-feature binary into .local/bin
mise run ci              # fmt-check, clippy -D warnings, tests (harness + CLI)
mise run docs-dev        # docs site on http://localhost:4321/
```

`yidam-build` here is `--features full`, deliberately: working on the CLI means being able to
run `index-build`, `serve --mcp`, and the sqlite/rdf exports. That is why `mise install`
provisions protoc and the rest, and why it is the *maintainer's* setup rather than the one
[Getting started](#getting-started) describes.

CI runs the harness and the light CLI build as parallel jobs on every PR; the full-feature
build (protoc, ML stack) runs on `main` and on a weekly schedule.

## Naming

The vocabulary is drawn from Tibetan Buddhist epistemology; the register is deliberate.

| Term | Role in the system |
|---|---|
| **yidam** | The chosen form one commits to — here, the durable infrastructure every derived repo carries |
| **sadhana** | *Practice* — the structural scaffold that gives a derived repo its shape |
| **samudaya** | *Arising* — the conditions seeded before a bootstrap, consumed at genesis |
| **sangha** | The collective — governance, electors, and resolution records |
| **rigpa** | *Clear seeing* — a settled collective understanding; the branch `rigpa/<evolution>` |
| **ma** | *Voice, position* — one elector's working branch, `ma/<name>` |
| **tonpa** | The bundle dependency manager — how one derived corpus draws on another |

Full definitions, including the `[verified]` / `[inference]` / `[open]` claim markers, are in
[docs/vocabulary.md](docs/vocabulary.md).

## Documentation

Start with [what-yidam-is.md](docs/what-yidam-is.md), then
[information-architecture.md](docs/information-architecture.md) and
[bootstrap-flow.md](docs/bootstrap-flow.md). The full index is in
[docs/README.md](docs/README.md); designs under review live in [docs/rfcs/](docs/rfcs/README.md).

## Status

Pre-1.0. The template, the bootstrap protocol, and the CLI version independently on separate
release trains — see [VERSIONING.md](VERSIONING.md) before bumping anything. Derived repos pin
what they inherited in their own `.yidam.toml`.

## License

[MIT](LICENSE).
