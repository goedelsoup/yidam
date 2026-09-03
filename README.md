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

## Read the documentation

**[goedelsoup.github.io/yidam](https://goedelsoup.github.io/yidam/)** — the documentation
site, and the route to prefer. It renders everything under [`docs/`](docs/README.md) with a
sidebar, search, and working cross-links.

It is versioned on the CLI, because that is what you have installed. The address above
documents the **current release**; the last three releases keep their own paths
(`/yidam/v0.7/`, and so on) and every page carries a menu to move between them.
[`/yidam/main/`](https://goedelsoup.github.io/yidam/main/) documents unreleased tooling and
says so on every page.

| Going to | Start at |
|---|---|
| Try it in twenty minutes | [Quickstart](https://goedelsoup.github.io/yidam/quickstart/) — install, read a worked corpus, break its gate and repair it |
| Understand the model first | [What yidam is](https://goedelsoup.github.io/yidam/what-yidam-is/), then [Information architecture](https://goedelsoup.github.io/yidam/information-architecture/) |
| Bootstrap a repository | [Bootstrap flow](https://goedelsoup.github.io/yidam/bootstrap-flow/) |
| Point an agent at a corpus | [Connecting an agent (MCP)](https://goedelsoup.github.io/yidam/mcp-server/) |
| Look up a term | [Vocabulary](https://goedelsoup.github.io/yidam/vocabulary/) |

The rest of this file is the repository's own map: how to install the CLI, where each layer
lives, and how to work on yidam itself.

## Two commit kinds, and no others

An **epistemic** commit means what the corpus knows has changed; its message is testimony,
not a changelog. An **operational** commit means the pipeline advanced — an extraction ran,
an index rebuilt — legitimate provenance, but not a knowledge event.

The distinction is carried by a closed vocabulary of leading verbs (`establish:`, `revise:`,
`open:`, `close:`, `synthesize:` … versus `extract:`, `refresh:`, `index:`, `regen:` …), which
`yidam lint --commits` enforces. See [prelude/GRAPH.md](yidam/prelude/GRAPH.md) for the full
list and the reasoning behind closing it.

## Getting started

> **In a hurry?** The [quickstart](https://goedelsoup.github.io/yidam/quickstart/) goes from
> no toolchain to a bootstrapped repository whose gate you have watched pass, fail, and pass
> again — by way of a worked corpus in [examples/streamflow/](examples/streamflow/). About
> twenty minutes. (Source: [docs/quickstart.md](docs/quickstart.md).)

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

If you already manage toolchains with [mise](https://mise.jdx.dev), it serves the same
release assets and keeps yidam in the same place as everything else:

```sh
mise use -g "github:goedelsoup/yidam[version_prefix=cli/v]@latest"
```

`version_prefix` is not decoration. This repository publishes four layers onto one release
list, so without it mise asks the repository-wide question and gets the editor's answer —
`@latest` resolves `editor/v*`, whose release ships only a `.vsix`. Drop `-g` to pin yidam
in one project's `mise.toml` instead.

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
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.8.0 --locked yidam
```

Either way, `yidam --version` should answer, naming the build and the features it carries.

`--features full` adds `index-build`, semantic `retrieve`, and the sqlite/rdf exports — and
with them protoc 31, an ONNX runtime, and a system C toolchain. That is the maintainer's
build, described under [Working on yidam](#working-on-yidam) below. It is **not** what
deriving a repository needs: `clone`, `overlay`, `tonpa` and both `serve` transports are all
in the light set, as is every report and gate. A default binary serves MCP; what it does not
serve is *semantic* retrieval, and it says so on every call.

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

### The editor surface

Two of them, and the split is deliberate. `yidam serve --lsp` is the language server —
diagnostics, definition, references, hover, and rename, computed by the same functions
`yidam lint` runs, for any LSP-capable editor. It is in the light default build, so the
binary you just installed already serves it; [yidam/editors/README.md](yidam/editors/README.md)
has the Neovim and Helix stanzas.

The other is a VS Code extension — five views over the corpus, lint and `graph-check`
verdicts as diagnostics, claim decoration, and the inherited mise tasks as editor tasks. It
**renders** verdicts and never computes them: `.yidam.toml` records which yidam governs a
corpus, so the extension resolves the binary that repository pins and bundles none of its
own. Install the CLI first or it has nothing to show.

**On VSCodium, Cursor, Windsurf, Gitpod or code-server**, which read [Open
VSX](https://open-vsx.org/extension/goedelsoup/yidam-vscode), search for *yidam* in the
extensions panel, or:

```sh
codium --install-extension goedelsoup.yidam-vscode
```

**On VS Code itself, download the `.vsix` from the [latest `editor/v*`
release](https://github.com/goedelsoup/yidam/releases) and install it by hand.** VS Code reads
the Microsoft Marketplace and nothing else, and this project does not publish there — the
publisher needs an Azure DevOps organisation that does not exist yet. That is stated rather
than papered over: a documented install line that cannot succeed is the failure
`install-channels.yml` was written to catch.

```sh
code --install-extension yidam-vscode-<version>.vsix
```

Or build it from a checkout, which is what a change to the extension needs anyway:

```sh
mise run ext-package -- dist/yidam-vscode.vsix   # packages, and checks what is in the package
code --install-extension yidam/editors/vscode/dist/yidam-vscode.vsix
```

`mise run ext-dev` instead opens an Extension Development Host against a staged fixture,
which is the loop for working *on* it. See
[yidam/editors/vscode/README.md](yidam/editors/vscode/README.md).

## Layout

| Path | Layer |
|---|---|
| [`yidam/prelude/`](yidam/prelude/) | The inherited model: scripture, identity, graph model, constitution, phases, conduct guidelines, skills, SDKs |
| [`yidam/cli/`](yidam/cli/) | The `yidam` binary — corpus analysis, linting, indexing, export, MCP server |
| [`yidam/tests/`](yidam/tests/) | Bootstrap test harness — scenarios scored against a judge [rubric](yidam/tests/rubric.md) |
| [`yidam/design/`](yidam/design/) | Design system and UI kits for the web surfaces |
| [`yidam/web/docs/`](yidam/web/docs/) | Astro/Starlight docs site, rendering `docs/` — published at [goedelsoup.github.io/yidam](https://goedelsoup.github.io/yidam/) |
| [`sadhana/`](sadhana/) | The scaffold copied into derived repos — directory shape, README stubs, root files, CI |
| [`samudaya/`](samudaya/) | Seed layer — axioms, hints, constraints, augmentations; consumed at genesis |
| [`packages/web/`](packages/web/) | Browser shell over an exported bundle — embeddings and vector search in WASM |
| [`docs/`](docs/README.md) | Documentation for yidam itself: design docs, RFCs, vocabulary — [read it as a site](https://goedelsoup.github.io/yidam/) |
| [`examples/`](examples/README.md) | Worked corpora for reading — not copied into a derived repository |
| [`BOOTSTRAP.md`](BOOTSTRAP.md) | The agent entry prompt a derived repo is bootstrapped from |
| [`mise.yidam.toml`](mise.yidam.toml) | The inherited task layer derived repos include |

Three layers meet in a derived repo and each has a different lifetime: `yidam/` is vendored
and re-vendored as the template evolves; `sadhana/` becomes the repo's own content at genesis
and diverges from there; `samudaya/` is consumed and deleted, surviving only in history.

## The CLI

```
# checks and gates — read-only, and exit nonzero on a problem
yidam doctor              is this setup sound? pin, PATH, prelude age, index, REGEN
yidam graph-check         orphans, broken links, missing labels — the gate CI runs
yidam lint --commits      corpus quality checks against a baseline ratchet, plus the commit vocabulary

# the practice — what is owed, which is not what is wrong
yidam due                 four clocks read together: index, catalog TTL, questions, phases

# README blocks — each rewrites its own <!-- REGEN --> block
yidam status              repo overview: nodes, open questions, catalog, index freshness, phases
yidam regen               refresh every REGEN block in one pass

# the corpus and its history
yidam graph               nodes, resolved edges, and the classes that license them
yidam query 'a -rel-> b'  a typed path over the resolved graph
yidam pack 'a -rel-> b'   that query's answer filled to a token budget, and what did not fit
yidam estimate '…'        what a query would cost before running it
yidam neighbors <node>    one node's neighbourhood — the traversal `serve --mcp` performs
yidam diff main..HEAD     node and edge changes between two refs
yidam check-diff a..b     types a code diff introduces that the ontology does not name
yidam log                 commit history classified as testimony or pipeline work
yidam replay              corpus health across the repository's whole history
yidam phases              active inquiry branches
yidam rename / migrate    rename a node, or change an ontology and every instance at once
yidam propose             draft findings as epistemic commits on a `propose/<head>` branch

# index, serving, export, and bundles
yidam embed               extract embedding text from corpus instances
yidam index-build         build the LanceDB vector index
yidam serve --mcp         serve the domain computer to MCP-capable agents over stdio
yidam serve --lsp         the language server — diagnostics, definition, references, rename
yidam bench               the committed goal set: anchored traversal against flat retrieval
yidam export --format …   bundle · web · rdf · graphml · sqlite · llms
yidam tonpa add …         manage bundle dependencies on other derived repos
```

That is a sample. `yidam --help` lists every command under these same groups, and marks with
`*` the ones that rewrite files in the repository they are run against — twenty-three do, and
that was previously visible only in each command's long help, where you had to already
suspect it to go looking.

Index subcommands (`corpus-index`, `skills-index`, `catalog-audit`, …) back the
`<!-- REGEN: yidam <subcommand> -->` markers embedded in README files. `mise run regen`
refreshes them all in one pass; in derived repos a stale REGEN block is a failing build.

`yidam due` is the other half of that pair, and the distinction is deliberate. `doctor` answers
*is this sound now* and is read under suspicion; `due` answers *is it time* and is read on a
cadence. It reads four clocks together — how stale the index is, whether a catalog source has
aged past its TTL, how long a question has gone unanswered, and how long a phase has been in
flight — and reports what is owed. **A corpus with three expired sources is not unhealthy, it
is owed**, so `due` exits zero however much it finds unless you pass `--strict`. Every interval
is declared by the corpus in `.yidam/config.toml`; a clock nobody has set reports what it
measured and never comes due.

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
| `index` | `index-build`, and upgrades `serve --mcp`'s `retrieve` from keyword to semantic | fastembed (ONNX) + LanceDB; needs protoc 31 at build time |
| `export-sqlite` | `export --format sqlite` | Bundled SQLite + sqlite-vec, compiled from C |
| `vault-s3` *(default)* | The `s3://` transport for `yidam vault` — the rest of the vault is ungated | hmac + reqwest (rustls) + tokio |
| `export-graph` *(default)* | `export --format rdf` | Pure Rust |
| `serve-http` *(default)* | `serve --mcp --http` — MCP over a URL, the transport every remote agent platform needs | hyper 1.x server features. **+1 package** (`httpdate`); hyper is already here for reqwest |
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
mise run docs-dev        # docs site on http://localhost:4321/yidam/
```

`yidam-build` here is `--features full`, deliberately: working on the CLI means being able to
run `index-build`, the semantic retrieval path, and the sqlite/rdf exports. That is why `mise install`
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
[Vocabulary](https://goedelsoup.github.io/yidam/vocabulary/) ([docs/vocabulary.md](docs/vocabulary.md)).

## Documentation

**[goedelsoup.github.io/yidam](https://goedelsoup.github.io/yidam/)** — read it there. The
site is built from [`docs/`](docs/README.md) on every push to `main`, and a page that no
sidebar entry names fails that build, so nothing in it is published-but-unreachable.

Start with [What yidam is](https://goedelsoup.github.io/yidam/what-yidam-is/), then
[Information architecture](https://goedelsoup.github.io/yidam/information-architecture/) and
[Bootstrap flow](https://goedelsoup.github.io/yidam/bootstrap-flow/). Designs under review
are in [RFCs](https://goedelsoup.github.io/yidam/rfcs/README/).

The markdown sources are [`docs/`](docs/README.md) — read those when you are working offline
or changing them, and `mise run docs-dev` serves the site locally from the same files.

## Status

Pre-1.0. The template, the bootstrap protocol, and the CLI version independently on separate
release trains — see [VERSIONING.md](VERSIONING.md) before bumping anything. Derived repos pin
what they inherited in their own `.yidam.toml`.

## License

[MIT](LICENSE).
