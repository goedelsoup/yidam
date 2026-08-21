# Parity

Cross-language fixture suite for the nine parity functions all three SDKs must implement
identically. Adding a new function to the parity surface without a fixture in this directory
is a build error — `mise run parity` enforces it before running any SDK tests.

## Parity surface

| Function | Contract |
|---|---|
| `parse_node` | Parse a markdown corpus file into a `CorpusNode` |
| `extract_claims` | Extract `Claim[]` from markdown content, inferring `EvidenceTag` from inline markers |
| `extract_links` | Extract `Link[]` from markdown content; exclude images and bare URLs |
| `classify_commit` | Classify a commit message as `Epistemic` or `Operational` by its verb |
| `parse_markers` | Parse `REGEN` and `TEMPLATE` markers from file content |
| `update_regen` | Replace the content inside a named `REGEN` section, preserving the marker |
| `find_reachable` | Return all nodes reachable from a given node following directed edges (BFS); result sorted |
| `find_citations` | Return all nodes that have a directed edge pointing to a given node; result sorted |

The parity surface is versioned in [`VERSION`](VERSION). Any change to a function's
contract — input shape, output shape, or classification logic — requires bumping this
version and updating ALL THREE SDK implementations in the same PR.

## Fixture format

Each fixture is a TOML file under `fixtures/<function>/`:

```toml
function = "<function name>"
description = "<what this case exercises>"

[input]
# function-specific input fields

[expected]
# expected output (or [[expected]] for array outputs)
```

Span fields (`span`, byte offsets) are intentionally excluded from fixtures. Byte boundary
behavior varies across parsers and is not part of the cross-language contract; text and
tag values are.

## The MUST rule

**Every parity function must have at least one fixture.** There is no grace period.

When adding a new parity function:
1. Add the function to all three SDK implementations
2. Add at least one fixture to `fixtures/<function>/` in the same PR
3. Bump `VERSION` if the contract is new or changed

**And every fixture directory must have a runner that reads it.** The rule runs both ways.
A directory nobody runs looks exactly like one that is doing work — it passes every gate and
asserts nothing — so anything under `fixtures/` that is not one of the nine has to be named
in `parity-check`'s exception list *and* given a section here saying who reads it. The check
enforces both halves; the section you are reading exists because the first run of it found
that `reports/`, the largest family here, was documented nowhere in this file.

`mise run parity` runs the fixture check before any SDK tests and exits non-zero on any of
the three failures. It is run by the `ci (parity)` job on every push and pull request —
which it was not until #145, so for as long as this paragraph claimed enforcement, the
enforcement was of a command nobody's CI ran.

## Adding a fixture

1. Create `fixtures/<function>/<descriptive-name>.toml`
2. Fill in `function`, `description`, `[input]`, and `[expected]`
3. Run `mise run parity` — the new fixture is automatically picked up by all three runners

Fixture filenames have no semantic meaning beyond identification in failure output. Use
kebab-case names that describe what the case exercises, not what it expects.

## The MCP contract

`mcp/` is not part of the nine-function parity surface either. It freezes the **tool and
resource surface** three servers are meant to share — names, capability tiers, input schemas,
and call/response cases — in the shape RFC-0005 specifies. See [mcp/README.md](mcp/README.md).

It exists because the list was previously frozen in three places at once: a Rust E2E test, a
TypeScript README, and a third implementation's source. They shared **one tool name out of
five capabilities**, and nothing compared them, because comparing them was nobody's file.

Run by `yidam/cli/tests/mcp_serve.rs`, which reads `mcp/tools.json` rather than restating it.
A server that declares a capability must pass its cases; one that declares it absent has
those cases skipped, and the capability flag is checked against the served tool list so the
two cannot disagree.

## The reports fixtures

`fixtures/reports/` is not part of the nine-function parity surface either, and no SDK
implements it. It holds **RFC-0001's report goldens**: a small derived repository under
`basic/repo/`, the recipe that turns it into a git repository in `basic/stage.toml`, and the
exact output of every report in every format under `basic/expected/`.

It is the largest fixture family here and it went undocumented in this file until the
check below started asking who reads each directory — which is the finding that check exists
to produce.

Runners: `yidam/cli/tests/report_goldens.rs` for the goldens themselves, and six test files
in `yidam/editors/vscode/test/` which drive the extension's reader against the same corpus,
so a fixture whose output changes fails the goldens and the extension together. Both stage
the repository through `basic/stage.toml` rather than each building its own — see
`basic/README.md` for what the corpus is deliberately built to reach, and for why there were
once seven copies of that staging.

## The diagnostic_severity fixtures

`fixtures/diagnostic_severity/` is not part of the nine-function parity surface above, and no
SDK implements it. It pins **RFC-0016's severity table**: how a lint finding's check severity
and its baseline membership together decide what an editor renders.

It exists because that table is the one verdict RFC-0016 licenses a client to recompute. The
rule everywhere else is that the CLI computes verdicts and a client computes affordances; this
row is the exception, and the reason is stated in both implementations — the alternative is an
editor that cannot render a diagnostic without a subprocess per keystroke.

So it lives in two languages:

| | |
|---|---|
| `severity_of(severity, in_baseline) -> u8` | `yidam/cli/src/cmd/lsp.rs` |
| `levelFor(severity, inBaseline) -> Level` | `yidam/editors/vscode/src/diagnostics.ts` |

Each was pinned by a test. Neither was pinned to the other, so the two were free to be
independently right about different tables — the same shape that put one tool name across five
capabilities in `mcp/`, and that put four copies of the open-question predicate in the CLI
before one of them was found under-reporting a consumer's corpus 26 to 2.

**The fixtures carry a level name, not a number.** Neither side's numbering is shared: LSP
counts from 1 and `vscode.DiagnosticSeverity` counts from 0, so a fixture holding either would
make one of the two transcriptions assert a translation it does not perform. The four names —
`error`, `warning`, `information`, `hint` — are what both already agree on, and each side maps
them at its own boundary.

Two of the six cases (`warn-baselined`, `info-baselined`) are states the CLI never emits: the
baseline records error severity and nothing else. They are pinned because both implementations
answer them anyway, and an input neither will see is exactly where two transcriptions drift
unobserved.

Runners: the `the_severity_table_is_the_shared_fixture` test in `yidam/cli/src/cmd/lsp.rs`, and
`the severity table is the shared fixture` in
`yidam/editors/vscode/test/diagnostics.test.ts`. Both read these files rather than restating
the table; `mise run parity` does not run them, because neither is an SDK.

## The embed_config fixtures

`fixtures/embed_config/` is not part of the nine-function parity surface above. It holds
the **embedding reproducibility contract**: the same sentence must embed to matching vectors
across fastembed (Rust), transformers.js (TypeScript), and sentence-transformers (Python),
so that every consumer of `embed.config.json` retrieves against the same vector space.

These fixtures are run by `mise run embed-parity`, not the default `parity` task — the
runners download model weights on first run and are gated behind `YIDAM_EMBED_PARITY=1`.
Runners: `yidam/cli/tests/embed_parity.rs` (Rust reference — fill `expected.prefix` from
its output), `typescript/tests/embed_parity.test.ts`, `python/tests/parity/test_embed_config.py`.

A runtime that cannot load the exact weights in `input.model_file` declares its measured
drift in a `[known_delta.<runtime>]` section with its own tolerance, rather than silently
widening the shared one.

**These numbers now travel.** `yidam index-build` writes the same probe, prefix, tolerance
and known deltas into every index's `embed.config.json`, and `yidam index-verify --provider
<cmd>` checks a consumer against them. A test asserts the shipped constants and this fixture
have not come apart — a witness proven here and a different witness shipped there would leave
both proving nothing.

The reason it matters is that a fixture never leaves CI. A consumer holding an index
directory and its own embedder had no probe to check itself against, so loading fp32 weights
where the index is quantized produced plausible cosine scores that were quietly wrong.
