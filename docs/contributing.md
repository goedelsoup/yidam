# Contributing

yidam is a template and a toolchain, not an application. The thing being maintained is the
model a derived repository inherits, the CLI that keeps it honest, and the harness that proves
a bootstrap still works. That shapes what a good change looks like here.

## Get set up

```sh
git clone https://github.com/goedelsoup/yidam
cd yidam
mise install          # provisions rust, protoc, python, uv, node
mise run yidam-build  # installs the full-feature binary into .local/bin
mise tasks            # everything available
```

[mise](https://mise.jdx.dev) manages both toolchains and tasks. Rust is pinned in `mise.toml`
and `rust-toolchain.toml`, which are kept in sync.

`yidam-build` is `--features full` deliberately: working on the CLI means being able to run
`index-build`, the semantic retrieval path, and the sqlite/rdf exports. That is why `mise
install` provisions protoc and the rest, and why it is the *maintainer's* setup rather than the
one [Installation](installation.md) describes.

It installs to `.local/bin`, which `mise.toml` puts first on `PATH`, so every task below runs
the binary built from this tree. That is per-repository on purpose — a per-machine install
location meant building here clobbered every derived repo's pinned binary, and building in one
of those clobbered this.

## Before you push

```sh
mise run ci
```

`fmt --check`, `clippy -D warnings`, and tests across both workspaces — the harness and the CLI.
**Check it by exit code**, not by reading the output: a task that fails mid-pipeline still
prints a lot of green.

Two things `mise run ci` does *not* cover, and both have bitten:

```sh
mise run docs-build   # the docs site — an unlisted page fails the build
mise run ci-vscode    # the extension
```

CI runs the harness and the light CLI build as parallel jobs on every PR, plus the extension,
the parity suite, an aarch64 cross-compile and the scaling bench. **The full-feature build runs
on `main` and weekly, not on PRs** — so a change behind `--features index` is not compiled by PR
CI. Verify those locally, and prefer taking gated facts as arguments so the logic around them
stays testable from the light build.

## What a change needs

**Every behaviour change needs a test.** Prefer testing library functions directly; use the
integration tests only for end-to-end CLI behaviour.

**Prefer discovering a set over listing one.** A hardcoded file list in a guard test stops
covering new files without ever going red. Where a test asserts something about "all the X",
find the X's rather than naming them.

**Break a new guard on purpose before trusting it green.** A file-scanning test that looks at
nothing passes.

**Run what you cite.** An RFC or a doc page that quotes code accurately can still contradict it;
if a page contains an example, run the example.

## Commit messages

This repository is bound by the same commit vocabulary a derived corpus is — the distinction
between an **epistemic** commit (what is known changed; the message is testimony) and an
**operational** one (the pipeline advanced) is enforced by `yidam lint --commits`.

```sh
yidam vocabulary                      # the closed list, with reasoning
yidam vocabulary --check "fix(cli): …" # check a subject before the commit exists
yidam lint --commits --range main..HEAD
```

The subject line convention here is a conventional-commit prefix followed by a clause naming the
actual defect or change, not a summary of the diff. `git log` is the best guide.

## Repository layout

| Path | What it is |
|---|---|
| `yidam/prelude/` | The inherited model — the only directory a derived repo vendors |
| `yidam/cli/` | The `yidam` binary |
| `yidam/editors/` | LSP notes and the VS Code extension |
| `yidam/tests/` | Bootstrap harness, scenarios, rubric, judge |
| `yidam/web/docs/` | This site |
| `sadhana/` | The scaffold copied into derived repos |
| `samudaya/` | The seed layer, consumed at genesis |
| `docs/` | Documentation for yidam itself — the source of this site |
| `examples/` | Worked corpora, gated by CI |

Three layers meet in a derived repo with three different lifetimes: `yidam/` is vendored and
re-vendored, `sadhana/` becomes the repo's own content at genesis, `samudaya/` is consumed and
deleted. Changing one is not the same kind of act as changing another.

## Where a change goes

**Changing the model** — the node model, the graph invariants, conduct norms — is a change to
`yidam/prelude/`, and every derived repository inherits it at its next re-vendor. That is the
highest-consequence directory in the repository.

**Changing behaviour** belongs in `yidam/cli/`, and the SDK boundary matters: the CLI consumes
the Rust prelude implementation as `yidam-core` rather than re-implementing the
parse-and-classify surface, so drift between the tool and the model becomes a test failure
instead of a mystery.

**A default is a claim about every corpus at once.** The recurring lesson here is that a number
compiled into the binary is one repository's judgement arriving as a build failure in another
that never agreed to it. Where a threshold is genuinely a corpus's own business, it belongs in
[`.yidam/config.toml`](configuration.md) with the argument for it, absent-means-off.

## The SDKs and parity

`yidam/prelude/sdks/` implements the prelude model three times — Rust (the reference),
TypeScript, and Python — held to the same TOML fixtures.

```sh
mise run parity          # cross-language parity for the SDK functions
mise run domain-parity   # the domain calculators in prelude/domains/
mise run embed-parity    # embedding reproducibility across runtimes
mise run verify          # Dafny specs and Lean 4 proofs (dafny + lake installed separately)
```

A change to one implementation that is not made in the others is a parity failure, which is the
point.

## The editor extension

```sh
mise run ext-dev       # compile, stage a fixture, open an Extension Development Host
mise run ext-fixture   # rebuild the staged fixture
mise run ext-package -- dist/yidam-vscode.vsix
```

**F5** from the repository root runs the same two steps as its `preLaunchTask`. The workspace it
opens is a staged copy of the reports fixture — the extension activates on `.yidam.toml` or
`.yidam/**`, and this repository is not a derived repository, so launching against the repo root
would activate nothing.

Tests need no editor:

```sh
cd yidam/editors/vscode && npm run test:unit
```

Hold the line from [RFC-0016](rfcs/0016-editor-surface.md): **TypeScript computes affordances,
the CLI computes verdicts.** A TypeScript re-implementation of a check is the drift the whole
RFC set exists to close.

## Proposing a design

Substantial changes get an RFC in [`docs/rfcs/`](rfcs/README.md) before they become behaviour —
a proposal held open for review, written in the same register as the rest of `docs/`: precise,
evidenced, and honest about what is not yet settled.

The set has a through-line worth reading before adding to it: yidam's parity surface used to
certify that three SDKs parse *Markdown* identically while the products a consumer actually
depends on drifted freely. The RFCs move the contract boundary to where integration happens.

## Documentation

This site is built from `docs/` at the repository root by an Astro/Starlight project in
`yidam/web/docs/`.

```sh
mise run docs-dev     # http://localhost:4321/yidam/
mise run docs-build   # what CI runs
```

**A new page needs a sidebar entry in `yidam/web/docs/astro.config.mjs` or the build fails.**
That is deliberate and runs in both directions: Starlight would otherwise render a page nobody
can reach, and a sidebar entry naming a page that no longer exists is a dead link. The check
names the file and tells you which side is wrong.

`docs/README.md` is deliberately not published — the sidebar is the site's contents table, and
publishing the file would ship a second copy that goes stale the first time the two disagree.

## Reporting a problem

[Issues](https://github.com/goedelsoup/yidam/issues). For anything about a specific repository's
behaviour, `yidam doctor` and `yidam --version` between them answer most of the first round of
questions — see [Troubleshooting](troubleshooting.md).
