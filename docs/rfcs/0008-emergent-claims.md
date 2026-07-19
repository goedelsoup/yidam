# RFC-0008 — Emergent claims and the scope of synthesis

- **Status:** Draft
- **Track:** G1 (governance)
- **Relates to:** RFC-0009 (resolution execution authority), `CONSTITUTION.md` Article V, `sangha/PROTOCOL.md`
- **Versioning layers touched:** template (constitutional commentary + bootstrap rubric note)
- **Downstream reference case:** none — this is a sangha-layer constraint that applies to every derived repo, not a BOSC integration gap.

## Summary

Article V says a resolution "may only synthesize knowledge present in the participating `ma/*`
positions" and "may not introduce nodes, edges, or claims that were not held by at least one
elector." It is silent on **jointly entailed** claims: elector one holds `A`, elector two holds
`B`, `A ∧ B ⊢ C`, and no elector holds `C`. May a resolution introduce `C`? This RFC ratifies the
strict reading — **entailment is generation until a voice holds it** — and closes the gap with one
sentence of Article V commentary. The round-trip of `C` through some elector's `ma/*` branch before
it can enter a rigpa is a feature, not friction: it keeps every claim attributable to a voice.

## Problem

The governing text ([`CONSTITUTION.md:45-49`](../../yidam/prelude/CONSTITUTION.md#L45-L49)):

> Resolution may only synthesize knowledge present in the participating `ma/*` positions. It may
> not introduce nodes, edges, or claims that were not held by at least one elector. Resolution is
> synthesis, not generation.

The protocol repeats it ([`PROTOCOL.md:42-44`](../../sadhana/sangha/PROTOCOL.md#L42-L44)):
"introduce only nodes and edges present in at least one elector's position." Both sentences quantify
over claims that are *held*. Neither says whether a claim that is not held but *follows from* held
claims counts as "present in the participating positions." Two readings survive the text:

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
([`CONSTITUTION.md:29`](../../yidam/prelude/CONSTITUTION.md#L29)).

## Proposal

**Ratify the strict reading.** Add a commentary paragraph beneath Article V:

> **On entailment.** A claim is *held* only if it appears as committed content in some elector's
> `ma/*` position. A claim that merely *follows* from held positions by inference or entailment is
> not itself held: introducing it at resolution is generation, not synthesis. If a synthesizer sees
> that held positions entail `C`, the sanctioned move is to commit `C` to a `ma/*` branch — as an
> `[inference]` claim, attributed to a voice — where it can be recognized in a later evolution, or
> to record it as an open-question node in this one. Entailment does not enter a rigpa anonymously.

This is deliberately the more constraining reading, and the constraint is the point:

- **Attribution survives.** The round-trip forces the derivation to be authored, tagged, and
  traceable. `[inference]` is exactly the marker the conduct norms already reserve for "a reasonable
  conclusion drawn from verified facts; not directly witnessed"
  ([`agent-conduct.md:42`](../../yidam/prelude/guidelines/agent-conduct.md#L42)). An entailed claim
  is an inference; it should wear the inference tag and carry an author.
- **No new generation surface at the one privileged moment.** The synthesizer's added power stays
  minimal (Article VI), and a contested entailment becomes an open-question node
  ([`CONSTITUTION.md:32-33`](../../yidam/prelude/CONSTITUTION.md#L32-L33)) rather than a silently
  asserted one.
- **The elegant reading is also the honest one.** "Synthesis, not generation" already leans strict;
  this commentary states what the sentence implies instead of leaving it to the pen-holder.

## Migration & compatibility

Template-layer patch. Append the commentary to
[`CONSTITUTION.md`](../../yidam/prelude/CONSTITUTION.md) Article V; add one bootstrap-rubric line so
a resolution introducing an unheld claim is flagged the way an orphan node is. No SDK or parity
change — this is a rule about resolution conduct, not a parser. Existing resolutions are unaffected
(none exist yet; `electors.md` lists none). Because the constitution "may not be overridden by
`PROTOCOL.md` or by any resolution decision" ([`CONSTITUTION.md:5-6`](../../yidam/prelude/CONSTITUTION.md#L5-L6)),
ratifying this as commentary — not as domain protocol — makes it invariant across derived repos.

## Alternatives considered

- **Permissive reading.** Rejected: unbounded, unattributed, model-dependent. It hands the
  synthesizer a generation power the rest of the system is built to deny, and it puts claims in a
  rigpa that trace to no `ma/*` tip — the exact provenance loss Article III forbids.
- **"Single-step entailment only."** A middle rule permitting only immediate consequences. Rejected:
  the line between one step and many is arbitrary and itself model-dependent, and the claim is still
  unattributed. If it is worth asserting, it is worth a voice committing it.

## Open questions

- **Claim identity.** "Held" needs a notion of *same claim*. Is a paraphrase or normalization of the
  same proposition held, or must the surface form match? This RFC assumes semantic identity judged by
  the synthesizer, but a stricter syntactic rule may be wanted if claim-matching is ever mechanized
  (it touches the node-model surface in RFC-0002/RFC-0013).
- **Recording vs. re-opening.** When held positions entail an important `C`, is an open-question node
  sufficient, or should the protocol *require* the synthesizer to open a `ma/*` position so the
  entailment is carried as a real, resolvable voice rather than a note? Left open pending RFC-0009's
  decision on who executes a resolution.
