# RFC-0001 — The report contract: reports as versioned rules with golden fixtures

- **Status:** Implemented
- **Track:** I1
- **Relates to:** RFC-0002 (node model the reports run on), RFC-0003 (a light binary to
  generate goldens from), RFC-0005 (the MCP `open_questions` tool shares this predicate)
- **Versioning layers touched:** SDK + parity (`prelude/sdks/parity/VERSION`)
- **Downstream reference case:** Project BOSC (`watermark-directory`)

## Summary

yidam's four corpus reports — `graph-check`, `lint`, `corpus-index`, `open-questions` — are the
instrument that keeps a derived knowledge graph honest, yet they exist only as Rust functions in
the `yidam` binary. No SDK exposes them, and their faithful re-implementation in another language
is asserted nowhere but a docstring. This RFC promotes the four reports from CLI behavior to a
**versioned contract with golden fixtures**: a `reports/` fixture family, each pairing a small
input corpus tree with the exact expected output of all four reports, run by `mise run parity`
against every re-implementer. A Python (or any) port then *proves* byte-equivalence instead of
claiming it — and the one divergence already live in BOSC (below) becomes a red test rather than
silent drift.

## Problem

**The reports are Rust-CLI-only.** All four render functions live in one crate, over the YAML
instance model (RFC-0002's model 2):

- `render_graph_check` (`yidam/cli/src/cmd/corpus.rs:57-169`) — per-instance integrity: missing
  `class:` (`corpus.rs:86`), unknown class *only if any `.ont.yml` exist* (`corpus.rs:87-91`),
  missing `label:` (`corpus.rs:94-96`), orphan / empty `links` (`corpus.rs:99-100`), link missing
  `target:` (`corpus.rs:105`), broken link — `dir.join(target)` filesystem-relative to the
  instance (`corpus.rs:107-110`), and a trailing "classes with schema but no instances"
  (`corpus.rs:160-166`). It is a **gate**: `graph_check()` bails nonzero when `issue_count > 0`
  (`corpus.rs:193-195`).
- `lint` (`yidam/cli/src/cmd/lint.rs:21-202`) — a superset carrying stable `kind` slugs on an
  `Issue{kind: &'static str}` (`lint.rs:9-14`): `missing-class`, `unknown-class`, `missing-label`,
  `no-description`, `orphan-out`, `broken-link`, `orphan-in`, `unused-catalog`
  (`lint.rs:76,86,97,106,117,128,149,176`). Default exits nonzero (`lint.rs:200`); `--warn` /
  `--suggest` exit 0 (`lint.rs:196-198`).
- `render_corpus_index` (`corpus.rs:11-35`) — a markdown table
  `| Instance | Class | Label | Links out | Lines |` (`corpus.rs:17`), the `Lines` column from
  `line_count` = `s.lines().count()` (`yidam/cli/src/walk.rs:76-80`).
- `render_open_questions` (`corpus.rs:37-54`) — a node is open iff `label.starts_with('?')` **or**
  `has_open_claim(&text)`, i.e. the raw text contains the literal `[open]` (`corpus.rs:44`;
  `has_open_claim`, `yidam/cli/src/cmd/mod.rs:48-50`).

None of this is reachable from `yidam-core`. The SDKs parse *Markdown* (`parse_node` et al.); the
reports walk `.yidam/corpus/<class>/<name>.yml` instances (`walk_corpus_instances`, `walk.rs:24-41`)
into `CorpusInstance` (`yidam/cli/src/parse.rs:20-32`) — a model no SDK can parse (RFC-0002). A
consumer that wants the reports has two options: shell out to the unpublished binary and its native
stack (RFC-0003), or re-implement.

**BOSC re-implemented, and the re-implementation already disagrees with the tool it cites.** The
`watermark.site.corpus_mirror` module reproduces all four renders, each docstring pointing at the
Rust symbol it claims to match — `render_open_questions` is "Faithful to
`yidam/cli/src/cmd/corpus.rs::render_open_questions`" (`corpus_mirror.py:686-693`), and likewise
`render_graph_check` / `render_lint` (`corpus_mirror.py:628,782`). But BOSC's projected nodes store
the open marker as a **structured** `claim_tag: open` field, never as literal `[open]` in the text
or `?` in the label. So its own predicate carries a third disjunct the Rust original does not:

```python
# src/watermark/site/corpus_mirror.py:700
if label.startswith("?") or "[open]" in text or data.get("claim_tag") == "open":
```

The consequence is precise and testable: run the **real** `yidam open-questions` over BOSC's own
mirror and it **under-reports** — `has_open_claim` (`mod.rs:49`) sees no `[open]` token, and unless
the label starts `?` the node is silently dropped. The replica and the tool it certifies against
already return different answers on the same corpus. Nothing detects this, because faithfulness is
docstring-only: no test invokes the reference behavior. The `cli_ref` pin BOSC records "so drift is
visible" (RFC-0004) is meaningless without a way to test conformance.

## Proposal

Add a **`reports/` fixture family** under `prelude/sdks/parity/fixtures/reports/`, a sibling of the
per-function fixtures, and make `mise run parity` diff every re-implementer's report output against
it — exactly as the eight function fixtures are diffed today (`prelude/sdks/parity/README.md:44-62`;
Rust runner `prelude/sdks/rust/tests/parity.rs:11-76`, Python `.../python/tests/parity/test_parity.py:9-42`).

Because a report takes a *corpus tree* rather than one string, each fixture is a **directory**, not
a single TOML:

```
prelude/sdks/parity/fixtures/reports/<case-name>/
  case.toml                       # description; which reports/lint-variants to run
  input/                          # the corpus the reports run against — repo root of the case
    .yidam/corpus/
      <class>.ont.yml             # optional class schemas (depth 1)
      <class>/<name>.yml          # instances (depth ≥ 2)   ── walk.rs:24-41
  expected/
    graph-check.txt               # golden text, paths relative to input/
    lint.txt
    corpus-index.md
    open-questions.md
    graph-check.issues.toml       # structured issues: [[issue]] node/kind/message
    lint.issues.toml              # structured issues with stable `kind` slugs
    exit.toml                     # exit-code contract (below)
```

`case.toml` mirrors the existing header (`function`/`description`, README:26-42) but names the
target family and the variants to exercise:

```toml
model = "yaml-instance"          # which node model the reports run on — see RFC-0002
description = "unknown-class fires only when a schema exists; orphan + broken link"
reports = ["graph-check", "lint", "corpus-index", "open-questions"]
lint_variants = ["default", "warn", "suggest"]
```

**The structured issue lists are the load-bearing contract.** `graph-check.issues.toml` and
`lint.issues.toml` freeze the *findings* independent of prose formatting:

```toml
# expected/lint.issues.toml
[[issue]]
node = ".yidam/corpus/reach/orphan.yml"
kind = "orphan-out"                       # the stable slug from lint.rs:117
message = "no outgoing links — isolated node"
```

A re-implementer must reproduce the same `(node, kind)` set with the same `kind` strings; the
`.txt`/`.md` goldens additionally pin the human rendering for consumers (like BOSC) that write the
reports into their repo. Report *text* is the byte-for-byte contract; the issue lists are what a
re-implementer diffs first, since they survive whitespace normalization.

**Exit codes are part of the contract.** The reports are not all pure renders — two are gates.
`exit.toml` freezes this:

```toml
graph_check   = 1     # nonzero when issue_count > 0   ── corpus.rs:193-195
lint          = 1     # default: nonzero on any issue  ── lint.rs:200
lint_warn     = 0     # --warn / --suggest report, never fail  ── lint.rs:196-198
corpus_index  = 0     # pure render
open_questions = 0    # pure render
```

**Extend the harness.** Add a `reports-check` mise task that asserts every `reports/<case>/` has an
`input/` and a complete `expected/`, and fold it into `mise run parity` (`mise.toml:139-147`)
alongside the existing `parity-check` (`mise.toml:125-137`). Each SDK's parity runner grows a
`reports` test: point the report at `<case>/input` as its repo root, capture stdout + exit code,
diff against `expected/` — the same load-and-assert loop already in `parity.rs:11-28` /
`test_parity.py:9-17`, extended to read a directory instead of a `.toml`.

**Make the reports a first-class part of the versioning contract.** Govern the `reports/` family by
the same joint `prelude/sdks/parity/VERSION` (`0.3.0` today) that already versions the eight
functions (`VERSIONING.md:53-64`). Adding the family bumps it once; thereafter the rule mirrors the
existing fixture discipline — **additions are safe, mutations are breaking.** Changing a report's
output shape (a new `kind`, a reordered column, a different open predicate) requires a fixture
change *and* a `VERSION` bump *and* a same-PR update to every SDK implementation, exactly as a
function-contract change does (`VERSIONING.md:56-61`). This is the precedent already set for the
`embed_config` fixtures, which live under `fixtures/` and are versioned with parity but run by a
dedicated task (`README.md:65-79`); `reports/` follows the same pattern.

## Dependency on RFC-0002

The report fixtures must declare **which node model they run on** — hence `model = "yaml-instance"`
in `case.toml`. Today that is forced: the reports read YAML instances (`parse.rs:20-32`), a model
no SDK parses, so the fixtures encode `.yidam/corpus/<class>/<name>.yml` trees directly. When
RFC-0002 unifies the two models, the same fixtures migrate to the unified representation (or dual-run
against both) by changing that one declared field, not the case bodies.

Crucially, the reports **canonicalize open-question detection on `?` / `[open]` in label/text**
(`corpus.rs:44`, `mod.rs:49`). Freezing that predicate in an `open-questions` fixture is *exactly*
the check that would have caught BOSC's `claim_tag`-only divergence: a fixture whose node carries an
open claim only as a structured field, with the golden asserting it is **not** reported, pins the
literal-`[open]` semantics — and BOSC's replica, run against it, fails until it either emits `[open]`
into node text or declares the divergence. The contract turns an invisible disagreement into a
diff.

## Migration & compatibility

**Rust is the reference implementation.** The golden `expected/*` files are *generated* by running
the real binary over each `input/` tree (a small `--emit-golden` / xtask mode). Rust never asserts
against a hand-written golden; it regenerates, and a mismatch means Rust's behavior changed and owes
a fixture update plus a `VERSION` bump. Every other implementer asserts.

**BOSC adopts by pointing its CI at the same fixtures.** A pytest loads each
`reports/<case>/input`, calls `corpus_mirror.render_graph_check` / `render_lint` /
`render_corpus_index` / `render_open_questions`, and diffs against `expected/` + `exit.toml`,
turning the three "Faithful to …" docstrings (`corpus_mirror.py:628,686,782`) into enforced tests.
Run today, the `open-questions` case fails on `corpus_mirror.py:700` — the divergence surfaces
immediately, which is the point.

**Existing derived repos are unaffected.** The change is purely additive: the Rust CLI's output does
not change, we only capture it. The `reports/` family and its `VERSION` bump introduce a new
obligation only for implementers claiming report parity.

## Alternatives considered

- **Shell out to the real binary in every consumer's CI.** Rejected. The binary is unpublished and
  drags the `fastembed` / `lancedb` / `protoc 31` native stack behind a `1.85+` toolchain
  (RFC-0003) — the very build friction that made BOSC re-implement rather than vendor. A consumer
  cannot reasonably install that toolchain to check `open-questions`. RFC-0003 (a light,
  reports-only binary) makes shelling-out viable *later*; even then, golden fixtures are what let a
  *native* re-implementation prove equivalence without the binary at all.
- **A shared WASM report module.** One compiled artifact, no re-implementation. But it re-imposes a
  build+runtime (wasm in Python), stays coupled to the heavy crate until RFC-0003 splits it, and
  denies a consumer an idiomatic native implementation it can extend (BOSC wants Python it owns).
  Complementary to the contract, not a substitute for it.
- **Property-based tests instead of golden fixtures.** Generate random corpora and assert invariants
  (graph-check clean ⇔ no issues). Valuable as a supplement, but it cannot pin the exact output
  shape — column order, issue ordering, `kind` strings, the `?`/`[open]` canonicalization — that a
  consumer must reproduce byte-for-byte. Goldens are the equivalence contract; property tests are a
  layer an implementer may add on its own.

## Open questions

- **Text normalization for cross-platform stability.** Report text embeds repo-relative paths
  (`corpus.rs:152`, `lint.rs:67-71`), `/`-joined on Unix and `\` on Windows; instance ordering is
  `sort()`ed by path (`walk.rs:18,39`) so stable, but lint issues interleave per-node in walk order
  (`lint.rs:65-154`). Proposal: normalize path separators to `/`, strip trailing whitespace with a
  single final `\n`, and treat `*.issues.toml` (ordered by `(node, kind)`) as the primary
  comparison, letting the `.txt`/`.md` goldens be the secondary human artifact. Needs a decision on
  whether issue *ordering* is contractual or set-valued.
- **Does `corpus-index`'s line-count belong in a portable contract?** `Lines` is
  `s.lines().count()` over the on-disk file (`walk.rs:76-80`) — an artifact of YAML formatting, not
  graph semantics; a re-implementer that *generates* its corpus (BOSC does) cannot match it without
  byte-identical YAML. Leaning toward marking it **advisory/excluded** from the portable diff,
  mirroring how byte spans are excluded from the function fixtures (`README.md:39-41`).
- **Which lint variants to freeze.** `default` / `--warn` / `--suggest` differ only in exit code and
  the extra `→ suggestion` line (`lint.rs:190-198`). Freeze all three, or freeze `default` plus a
  declared exit-code delta?
- **Sharing the open-questions golden with the MCP surface.** The `?`/`[open]` predicate is
  duplicated across `status.rs`, `serve/resources.rs`, and the MCP `open_questions` tool (per
  RFC-0005). The `open-questions` fixture could be the single source those four copies are diffed
  against, keeping the MCP contract (RFC-0005) and the report contract from drifting apart.
