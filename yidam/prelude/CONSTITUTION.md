# Constitution of the Sangha

> **Scope.** This document governs *collective resolution* — the reconciliation of positions
> held by several electors. A repository bootstrapped as `governance: single-elector` has no
> sangha, no `ma/*` branches, and no resolution events, and this constitution is dormant in
> it: vendored with the rest of the prelude, binding on nothing, waiting for a second elector.
> Nothing here is a prerequisite for maintaining a corpus. If you are the only one holding
> positions in this repository, the articles below describe a mechanism you do not need —
> read [PHASES.md](PHASES.md) for how inquiry is bounded instead.

The prelude establishes how knowledge is maintained; this document establishes how the
sangha — the collective of participants who maintain it — governs itself. Every resolution
event is bound by these articles. Domain-specific procedure may vary; these constraints
may not be overridden by `PROTOCOL.md` or by any resolution decision.

The constitution may be extended for a specific domain by samudaya augmentations during
bootstrap. Augmentations add domain-specific articles; they may not contradict Articles I–VI.

---

## Article I — Primacy of the Prelude

The prelude is not subject to resolution. Its identity model, graph encoding, conduct norms,
and directory conventions are the ground on which the sangha operates. A resolution that
contradicts the prelude is invalid and must not produce a `rigpa/*` commit.

If the prelude itself must evolve, that is an yidam-level change — not a sangha resolution.

## Article II — Epistemic Equality

No elector's position is privileged by identity, seniority, or the model that produced it.
Human and agent electors are equal participants. Resolution is governed by coherence within
the corpus and fidelity to the domain, not by authority.

## Article III — Provenance

Resolution must preserve the ancestry of synthesized knowledge.

- The `rigpa/<evolution>` commit message must record which `ma/*` tips were read.
- Tensions that could not be resolved must not be silently discarded. Each unresolved tension
  becomes an open question node in the corpus, linked from the resolution commit.
- Source positions that were superseded remain in git history. Do not rewrite `ma/*` branches
  after a resolution; let the history stand as provenance.

## Article IV — Legibility

A resolution that cannot be described legibly must not proceed.

The `rigpa/<evolution>` branch name and commit message must state: what domain question was
resolved, what changed in the collective understanding, and what remains open. A resolution
event is a knowledge act — it must read as one in the graph.

## Article V — Scope Fidelity

Resolution may only synthesize knowledge present in the participating `ma/*` positions.
It may not introduce nodes, edges, or claims that were not held by at least one elector.
Resolution is synthesis, not generation.

**On entailment.** A claim is *held* only if it appears as committed content in some elector's
`ma/*` position. A claim that merely *follows* from held positions by inference or entailment is
not itself held, and introducing it at resolution is generation. Where held positions entail `C`,
the sanctioned move is to commit `C` to a `ma/*` branch — as an `[inference]` claim, attributed to
a voice — where a later evolution can recognize it. Entailment does not enter a rigpa anonymously.

**On identity.** The three objects of this article are not alike, and only two of them can be
decided. A node and an edge carry an identity of their own — a node id, and a subject, verb and
object — so *held by at least one elector* is set membership across the participating tips, and a
check may settle it. A claim carries no such identity: it is a statement in a node, held at a
standing, and whether two sentences assert the same thing is a judgement. This article leaves that
judgement with the synthesizer under Article II and does not delegate it to a checker.

What binds the synthesizer for a claim is therefore the standing, which is a closed vocabulary
rather than a matter of wording. A resolution may carry a claim at the standing an elector held it
at, or lower it — `[verified]` to `[inference]` to `[open]` — because declining to assert
introduces nothing. It may not raise one, and a claim asserted at a standing no participating
elector held is a new claim, taking the entailment route above.

**A class is not its instances.** A position arguing that a class should exist does not thereby
hold the instances of it. The class is what was argued; each instance is a new node, and a
resolution that seats instances alongside the class it adopts is introducing nodes no elector
held. If they are to enter the corpus with the resolution, an elector commits them first.

**One exception, and no others.** An open-question node standing for a tension that could not be
resolved is required by Article III and is held by no elector by construction — it exists because
the positions disagreed. Introducing it is not what this article forbids. No other exception is
licensed, by this constitution or by a domain extension.

## Article VI — Minimal Authority

The sangha exercises the minimum authority needed to produce a coherent shared baseline.
Positions that do not conflict are inherited by the rigpa branch as-is. Resolution focuses
on genuine tensions, not on imposing uniformity where none is needed.

An elector's `ma/*` branch may diverge freely from `rigpa/*` after a resolution. Divergence
is normal and expected; it is not a violation.

---

## Domain extensions

Bootstrap-time augmentations from `samudaya/` may append domain-specific articles here.
Extensions are committed into the derived repo during the genesis event and become part of
that repo's constitution permanently. They must be consistent with Articles I–VI.

Examples of valid extensions: quorum requirements for a specific domain's resolutions,
constraints on which node types may be resolved collectively vs. individually, additional
legibility requirements specific to the domain's communication norms.
