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

`mise run parity` runs the fixture check before any SDK tests and exits non-zero if any
function directory is missing or empty. This is not advisory — it is enforced on every run.

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
