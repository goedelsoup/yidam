# RFC-0043 — A figure published twice, computed once — an inline REGEN block and the `count` generator

- **Status:** Implemented
- **Commands:** `count`
- **Track:** I32
- **Relates to:**
  - RFC-0018 (the query surface, which this adopts verbatim as the generator's argument rather than coining a second way to name a set)
  - RFC-0035 (class extent — a `coverage:` declaration is a claim about how many there should be; this is a claim about how many there are)
  - RFC-0001 (the report contract — `count` is a report with a `--format json` shape like the rest)
- **Versioning layers touched:** template `v0.8.0` → `v0.9.0` (the REGEN marker format gains a form, which VERSIONING.md's Layer 1 table calls major, and §"Migration" argues it is right to) / SDK parity (`scan_markers` and `update_regen` in all three SDKs, the `graph.dfy` model, nine new fixtures) / `yidam-core` minor (a new public field on `Marker::Regen`'s sibling type; no existing signature changes) / CLI surface (one new command, one new generator). No MCP contract change, no bootstrap-protocol change, no migration of corpus content.
- **Downstream reference case:** `matt-huffman`'s `crates/corpus/tests/roadmap.rs` and `demi-moore`'s `crates/demi-feed/tests/readme.rs` — two repositories that built the same gate independently, and each of whose header states the limit this RFC removes.

## Summary

A number that states a fact about the corpus is published in two places: in the graph, where it
is computed, and in a sentence, where it is remembered. The remembered copy drifts. Three
derived repositories have now built a gate against it, none of the gates can reach a number in
the middle of a sentence, and two of them say so in their own headers. This RFC adds
`<!-- REGEN: yidam count <query> -->44<!-- /REGEN -->` — an RFC-0018 query, a number, and the
existing `regen --check` contract — so that the figure in the sentence is the figure in the
graph by construction rather than by assertion.

Getting there costs a marker-format change, because the form the sentence needs is **inline**
and the marker contract has never had one. It turns out that writing one today is not merely
unsupported: it is silently destructive, and it is destructive because the two functions that
read a REGEN block disagree about what one is.

## Problem

### A figure published twice drifts, and three repositories have built the same gate

`demi-moore`'s `crates/demi-feed/tests/readme.rs` opens with the inventory:

> The front page of the domain computer said "twelve crates" against fourteen, "the seven
> connectors" against nine, and "13 invariant checks" against thirty-one — and the two crates it
> had never heard of were missing from its table as well. Nothing was careless; there was simply
> no gate.

`matt-huffman`'s `crates/corpus/tests/roadmap.rs` opens with the same finding and adds the
measurement of what its own gate could not see:

> This file gates an enumerated list of figures: the headline counts, the position row, the
> tenures row, the resolution heading, the crates, the links. It does not and cannot gate an
> arbitrary number in a narrative section, because nothing tells it which numbers those are. So
> when `3,104` went stale to `3,111` in the roadmap's committee-rosters section, this gate
> **could not have caught it** — it did not fail, it was never pointed at it.

Its conclusion is this RFC's summary, written by the repository that needed it: *"The general
fix is not more assertions here; it is that a figure worth publishing twice should be computed
once."*

#1071 names a third, `ohio-136th-assembly-hb-96`, with a fourth symptom — prose saying "43
districts" against a graph holding 44.

### The gate a repository can build is pointed by hand, and prose is where it cannot point

`demi-moore`'s header names the gap in yidam's own terms:

> `regen-check` cannot cover this: it compares REGEN blocks against what the yidam CLI
> regenerates, and these numbers sit in hand-written prose above the block.

That is exactly right, and it is a fact about the marker format rather than about `regen`. A
REGEN block is line-shaped. Its open tag must begin a line, its close tag must be the whole of
another, and [`update_regen`](../../yidam/prelude/sdks/rust/src/markers.rs) wrote a body
bracketed by newlines. A block can therefore hold a table row, a list, a paragraph or a
whole section — and cannot hold the word *44* between *has* and *districts*.

### Writing the form anyway is destructive, and silent

This is the part that was not known when #1071 was filed. Run at `1dc8063`, against
`Ohio has <!-- REGEN: yidam count district -->44<!-- /REGEN --> districts.`:

- `scan_markers` returns **no marker at all**. Its open-tag test is anchored to the start of a
  line, and this one does not start one:

  (in [`markers.rs`](../../yidam/prelude/sdks/rust/src/markers.rs), at `1dc8063`):

  ```rust
        let Some(rest) = trimmed.strip_prefix("<!-- REGEN:") else {
            i += 1;
            continue;
        };
  ```

- `update_regen` **does** find it — it searches bytes, not lines — and rewrites it into the block
  form, so the sentence comes back as `Ohio has <!-- REGEN: … -->\n99\n<!-- /REGEN --> districts.`

Move the same block to the start of a line and it gets worse. `scan_markers` reads the open tag,
fails to find a `-->` at the end of the line, takes the multi-line branch, and runs forward
looking for a line that ends in `-->` — which the *next* block's open tag does. The result is one
marker whose command is `yidam count district -->44<!-- /REGEN --> districts.`, whose content is
the following block's body, and whose extent is reported as **well-formed**: `malformed` is
empty. A block was consumed and nothing said so.

`unclaimed_in` would then report the garbage command as a generator that does not exist, which is
the one visible symptom, and it would report it about the wrong thing.

### Two scanners, two answers

The bug above is not two bugs. `scan_markers` was line-anchored and
[`update_regen`](../../yidam/prelude/sdks/rust/src/markers.rs) byte-anchored, and nothing held
them to each other. Every existing document happens to satisfy both, so the
disagreement has cost nothing; the first document that does not is the one this RFC wants to
write.

The published contract already promised the thing that would have caught it. Of
`parse_markers`, `yidam/prelude/sdks/README.md` said: *"Return them in document order with
their spans."* No span was returned. `Marker::Regen` carries a command and a content string and
nothing that locates them, which is why a second, independent locator had to exist for the
writer.

The model has that second locator and does not have the first. `RegenSpan` computes a block's
four offsets by byte search, and
[`RegenSpanFindsEveryBlock`](../../yidam/prelude/sdks/spec/graph.dfy#L267-L268) proves it finds
every block that exists:

```dafny
  lemma RegenSpanFindsEveryBlock(text: string, command: string)
    ensures HasRegenFor(text, command) <==> RegenSpan(text, command).Some?
```

That lemma is about the writer's notion of a block. `scan_markers`' notion is modelled nowhere,
so the proof obligation that would have caught this — *the two agree* — has never been
statable, let alone discharged.

### `count` cannot be registered the way the other fourteen are

Every generator in [`GENERATORS`](../../yidam/cli/src/cmd/regen.rs#L31) writes a block at a path
it knows, under a command it spells as a literal. `count` cannot: its blocks are wherever a
document put them, and its command carries the query, so the set of `(file, command)` pairs is a
property of the tree and not of the source. Three things assume otherwise.

- [`every_generator_in_the_crate_is_listed`](../../yidam/cli/src/cmd/regen.rs#L441-L470) requires
  every `update_file_regen` call site to name its generator with a `"yidam …"` literal, so that
  the guard can see which block each one writes. A computed command has no literal.
- `update_regen` matches its open tag by **prefix**, so `yidam count district` finds and
  overwrites `<!-- REGEN: yidam count district[party=R] -->`. This is #1058's defect with a much
  larger surface: today two generator names collide only if somebody chose them badly, and with
  arguments the collision is a property of the query text.
- `update_regen` rewrites the **first** match only. Two blocks in one document asking the same
  question — *"44 districts … of those 44 districts"* — leave the second stale forever, and
  `--check` will not report it, because a block nothing writes is a block nothing records.

## Proposal

### The block

```markdown
Ohio has <!-- REGEN: yidam count district -->44<!-- /REGEN --> districts.
```

The command is `yidam count` and the rest of the open tag is an RFC-0018 query. The body is the
decimal count of nodes the query matched, and nothing else. `yidam count` refreshes every such
block in the tracked tree; `yidam regen` runs it with the other fourteen; `yidam regen --check`
reports a stale one with the remedy it reports for every other block.

### The argument is a query, not a second way to name a set

#1071 proposes `class=district`. This RFC takes the query language instead, for three reasons.

It already exists and is already typechecked: `query::check` resolves the class against the
ontology, resolves each predicate's property against the class, and refuses an ordering on a type
that has no order. A `class=` mini-language would re-answer all of that, worse, in a second
place.

It reaches what the gates actually count. `roadmap.rs` binds four figures, and exactly one of
them is a class count. The others are the distinct targets of an edge — every `tenure` some
`position` is `recorded-during` — the distinct values of a property ref, and a set difference.
The first is `position -recorded-during-> tenure` and the count is the distinct terminal nodes,
which is what `QueryReport::matched` already means. `class=` can express none of them.

And it makes the open questions a corpus can ask of itself one surface rather than two.
`question[status=open]`, `legislation[enacted<2024]`, `district[population?]` are all countable
the day this lands, because RFC-0018 and RFC-0040 already defined them.

### An inline block is one the document wrote inline, and it stays that way

The form is not a flag on the call. It is read off the block:

> **A REGEN block whose body contains no newline is written back without newlines, unless the
> content being written has a newline of its own. Every other block is written back as it is
> today.**

`<!-- REGEN: a -->44<!-- /REGEN -->` has the body `44` and stays on its line.
`<!-- REGEN: a --><!-- /REGEN -->` has the body `""` and stays on its line, which is what a
hand-authored empty block looks like. `<!-- REGEN: a -->\n<!-- /REGEN -->` — the shape
`update_regen` writes when it clears a section — has the body `"\n"` and keeps the block form,
so the `empty-new-content` fixture is unchanged and so is `ClearingASectionLeavesNoBlankLine`.
In the model this is two conditions in
[`RegenBody`](../../yidam/prelude/sdks/spec/graph.dfy#L314-L318), which takes the block's form
as a second argument and returns the content unwrapped only when the block was inline *and* the
content will fit there.

The `unless` is not a hedge, and it was not in the first draft of this section. A value holding
a newline cannot be written on one line; writing it there anyway leaves a document whose body no
longer matches the form it declares, and the *next* run reads that body as block form and
rewrites it — so `regen --check` reports drift it caused itself. Deciding on the content as well
as on the block is what keeps a second call a no-op. The idempotency clause of `UpdateRegenSpec`
is what says so: with the newline test removed the model stops verifying, which is how the
condition was found.

The rule has the property that matters for a document people edit: the author decides, once, by
writing the block, and no later regeneration second-guesses them.

### One scanner

`scan_markers` learns the inline form — an open tag whose `-->` is followed, on the same line,
by the block's own `<!-- /REGEN -->` closes there, and the scan resumes after it, so a line may
carry more than one block — and it records each block's extent as it goes, which is the reader's
counterpart to `RegenSpan` and the honest version of what `sdks/README.md` already promises.

An extent is an offset into the text, and an offset is **not** a parity contract. Rust counts
bytes, JavaScript counts UTF-16 code units and Python counts code points, so a fixture asserting
`open = 41` would assert three different things about one document the moment it held a
character outside ASCII — which is the trap `find_reachable`'s sort order already fell into and
`parity/README.md` already warns about. The extents are therefore each language's own, in that
language's own unit, and no fixture asserts one. What the fixtures hold is behaviour: the marker
sequence `parse_markers` returns, and the string `update_regen` writes.

`update_regen` is then respecified over those extents rather than over a second search of its
own. Three things follow, and all three are defects closing rather than features opening:

1. It matches a command **exactly**, where it matched a prefix. `yidam count district` no longer
   finds `yidam count district[party=R]`.
2. It rewrites **every** block with that command, where it rewrote the first.
3. It and `scan_markers` can no longer disagree about what a block is, because there is one
   answer.

(1) and (2) are behaviour changes to a proved function, and §"Migration" states what they cost.

(3) lands in the code and not yet in the model, and this RFC says so rather than leaving it to
be found. `RegenSpan` is still a prefix search of its own that stops at the first hit, so on a
document holding a block for `ab` the model, asked for `a`, answers with that block where
`update_regen` answers with nothing.
[`TheModelsLocatorIsStillAPrefixSearch`](../../yidam/prelude/sdks/spec/graph.dfy#L620-L625) is
that document, proved, so the divergence is a checked statement in the file rather than a
comment. Closing it is one change — `RegenSpan` becomes a selection over a modelled scan, and
*the two agree* stops being a lemma nobody can write and starts being the definition — and the
cost is not the definition but `UpdateRegenSpec`'s re-scan clause, which then needs an induction
showing the scan of the result reproduces every block below the edit. It is tracked as #1097.

**It is a debt, not a blocker — corrected from an earlier draft.** This section first said the
model was not optional past `count`, on the reasoning that `count`'s command carries a query, so
`yidam count district` is a prefix of `yidam count district-at-large` and the document above
becomes one a corpus can write by accident. The premise is right and the conclusion was not: the
hazard is a property of the *old* locator, and clause (3) is what removed it. `update_regen`
matches by equality over the one scan and writes every match, so on the implementation `count`
ships against, neither the clobber nor the unwritten second block is reachable —
`a_query_that_prefixes_another_does_not_clobber_it` and
`two_blocks_asking_the_same_question_are_both_written` in `yidam/cli/tests/count.rs` are those
two documents, run. What #1097 still buys is a model that says so; what it does not buy is
safety `count` is waiting on.

### `count` is pull-shaped, and the guard says so

`count` reads the tracked markdown set — `tracked::list`, for the reason
[`unclaimed_blocks`](../../yidam/cli/src/cmd/regen.rs#L234) already gives at length — collects
every block whose command begins `yidam count `, runs each distinct query once, and writes each
block through the single write point.

[`every_generator_in_the_crate_is_listed`](../../yidam/cli/src/cmd/regen.rs#L441-L470) is
amended, not exempted. Its `named == call_sites` clause exists so that the guard can see which
block each call site writes; a call site whose command is computed is one the guard genuinely
cannot read, and the honest amendment is to require that such a site name its generator's
**prefix** as a literal, and to assert that the prefix is itself a listed generator name. A site
that names neither still fails. The floor clause is unchanged.

**Revised on implementation.** The prefix a call site can spell is not `"yidam count "` but
`format!("yidam count {}", argument)` — the command has to be built to be passed, and a site
that spelled the bare prefix beside a computed command would be naming a string it does not
use. So [`generator_named_by`](../../yidam/cli/src/cmd/regen.rs#L411-L417) reads a `"yidam …"`
literal containing a `{` as a prefix and truncates at the brace: `"yidam count {}"` names
`count`. The clause the RFC asked for is unchanged in effect — the site names its generator, and
the guard can attribute the block — and the literal it reads is now one the code actually
evaluates rather than one kept beside it to be read.

### The bare form is a command no generator writes

`<!-- REGEN: yidam count -->` names no query. No run can ever write it, and it is not `count`'s
to guess at — but registering `count` in `GENERATORS` would have made
[`unclaimed_in`](../../yidam/cli/src/regen.rs#L87-L101) match it whole and call it **claimed**.
The block would then have gone from correctly reported by #1062's gate to silently accepted,
which is the exact defect that gate exists to close, introduced by closing this one.

So a generator carrying an argument is claimed a second way and not the first.
[`PARAMETERISED`](../../yidam/cli/src/cmd/regen.rs#L94) lists `count` with its usage, and
[`claimable`](../../yidam/cli/src/cmd/regen.rs#L98-L110) partitions the fifteen: fourteen are
matched whole, `count` is matched only as its name followed by a space and an argument. **The
separator is the rule.** Without it `yidam counterexamples` would be claimed by `count`, which is
#1058's collision wearing a different hat.

The verdict's wording changed with it. `unclaimed` used to read *"name a generator that does not
exist"* and then print a list — self-contradictory the moment that list contains `count`, since
the generator plainly exists and the block is still not one it writes. It now reads *"name a
command no generator writes"*, and the list shows `count <query>` rather than `count`, so the
remedy for the bare block is visible in the message that reports it.

### A query that does not typecheck

`count` refuses the block, names the file, the query and the diagnostic, and exits 1 — the
report-then-fail shape `regen`, `doctor` and `query` already have. It does not write a `0`: a
query that cannot run has not counted zero of anything, and a gate whose failure mode is to
publish a plausible number is worse than no gate. The block keeps whatever it holds, and it
says which block and why. Every block is visited before the failure, so one bad query neither
hides the next nor costs the rest of the document its refresh.

**It fails as an error, not as a verdict — including under `--check`.** `regen` runs its
generators with `.with_context(|| format!("running {name}"))?`, so any generator returning `Err`
aborts the pass before a report is rendered; under `--format json` that means no RFC-0016 report
at all, only the anyhow chain on stderr. This is pre-existing structural behaviour rather than
something `count` introduces, but `count` is the first generator that can fail on **corpus
content** rather than on I/O, so it is the first time the path is reachable in ordinary use. A
machine consumer of `regen --check --format json` therefore has to read the exit code, not the
absent `passed` field. Widening the report to carry a third verdict is the open question below,
and this is the second reason to answer it.

## What this does not touch

**Number formatting.** The body is decimal digits. `roadmap.rs` needed a thousands-grouping
helper *and* an English-number helper to bind one repository's prose, so both forms are real;
neither is in this RFC. See §"Open questions".

**Counts that are not corpus counts.** `demi-moore`'s three drifted figures are crates,
connectors and invariant checks — facts about a workspace manifest and a Rust source tree, not
about `.yidam/corpus/`. A query cannot answer them and this RFC does not claim to. What it
removes is the corpus-shaped subset, which is the whole of #1071's case and part of the other
two.

**`<!-- TEMPLATE: … -->`.** The template marker is single-line already and gains nothing here.

**The `yidam ` prefix.** A block belonging to another program is still not yidam's to judge, and
an inline block belonging to another program is still not yidam's to write. #1058's namespacing
question is untouched and stays open.

## Migration & compatibility

**A derived repository does nothing.** No existing document changes, no existing block changes
shape, no corpus content moves. Every generator writes exactly what it wrote.

**Prose that shows the marker becomes prose that has one — measured.** Twenty-eight lines across
the fourteen repositories on disk carry an open tag and a close tag on one line, and every one of
them is the same vendored document: `yidam/prelude/sdks/README.md` and its thirteen
`.yidam/.vendor/` copies, describing the marker format inside a code fence. None of them begins
its line with the open tag, but the inline rule is not line-anchored — the mid-sentence case is
the point — so under this RFC they parse as blocks named `cmd` and `command`.

The harm is nil and is worth stating rather than assumed. `unclaimed_in` masks fenced code, so it
does not report them. They are well-formed, so `malformed-regen-block` has nothing to say. No
generator is named `cmd` or `command`, so nothing writes them. What changes is the marker
*sequence* `parse_markers` returns for one document that no generator regenerates. The template's
own copy is reworded so the count goes to zero going forward; the thirteen vendored copies fix
themselves on the next re-vendor, and cost nothing until then.

**Why the template bump is major.** VERSIONING.md's Layer 1 table calls a REGEN marker format
change major, and the instinct is to argue that this one is additive. The instinct is wrong, in
the direction that matters. The format is additive for *documents* and not for *readers*: a
document carrying an inline block, read by an SDK from before this change, is the destructive
case §"Problem" measures — silently, and against the reader's own repository. That is the
pattern where a new producer strands an old consumer, and the honest bump is the one that makes
a pin say so. Template `v0.8.0` → `v0.9.0`, following the bootstrap layer's `0.3.0` → `0.4.0`
precedent for a major in `0.x`.

**The two `update_regen` behaviour changes.** Exact-command matching can only stop a write that
was landing on the wrong block, and no generator name in this repository is a prefix of another —
checked over [`generator_names`](../../yidam/cli/src/cmd/regen.rs#L132), the fifteen are pairwise
non-prefixing. Writing every match instead of the first can only write a block that was stale.
Both are covered by new parity fixtures, and both are stated as postconditions in the model.

## Alternatives considered

**A test per figure, which is what three repositories did.** It works and it does not scale: each
figure is pointed at by hand, an unpointed figure is invisible, and `roadmap.rs` measured its own
blind spot at one figure in a narrative section. The deeper objection is `allen-county-ohio`'s,
which declined to pin node counts at all: *"a test reading `assert_eq!(marks, 129)` fails on the
next phase that adds a site, and a gate that fails for the wrong reason gets switched off."* That
objection is correct about tests and does not reach a REGEN block, because a block's remedy is
mechanical — `yidam regen` rewrites it, and the only thing CI asks is that the rewrite be
committed. The figure moving is not a failure; the figure moving *without the sentence moving* is.

**`class=district`, as filed.** A second way to name a set, unable to express three of the four
figures the reference gate binds. Declined in favour of RFC-0018.

**A block-shaped `count`, no marker change.** Ships in a day and answers table cells and list
items, which is where some of these figures live. It does not answer a number in a sentence,
which is where #1071's is, and it would leave the two-scanner disagreement in place for whoever
meets it next.

**Line-anchoring the inline form.** Requiring the open tag to begin its line gives zero false
positives against the twenty-eight lines measured above — and forbids the sentence the RFC exists
to write. Declined for the reason it was proposed: the position of the number in the sentence is
not the author's to negotiate.

**Masking fenced code inside `scan_markers`.** The alternative to living with the twenty-eight.
Declined: `mask_code` is a CLI concern, the parser is a parity function in three languages, and
the measured harm does not pay for a markdown-aware scanner in each of them.

## Open questions

**Grouping and words.** `roadmap.rs` carries `grouped()` for `3,104` and `number_word()` for
`twelve`, in one repository, for one document. A count generator that can write neither will be
adopted for the small figures and worked around for the large ones. The candidate is a modifier
on the command — `yidam count district --grouped` — and the question is whether the separator is
the author's (`,` versus `.` versus a space) and therefore a corpus declaration rather than a
flag. Not answered here; the two helpers are the evidence that it has to be answered.

**What `--check` says about a block whose query stopped typechecking.** Today `stale` and
`unclaimed` are the two verdicts and each has a remedy that clears it. A block whose class was
renamed out of the ontology is neither: `yidam regen` cannot fix it and the name is a real
generator. A third list, or `unclaimed` widened, or `rename` taught to rewrite a query — three
answers, none obviously right.

The implementation forces half an answer and leaves the other half open. `count` returns `Err`,
which `regen` propagates before rendering anything — so under `--format json` the run emits no
RFC-0016 report at all, and a machine consumer has to read the exit code rather than a missing
`passed` field. That is honest (the pass did not complete) but it is not a verdict, and it is
the shape every generator's I/O failure already had. Whichever of the three answers wins, it
also has to say whether a content failure stays an error or becomes a third list inside a report
that still renders.

**When the model reaches the scan.** §"One scanner" clause 3 landed in the code and not in the
model, and #1097 carries the rest: `RegenSpan` becoming a selection over a modelled `RegenScan`,
and `UpdateRegenSpec` gaining the induction that says the scan of the result reproduces every
block below the edit. It is not blocking — §"Two scanners, two answers" records why, and the two
documents the lemma describes are integration cases that pass. What is not settled is the price:
the lemma names the hazard and the definition is the cheap half, but nobody has costed the
induction, and until somebody does, "when" has no answer to give.

**Whether a block should be able to state its own expectation.** `<!-- REGEN: yidam count
district -->44<!-- /REGEN -->` publishes a number and asserts nothing about it. RFC-0035's
`coverage:` is the place a corpus says how many there *should* be, and a block that could say
`expect>=44` would be a gate rather than a report. That is a different surface and probably
RFC-0035's, but it is the first thing a reader asks.
