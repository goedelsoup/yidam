# RFC-0037 — An open question is a node; `[open]` is a claim's standing

- **Status:** Accepted
- **Track:** G7
- **Relates to:**
  - RFC-0001 (the report contract, which is where the `?` convention was first written down — and, until this RFC, the only place)
  - RFC-0006 (which named the four inlined copies of this predicate that were folded into one function; the rename here must not re-create one)
  - RFC-0020 (the findings/proposal family, whose carried `standing: open` is the one arm of the predicate that **no corpus supplies**)
  - RFC-0024 (a gate loosened quietly; the reason the marker ratified here gates nothing)
- **Versioning layers touched:** template (`GRAPH.md` gains one subsection and a corrected verb gloss; `PROTOCOL.md` step 5 edited) and tooling (one symbol renamed, one predicate added, four tests, two docstrings corrected). **No contract bump** — the MCP tool contract stays at 0.24.0, because no arm of the computation changed. **Nothing on disk in any corpus moves.**
- **Downstream reference case:** the 16 corpora with tracked nodes in 18 derived repositories, 2,770 instance nodes, measured read-only on 2026-09-23.

## Summary

[Article V](../../yidam/prelude/CONSTITUTION.md) licenses exactly one exception to scope
fidelity — an open-question node standing for a tension that could not be resolved — and #569
observed that nothing in the corpus marks such a node.

The marker existed. A `label` beginning with `?` has been read by `yidam open-questions` since
the genesis commit, is the first arm of the predicate every open-question surface calls, and is
frozen in the MCP tool contract. What it had never been is **declared**: no prelude document
said so, which is why the issue could be filed in good faith. This RFC declares it, and it
builds no gate.

The measurement that establishes this also falsifies the issue's proposed marker, and renames
the function at the centre of it. The issue suggested a declared `type: claim` property, on the
grounds that one corpus already uses `claim_tag: open` this way. That property is real, but its
population is **60.1% of every node in every corpus** — it is a claim's *standing inside* a
node, not a statement about what the node **is**. The predicate that reads it was called
`is_open_question` and is now called `has_open_claim`.

## Problem

### Three of the filing issue's premises move

#569 asked for a marker and asked, first, for a measurement. The measurement changes what should
be built, so it is stated before the design rather than after it.

| #569 says | Measured, 2026-09-23 |
|---|---|
| "nothing in the corpus marks it" | **False, and the reason it reads as true is real.** The `?`-prefixed label is the marker, it is the first arm of the predicate, and it is in the frozen contract. It appears in no prelude document — only in RFC-0001 and an information-architecture note, neither of which a corpus author reads |
| a declared property is the candidate marker, since `claim_tag: open` "already exists and corpus A is using it exactly this way" | **The property exists; it is not a marker.** It is one of four arms of a *claim-standing* predicate whose population is 1,665 of 2,770 nodes. Adopting it as a node-identity marker is wrong by a factor of twenty |
| Article V is the thing to serve | **Article V's population is one repository, and its exception has fired zero times.** 30 resolution records carry a `What remains open` section; none names a corpus node. Building the marker *for* Article V is building for n=0 |

Two corpora invented their own answer while this was undeclared, and a third wrote the very
distinction this RFC draws into its own class description. That is the evidence that the gap is
a documentation gap.

## The measurement

`yidam open-questions --format json` over every repository with tracked corpus nodes, with each
node attributed to the arm that matched it. Only `git ls-files` content is read: a corpus whose
`.yidam/corpus/` is a git-ignored projection measures its generator, not itself.

The three public corpora are named so the aggregate has a falsifier; the rest are lettered.

| corpus | authored commits | nodes | node-scoped open | `?` label | declared field | carried finding | prose `[open]` only |
|---|---:|---:|---:|---:|---:|---:|---:|
| `goedelsoup/ohio-budget` (public) | 83 | 777 | 408 (52.5%) | 0 | 0 | 0 | 408 |
| `goedelsoup/allen-county-ohio` (public) | 1449 | 695 | 457 (65.8%) | 0 | 9 | 0 | 448 |
| corpus A | 709 | 194 | 119 (61.3%) | 7 | 0 | 0 | 112 |
| corpus B | 104 | 183 | 132 (72.1%) | 0 | 0 | 0 | 132 |
| corpus C | 69 | 134 | 81 (60.4%) | 3 | 0 | 0 | 78 |
| `goedelsoup/ohio-education-funding` (public) | 645 | 134 | 93 (69.4%) | 0 | 0 | 0 | 93 |
| corpus D | 247 | 125 | 115 (92.0%) | 0 | 5 | 0 | 110 |
| corpus E | 811 | 106 | 93 (87.7%) | 0 | 0 | 0 | 93 |
| corpus F | 157 | 98 | 51 (52.0%) | 0 | 0 | 0 | 51 |
| corpus G | 73 | 81 | 14 (17.3%) | 0 | 0 | 0 | 14 |
| corpus H | 122 | 71 | 13 (18.3%) | 0 | 0 | 0 | 13 |
| corpus I | 82 | 51 | 26 (51.0%) | 0 | 0 | 0 | 26 |
| corpus J | 104 | 43 | 23 (53.5%) | 0 | 0 | 0 | 23 |
| corpus K | 24 | 37 | 16 (43.2%) | 0 | 0 | 0 | 16 |
| corpus L | 8 | 31 | 24 (77.4%) | 0 | 0 | 0 | 24 |
| corpus M | 3 | 10 | 0 (0.0%) | 0 | 0 | 0 | 0 |
| **total (16)** | | **2,770** | **1,665 (60.1%)** | **10** | **14** | **0** | **1,641** |

**The four arm columns partition the population**: 10 + 14 + 0 + 1,641 = 1,665, and not one node
matched two arms. The last column is therefore *matched by prose alone*, not *contains `[open]`
prose*. Every row is shown, including the floor, so the band below contains its evidence.

### Finding 1 — the `[open]` population is 60% of a corpus, so it is not an exception to anything

**1,665 of 2,770 nodes — 60.1%, per corpus 0% to 92.0%** — satisfy the predicate that four
surfaces call to answer *"is this an open question?"*. The floor is corpus M, a 3-commit,
10-node prototype; the fifteen corpora past that size run **17.3% to 92.0%**.

Sixty percent of a corpus is not a set of tensions that could not be resolved. It is what a
corpus that tags its evidence looks like, and the tags are the point. The population is *nodes
containing at least one open claim* — a different question with a different answer.

### Finding 2 — the marker already exists, and is the only arm that is about the node

Of the four arms, exactly one is a statement about node identity: the `?`-prefixed label. The
other three report the standing of a claim *inside* a node and say nothing about what the node
is. Corpus D declares a `type: claim` property named `attestation_standing`; its
`technique/blast-beat.yml`, labelled *Blast Beat*, reads `open` there, because how well the
technique is attested is genuinely unsettled. That is the field working exactly as designed on a
node that is plainly not a question.

**10 nodes of 2,770 (0.36%), in 2 of 16 corpora**, carry the `?`.

### Finding 3 — the `?` must be quoted, and the failure mode is silent

A bare `?` followed by a space is YAML's explicit-key indicator. `serde_yaml` and npm `yaml`
both refuse `label: ? Whether …` at the same column, and `parse_instance` is
`from_str(..).unwrap_or_default()` — so an unquoted marker does not raise, it yields an **empty
instance**: no label, no class, no properties. `yidam lint` does catch it as `malformed-yaml`,
and every report that runs before lint does not; the node is listed with a blank label and the
marker is gone.

Every instance in every corpus that uses the marker quotes it. A declaration that showed the
bare form would hand out the one spelling that silently loses the thing being declared, so the
declaration shows the quotes and says why.

### Finding 4 — three corpora built a question class, and one of them wrote this distinction down

Not two. `question` in `goedelsoup/allen-county-ohio` and corpus F, `inquiry` in corpus A. **34
question-class instances exist across the three, and 31 are found by the predicate.** The 3
misses are not defects — they are *answered* questions, and `goedelsoup/allen-county-ohio`
distinguishes them with `claim_tag` plus a `closed:` date, which is the correct shape.

Read the other way: of the 1,665 nodes the predicate reports, **31 — 1.9% — are instances of a
class named for a question at all**; read against just the three corpora where such a class
exists to be used, **4.9%**. Either reading is wrong by more than a factor of twenty as a
statement about node identity.

Corpus A's `inquiry` class describes itself as holding the questions that node-local `[open]`
claims *bear on*. A derived corpus wrote the distinction this RFC draws, in its own ontology,
because the prelude did not.

### Finding 5 — the `open` verb was glossed for an act the corpora do not perform

`GRAPH.md` glossed `open` as *"A question opened — or, in collective mode, an elector's position
opened"*, leading with the sense that turns out to be the rare one.

Of **57** `open:` commits across eighteen repositories, **41 touch no corpus node at all, and
all 41 are in the one repository that has run the sangha protocol** — 40 of them writing a
`sangha/positions/*.md`. And not one of the ten `?`-marked question nodes in any corpus was
introduced by an `open:` commit: they arrive under `establish` (7), `assess` (2) and `genesis`
(1). Opening a question and marking a node as one are, in practice, unrelated acts.

### Finding 6 — Article V's exception has never fired

In the one repository that has run the protocol, **30 resolution records carry a
`What remains open` section and none names a corpus node in it.** Two name a `.yml` at all, and
neither is an instance: one is a CI workflow, the other a `.yidam/decisions/` node. Read from
the other side, 4 instance files were introduced by a resolution across the whole history and 0
are named under `What remains open`.

A licensing clause with no subject cannot be what a marker is designed against.

## Decisions

### 1. Ratify the marker; gate nothing

`GRAPH.md` gains a subsection under `## Nodes` declaring that a node whose `label` begins with
`?` is a question the corpus has not closed, with the quoting rule from Finding 3 and the
adoption figure stated plainly. `PROTOCOL.md` step 5 tells a resolution to write the marker.

No lint reports its absence, no class may require it, and a corpus that never writes a `?`
stays well-formed. At 10 nodes in 2 of 16 corpora, a gate would report the other fourteen
corpora as defective for following a convention nobody had published. RFC-0024's rule about
gates cuts the same way one level out: a convention is ratified first and gated later, if ever.

### 2. Rename the predicate, and add the narrow one

`is_open_question` becomes **`has_open_claim`** — 33 occurrences across 15 files. The name
asserted node identity while the function measured claim standing, and four surfaces called it
believing the name.

A new **`is_question_node`** answers the other question. `has_open_claim` **delegates** its
first arm to it rather than respelling `starts_with('?')`: splitting the name must not split the
rule, and re-creating one of the copies RFC-0006 removed, inside the function renamed for
clarity, would be the same defect wearing the repair's clothes.

The arms, the command, the report field and the MCP tool are all still called `open_questions`.
Nothing about the computation changed, so **contract 0.24.0 does not bump**. Only the symbol
moved, and only so that a caller has to choose.

*Recorded dissent.* The recommendation put to the owner was to rename only, on the grounds that
`is_question_node` would ship as a `pub` function with no in-tree consumer — the defect already
noted against `claims_in_node` in #568. The owner directed that it be added now. The delegation
above is what answers the objection: the narrow predicate has a caller from its first commit,
and it is the broad one.

### 3. Correct the `open` verb gloss in this change

The table row now leads with the elector's position, the paragraph carries Finding 5's numbers,
and it says outright that the old gloss led with the sense the corpora do not use.

*Recorded dissent.* The recommendation was to file this separately, as an unrelated defect found
while measuring. The owner directed it be fixed here. It is one paragraph and one table cell, and
the measurement that condemns the gloss is in this RFC.

### 4. Do not promote a question class, and do not touch Article V's exception

Three corpora built one and they do not agree on its name. A class the prelude declares would
either contradict two of the three or oblige them to migrate, for a population of 34 nodes. The
`?` costs a corpus nothing and interferes with no ontology.

`excepted()` in the scope-fidelity check is **unchanged**. A marker says *this node is an open
question*; it cannot say *and this record is why that was allowed*, because it is written by the
same commit it would license. Reading the marker there would let a resolution except itself by
titling a node well. The record stays the authority; the marker is for whoever reads the corpus
next.

## What is not built, and what would build it

- **No gate on the marker.** Reversed by adoption: if the `?` reaches corpora beyond the two
  that already use it — say 5 of 16, or any corpus adopting it after reading `GRAPH.md` — a
  `missing-question-marker` check on nodes of a question-named class becomes measurable.
- **No prelude question class.** Reversed if the three existing classes converge on a name, or
  if a fourth corpus invents a fifth.
- **No Article V mechanism.** Reversed the first time a resolution actually names a corpus node
  under `What remains open`. Until then there is nothing to tune.

## Migration & compatibility

- **Corpora:** nothing to do. No file moves, no field is added or removed, and the marker was
  already being read.
- **The contract:** `mcp/tools.json` stays at 0.24.0. The four arms and the edge arm are spelled
  there and none changed.
- **Four RFCs cite `is_open_question` in prose** — RFC-0006, RFC-0020, RFC-0028 and RFC-0031 —
  and are **deliberately not edited.** Each records a decision taken at a point in time when
  that was the symbol's name, and rewriting the record to match today's code would make the
  register a snapshot rather than a history. A reader following one of those citations finds
  `has_open_claim` and this RFC's first section explaining the rename.
- **Downstream:** `is_open_question` was never exported across a language boundary; the three
  SDKs name the report field, which is unchanged.

## Falsifier

Re-run the arm attribution in a quarter, on 2026-12-23, over whatever the derived population is
then. Three things are predicted, and each fails visibly:

1. **`is_question_node` still has exactly one caller in-tree.** If no surface has asked the
   narrow question in three months, the recorded dissent was right and the function should be
   reconsidered rather than kept out of politeness.
2. **The `?` count has grown past 10, or `GRAPH.md` did not cause adoption.** Declaring a
   convention is the intervention; the count is the reading.
3. **The carried-finding arm is still 0.** An arm of a frozen contract that no corpus has ever
   supplied is a surface with no consumer, and if it is still empty it belongs in the next
   contract review, not in this RFC.
