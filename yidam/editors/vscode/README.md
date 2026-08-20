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

## Layout

| Path | |
|---|---|
| `src/binary.ts` | resolution. No `vscode` import. |
| `src/handshake.ts` | reading `format_version` and the `yidam` block. No `vscode` import. |
| `src/reports.ts` | typed views of the contract. Transcription only — nothing is derived here. |
| `src/diagnostics.ts` | the severity mapping. No `vscode` import. |
| `src/runner.ts` | spawn, per-OID cache, debounce. No `vscode` import. |
| `src/report-run.ts` | one pass: run both reports, map both, merge. No `vscode` import. |
| `src/extension.ts` | activation, status bar, terminal actions. Thin by design. |
| `test/` | `node --test`, no Electron |

The two modules that carry logic import nothing from `vscode`, so they are exercised by
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

`@vscode/test-electron`. The only `vscode`-importing code today is status-bar wiring, and an
Electron download that asserts a status-bar string is ceremony. `test/manifest.test.ts`
catches the class of defect that actually needs an editor to surface — a `main` pointing at
nothing, a contributed command nobody registers, an activation event that never fires — in
plain node. The Electron harness earns its cost with the first real UI behaviour, which is
diagnostics.
