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

Designs under review are in [RFCs](https://goedelsoup.github.io/yidam/rfcs/README/). The
markdown sources are [`docs/`](docs/README.md) — read those when you are working offline or
changing them, and `mise run docs-dev` serves the site locally from the same files. A page no
sidebar entry names fails the build, so nothing on the site is published-but-unreachable.

The rest of this file is the repository's own map: how to install the CLI, where each layer
lives, and how to work on yidam itself. It does not restate the site — where a subject has a
page, this file says the one thing specific to the repository and links out.

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

Four more channels reach the same artifact — mise, `cargo binstall`, an `.mcpb` bundle for
Claude Desktop, and `cargo install` from source — and
[Installation](https://goedelsoup.github.io/yidam/installation/) has the line for each, the
version each one resolves, and the reason mise's declaration needs `version_prefix = "cli/v"`.
It is the page to read rather than this one: four layers publish onto a single release list, so
a resolver that asks the repository-wide question gets whichever layer tagged last.

`yidam --version` should then answer, naming the build and the features compiled into it.

Then create a derived repository, or overlay the infrastructure onto one that already exists.
Both copy the template, so both are run from a checkout of it — an installed binary carries the
CLI, not the template:

```sh
git clone https://github.com/goedelsoup/yidam && cd yidam
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
`yidam lint` runs, for any LSP-capable editor. It is in the light default build, so the binary
you just installed already serves it. The other is a VS Code extension: five views over the
corpus, verdicts as diagnostics, claim decoration, and the inherited mise tasks as editor
tasks. It **renders** verdicts and never computes them — `.yidam.toml` records which yidam
governs a corpus, so the extension resolves the binary that repository pins and bundles none
of its own, which is why the CLI is installed first.

[Editor setup](https://goedelsoup.github.io/yidam/editor-setup/) has the Neovim and Helix
stanzas, the install line for each marketplace that carries the extension, and why VS Code
proper needs the `.vsix` from a GitHub release. To work *on* either surface, see
[yidam/editors/README.md](yidam/editors/README.md).

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
yidam check-diff          types this branch introduces that the ontology does not name
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

That is a sample. `yidam --help` is a shorter one — the thirteen commands a session usually
needs — and `yidam --help-all` lists all fifty-eight under these same groups. Both mark with
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

The binary is partitioned by cargo feature so the common case stays cheap to install, and the
default set is what every released artifact carries. [Check which build you
have](https://goedelsoup.github.io/yidam/installation/#check-which-build-you-have) is the
table — which feature adds what, what each one costs to compile, and the two things about it
that are easy to get backwards. Reading an index is much cheaper than building one, and they
are separate features.

That table is the only copy. This file carried a second one until #947, and by then it had gone
out of step with it: `vector-read` had no row at all, and the capability that feature adds was
credited to `index` — so a reader who wanted semantic retrieval without protoc could not learn
from here that the build exists. `a_feature_table_marks_exactly_the_default_features` now holds
every declared feature to a row rather than only the default ones.

MSRV is Rust 1.88, the toolchain this repo pins; [VERSIONING.md](VERSIONING.md) records why
raising it is a minor bump at least.

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

## Status

Pre-1.0. The template, the bootstrap protocol, and the CLI version independently on separate
release trains — see [VERSIONING.md](VERSIONING.md) before bumping anything. Derived repos pin
what they inherited in their own `.yidam.toml`.

## License

[MIT](LICENSE).
