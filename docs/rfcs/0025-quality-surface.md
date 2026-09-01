# RFC-0025 — The instrument, turned around: measuring the repository that measures

- **Status:** Draft
- **Track:** I20
- **Relates to:** RFC-0001 (the report envelope this reuses rather than extends), RFC-0003 (the
  feature split that constrains what a coverage number may claim), RFC-0023 (the read/build
  split that created the middle build nobody's CI compiles), RFC-0024 (the guard-mirror
  precedent, and the mutation discipline this automates without replacing)
- **Versioning layers touched:** **none released.** This is repository infrastructure and a docs
  deployment. Layer 4 is touched only incidentally — `export-web`'s stylesheet is rebuilt from
  the token source with no change to its output format. No new subcommand, no `format_version`
  bump, no parity-surface change, no MCP contract change, no bootstrap-protocol change.
- **Parent epic:** #459 — this RFC specifies **#461** through **#468**, one per phase

## Summary

This repository builds an instrument for keeping a knowledge corpus honest: four reports, a lint
with a baseline, a golden-fixture parity surface, a judge rubric, and a bootstrap harness that
recomputes verdicts and fails on drift. Every one of those is pointed **outward**, at a derived
repository. Almost none of it is pointed at this tree.

The consequence is not that quality is low here — `mise run ci` passes 1,399 tests, and that is
only the light build. It is that **quality is unobservable**. There is no coverage data of any
kind, no machine-readable test result, no count of what was skipped, no baseline any benchmark
is compared against, and no record that any of those numbers ever had a different value. The
gates are good and they report to nobody.

This RFC specifies the instrumentation, its contract, and the surface that renders it — plus the
two things that surface depends on and does not have: a design system that can be imported
rather than retyped, and a documentation site that says which release it describes.

## Problem

### A check that is named, is documented, and cannot run

`mise.toml:272-278` declares formal verification over 658 lines of specification —
`graph.dfy`, `sangha.dfy`, `Core.lean` — last touched 2026-08-20. It has never executed. Three
faults, independent of one another:

```
$ lake build Yidam                       # from the repo root, as the task runs it
error: [root]: no configuration file with a supported extension:
  ././lakefile.lean
  ././lakefile.toml

$ cd yidam/prelude/sdks/spec && lake build Yidam
error: ././lakefile.lean:5:10: error: type mismatch
  "Yidam" has type String : Type but is expected to have type Name : Type
```

The task has no `dir`, so mise runs it from the repository root where there is no lakefile. The
lakefile does not compile even from the right directory. And nothing pins LEAN while `dafny` is
absent from the mise registry, so `mise install` provisions neither and the two `dafny verify`
lines have never run on any machine we can evidence.

No workflow references `dafny`, `lake`, or `lean`.

This is not a check that regressed. It is a check that has never executed, in a repository whose
own CI header names the failure mode exactly — *"a green check for a build nobody ships"*
(`ci.yml:148`).

### The gates report to nobody

**`$GITHUB_STEP_SUMMARY` appears zero times in 1,527 lines of workflow.** Every job is a colored
dot; diagnosing one means expanding raw log scrollback. `cargo test` emits no JUnit, so no
downstream consumer — a summary, an artifact, a page — can read a result at all.

The skips are the sharp edge. Four suites skip on environment and three tests are `#[ignore]`d:

| Where | Gate |
|---|---|
| `tests/embed_parity.rs:27` | `YIDAM_EMBED_PARITY=1` |
| `tests/vault_s3.rs:36`, plus three `#[ignore]` | `YIDAM_S3_TEST` and a live MinIO |
| `tests/query_history.rs:640,664` | `ssh-keygen`, and a git that can sign with it |

The discipline is right: each prints why it skipped. Nothing counts them. So *"how much of this
suite actually ran on this PR?"* has no answer, and an honest `eprintln!` scrolls past in a log
nobody opens. **A suite that silently stops running is indistinguishable from one that passes.**

And 54,833 lines of Rust across 112 files under `yidam/cli/src` alone have no coverage
measurement of any kind.

### One palette, three hand-copies, and a lint with no consumer

`yidam/design/tokens/*.css` is 482 lines across eight files. It has two consumers and neither
imports it:

| | Where | What |
|---|---|---|
| 1 | `yidam/design/tokens/*.css` | the source of truth |
| 2 | `yidam/web/docs/src/styles/custom.css:17-31` | hex values retyped into Starlight's variable slots, token name in a trailing comment |
| 3 | `yidam/cli/assets/web/main.css:1` | admits it in its own first line — *"token subset of yidam/design/"* |

`yidam/design/_adherence.oxlintrc.json` exists to catch precisely this — it forbids raw hex, raw
px, and off-system fonts. **Nothing invokes it**: not `mise.toml`, not any workflow, not any
`package.json`. A surface with no consumer, the pattern #194's audit named.

This is the same shape RFC-0024 is removing one family over. There, one question is answered
four times in two languages and the copies are held together by a containment assertion. Here,
one palette is transcribed three times and held together by nothing at all.

### Four versioned layers, one unversioned site

`VERSIONING.md` is emphatic that four layers release independently and that no layer may be
bumped as a side effect of another's. The docs site publishes **one build of `main`, always**. A
reader on `cli/v0.2.0` gets documentation for unreleased tooling with nothing on the page saying
so — including `cli-reference`, which `cli_reference.rs` keeps faithful to `main`'s binary and to
no other.

## Design

### The measurement contract is the existing envelope, reused as a type

The measurements P1 and P3 produce need one shape. The repository already has one — `report.rs:94-101`:

```rust
pub struct Envelope<T: Serialize> {
    pub format_version: &'static str,
    pub yidam: YidamBlock,   // version, commit, features
    /// Absolute path to the repository the report was computed over.
    pub root: String,
    #[serde(flatten)]
    pub report: T,
}
```

**A new envelope would be a fifth answer to a question this repository has already answered
four times.** So `quality-report.json` rides this one: same `format_version`, same `yidam` block,
and a consumer that already reads a yidam report reads this without learning a second shape.

**But `yidam` does not grow a subcommand for it.** The four reports are about a *corpus*; a test
result is about *this repository's build*, and a `yidam quality-report` would ship in every
derived repo's binary as a command that can never mean anything there. RFC-0018's precedent runs
the other way and does not apply: a query surface is a corpus surface.

That leaves a real tension, and it must be settled rather than absorbed. A CI-side generator that
*writes JSON matching* the envelope is a transcription, and this RFC is substantially about
transcriptions. **The resolution: the generator is a binary in the CLI workspace that serializes
the real `Envelope<T>`.** It is compiled against the type, not against a description of it, so
the contract is shared by construction and a field added to `YidamBlock` reaches the quality
report without anybody remembering. It is not on the `yidam` command surface, ships in no
release, and is not installed by any channel.

### Coverage may not claim what it did not compile

PR CI compiles the light default. `--features index` code is never built there — a deliberate
latency trade, argued at `ci.yml:211-214`. A coverage run under that build sees every gated file
at 0%.

Publishing that as one percentage would be **false in the specific way this repository cares
about**: it would say *untested* where the truth is *not compiled here*. `report.rs:41-43` already
states the principle, about the `features` list it carries:

> Cargo features compiled in. `reports` names the base and is always present; the rest gate whole
> subcommands, so a consumer can tell "this binary cannot do that" from "that failed".

The same field, on a coverage report, separates **unmeasured** from **untested**. It is carried,
and the pages render it as a distinct state rather than as a zero. The full number comes from
`ci-cli-full`, where the gated paths actually compile.

**And the gate is diff coverage, not a floor.** The number that changes what somebody does is
*lines this change added that no test executes*. A repo-wide percentage does not, and a threshold
invites the ratchet-by-one-line game rather than a test.

### One deployment, and the question that already broke `curl | sh`

The quality surface is a segment of the existing Pages deployment at `/yidam/quality/` — not a
second site. A second site is a second host, a second `base`, a second deploy, and **another call
site of "which tag?"**.

That question already has a history here. `install.sh`, `[yidam-build]`, the tap, and
`install-channels.yml` each ask it, and #397 is what it cost: `releases/latest` spans four layers,
so the tag pushed last answers for all of them, and the install line resolved to a release that
was not the CLI's.

P5's version switcher is the next place it will be asked. **It must filter on `cli/v*`
explicitly**, and #466's test asserts it by publishing a fixture tag from another layer and
confirming the version list ignores it. Naming the trap in the specification is cheaper than
finding it in an install script for the second time.

### Building docs per ref, and why the plugin does not fit

`starlight-versions` wants content under `src/content/docs/<version>/`. This site has none:
`docs-source.ts:11` sets `DOCS_BASE = '../../../docs'` and a custom loader reads `docs/` at the
repository root live, while `astro.config.mjs` holds a hand-written sidebar behind a
**bidirectional** completeness gate — an unlisted page fails the build, and so does an entry
naming a page that no longer exists.

Adopting the plugin means fighting the loader and the gate together, and the gate is worth
keeping: it is the reason seventeen RFCs stopped being rendered-but-unreachable.

**So build per ref.** Check out each tag, build with `base: /yidam/v<x>/`, and assemble every
build into one Pages artifact alongside `/yidam/` for `main`. The loader is untouched. The
sidebar gate keeps working *per version*, because each build is its own checkout reading its own
`docs/` — which is a stronger property than the plugin offers, and it falls out of the
arrangement rather than being added.

### What versions on which layer

**The site versions on Layer 4 (tooling), because that is what a reader has installed.**
`cli-reference`, `configuration`, `mcp-server`, and `artifact-vaults` describe the binary.

The cost is real and is stated rather than hidden: template-layer pages — `bootstrap-flow`,
`vocabulary`, `information-architecture` — are approximately-versioned as a consequence. A
reader on `cli/v0.2.0` sees the bootstrap flow as `main` describes it.

**Per-page layer pinning is deliberately deferred.** It is more correct and considerably more
machinery: every page would declare its layer, the build would resolve four tag streams instead
of one, and the switcher would have to explain to a reader why one page moved and another did
not. Nobody has asked for it. Recorded here so the next reader finds the decision rather than the
symptom.

Retention is the last **3** released `cli/v*` tags plus `main`. Build cost is linear in that
number and the docs workflow does not gate pull requests beyond its own build step.

## Phasing

| | Issue | What lands |
|---|---|---|
| P0 | #461 | `verify` runs, in CI, over specs that compile |
| P1 | #462 | nextest + JUnit, a job summary per gate, failure artifacts, the skip census |
| P2 | #463 | supply chain and release integrity — `cargo-deny`, `semver-checks`, dependabot, one toolchain |
| P3 | #464 | coverage, measured against the feature split; `[lints]` tables |
| P4 | #465 | the design system becomes importable, and its adherence lint runs |
| P5 | #466 | versioned docs |
| P6 | #467 | the quality report contract, and the pages that render it |
| P7 | #468 | trends and ratchets — the bench baseline, the series, mutation survivors |

P0 through P3 change no behavior and no published surface. P4 is a refactor with a lint attached.
**P5 is the only phase with a migration**: it changes URLs, moving `/yidam/<page>/` from `main` to
the latest release.

The dependency edges are few. P3 needs P1's summary to land a number in. P6 needs P1, P3, and P4.
P7 needs P6 to render, and is sequenced after #440 for the mutation work. P2 and P5 depend on
nothing and can run whenever there is capacity.

## Testing

**The recurring instruction across all eight issues is to mutate before trusting green**, because
every phase here adds a check whose failure mode is silence:

- A verification job that cannot see a false spec (P0).
- A summary generator that emits nothing and exits zero (P1).
- An advisories file of undated ignores — a check that has stopped checking, arrived at from a
  different direction than P0's (P2).
- A coverage integration that reports gated files at zero instead of as unmeasured (P3). **This is
  the assertion a naive integration will miss**, and it is the one that decides whether the number
  is honest.
- A lint invoked with no rules (P4).
- A mutation run configured to test nothing, reporting no survivors, looking like a clean bill of
  health (P7).

Two more that are specific rather than structural:

**The specs will probably not verify.** They have never been checked. P0 treats a spec that fails
as its finding rather than its blocker: fix it, or record what it asserts that is false and open a
follow-up. **Do not weaken an assertion to make the job green** — that converts this RFC's premise
into its own counterexample.

**One existing test breaks, and it is right to.** `docs_site.rs` parses `const BASE = '…'` out
of `astro.config.mjs` textually and asserts `README.md` advertises `SITE + BASE`. P5 makes
`BASE` env-driven, removing the literal it reads. It must not be deleted or made lenient: its
own comment names the failure it exists for — *"a working site and a dead link, and only the
second is visible in a diff"* — and versioning multiplies that risk rather than retiring it.
The question becomes plural, and the test asks it of every base the build emits.

**A fully-skipped suite must not render as a passing one.** P6's pages take a run with a failing
suite, an empty suite, and a fully-skipped suite as three distinct fixtures. The last is easy to
lose in a template, and losing it discards the entire argument of P1's skip census.

Beyond that, each phase golden-fixtures its output where output is a contract:
`quality-report.json` beside `tests/goldens/`, as every other report is goldened.

## What this does not touch

- **No new rule for a derived corpus.** The reports, the lint, and the rubric keep their scope.
  This is instrumentation of *this repository's own build*. Nothing here vendors into `sadhana/`,
  which carries no design tokens, no docs site, and no CSS.
- **No overlap with #436.** RFC-0024 is removing the guard-mirror in the disclosure family. P7's
  mutation work is sequenced **after** #440 and inherits its call sites rather than filing a
  second opinion on them. Where the two touch the same guard tests, RFC-0024 wins.
- **No coverage threshold that fails a build.** Diff coverage is reported and read. Whether it
  ever becomes a gate is a decision for after the first month of data.

- **No mutation gate.** `cargo-mutants` reports survivors into a job summary and fails on
  nothing. It is measured weekly and scoped by cost: #468 measured one mutant at roughly 80
  seconds — a 28s build and a 56s test run — so the 586 mutants in `cmd/lint/` alone would be
  thirteen hours. The scope is the modules where a survivor means something, not the codebase.
- **No CI latency budget increase beyond ~5 minutes on a pull request.** Today a PR gate is 1-3
  minutes. nextest is faster than `cargo test`; `cargo-deny` is seconds. `cargo-mutants` and
  full-repo coverage go on the weekly schedule `cli-full` already uses.
- **No `format_version` bump.** A new report under the existing envelope is additive, and
  `VERSIONING.md:241` reserves the bump for a removed field, a changed meaning, or a narrowed
  type.

## Open questions

1. ~~**Does the series belong in git?**~~ **Settled in #468: git, on an orphan branch.**

   Git, for the reason the question gave — it is the one store this repository already trusts,
   and a Pages-side artifact has no history older than the last deploy. But on a
   `quality-series` orphan branch rather than a file on `main`, which the question did not
   consider. A bot commit per push to `main` would land in `git log main` beside real work,
   would race a human push, and — `ci.yml` being `on: push: branches: [main]` — would
   re-trigger the run that wrote it. The orphan branch costs one ref and a `git fetch` in the
   docs build, and none of that.

   `yidam/tests/harness/ci-report/series-branch-README.md` is committed to that branch and
   carries the reasoning where somebody who lands on it will read it.
2. **Which surfaces are inside the adherence lint?** P4 discovers consumers by token reference,
   which is correct for CSS and JSX and says nothing about the VS Code extension, whose colors come
   from the editor's own theme API. Is a webview in scope? It renders in this repository's design
   language and cannot use its tokens directly.
3. **What happens to a version's docs when its tag is yanked?** P5 builds from released `cli/v*`
   tags. `publish-crates.yml:20-24` is clear that a yanked crate version is still downloadable by
   anything that already resolved it — so its documentation arguably must stay up. Nothing decides
   this, and it is the same shape as question 1: cheap now, expensive after the fact.
