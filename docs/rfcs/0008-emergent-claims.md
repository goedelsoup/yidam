# RFC-0008 — Emergent claims and the scope of synthesis

- **Status:** Accepted
- **Track:** G1 (governance)
- **Relates to:** RFC-0009 (resolution execution authority), `CONSTITUTION.md` Article V, `sangha/PROTOCOL.md`
- **Versioning layers touched:** template (constitutional commentary)
- **Downstream reference case:** none — this is a sangha-layer constraint that applies to every derived repo, not a BOSC integration gap.

## Summary

Article V says a resolution "may only synthesize knowledge present in the participating `ma/*`
positions" and "may not introduce nodes, edges, or claims that were not held by at least one
elector." It is silent on two things, and this RFC settles both.

The first is **entailment**: elector one holds `A`, elector two holds `B`, `A ∧ B ⊢ C`, and no
elector holds `C`. May a resolution introduce `C`? This RFC ratifies the strict reading — **entailment
is generation until a voice holds it**. The round-trip of `C` through some elector's `ma/*` branch
before it can enter a rigpa is a feature, not friction: it keeps every claim attributable to a voice.

The second is **identity**: "held" presupposes a notion of *same claim*, and the first draft of this
RFC left it open, assuming "semantic identity judged by the synthesizer." That assumption is what
made Article V unmechanizable, and it does not survive measurement. **A claim has no identity a
checker can read; a node and an edge do.** Article V therefore splits: its node and edge clauses are
set membership over the participating tips and can be gated, and its claim clause binds the
synthesizer's conduct through the *standing* — a closed vocabulary — rather than through wording.

## Problem

The governing text ([`CONSTITUTION.md:55-57`](../../yidam/prelude/CONSTITUTION.md#L55-L57)):

> Resolution may only synthesize knowledge present in the participating `ma/*` positions. It may
> not introduce nodes, edges, or claims that were not held by at least one elector. Resolution is
> synthesis, not generation.

The protocol repeats it ([`PROTOCOL.md:137-138`](../../sadhana/sangha/PROTOCOL.md#L137-L138)):
"introduce only nodes and edges present in at least one elector's position." Both sentences quantify
over things that are *held*. Neither says whether a claim that is not held but *follows* from held
claims counts as present, and neither says when two statements are the same claim.

### Entailment

Two readings survive the text:

- **Strict.** A claim is present only if some elector committed it. Synthesis reconciles and
  combines held positions; it does not derive new ones. `C` is absent until authored.
- **Permissive.** `C` is "present in" the joint positions *by entailment* — the electors are
  committed to it whether they wrote it or not — so surfacing `C` is synthesis, not generation.

The ambiguity is load-bearing because resolution is the one moment where authority concentrates:
Articles III–V exist precisely to bound what the pen-holder may do at settlement. A synthesizer
empowered to introduce entailed claims is empowered to introduce a great deal — deductive closure
is unbounded, and *which* consequences look "obviously entailed" is model-dependent and unauditable.
Under the permissive reading a rigpa can carry claims that appear in no elector's history, defeating
Article III's promise that "resolution must preserve the ancestry of synthesized knowledge"
([`CONSTITUTION.md:37`](../../yidam/prelude/CONSTITUTION.md#L37)).

### Identity

Entailment can be settled by argument. Identity cannot, because it is a question about artefacts:
*given two commits, is this the same claim?* The first draft assumed the synthesizer would judge it
and noted that "a stricter syntactic rule may be wanted if claim-matching is ever mechanized."

That deferral is exactly what blocks a check. [#273](https://github.com/goedelsoup/yidam/issues/273)
proposes to trace every claim in a resolution to a participating tip and cites this RFC as having
already decided what *present in* means. It has not. Nothing can be built on the deferral, so the
question is settled below rather than left to be rediscovered inside an implementation.

## Proposal

### On entailment — ratify the strict reading

> **On entailment.** A claim is *held* only if it appears as committed content in some elector's
> `ma/*` position. A claim that merely *follows* from held positions by inference or entailment is
> not itself held, and introducing it at resolution is generation. Where held positions entail `C`,
> the sanctioned move is to commit `C` to a `ma/*` branch — as an `[inference]` claim, attributed to
> a voice — where a later evolution can recognize it. Entailment does not enter a rigpa anonymously.

This is deliberately the more constraining reading, and the constraint is the point:

- **Attribution survives.** The round-trip forces the derivation to be authored, tagged, and
  traceable. `[inference]` is exactly the marker the conduct norms already reserve for "a reasonable
  conclusion drawn from verified facts; not directly witnessed"
  ([`agent-conduct.md:42`](../../yidam/prelude/guidelines/agent-conduct.md#L42)). An entailed claim
  is an inference; it should wear the inference tag and carry an author.
- **No new generation surface at the one privileged moment.** The synthesizer's added power stays
  minimal (Article VI), and a contested entailment becomes an open-question node
  ([`CONSTITUTION.md:40-41`](../../yidam/prelude/CONSTITUTION.md#L40-L41)) rather than a silently
  asserted one.
- **The elegant reading is also the honest one.** "Synthesis, not generation" already leans strict;
  this commentary states what the sentence implies instead of leaving it to the pen-holder.

### On identity — the article's three objects are not alike

> **On identity.** The three objects of this article are not alike, and only two of them can be
> decided. A node and an edge carry an identity of their own — a node id, and a subject, verb and
> object — so *held by at least one elector* is set membership across the participating tips, and a
> check may settle it. A claim carries no such identity: it is a statement in a node, held at a
> standing, and whether two sentences assert the same thing is a judgement. This article leaves that
> judgement with the synthesizer under Article II and does not delegate it to a checker.
>
> What binds the synthesizer for a claim is therefore the standing, which is a closed vocabulary
> rather than a matter of wording. A resolution may carry a claim at the standing an elector held it
> at, or lower it — `[verified]` to `[inference]` to `[open]` — because declining to assert
> introduces nothing. It may not raise one, and a claim asserted at a standing no participating
> elector held is a new claim, taking the entailment route above.

The ordering `[verified]` → `[inference]` → `[open]` is established by this commentary, not
inherited: [`agent-conduct.md`](../../yidam/prelude/guidelines/agent-conduct.md) defines the three
tags and ranks them for no purpose. Ranking them here is narrow and only for resolution — a claim
carried at a weaker standing asserts less than the elector did, and Article V bounds what a
resolution may *add*.

Two further paragraphs close the cases the measurement below turned up: **a class is not its
instances**, and the open-question node Article III *requires* is the one exception, with no others
licensed. Both are in the constitution beside the text above.

The protocol had already reached half of this on its own without anyone noticing: its restatement of
Article V binds "nodes and edges" and drops claims. That is not a transcription error to correct
upward — it is the mechanizable half, written by someone applying the article rather than quoting it.

## What decided the identity question

Measured 2026-09-04, read-only, against **repository A** of
[`post-genesis-measurement.md`](../post-genesis-measurement.md) — the only derived repository known
to have run a sangha, at **29 resolutions and 70 recorded `ma/*` tips**. Historical states were read
with `git ls-tree` / `git show`, so nothing touched its working tree. Claims were extracted by a
port of `claims.rs`'s prose pass, validated by reproducing `yidam status`'s claim tally on that
corpus exactly (549 `[verified]` / 126 `[inference]` / 251 `[open]` over 106 nodes).

**1. Every recorded tip resolves.** 70 of 70 `ma/<elector>@<short-hash>` references in the records
name a commit that exists, and 27 of 29 `rigpa/<evolution>` branches are still present. Tracing a
resolution to its participating tips is mechanically available; that was never the obstacle.

**2. Surface-form identity is not merely strict, it is unsound.** The corpus hard-wraps prose, and
`statement_around` ends its search for a sentence's start at a newline. So the same claim, written
into two nodes with the line breaks falling differently, extracts as two different strings — one of
them a fragment beginning mid-clause. Identity by surface form would therefore be identity by *line
wrapping*, which no author is choosing and no rule should turn on. Schematically:

```yaml
# node-a.yml — wraps before the clause
  … the whole membership is printed alongside both tallies, so the figure is computed
  from the cited document rather than supplied. [verified]
# node-b.yml — wraps after it
  … the whole membership is printed alongside both tallies,
  so the figure is computed from the cited document rather than supplied. [verified]
```

One claim. Two extractions. (`statement_around`'s newline boundary was a defect in its own right,
filed as [#562](https://github.com/goedelsoup/yidam/issues/562) and since fixed: it is documented as
returning "the sentence a marker sits in" and did not, for any corpus that hard-wraps. **Nothing in
this finding turns on that fix.** A statement bounded by blocks rather than by line breaks is a
whole sentence, which is a better answer to *what does this node assert* and still not an identity —
finding 3 is measured on the wording, and synthesis restates.)

**3. Nothing matches anyway.** Across the 29 resolutions, 22 claims entered the corpus in a
resolution commit. **Not one** of them appears byte-identically at any participating tip — not in
that tip's corpus and not in its position files. Resolutions author corpus prose at synthesis; the
positions argue in their own words. A strict syntactic rule would flag 22 of 22.

**4. And claims are not where resolutions act.** 21 of the 29 resolutions introduced no new claim at
all. What they change is nodes, edges, class definitions and ontology text — objects that do have
identities. A check built on the claim clause would therefore have nothing to say about 21 of 29
settlements, while spending its whole design budget on the half of the article that cannot be
decided.

**5. The formal spec had already answered the question, by construction.**
[`sangha.dfy`](../../yidam/prelude/sdks/spec/sangha.dfy) proves that union synthesis satisfies
Article V, over `datatype Claim = Claim(text: string)` — so a claim *is* its text there, and
`ArticleV` is decided by string equality. The theorems are sound; they are about a sequence of
values with decidable equality, and nothing above disturbs them. But the type is the surface-form
rule findings 2 and 3 reject, sitting unremarked in the one artefact that looks like it settles
the question. A note now says so beside the datatype. This is the shape worth remembering: the
identity question was never open in the sense of unanswered. It was answered three ways in three
places, none of them reading the others — this RFC deferred it to the synthesizer's semantic
judgement, the spec decided it by string equality, and the protocol sidestepped it by binding only
nodes and edges. The protocol was right.

**6. The node clause has teeth, and it is not vacuous.** Three resolution events seated **4 corpus
nodes that no participating tip held**. Under the strict reading those are Article V violations, and
they are what a gate on the node clause would report. They are also not obvious to a reader — in the
clearest case both electors argued for a *class* and the resolution seated the class's first
instances alongside it. That is the rule the constitution now states explicitly, because a check
that discovered it by itself would be inventing a rule rather than enforcing one.

## Migration & compatibility

Template-layer change to [`CONSTITUTION.md`](../../yidam/prelude/CONSTITUTION.md) Article V:
commentary appended beneath the article, whose own three sentences are untouched. Derived repos
adopt it by re-vendoring. No SDK or parity change — this is a rule about resolution conduct, not a
parser. Because the constitution "may not be overridden by `PROTOCOL.md` or by any resolution
decision" ([`CONSTITUTION.md:14`](../../yidam/prelude/CONSTITUTION.md#L14)), ratifying this as
commentary — not as domain protocol — makes it invariant across derived repos.

**Bump size is genuinely ambiguous and the ambiguity is named here rather than resolved quietly.**
[`VERSIONING.md`](../../VERSIONING.md) puts "constitutional revision" under **major** and "typo or
documentation fix in prelude" under **patch**. This change revises no article's text; it states what
Article V already meant and says which half of it is decidable. That reads as a patch, and it is
what this RFC proposes. Anyone who reads "constitutional revision" as covering commentary should say
so before the next template tag, not after.

**Composition direction, settled here for the constitutional family.**
[RFC-0024](0024-policy-as-code.md#composition-and-the-one-decision-this-rfc-defers) fixed the policy
layer's direction as *authoritative* for disclosure and deferred the constitutional family's,
naming **tighten-only** as what it would need and leaving open "where that gets declared — in the
policy itself, or in the Rust that owns the family." [#253](https://github.com/goedelsoup/yidam/issues/253)
asked for the direction to be settled before any constitutional decision was written rather than
discovered inside one, so it is settled here and in the third place neither considered: **in the
article**. The commentary states it in the form an extension has to obey — an augmentation may add
refusals to Article V and may not remove one, and the open-question node is the only carve-out this
article licenses. Declaring it in the text rather than in an engine costs nothing and binds a
derived repository that never installs a policy engine at all. Where the *machinery* lives is still
RFC-0024's question.

**No rubric line.** An earlier draft proposed one. The rubric evaluates a *bootstrap* result, and a
genesis event has no resolutions to check — the line would have had nothing to fire on.

## Alternatives considered

- **Permissive reading of entailment.** Rejected: unbounded, unattributed, model-dependent. It hands
  the synthesizer a generation power the rest of the system is built to deny, and it puts claims in a
  rigpa that trace to no `ma/*` tip — the exact provenance loss Article III forbids.
- **"Single-step entailment only."** A middle rule permitting only immediate consequences. Rejected:
  the line between one step and many is arbitrary and itself model-dependent, and the claim is still
  unattributed. If it is worth asserting, it is worth a voice committing it.
- **Claim identity by surface form.** Rejected on finding 2: the extracted text is a function of
  where the source wraps, so the rule would key on a decision no author makes. Finding 3 says what it
  would cost even if it were sound — every claim a real resolution introduced, flagged.
- **Claim identity by normalized or fuzzy match.** Rejected without measuring, because the failure
  mode is worse than either exact rule: a threshold that is right on one corpus is a knob, and a gate
  with a knob is argued with rather than fixed. `resolution-annotation-decides` is already `Warn`
  rather than `Error` for the same reason — it is a heuristic over wording, and "gating on it would
  make every false positive a blocked commit and the check would be switched off within a week."
- **Claim identity by carrier — the node and property a claim sits on.** Rejected as too weak on its
  own: it would let a resolution replace the content of a claim on an existing node with anything at
  all. What survives of it is the standing rule above, which binds the part of a claim that *is* a
  closed vocabulary.

## Open questions

- **Recording vs. re-opening.** When held positions entail an important `C`, is an open-question node
  sufficient, or should the protocol *require* the synthesizer to open a `ma/*` position so the
  entailment is carried as a real, resolvable voice rather than a note? Left open pending RFC-0009's
  decision on who executes a resolution.
