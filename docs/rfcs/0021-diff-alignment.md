# RFC-0021 — Code that names what the ontology has not (`yidam check-diff`)

- **Status:** Implemented
- **Track:** I16
- **Relates to:**
  - RFC-0020 (the proposal surface a finding here becomes)
  - RFC-0018 (the typed query this matches against)
  - RFC-0001 (the report contract it emits on)
  - RFC-0003 (the light binary it must run in)
  - RFC-0016 (the severity table an editor renders it through)
- **Versioning layers touched:** tooling (`yidam` CLI) — **no parity-surface change, no
  template change, no MCP contract change**; see [What this does not
  touch](#what-this-does-not-touch)
- **Downstream reference case:** Project BOSC (watermark-directory)
- **Parent epic:** #23, restructured — this RFC specifies **#342** (Phase A). The semantic half
  is **#343**, specified in RFC-0022, which measured the embedding pass and dropped it.

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

## Proposal

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
- **The template.** No new directory, no new frontmatter field, no rule the prelude enforces.
  A derived repository gets the check by re-vendoring the binary. `guidelines/directories.md`
  gained a paragraph under `crates/` saying the check exists, that it never gates, and that
  `.yidam/authorship.yml` is the vocabulary for excluding test code — the same amendment #270
  made to `agent-conduct.md`, and for the same reason: a check nobody in a derived repository
  knows about is a report nobody reads.
- **The MCP contract.** No tool is added.
- **The network, and any model.** Phase A is deterministic. Whether the tool may ever call a
  model is #343's first question, and it is unanswered.
- **`--features index`.** Nothing here needs the vector index, which is what keeps it testable
  in the CI that would run it — PR CI never compiles that feature.

## What implementation found

Added after #342 landed, because four things the design treated as incidental turned out to
be load-bearing and one open question answered itself.

**`.rs` is not a formality.** `crates/README.md` in one repository contains the sentence
fragment *"a struct rather"*, which the declaration pattern matches happily. A line-based
extractor over a diff of `crates/` reads whatever `crates/` holds, and the file-type filter
is what stops it reporting prose as a domain concept.

**The removal guard is what makes the diff scoping honest.** C's busiest code commit in the
sample adds 42 declarations — the maximum in the table above — and reports **zero**. It is a
file split: *"the feed's types and its serializer stop sharing a file"*, 42 added and 42
removed. Without the guard the single loudest finding this check could produce in three
repositories would have been a no-op refactor, which is exactly the shape of noise that gets
a check turned off.

**`trait` and `type` stay out, measured.** Widening to both adds 14, 3 and 13 names across
A, B and C, and none of them is a concept — `Connector`, `Result`, `BoxFut`. A trait names a
capability and an alias names a spelling.

**Authorship-only exclusion costs nothing.** Of the 68, 101 and 20 types introduced across
each repository's most recent 60 code-touching commits, **2 apiece** sit under a `tests/`
path. The second vocabulary #23 asked for would have bought six findings' worth of quiet
across three repositories.

**Property and relationship names carry real weight.** The ontology vocabulary is 152, 84 and
189 names against 15, 12 and 18 classes. A check reading class names alone would ask about a
concept the corpus already models, as a property of something else or as the name of an edge.

**The `open:` proposal has nowhere to land, and RFC-0020 already said why.** [Under E4, a
finding becomes a question](#under-e4-a-finding-becomes-a-question) claims `yidam propose`
drafts an `open:` for a finding here on ordinary terms. It does not, and #342 does not wire
it. Each of `propose`'s three acts writes a paragraph into a corpus node or a catalog entry,
and an unmodelled concept has neither *by construction* — that it is in no file is the
finding. Proposing the class would be composition, which the carriage rule forbids; choosing
which existing node should carry the question is RFC-0020's own *"the ontology names a class
and not a node"* correction one step further along, with no candidate rather than seven. And
`propose` takes no range, so wiring it would mean answering the `--from`/`--to` question below
instead of deferring it. That section stands as an intent, not as built behaviour.

## Open questions

- **A stoplist does not survive its own measurement, and that is the answer.** The question
  above asked for recurrence across A, B and C before writing one. Measured: of **514 distinct
  type names**, exactly **one** — `Coverage` — appears in all three, and 31 appear in two. That
  list is not infrastructure either; it holds `Chamber`, `Reach`, `Sponsor`, `Provenance` and
  `Standing`, every one of them a domain noun in the repository that has it. A recurrence-derived
  stoplist would be a handful of entries long and would suppress real concepts to get there. If
  the forty-types-in-one-commit case needs something, it is not this.

- **What counts as a domain type.** Diff-scoping makes this much less pressing — each type is
  asked about once, where the author is — but a repository that lands forty types in one
  commit will want something. See the measurement above: the obvious answer is measurably
  wrong, and C's 42-declaration commit turned out to report nothing anyway.
- **Near-miss matching**, which RFC-0022 settles: an embedding pass is not worth its feature
  gate, and the candidate belongs on this finding as a `nearest` field rather than in a pass
  of its own.
- **The calculator/connector boundary**, which #23 raises and this RFC does not need for its
  one finding. It becomes load-bearing the moment a finding's *text* depends on which kind of
  code it is in. Measurable now: A has ~17 connector-ish and ~13 calculator-ish files, C ~32
  and ~49.
- ~~**Removal signals.**~~ **Settled** (#33), and beside E1's checks as this predicted. It
  is also where the rename question went: a diff shows a rename as a removal plus an
  addition and correlating the two is the hard part, while a check that scans the tree finds
  the class under its new name and says nothing. What did not survive was the unconditional
  reading — see RFC-0022's note for the measurement that killed it.
- ~~**What `--from`/`--to` default to.**~~ **Settled** (#389): the range is optional and
  defaults to the merge-base with `main` (or `master`). The CI half of the question lapsed
  when #29 closed — nothing gates and no workflow ships — so what remained was plainer: this
  command copied `diff`'s required range, but `diff` compares two corpus states and neither
  is privileged, while this asks *what did this branch's worth of work name that the ontology
  has not*, and that question has an obvious range. It is closer to `log` than to `diff`.

  The measurement the original copy lacked, taken over every branch the derived corpora have
  merged into their baseline: **250 of 381 touched `crates/`** — 90% and 88% in the two
  largest, 64% and 47% in the next two. So the default asks about real code most times it
  fires.

  The same measurement is why the degenerate positions are **errors rather than empty
  reports**. Eight of thirteen corpora sit *on* their baseline at any given moment, where the
  merge-base is HEAD; an empty report there reads "No type declaration was introduced in
  `crates/` by main..HEAD", which is true and indistinguishable from the informative answer —
  *your branch introduced no types*. That is the failure `example_corpus.rs` argues against
  from the other direction. A repository with no baseline at all, mid-bootstrap, refuses for
  the same reason. Detached HEAD, filed as the second surprising case, is not one:
  `merge-base main HEAD` resolves there normally and the default is exactly right.

  The default resolves to `main..HEAD` only while the baseline has not moved on, and to the
  merge-base sha once it has. A two-dot range compares endpoints, so against a moved `main` it
  reports a type that existed at the branch point and that `main` has since deleted as newly
  introduced — a question put to an author about code their branch never touched.
