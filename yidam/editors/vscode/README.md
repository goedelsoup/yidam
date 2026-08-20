# yidam for VS Code

The editor surface for a yidam-derived knowledge repository — RFC-0016.

It lives here, beside `yidam/cli`, and not in a repository of its own. The extension and the
JSON report contract it consumes must version together; separating them re-creates, between
the extension and the CLI, exactly the drift the RFC set exists to close. The vendor step
copies only `yidam/prelude/`, so derived repositories carry none of this.

## The rule

> **TypeScript computes affordances. The CLI computes verdicts.**

An affordance is a navigation or authoring convenience whose failure mode is *not helping* —
go-to-definition on an edge, completion in a commit message. A verdict is a statement about
whether the corpus is sound. Verdicts cross the process boundary as JSON from the binary the
repository pins, and this extension renders them. It never derives them.

A TypeScript re-implementation of the checks is the failure this whole set of RFCs exists to
close. One downstream project already wrote ~1,600 lines of it, in Python, whose docstrings
claim faithfulness to Rust symbols it has since drifted from.

## Which binary

Resolved once at activation and re-checked when `.yidam.toml` or the setting changes:

1. the `yidam.path` setting
2. `yidam` on `PATH`
3. a mise shim resolved from the workspace
4. not found

**Not found is a first-class state**, not an error path: verdict features are disabled, the
status bar says so, and the single offered action runs `mise run yidam-build` in a terminal
where you can watch it.

The extension never bundles, downloads, or builds a binary. `.yidam.toml` records which
yidam commit governs a corpus, and only it gets to say — an editor that quietly installed a
different one would make its verdicts disagree with CI's.

## Diagnostics

`lint --format json` and `graph-check --format json`, on save, on ref change, and on demand
— debounced, cached per git OID plus a generation counter that a save bumps (the reports
read the working tree, not the commit).

**The severity mapping is the whole feature.** `yidam lint` does not ask *is the corpus
clean?* It asks *did this change make it less clean?* An extension that rendered every
finding as an Error would fill the Problems panel with inherited debt no commit caused, and
the panel would stop being read — the same failure the baseline exists to prevent, one layer
up.

| | |
|---|---|
| `error`, not baselined | **Error** — this is what fails CI |
| `warn` / `info`, not baselined | Warning / Information |
| any severity, **baselined** | Hint, faded, source `yidam (baseline)` |
| stale baseline entry | **not a diagnostic** — a repository condition with a Bless action |

The last row matters: a baseline entry that no longer occurs also fails the gate, but its
problem is that the file *no longer has* a problem. A squiggle would point at a line that is
now correct.

Every diagnostic carries its check id as `code` and its rationale as hover, so `--explain`
is available without a second command.

### Why lint owns the diagnostics

`graph-check`'s node checks are a subset of lint's, and it carries neither a baseline nor
spans. Measured: on a real 90-node corpus it finds nothing where lint finds 92; on the
reports fixture it reports the *same* broken edge as `dangling-edge` — but with no span, so
rendering both puts one accurate mark on the offending line and a redundant one at line 1.

So lint owns the marks and `graph-check` fills only the gap: any node it objects to that
lint did not mention, plus its own pass/fail as a repository condition. It is a gate CI runs
and its verdict is not dropped.

## The views

Five trees in one activity-bar container, all fed by `--format json`.

| View | Report | |
|---|---|---|
| **Corpus** | `corpus-index` + `open-questions` | classes, then instances. Open questions marked. |
| **Open questions** | `open-questions` | flat. Previously answerable only by reading a REGEN table in a README — which is to say, correct as of the last time somebody ran the generator. |
| **Phases** | `phases` | `ma/*` and `rigpa/*`, grouped. The branch model is the most distinctive thing about these repositories and was invisible in every editor. |
| **Health** | `lint` + `graph-check` + `index-status` | three gates and two acts — see below. |
| **Sangha** | `sangha` | electors, their positions, the settled record. Read-only. |

The Sangha view is hidden unless `sangha --format json` reports `collective: true`, which is
keyed on registered electors rather than on the directory: the template ships `sangha/` with
a placeholder table, so a directory test would show the view in every derived repository
from its genesis commit.

**Read-only is constitutional, not a scoping decision.** Article V confines synthesis to
resolution events, so a surface that wrote a position or drafted a resolution would be
performing one outside the protocol that routes them. RFCs 0009 / 0011 / 0012 have to settle
first.

### Three gates and two acts

`graph-check`, `lint` and `index-status` each answer a verdict, and the Health view renders
it. **REGEN freshness and vendored-prelude drift do not**: nothing reports whether a REGEN
block is stale without rewriting it, and drift against the pin is not knowable without the
network. So those two rows are offered as things to run, and they say so. A green tick on
either would be this extension asserting something no command answered.

A report that fails to arrive renders as *unavailable* — its own state, not a failure.
Folding it into red would show an X about the corpus because a subprocess died.

Blessing the baseline is offered as the Lint row's action only when the debt is **stale and
nothing is new**. One click that turns fresh violations into inherited debt would make
laundering a regression the easiest thing on the screen.

### Why two cached groups

Measured against a real 105-node corpus with 23 settled resolutions: seven of the eight
reports finish in under 200 ms, and `phases` takes **1.26 s** — it spawns three git
processes per ref, and a sangha has dozens. So what a *save* can change is one cache, and
what a *ref* can change is another. A sangha edit bumps the ref generation, because that is
the view it moves.

## The commit box

VS Code's commit input is a real text document with language id `scminput`, so language
features register against it.

- **Completion** on the verb, epistemic before operational, each carrying its **When** cell
  as detail text. Offered only while the cursor is before the first `: ` — past it you are
  writing prose, and thirty verbs there is noise.
- **A squiggle** for a verb outside the vocabulary, or one carrying a conventional-commits
  `(scope)` suffix.

Both rules already existed and were already checked — by `yidam lint --commits`, *after* the
commit. GRAPH.md says why moving them matters: the check is Warn severity and correctly so,
since history cannot be rewritten to fix a verb — *"that also means it reports drift only
after the drift is permanent."*

**`(scope)` earns the squiggle on its own.** Everything before the first `: ` is the verb,
so `vendor(yidam):` is read as the verb `vendor(yidam)`. That costs twice: it is outside the
vocabulary, **and** classification falls through to Epistemic, silently filing an operational
commit as a change in understanding. The bootstrap skill prescribed exactly that form, and
every derived repository's first three commits were reported by its own lint.

### Nothing here decides what is legal

The list and the verdict both come from `yidam vocabulary --format json`. Membership is
`is_recognized_verb` and the kind is `classify_commit` — the parity-certified pair, proven
total against the Dafny spec. A hardcoded list in TypeScript would be a second source of
truth for a rule the prelude owns and re-vendoring may change: `resolve`, `scope` and
`adopt` all arrived that way, and the vendored `GRAPH.md` is watched for exactly that.

The severity is transcribed too, read from the `unrecognized-verb` check rather than
chosen. A commit box that squiggled harder than the gate would be asserting a verdict
nobody agreed to — and this is precisely where escalating feels helpful.

### And the "kept in sync" comment became checkable

`git.rs`, `git.py` and `git.ts` each say their constants are *"kept in sync with the commit
vocabulary in `prelude/GRAPH.md`"*. Three comments, three languages, nothing verifying it.
`vocabulary` parses the tables to fetch their **When** prose, so comparing them costs
nothing — `drift` is a field on the report rather than a hope.

## Layout

| Path | |
|---|---|
| `src/binary.ts` | resolution. No `vscode` import. |
| `src/handshake.ts` | reading `format_version` and the `yidam` block. No `vscode` import. |
| `src/reports.ts` | typed views of the contract. Transcription only — nothing is derived here. |
| `src/diagnostics.ts` | the severity mapping. No `vscode` import. |
| `src/runner.ts` | spawn, per-OID cache, debounce. No `vscode` import. |
| `src/report-run.ts` | one pass over the reports, in two cached groups. No `vscode` import. |
| `src/tree/model.ts` | the five views, as data. No `vscode` import. |
| `src/vocabulary.ts` | what to offer, where to underline. No `vscode` import. |
| `src/tree/provider.ts` | `TreeNode` → `vscode.TreeItem`. The adapter, with no judgement in it. |
| `src/extension.ts` | activation, status bar, terminal actions. Thin by design. |
| `test/` | `node --test`, no Electron |

Every module that carries logic imports nothing from `vscode`, so they are exercised by
plain node. `test/contract.test.ts` drives them against the same `reports/` golden corpora
that certify the CLI's own output: a fixture whose output changes fails the parity run and
these tests together.

## Running the tests

```
mise run ci-vscode          # compile, lint, unit tests — what CI runs
npm run test:unit           # from this directory
```

Tests that need a binary **skip** when none resolves, or when the one that resolves predates
the contract — a stale `yidam` in `~/.cargo/bin` should not turn a contributor's suite red
over somebody else's work. CI sets `YIDAM_REQUIRE_CONTRACT=1`, which turns both skips into
failures, so skipping can never be how this job goes green.

## Not here yet

`@vscode/test-electron`. The `vscode`-importing code is the status bar, the diagnostic
collection, and one `TreeDataProvider` adapter with no judgement in it — every shape
decision is settled in `src/tree/model.ts` against plain data, and asserted there.
`test/manifest.test.ts` catches the class of defect that actually needs an editor to
surface — a `main` pointing at nothing, a contributed command nobody registers, a view id
nothing provides — in plain node.
