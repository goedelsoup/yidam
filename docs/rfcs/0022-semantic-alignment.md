# RFC-0022 — What a tool may say about code it cannot read (`check-diff`, Phase B)

- **Status:** Draft
- **Track:** I17
- **Relates to:** RFC-0021 (Phase A, which this continues), RFC-0020 (the carriage rule it
  inherits), RFC-0003 (the light binary it must run in), RFC-0016 (the severity table an
  editor renders it through), RFC-0001 (the report contract it emits on)
- **Versioning layers touched:** tooling (`yidam` CLI) — **no parity-surface change, no
  template change, no MCP contract change**
- **Parent:** #343, which is #23's other half — three unmade decisions rather than an
  implementation. Phase A is #342, shipped in #345.

## Summary

#23's passes 3–4 — semantic/embedding matching and an LLM judge — kept that issue closed
from 2026-07-01. Not for difficulty: they turned on three questions nobody had answered, each
larger than the feature. This answers all three.

The first two are decisions and are recorded as such. The third asked how the vector-index
feature gate should be handled in a CI that never compiles it, and answering it honestly
required calibrating the semantic pass first. **That measurement inverts the phase plan.**

Across the three instrumented derived repositories, embedding similarity finds 55 candidate
matches for 516 unmatched type names. **A four-character prefix rule — no model, no feature
gate, no network, running in the light build — finds 50 of them.** The embedding adds five,
one of which is wrong.

So Phase B is not an embedding pass. It is a lexical near-miss annotation on the finding
Phase A already emits, and the feature-gate question dissolves because nothing here needs the
feature.

## The three questions

### 1. May the tool call a model? Only the one it already calls.

**The premise needed correcting before the question could be answered.** #343 says there is
not one call to any model provider in `yidam/cli/src`, and that is true. It is also
incomplete: the CLI **already runs a model**. `fastembed` is a local ONNX embedding model
behind `--features index`, and `yidam embed` and `yidam index-build` run it today. No
credentials, no provider API, no egress beyond a one-time weight download.

That splits the question in two, and only one half was ever open:

| | | |
|---|---|---|
| a local embedding model | pass 3 | **already shipped**, behind a feature |
| a remote LLM judge | pass 4 | genuinely unprecedented |

**Decision: no remote model, and pass 4 is dropped rather than deferred.**

Three settled things run against it, each with a stated reason. `doctor` writes nothing and
does no network, and `cmd/doctor.rs` calls that a constitutional limit rather than a scoping
one. The **light default build** is what derived-repo CI downloads, so a command behind a
feature nobody compiles is a command nobody runs. And `tonpa`'s own module comment records
the same reasoning about buying the network.

A third posture exists and was considered: the test harness's judge holds no credentials
either — `judge.rs` shells out to `claude --print` and reads stdout, so the network surface
is a subprocess. It was rejected on the ground that it names one vendor's CLI as a
dependency of a gate, and makes the judge's model whatever that CLI happens to resolve — *"a
yardstick that moves with the thing it measures measures nothing"*, which is `judge.rs`'s own
comment about why it pins its scorer.

**What it costs to be wrong.** `CONFLICT` — code contradicting a corpus claim — is #23's
central idea, and this abandons it rather than postponing it. If a derived repository later
demonstrates a real contradiction that only a judge could have caught, this decision is the
reason nothing caught it. That is the trade, stated plainly: the alternative is a build gate
whose strongest verdict comes from its least reproducible source.

### 2. May such a verdict gate? No.

#23 gives `CONFLICT` a default severity of **blocking**. **Decision: nothing in Phase B
gates.** Warn, exit 0, matching `check-diff` as shipped.

Everything E4 settled runs the same way and each for a stated reason: `propose` refuses to
decide anything; `verified-unsourced` is Warn because the fix is an author's judgement;
`catalog-expired` reports because refreshing a source is a knowledge event a person owns; and
E3's `render_movements` prints *"nothing was changed, and no claim was re-tagged"* precisely
to refuse a verdict it could have stated.

With pass 4 dropped this is nearly moot — a lexical near-miss annotation could not
responsibly gate anything. It is recorded anyway, because the reason is the same reason and
the next person to propose a gate here should have to argue against it.

**What it costs to be wrong.** A genuine contradiction can be merged, and the report is the
only thing standing in the way. Against that: gating would make adoption a build break in
every repository that predates it, which is the ratchet failure
`docs/post-genesis-measurement.md` recorded.

### 3. How is the index feature gate handled? By not needing it.

The question was real. Embedding matching needs `--features index`; PR CI never compiles it,
and the `cli · full features` job shows `skipping` on every pull request. #264's decision 3
established the trap: `retrieve` degrades to keyword matching without the feature, so a
benchmark on PR CI would beat keyword search and report nothing about retrieval. A semantic
pass that silently degrades to name matching is the same failure with a different name.

**Decision: refuse rather than degrade** — the rule `bench` already follows, which bails with
*"beating keyword search proves nothing about retrieval"* rather than scoring the fallback.

**And then the calibration made the decision almost unnecessary**, which is the next section.

## Calibration, and what it changed

#343 requires this section and forbids one way of writing it: never `examples/streamflow`,
whose arithmetic floor #264 established makes it unusable as evidence. Measured against A, B
and C.

### The subject

Phase A reports a type whose kebab-cased name matches no declared class, property or
relationship. Pass 3's job is to rank those against the ontology and say *this one is nearly
`sponsored-by`*.

| | A | B | C | total |
|---|---|---|---|---|
| unmatched type names | 267 | 46 | 203 | **516** |
| ontology vocabulary | 150 | 83 | 189 | |

### The embedding arm

`AllMiniLML6V2Q` — the model `embed_config.rs` already names — over both lists, cosine on the
top-1 declared name.

| | A | B | C |
|---|---|---|---|
| median top-1 cosine | 0.453 | 0.416 | 0.454 |
| p90 | 0.672 | 0.729 | 0.736 |
| ≥ 0.70 | 22 | 5 | 28 |

**55 candidates for 516 types.** Read them and the character is unmistakable: they are
overwhelmingly *morphological*, not semantic.

```text
tenures    -> tenure          0.941      precinct  -> precincts   0.913
computed   -> computed-for    0.906      sponsor   -> sponsors    0.900
functions  -> function        0.892      release   -> released    0.876
district-enrollment -> enrollment 0.801  label-ref -> label       0.802
```

Plurals, tenses, spellings, and compounds sharing a root. Not one of those needs a model.

### The free arm

A rule with no dependency at all: split both names on `-`, and count two words the same when
they share a four-character prefix and differ by at most three trailing characters.

| | A | B | C | total |
|---|---|---|---|---|
| embedding matches ≥ 0.70 | 22 | 5 | 28 | **55** |
| the prefix rule also finds | 19 | 5 | 26 | **50** |
| **embedding-only** | 3 | 0 | 2 | **5** |

The five, in full, with what they are:

| pair | cosine | verdict |
|---|---|---|
| `answer` → `question` | 0.709 | **wrong** — related, not the same concept |
| `held` → `holds` | 0.800 | right, and morphological — an irregular verb the prefix rule cannot see |
| `placement` → `position` | 0.712 | arguably right |
| `edition` → `version` | 0.708 | right, and genuinely semantic |
| `totals` → `amount` | 0.728 | right, and genuinely semantic |

**Three genuinely semantic hits and one false positive, across three repositories**, in
exchange for `--features index`, a 23MB ONNX download, protoc at build time, and a CI story
#264 already showed is a trap.

### The fair arm, which is worse

The first arm embeds bare names, and a strawman arm is exactly what #264 forbids — `yidam
embed` embeds a node's prose, not its name. So the ontology side was rebuilt with each name
carrying its declared `description`, which is what would actually ship.

| | A | B | C |
|---|---|---|---|
| median top-1 cosine, names only | 0.453 | 0.416 | 0.454 |
| median top-1 cosine, with descriptions | **0.334** | **0.302** | **0.350** |
| ≥ 0.70, with descriptions | 0 | 0 | 4 |

**The richer arm is worse, and 55 candidates become 4.** A bare type name against a
thirty-token description is a one-token query against a document, and cosine collapses on the
asymmetry. Giving the semantic pass its best shot made it worse, which is a stronger result
than the first measurement rather than a weaker one.

### What this did not test, and the ceiling it leaves

One arm remains unmeasured: embedding the type **with its code context** — doc comment, field
names, enclosing module — so that both sides are documents rather than one being a token.
That is a real variant and this RFC does not claim to have refuted it.

It is bounded, though. The name-vs-name arm is the framing most favourable to a bi-encoder,
and it found 55 candidates in 516 types of which 50 were reachable for free. Whatever a
richer query representation buys, it is bidding against a rule that costs nothing and already
covers 91% of the ground.

### And so the threshold stops being load-bearing

#23's last open question is *"confidence threshold calibration for semantic matching pass"*,
and it is a hard question when a score decides whether a finding exists. It is not one here,
because **the candidate annotates a finding rather than creating one**.

Phase A has already decided to report the type. A near-miss adds a clause to a row that
exists either way, so a wrong candidate costs a bad lead and never a false finding. Measured,
the prefix rule offers a candidate for 57 of A's 267 unmatched types, 10 of B's 46 and 85 of
C's 203 — and adds no rows to any report.

### Re-measured against the shipped rule

The section above was measured 2026-08-22 against a prototype. Re-running the rule as built
(#349) against the same three repositories on 2026-08-26 gives **56 of A's 273, 11 of B's 46
and 79 of C's 203**. The vocabularies are unchanged at 150, 83 and 189, and B's and C's
unmatched populations are unchanged too; A has gained six types since.

C is the one that does not reconcile — the same 203 names and the same 189 declarations,
79 candidates against 85. Every looser reading of *"differ by at most three trailing
characters"* was tried and none of them lands on 85 while leaving B at 10: dropping the
length bound gives 83/13, letting sub-four-character words match on equality gives 81/11.
**The prose is the specification and the prototype is not recoverable from it**, so what
shipped is the prose, and these are its numbers. The conclusion is untouched — a candidate on
roughly a quarter of unmatched types, and no new rows.

### The shared root is what makes a bad lead cheap

The measurement #349 was asked for is whether the *suggested* name is right, not how many are
suggested. Read in full, wrong suggestions concentrate almost entirely in one place: a shared
root of exactly four characters that is a truncation rather than a whole word — `state` →
`status` on `stat`, `profile` → `profession` on `prof`, `lever` → `level` on `leve`. Where
the four characters are a whole word on both sides (`vote`, `plan`, `case`, `line`, `read`)
the suggestion is almost always right, and every root of five characters or more is.

That is an argument for reporting the root rather than a score, and not an argument for a
fifth character. `shared: "stat"` tells a reader at a glance that the lead is weak, which a
fabricated 0.83 would have concealed; and lifting the floor would take `vote` and `case` with
it. Recorded as evidence for the open question below rather than acted on.

## Design

### One field, not one pass

`unmodelled-concept` gains an optional `nearest`. No new check, no new command, no new
severity, no feature gate.

```json
{
  "check": "unmodelled-concept",
  "severity": "warn",
  "concept": "Sponsorship",
  "name": "sponsorship",
  "nearest": { "name": "sponsored-by", "shared": "sponsor" },
  "question": "nothing the ontology declares is named `sponsorship`. The nearest declared name is `sponsored-by`, which shares the root `sponsor`. Is this that relationship under another name, or a concept of its own?"
}
```

### It reports the reason, not a number

The decision this RFC records is *a question, with the candidate as evidence — the score
reported, never used as a verdict*. That decision was taken while the candidate was expected
to come from an embedding, where the score is the only thing there is to report.

A prefix rule has no such score. Reporting an overlap fraction as though it were a confidence
would be false precision — a number invented by a string comparison, dressed as a
measurement. So `shared` carries **the root the two names have in common**, which is what a
reader would check anyway and is not falsifiable in the way a fabricated 0.83 is.

The register is unchanged from Phase A and from `citations::moved`: *"Phrased as a question,
deliberately. The answer is a person's."* A near-miss is a lead, and the report must keep
saying that matching is by name.

### `CONFLICT` is retired, not deferred

#23's finding table has three rows. Phase A shipped `GAP` as `unmodelled-concept` and turned
`ALIGNED` into a count. `CONFLICT` is withdrawn by decision 1, and the table should be closed
rather than left implying a fourth phase is coming.

This is the honest reading of what a deterministic tool can say. `Sponsorship` sharing a root
with `sponsored-by` is a fact about two strings. *"This calculator's rounding contradicts the
claim in `concept/low-flow.yml`"* is a fact about meaning, and nothing here can hold one.

### The meta-rubric question dissolves

#343 asks whether the evaluator quality suite should reuse `yidam-harness` — its `judge.rs`,
its `rubric.rs` that reads the rubric rather than restating it, its `diff.rs` that refuses to
compare across protocol versions. With no judge there is nothing for a meta-rubric to score:
a prefix rule is tested by unit tests, the way `source_classes` is.

The harness stays what it is — the bootstrap judge, deliberately outside any gate.

## What this does not touch

- **The parity surface.** No SDK gains a function. Phase A already argued why, and a rule with
  one caller has not become three.
- **The template.** No new directory, no new frontmatter field, no rule the prelude enforces.
- **The MCP contract.** No tool is added.
- **`--features index`.** Nothing here needs the vector index — which is the finding, not an
  accommodation.
- **The network, and any model.** Restated because it is the decision: yidam calls no remote
  model, and `check-diff` adds no egress path.

## Open questions

- **The unmeasured arm.** Embedding a type with its code context, so that both sides are
  documents. Bounded by the numbers above, and worth revisiting only if a repository produces
  near-misses that share no root — which is a thing that can be *observed* from the reports
  this ships, rather than guessed at now.
- **Where a semantic finding would live if one were ever justified.** Probably not here. Its
  subject would be a claim rather than a diff, which puts it beside E1's class-contract checks
  and RFC-0019's citation survey rather than in a command that reads `git diff`.
- **Removal signals.** RFC-0021 left `unimplemented-class` open, and decision 2 sharpened the
  question rather than answering it: that finding *is* decidable without a model, so it is the
  one candidate in this area that could carry a gate. It belongs with E1's contract checks and
  should be argued there.
- **Whether the prefix rule's constants survive contact.** Four characters and three trailing
  is what fits A, B and C. It is a string rule with two magic numbers and no corpus has argued
  with it yet.
