# Agent Conduct — evidence

Why each rule in [agent-conduct.md](agent-conduct.md) says what it says: the incident that
produced it, the measurement that set its threshold, and the failure it was built against.

**This file is not part of the recurring read**, and bootstrap does not read it — step 1 of the
bootstrap skill names the files to internalize and forbids the rest of the prelude, which
includes this one. Read a section when you are deciding a hard case the rule does not settle,
arguing that a rule is wrong, or changing it. One section per rule, named by the rule's `[why]`
link.

A rule is not weaker for having its reasoning here. It is the same reasoning, in the place a
reader goes when the rule alone is not enough — which is the minority of the time, and was
costing every reader the whole essay every time.

## claim-tag-malformed

`[verified — Pearl 2009]` is not a tag, and the reason is mechanical: the counters match the
three tokens exactly, so a bracketed form that folds a citation inside matches nothing and the
claim is counted as untagged. It looks tagged to a reader and reads as bare assertion to every
tool — the worst of the two, because nothing reports a disagreement between them.

## verified-unsourced

The rule that a `[verified]` claim needs a source is the first line of the claim-confidence
section, and until this check existed nothing echoed it back: `catalog/` recorded provenance and
three checks verified the catalog's own bookkeeping, while no check asked whether a claim rested
on anything at all.

Over-counting evidence is the flattering error and it is the one this vocabulary exists to
prevent. A mature corpus measured eight miscounts and **every one of them promoted** — which is
why the remedy is stated as a choice between a citation and a demotion, and why nothing in the
tooling proposes the promotion.

## naming-a-tag

A node whose subject touches the evidence vocabulary has to write the tokens in order to talk
about them, and a scanner reading bytes cannot tell that from an assertion. Hence a rule, and
hence a rule about grammar rather than about typography.

**Backticks decide nothing, and this rule used to say the opposite.** A derived corpus writes 80%
of its `[open]` claims in inline code — an open question is written mid-sentence with the token
set off from the prose around it, *"whether the two are connected is `[open]`"*, while
`[verified]` ends a sentence of fact and reads fine bare. Honouring the backtick made that
repository understate its open questions **fivefold** on its own front page, with no diagnostic.
The tags exist so a corpus cannot overstate what it knows, and that is the one direction the rule
must never fail in.

The present tense and the copula were both cut from the rule by measurement, at eight false
positives between them: *"this node now carries `[open]`"* applies a tag rather than narrating
one, and *"why the appointment was made is `[open]`"* is a live claim.

The rule is frozen alongside the `open_questions` arms in `sdks/parity/mcp/tools.json` because a
contract that says which arms exist and leaves *what counts as a claim* unsaid lets two
conforming implementations disagree fivefold — which is exactly what happened.

## edge-is-a-claim

`a →[requires]→ b` asserts that a requires b as flatly as a sentence would, and it asserts it in
the form a reader is least likely to check: the target exists, the relationship is in the class's
declared vocabulary, and the graph gate passes. Nothing about a well-formed edge indicates whether
it is true.

A missing edge is a gap someone can find and fill, while a wrong edge is knowledge the corpus now
asserts — and every correct edge around it lends it credibility. `relates-to` between two things
that genuinely relate is worth more than `causes` between two things that might not, because the
first is honest about how much it knows.

## edge-claim-keys

Saying what an edge rests on *in the node body* was the only remedy this section could offer for
as long as a link had nowhere to put a tag, and it puts the tag where nothing associates it with
the edge it is about — a node whose description says in so many words that a `resided-in` edge is
a legal inference, with the edge two lines below saying nothing at all.

## edge-claims-opt-in

Unconditionally, `edge-untagged` would open with one finding per edge in the graph — a gate
arriving in a corpus that never agreed to it.

The two keys are separate on purpose. Naming the verbs that are bookkeeping is a fact about a
vocabulary, and recording that fact must not switch a gate on as a side effect; a corpus that
wants to write its structural verbs down should not thereby acquire an error class it never asked
for.

## edge-standing-unheld

`verified` across a relation between two nodes this corpus grades `[open]` claims more about the
relationship than the corpus claims about either end of it.

The standing compared is the node's declared claim-typed field, not the weakest marker in its
prose, because a synthesis node carries all three tags by design — grading it by its prose would
report every synthesis node in the corpus. A node that declares no claim-typed field has no
standing and is compared to nothing.

The check is one-directional because an `open` edge between two `verified` nodes is not a defect:
it says the corpus knows both things and not that they are related, which is what the vocabulary
is for.

## edge-tags-reported-beside

An edge tagged `open` is an open question in its own right, because the triple is what it
asserted, and it is addressed by that triple.

The counts are reported **beside** the node ones rather than added to them: a node's claims are
measured over its text, an edge is in no node's text, and one figure over two denominators answers
a question nobody asked.

The structural exemption in `edge_claims` is an exemption from being *asked* for a standing. An
edge that writes one is read on it either way.

## claim-typed-field

Without a declaration the only thing readable is the bracketed token in a file's bytes — which
makes "is this node open?" a property of the node's *serialization* rather than of the node. A
consumer with a typed vocabulary found this by running the binary over its own mirror: **2 open
questions reported against its own count of 26.** The other 24 said they were open, in a
machine-readable field, and were counted as nothing.

It could not be fixed by matching a bare `open` under any key, either: a node with `status: open`
would become an open claim, and no corpus could opt out of that. So the corpus names the field,
and only that field is read.

## provenance-not-confidence

The case that forces the distinction: the source of record is unreachable — the filing authority
blocks automated clients, the publisher is offline — and an aggregator or republisher carries the
same figures. A republished bulk file is closer to the source than a derived presentation is, and
it is still not the source.

Making it unrepresentable rather than advisory is worth more than a paragraph telling agents to be
careful. A provenance type whose aggregator variants cannot produce the stronger tag, and a test
asserting it, is the shape:

```rust
impl Provenance {
    /// False for every aggregator kind. `[verified]` is about provenance, and an
    /// aggregator is not the source of record however accurate its figures are.
    pub fn supports_verified(&self) -> bool { ... }
}
```

## class-asserts-purpose

A class definition is the meaning every instance takes on by being filed under the class —
asserted identically, silently, untagged, and for each.

The worked case: a class defined at genesis as "a procedural mechanism **deployed to obtain** an
outcome the ordinary path would not yield," in a corpus whose first evidentiary rule was *attribute
intent, never assert it*. Every instance asserted a purpose by existing. It survived five
resolutions and three arguments about its instances, because every safeguard was pointed at
instances.

`class-claim-uncounted` is reported because nothing *counts* a class's tags. `yidam status` counts
claims in nodes, and a class is not one.

## reference-class

This corollary is the one that costs something, and it was learned by audit: of six nodes written
under the base-rate rule, **five had no computable denominator, and four of those failed the same
way.**

The failure is seductive because it feels like diligence. You gather the cases resembling the one
at hand until a fraction appears — but the filters get chosen *after* the outcome is known, so the
class ends up holding only cases that could have come out the way this one did.

A fabricated denominator is worse than none because it launders the sequence into arithmetic: the
reader who would have discounted a bare sequence credits a rate.

## outbound-claims

A repository whose output is internal is checked by the gate; this section exists because a
repository that publishes is not, and the derivation is where the tags stop being enforced.

Declared tiers drift the moment a supporting node is revised, which is why the tier is computed —
a downgrade upstream then propagates on the next build rather than waiting for somebody to notice.

Citing a span does not verify the inference and nothing can. What it buys is that the actual
sentence sits beside the assertion, where the gap between them is visible to a reader.

The refusal rule is invisible to the tag apparatus, and that is why it is stated separately: the
case that produced it was a node stating a fact at `[verified]` and refusing the inference from it
one sentence later. A tag-only gate passes that. **Those refusal sentences are among the most
valuable text a corpus holds, and until something reads them, nothing does.**

## foreign-nodes

An edge is a claim and the constitution governs who may assert one; a citation into a corpus with
a different ontology, its own electors, and its own revision history is a different object. That
is why the boundary is enforced in the tooling rather than left to conduct — and why the rule
spends its words on what you do instead.

## foreign-tags

A bundle carries `corpus/`, `skills/` and `decisions/` — no sangha, no elector register, no
resolution history. You receive conclusions without the apparatus that made them accountable,
which is why a foreign tag is recorded and never transferred: there is nothing on your side of the
boundary that could check it.

## span-is-load-bearing

A node reference alone rots invisibly, because the node keeps its name while its content is
rewritten and the citation still resolves. The span is the only check that survives this boundary,
because it needs nothing from the producer — and the producer's apparatus is exactly what a bundle
does not carry.

`commit` is recorded rather than enforced because a producer cutting a release must not be able to
turn your build red.

## local-citation

Measured across seventeen derived corpora, 86% of the markdown links that reach a corpus node are
written from one node to another — and until this shape was given a meaning, none of that could be
said in a form a gate could read.

`tag` is the field that differs from the external form. Across a boundary a foreign tag is the
producer's, recorded and never transferred, and no gate can check it. **Inside one corpus the
producer is you**, so a citation declaring `[verified]` over a paragraph this corpus tags
`[inference]` says something its own corpus denies.

Writing one is opt-in, and that is what makes the four checks that read them errors rather than
warnings: every one has an empty population in a corpus that writes no `cites:` at all.

## no-forum

A settlement reaching the other corpus would bind a sangha that never seated it — its electors did
not register here, filed no positions, and read no tips — which is what Article I forbids, pointed
sideways. `CONSTITUTION.md` Article V says a resolution may synthesize only what a participating
`ma/*` position held, and across this boundary there are no participating positions at all.

A corpus is accountable for its own nodes to its own electors. Two honest records that disagree are
two independent inquiries, which is the normal condition and not a defect in the model.

The prelude case works because the relation is constitutive, there is a delivery channel, and one
party has standing over both ends. A peer corpus gives you none of the three, and no mechanism
manufactures the third.

> **Settled 2026-09-22.** *No forum* is the answer and not a reading of the current count: what is
> missing across this boundary is a party with standing over both corpora, and scale does not
> produce one. The occasion that would re-open it is a concrete one — **the first corpus that cites
> another, disagrees with it, and finds that both sides want it settled and cannot.**

## cannot-go-silent

The failure worth fearing across this boundary is your corpus going on denying a sentence the
other side has since rewritten, withdrawn or demoted — and every form of it is already reported.

## contradiction-unmarked

The field that would say so — `rests-on` against `contradicts` — is worth having only because span
drift reads in opposite directions for the two, and across every derived corpus measured on
2026-09-22 there are **2,770 instance nodes and not one `cites:` block**. Adding a field to a
family with no subjects is how this layer has shipped surfaces nobody uses. Build it the day a real
citation disagrees, for that reason.

## selection

This is the finding that generalizes furthest and the one easiest to feel exempt from.

Motivated reasoning does not produce untagged inference. It produces a claim that is true,
correctly sourced, correctly tagged, and standing in front of the twenty that went unmentioned.
Every check passes. The corpus is wrong anyway.

The repository that found this had caught eight errors from the inside and **not one of them was a
selection error** — all eight were caught by an elector who wanted nothing. Selection is invisible
to a gate because a gate reads what is there.
