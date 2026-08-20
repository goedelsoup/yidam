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

## Layout

| Path | |
|---|---|
| `src/binary.ts` | resolution. No `vscode` import. |
| `src/handshake.ts` | reading `format_version` and the `yidam` block. No `vscode` import. |
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
