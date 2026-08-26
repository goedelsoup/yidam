# RFC-0021 — Code that names what the ontology has not (`yidam check-diff`)

- **Status:** Draft
- **Track:** I16
- **Relates to:** RFC-0020 (the proposal surface a finding here becomes), RFC-0018 (the typed
  query this matches against), RFC-0001 (the report contract it emits on), RFC-0003 (the light
  binary it must run in), RFC-0016 (the severity table an editor renders it through)
- **Versioning layers touched:** tooling (`yidam` CLI) — **no parity-surface change, no
  template change, no MCP contract change**; see [What this does not
  touch](#what-this-does-not-touch)
- **Downstream reference case:** Project BOSC (watermark-directory)
- **Parent epic:** #23, restructured — this RFC specifies **#342** (Phase A). The semantic half
  is **#343** and is deliberately not here.

## Summary

Application code in a derived repository is the implementation of the ontology, and nothing
checks that the two still agree. #23 has said so since 2026-07-01 and stalled, because the
half of it that needs no new decision was filed together with the half that needs three.

This specifies the first half: a deterministic check over a **diff** of `crates/`, matching
type names against what `.ont.yml` declares, reporting a concept the diff introduces that no
class models. No model call, no vector index, no parity-surface change.

Three things measured before writing this, each of which changed the design:

1. **The subject is real and constant.** 32% of one derived repository's history and 49% of
   another's touches `crates/`.
2. **The vocabulary gap is enormous.** One repository declares **15 classes** and its code
   defines **275 types**, of which **20 match a declared name — 7%**. The others measure 4%
   and 2%. This is not a scattering of drift; the code models a far richer domain than the
   ontology does.
3. **Which is exactly why the check must be diff-scoped, and why #23's own name is right.**
   Run over the corpus, this reports 255, 46 and 202 findings — unusable, and a textbook
   permanently non-empty report. Run over a diff, the median commit reports **zero**.

## Problem

### Nothing reads the ontology from the code side

E1 made `.ont.yml` a contract that checks enforce: instance validation, property types, edge
licensing, target classes. Every one of those reads the *corpus*. A connector that starts
returning a new field, a calculator that introduces a domain type, a struct that models a
concept nobody wrote a class for — all invisible.

`yidam diff` exists and diffs the **corpus** between two refs ([`diff.rs`](../../yidam/cli/src/cmd/diff.rs)).
There is no equivalent for the code that implements it.

### Measured, on three instrumented repositories

A, B and C as lettered in `docs/post-genesis-measurement.md`.

| | A | B | C |
|---|---|---|---|
| `crates/` source files | 326 | 27 | 169 |
| commits touching `crates/` | 249 of 780 (**32%**) | 38 of 250 (15%) | 215 of 438 (**49%**) |
| classes declared | 15 | 12 | 18 |
| ontology vocabulary (classes + properties + relationships) | 150 | 83 | 189 |
| type definitions in code | 275 | 48 | 207 |
| types matching a declared name | 20 (**7%**) | 2 (4%) | 5 (**2%**) |

None of the three has a `packages/` directory.

### The unmatched types are domain concepts, not infrastructure

This is the finding that reshaped the design, and the first assumption it killed was mine.
A's unmatched types are overwhelmingly domain nouns:

```text
Absence  Activity  Adjudication  AgendaItem  Amendment  AppointingAuthority
Assembly  Assignment  Attribution  Bill  BillVotes  Candidate  Canvass
Caucus  CaucusStanding  Chamber …
```

Infrastructure is the minority — `Args`, `Anchor`, `Cell`, `Check`, `Channel`. So the 7% is
not "name matching is too naive to see through boilerplate". It is **the corpus modelling
fifteen classes while its code models two hundred concepts**, which is precisely the `GAP`
#23 was written about, at a scale its severity table does not anticipate.

### Which is why it is a diff check, and not a corpus check

The obvious implementation — walk `crates/`, report every unmodelled concept — produces 255
findings in A. `example_corpus.rs` states the rule that forbids this: *"a permanently
non-empty report is where a real finding gets lost."*

Scoped to a diff, the same signal is tractable. Sampling the most recent 60 commits touching
`crates/` in each repository, counting type definitions the diff **adds**:

| | A | B | C |
|---|---|---|---|
| commits introducing ≥1 new type | 28% | 13% | 25% |
| median new types per commit | **0** | **0** | **0** |
| p90 | 5 | 2 | 4 |
| max | 10 | 35 | 42 |

**The median commit reports nothing.** When it fires, it fires with a handful. That is a
report someone reads.

It also dissolves the hardest open question. "Is `Cell` a domain concept or infrastructure?"
does not need a general answer, because a diff-scoped check asks about each type **once, at
the commit that introduces it**, with the author present. A corpus-scoped check would ask
about `Cell` on every run forever.

## Design

### One finding

**`unmodelled-concept`** — this diff introduces a type whose name matches no class, property
or relationship the ontology declares.

That is the whole of Phase A. `CONFLICT` (code contradicting a claim) requires reading
semantics, which is #343. `ALIGNED` is not a finding at all — see below.

### Extraction is line-based, and stays that way

Type and enum declarations, by regex over the added lines of the diff. No Rust parser: the
light `reports` build carries `regex`, `walkdir`, `serde` and `pulldown-cmark` and no native
libraries, and adding `syn` to read names out of a diff would buy precision this does not
need at a cost `--features index` already shows the shape of.

It matches the idiom the repository already uses for exactly this kind of job —
`rename.rs`'s `target_on` reads a link target out of a line rather than parsing YAML, for the
same reason.

**Rust only.** No derived repository has a `packages/` directory, and
`prelude/guidelines/directories.md` already recorded why before this RFC measured it:

> across the two repositories derived from this template, `agents/` and `packages/` never
> received a single file

It is three now. A three-language extractor is two implementations with no consumer.

**Not a parity function**, and #23's plan to make it one is the expensive half of that
mistake. Adding to the parity surface costs a `parity/VERSION` bump, three implementations,
fixtures, and a matching major bump of all three SDK packages — for a function with one
caller.

### Matching is by name, and says so

A type matches when its name, kebab-cased, equals a declared class, property or relationship
name. `AppointingAuthority` → `appointing-authority`.

This is deliberately shallow, and the report must not imply otherwise. A type that matches
nothing may be a genuine gap in the ontology, or a helper the ontology has no reason to know
about. The check cannot tell, does not claim to, and phrases the finding accordingly — the
register `citations::moved` established, whose `Movement::question` carries
`/// Phrased as a question, deliberately. The answer is a person's.`

### Severity: Warn, and never anything else in Phase A

A concept the ontology has not modelled is a question about the corpus, not a defect in the
build. The fix is either a new class or a decision not to have one, and both are an author's
judgement. Gating would also make adoption a build break in every repository that predates
it — the ratchet failure `docs/post-genesis-measurement.md` recorded.

`CONFLICT` is given "blocking" by #23. That decision belongs to #343 and this RFC does not
take it, but the measurement above is relevant to it: a check that fires on a quarter of
code-touching commits is not one to hand a blocking verdict lightly.

### `ALIGNED` is a count

#23 makes it an informational finding. One per correctly implemented concept is a
permanently non-empty report by construction. It is a number in the summary line — *"12 of 17
types matched a declared concept"* — which is the same thing a reader wanted and none of the
cost.

### Exclusions come from `authorship.rs`, and nowhere else

Tests, fixtures and vendored code are excluded through the mechanism that already exists:
declared regions of kind `generated` (requires `by:`), `imported` (requires `from:`) or
`excluded` (requires `why:`), reported at info severity for the first two and silent only for
the third, which is named so a reviewer sees the escape hatch as one.

#23 lists "scope of application code — test and fixture exclusions" as an open question
needing a decision. It does not: the decision was made, argued and built, after that issue
was written. Do not invent a second vocabulary.

### Under E4, a finding becomes a question

`unmodelled-concept` is a finding about the corpus — something the code knows that the graph
does not — so `yidam propose` drafts an `open:` for it. No new mechanism: RFC-0020 governs
the surface, and #271 already established that a Warn check can be proposal-eligible when
something licenses it.

What licenses this one is the finding itself, which is RFC-0020's ordinary case.

## What this does not touch

- **The parity surface.** No SDK gains a function; see above.
- **The template.** No new directory, no new frontmatter field, no prelude rule. A derived
  repository gets this by re-vendoring the binary.
- **The MCP contract.** No tool is added.
- **The network, and any model.** Phase A is deterministic. Whether the tool may ever call a
  model is #343's first question, and it is unanswered.
- **`--features index`.** Nothing here needs the vector index, which is what keeps it testable
  in the CI that would run it — PR CI never compiles that feature.

## Open questions

- **What counts as a domain type.** Diff-scoping makes this much less pressing — each type is
  asked about once, where the author is — but a repository that lands forty types in one
  commit (C's maximum is 42) will want something. A stoplist is the obvious answer and the
  obvious way to get it wrong; measuring which names recur across A, B and C before writing
  one is the cheaper order.
- **The calculator/connector boundary**, which #23 raises and this RFC does not need for its
  one finding. It becomes load-bearing the moment a finding's *text* depends on which kind of
  code it is in. Measurable now: A has ~17 connector-ish and ~13 calculator-ish files, C ~32
  and ~49.
- **Removal signals.** #23 wants an `unimplemented-class` finding when an implementation is
  deleted. That is the inverse of E1's class-contract checks and probably belongs beside them
  rather than here, since its subject is the ontology rather than the diff.
- **What `--from`/`--to` default to.** `yidam diff` takes an explicit range. A check meant for
  CI probably wants the merge-base with the default branch, which is a different default and
  one this RFC has not measured a need for.
