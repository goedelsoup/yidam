# Parity

Cross-language fixture suite for the parity functions all three SDKs must implement
identically. Adding a new function to the parity surface without a fixture in this directory
is a build error — `mise run parity` enforces it before running any SDK tests.

## Parity surface

| Function | Contract |
|---|---|
| `parse_instance` | Parse a corpus node — `.yidam/corpus/<class>/<name>.yml` — into a `CorpusInstance`, keeping every top-level key it does not name |
| `classify_commit` | Classify a commit message as `Epistemic` or `Operational` by its verb |
| `parse_markers` | Parse `REGEN` and `TEMPLATE` markers from file content, and report blocks that took lines which were not theirs |
| `update_regen` | Replace the content inside a named `REGEN` section, preserving the marker |
| `find_reachable` | Return all nodes reachable from a given node following directed edges (BFS); result sorted by code point |
| `find_citations` | Return all nodes that have a directed edge pointing to a given node; result sorted by code point |
| `is_recognized_verb` | Whether a leading commit verb is in the closed vocabulary |
| `compile_class_schema` | Compile a `.ont.yml` class definition into a JSON Schema for its instances |
| `parse_reference` | Parse RFC-0032's reference grammar — `yidam://<corpus>/<kind>/<path>[@<rev>][#<property>]`, the relative forms, and the legacy spellings — into a `Reference`. Total: `null` only for input naming no thing at all |
| `render_reference` | Render a `Reference` back to a string. The only place an identifier is built |
| `reference_conforms` | Whether every segment of a `Reference` is a slug, so it needs no escaping in any rendering — and whether its fragment names something, per kind |

The parity surface is versioned in [`VERSION`](VERSION). Any change to a function's
contract — input shape, output shape, or classification logic — requires bumping this
version and updating ALL THREE SDK implementations in the same PR.

**The table is the count.** This document used to say "the nine" in five places while the
table held ten, and `VERSIONING.md` used to say "the six". A number written beside a list is
a second copy of the list's length with nothing keeping it honest, so the numbers are gone
and the rows are what a reader counts.

**`references:` is a named field, not a key that survives `extra`.** A node's reference to a thing
that is not a node — an issue, a crate, a catalog entry — goes in `references:`, as a **string** in
the grammar rather than a struct: `parse_reference` is the reader, and a struct would be a second
encoding of the identifier the grammar already spells. `parse_instance/references-are-read-as-written.toml`
also pins that an unreadable entry is **kept**: a lint check is what names it, and a parser that
dropped it would hide it from the check.

It had to be named. The key already reached `extra`'s passthrough, so a corpus could write it today
and nothing would read it — which is how `[unentered]` happened one layer up, and the exit there was
structure beside the tag rather than a mark the tooling guessed at.

**Two kinds arrived by measurement, not by design (#783).** Across the five corpora that write
evidence-tag details, 496 references sit inside brackets where nothing can read them, and the two
largest shapes had no form in the grammar:

| Shape | Count | What it needed |
|---|---|---|
| `[verified — #362]` | 252 | a sixth `kind`, `issue` |
| `` [verified — `dispersion::ohio_panel::equalization_by_year`] `` | 32 | a fragment that can name a code item |

So `kind` is six, and `fragment_conforms` takes the kind: for a `crate` the fragment names a code
item or a file — `::`, `/`, uppercase and `.` all admitted, because a foreign toolchain names those
— and for everything else it stays §4.4's dotted property path.
`reference_conforms/a-code-fragment-on-a-node-does-not-conform.toml` is what fails if the widening
leaks past `crate`. §4.4's refusal is untouched: what it refuses is a fragment naming a **claim**,
which RFC-0008 measured to have no identity by surface form, and a Rust item has one a compiler
enforces.

**The reference grammar is one parser, and the round trip is why.** `parse_reference` and
`render_reference` are inverses, and `render_reference`'s fixtures are graded twice — once against
their expected text, and once by parsing that text back. The case that makes it a contract rather
than a nicety is a corpus with a class named after a kind: `skill/foo` read as a relative reference
is the *skill* `foo`, because the grammar puts a kind in that position, so a **node** in a class
called `skill` has to render `node/skill/foo`. Drop that rule in one language and only that
language's round trip fails, on only the corpora that name a class after a kind. See
`render_reference/a-class-shadowing-a-kind-names-node.toml`.

Everywhere else the shape is unambiguous without a rule, because a path's segment count decides:
`node/concept/foo` leaves a two-segment node path so `node` is the kind, and `node/foo` would leave
one so `node` is a class name.

**`reference_conforms` is reported, never required, and two of its fixtures exist to stop three
languages agreeing by coincidence.** The predicate is an explicit ASCII range in all three, not a
regex and not a library call, because `\w` matches `_` in both JavaScript and Python and
`str.islower()` is true of `é` in Python as `char::is_lowercase` is in Rust. A rule written the
convenient way in each language would pass every all-ASCII fixture and diverge on the first corpus
that was not. `an-underscore-does-not-conform.toml` and `non-ascii-lowercase-does-not-conform.toml`
are what fail when one of them is written the convenient way.

The reason conformance is reported rather than enforced here is measured: fourteen of the sixteen
corpora with tracked nodes conform, and two do not — `yidam lint --bless` is the supported way to
be one of the two (#777). A parser that refused non-conforming input would leave six of one
corpus's ten nodes unnameable and grow a fallback path in every consumer, which is the
per-surface improvisation the grammar exists to end.

**A YAML date is a string, and the three libraries do not agree about that.** The node
parser is the only parity function that reads arbitrary YAML, and scalar resolution is where
three languages quietly diverge. Given `occurred: 1886-07-04`, `serde_yaml` and the `yaml`
package both answer the string `"1886-07-04"`; PyYAML implements YAML 1.1 and answers
`datetime.date`, which `json.dumps` then refuses outright. 41% of the nodes in the measured
population carry a date, so this is not a corner. The contract is the string, the Python SDK
strips its timestamp resolver to get there, and `parse_instance/unquoted-dates-stay-text.toml`
is what fails when it is put back.

**Non-string scalar resolution inside `properties` and `extra` is out of contract**, and the
fixtures avoid it, for the same reason span fields are excluded. Given `010`, `serde_yaml`
answers the string `"010"`, the `yaml` package answers `10`, and PyYAML answers `8`; `no` is a
string to the first two and `false` to the third. That is YAML 1.1 against YAML 1.2 plus one
library's own reading, and no fixture makes three implementations of two specifications agree.
The ontology is what types these values — `<class>.ont.yml` declares them — and every consumer
reads them as text. What *is* in contract is every string value, the key sets of `properties`
and `extra`, and the difference between an absent key and a present empty one.

**Sorted means code-point order.** Rust sorts a `String` by UTF-8 bytes and Python by code
point, and those two agree everywhere. JavaScript's default comparator orders by UTF-16 code
unit, which puts an astral character *below* every BMP character from U+E000 up — so the
TypeScript SDK carries an explicit comparator, and `find_reachable/astral-and-bmp.toml` and
`find_citations/astral-and-bmp.toml` are what fail when it is removed. Every ASCII fixture
passes either way, which is the shape the divergence would have hidden in.

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

### `parse_markers` and malformed blocks

`parse_markers` has a second output, reached through `scan_markers` (`scanMarkers` in
TypeScript): the blocks whose extent the scan could not read the way they were meant. The
marker sequence itself is unchanged and stays the contract it was; this is the diagnostic
beside it (#524).

A fixture declares them as an array beside `[[expected]]`:

```toml
[[expected_malformed]]
command = "yidam status"
line = 3                       # 1-indexed, the open tag's line
fault = "ClosedOnAnothersTag"  # or CloseTagMissing, OpenArrowMissing
swallowed_lines = 6            # lines after the open tag this block took as its own
swallowed_markers = 1          # how many of those open a marker — markers now lost
```

**Absent means none, not "not checked".** All three runners read the field as an empty list
when it is missing and assert the scan reported nothing, so every fixture written before it
existed is now asserting its input is well-formed. A field that were merely optional would
leave four fixtures silently ungraded on the half that was just added.

`line` is why this is a contract and not three separate diagnostics. TypeScript reached it
with `split('\n')`, which leaves a trailing empty element on any text ending in a newline —
invisible while the only output was markers, and one extra swallowed line the moment one is
counted. `close-tag-missing.toml` is what fails when that is reintroduced.

## The MUST rule

**Every parity function must have at least one fixture.** There is no grace period.

When adding a new parity function:
1. Add the function to all three SDK implementations
2. Add at least one fixture to `fixtures/<function>/` in the same PR
3. Bump `VERSION` if the contract is new or changed

`compile_class_schema`'s fixtures compare **parsed JSON**, not text. Key order and
whitespace are not part of the contract — three languages will not agree on either, and a
fixture demanding they did would be pinning serializer behaviour while claiming to pin a
schema.

**And every fixture directory must have a runner that reads it.** The rule runs both ways.
A directory nobody runs looks exactly like one that is doing work — it passes every gate and
asserts nothing — so anything under `fixtures/` that is not a parity function has to be named
in `parity-check`'s exception list *and* given a section here saying who reads it. The check
enforces both halves; the section you are reading exists because the first run of it found
that `reports/`, the largest family here, was documented nowhere in this file.

`mise run parity` runs the fixture check before any SDK tests and exits non-zero on any of
the three failures. It is run by the `ci (parity)` job on every push and pull request —
which it was not until #145, so for as long as this paragraph claimed enforcement, the
enforcement was of a command nobody's CI ran.

**And the third question: does every SDK answer every function?** The two rules above are
about fixtures, and both passed for as long as `find_reachable` and `find_citations` existed
in the Rust SDK alone — the directories were there, and one runner read them. Two thirds of
what the first line of this file promises was missing with every gate green (#530). What asks
now is `yidam/cli/tests/parity_implementations.rs`, in two tests: every SDK *defines* every
function, and every SDK's runner *loads* every fixture directory. Both sides are discovered —
the functions out of `parity-check`'s own `functions` loop, the SDKs by walking
`prelude/sdks/*/tests/` — so neither can rot into naming one SDK and forgetting the others.
It reads the runners with their comments stripped, because a check that greps a whole file is
answered by the paragraph explaining what it looks for.

## Adding a fixture

1. Create `fixtures/<function>/<descriptive-name>.toml`
2. Fill in `function`, `description`, `[input]`, and `[expected]`
3. Run `mise run parity` — the new fixture is automatically picked up by all three runners

Fixture filenames have no semantic meaning beyond identification in failure output. Use
kebab-case names that describe what the case exercises, not what it expects.

## The MCP contract

`mcp/` is not part of the parity surface either. It freezes the **tool and resource
surface** three servers are meant to share — names, capability tiers, input schemas, and
call/response cases — in the shape RFC-0005 specifies. See [mcp/README.md](mcp/README.md).

It exists because the list was previously frozen in three places at once: a Rust E2E test, a
TypeScript README, and a third implementation's source. They shared **one tool name out of
five capabilities**, and nothing compared them, because comparing them was nobody's file.

Run by `yidam/cli/tests/mcp_serve.rs`, which reads `mcp/tools.json` rather than restating it.
A server that declares a capability must pass its cases; one that declares it absent has
those cases skipped, and the capability flag is checked against the served tool list so the
two cannot disagree.

## The reports fixtures

`fixtures/reports/` is not part of the parity surface either, and no SDK implements it. It
holds **RFC-0001's report goldens**: a small derived repository under `basic/repo/`, the
recipe that turns it into a git repository in `basic/stage.toml`, and the exact output of
every report in every format under `basic/expected/`.

It is the largest fixture family here and it went undocumented in this file until the
check below started asking who reads each directory — which is the finding that check exists
to produce.

Runners: `yidam/cli/tests/report_goldens.rs` for the goldens themselves; six test files in
`yidam/editors/vscode/test/` which drive the extension's reader against the same corpus; and
`yidam/editors/web/test/gate.mjs`, which reads the committed `expected/lint.json` directly. So
a fixture whose output changes fails the goldens and both editors together. The first two stage
the repository through `basic/stage.toml` rather than each building its own — see
`basic/README.md` for what the corpus is deliberately built to reach, and for why there were
once seven copies of that staging.

**The extension's runners need `YIDAM_BIN`, and skip silently without it.** They are the only
readers here that go through a real binary, so `npm run test:unit` in `yidam/editors/vscode`
reports a clean pass while checking nothing about a fixture change. Build the CLI and export
`YIDAM_BIN` before believing that suite about anything under `fixtures/reports/`.

## The diagnostic_severity fixtures

`fixtures/diagnostic_severity/` is not part of the parity surface above, and no SDK
implements it. It pins **RFC-0016's severity table**: how a lint finding's check severity and
its baseline membership together decide what an editor renders.

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

`fixtures/embed_config/` is not part of the parity surface above. It holds the **embedding
reproducibility contract**: the same sentence must embed to matching vectors across fastembed
(Rust), transformers.js (TypeScript), and sentence-transformers (Python),
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
