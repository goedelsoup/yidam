# RFC-0011 — Partial-sangha resolutions and participant-scoped binding

- **Status:** Draft
- **Track:** G4 (governance)
- **Relates to:** RFC-0010 (explicit baselines), RFC-0009 (participants = tips read),
  `CONSTITUTION.md` Articles II / VI, `sangha/PROTOCOL.md`, `docs/sangha-resolution-flow.md`
- **Versioning layers touched:** template (protocol commentary); reuses RFC-0010's baseline
  declaration
- **Downstream reference case:** one derived repository, 29 resolutions — measured, see below.

> **Decided 2026-09-04: the declaration binds, not the merge.** Proposal 1 is settled in the form
> below and the rule now lives in `sangha/PROTOCOL.md`, enforced at Info by
> `elector-holds-unadopted`. Every RFC in this repository carries `Status: Draft` and there is no
> `Accepted` state to move to, so settlement is recorded here rather than in the field — inventing
> a lifecycle for one RFC would say more about process than about this decision. The three
> questions the amendment added are answered under **What this decides**, below.
>
> **Amended 2026-09-04.** The premise holds and is stronger than the original claimed: partial
> resolutions are not merely permitted, they are **9 of 29** in the one repository running a sangha,
> and the repository's owner is the abstainer in four of them. What does not hold is the closing
> claim that scoped binding *"forecloses the failure mode where a subset silently rebases the
> collective."* It forecloses nothing. **All nine partial resolutions already bind their abstainers**
> — absorbed through `merge main`, with no adoption act and nothing recording that it happened. The
> proposal below is therefore a change to what is *true*, not a guard against something that might
> become true, and it has to reckon with a workflow already going the other way.

## Summary

A subset of electors can resolve while others abstain, and the constitution never says whether the
resulting rigpa binds the abstainers' baseline. This RFC proposes: a partial resolution is
baseline-of-record for its **participants only**; abstainers keep their prior baseline until they
**adopt**, and adoption is an explicit act — RFC-0010's baseline declaration supplies it. Two RFCs,
one field.

**The measurement changes what that proposal costs.** It is not a rule that formalises current
practice; it contradicts it. Nine of nine abstainers are bound today, silently, by the ordinary
`merge main` the protocol prescribes for a different purpose.

## What was measured

One derived repository: 29 resolutions, 3 registered electors, 126 elector commits, read-only.

**1 — Subsets are the real case, not a hypothetical.** Of 29 resolutions, **9 read fewer than the
seats registered on that date**. Discounting the 12 that predate the third elector's registration,
this is a live pattern rather than a startup artifact — and it runs both ways: `ma/advocate`
abstained from 5, `ma/goedelsoup` — the owner — abstained from 4.

**2 — Every abstainer is already bound.** For all **9 of 9**, the abstaining elector's branch
contains the settlement. Six of them also contain the `rigpa/*` tip; the other three contain the
settlement through `main` while the branch was never merged. There is no case of an elector holding a
baseline that excludes a resolution they sat out.

**3 — It happened without an adoption act.** Across 126 elector commits: **1** `adopt:` commit, **72**
`merge main` commits. The mechanism that bound the abstainers is the same merge the protocol
prescribes in step 3 for *reading what the others filed* mid-loop.

**4 — And the gate cannot see it.** Those merge subjects are authored (`merge main — the baseline
through property-claims`), so they are not the git-generated form the commit check exempts. They are
outside the closed vocabulary — `no-verb` — and `main..ma/auditor` reports **30** such findings. The
default range never walks an elector branch, so a workflow used 72 times is invisible to CI.

## Problem

**Subsets are normal.** Resolution triggers on a question "sufficiently explored across ≥2 `ma/*`
branches"; nothing requires the whole sangha, and the "participating `ma/*` branches" phrasing
throughout presumes non-participants exist.

**Binding is unaddressed, and the documents cut both ways.**

- *Against* global binding: Article VI — the sangha "exercises the minimum authority needed",
  "positions that do not conflict are inherited", resolution "focuses on genuine tensions, not on
  imposing uniformity" — and an elector's branch "may diverge freely from `rigpa/*`... Divergence is
  normal and expected; it is not a violation."
- *Toward* global binding: "The new `rigpa/<evolution>` is the active baseline", read as one global
  fact.

**And practice has already decided, without deciding.** The reading that won is global binding, by
default, through a merge nobody performs *as* an adoption. That is the worst of the three outcomes:
it is not the scoped rule Article VI argues for, it is not a declared quorum rule either, and it
leaves no record of having happened.

## Proposal

**1 — Binding is participant-scoped.** A rigpa is baseline-of-record only for the electors whose tips
it read. Abstainers are not bound; their declared baseline (RFC-0010) is unchanged by a resolution
they were not part of. This is Article VI applied literally, and Article II denying a subset the
power to impose a baseline on the whole.

**2 — Adoption is an explicit act.** An abstainer adopts by re-declaring their baseline to it. Until
then they keep their prior baseline.

**3 — Not bound ≠ invisible.** The rigpa is a permanent, legible evolution for the whole sangha:
anyone may read, cite, adopt or build on it. "Not your baseline yet" is a statement about
*measurement*, not visibility.

**4 — What to do about the nine.** They stand. A resolution absorbed a year ago is not undone by a
rule written today, and rewriting elector history to un-adopt them would breach Article III far more
seriously than the silent adoption did. What the rule changes is what happens next, and the honest
migration is that the declaration starts from where each branch is.

## What this decides

**Merging `main` binds nobody.** It is how the corpus stays shared and step 3 requires it mid-loop,
so it cannot be the act that binds without making the ordinary workflow illegal. The `Baseline:`
trailer from RFC-0010 is the adoption act, and it is the only thing that binds.

That is what makes scoped binding enforceable in a repository with a shared `main`, and it costs
the nine nothing: they hold what they hold, no history is rewritten, and the rule changes only what
being *measured against* something means.

**An abstainer's baseline is what they last declared.** A branch that holds a settlement it has not
declared is measured against an older evolution while its tree carries a newer corpus. **That is an
expected state, not a defect** — it is what every branch is in today, and saying so plainly is
cheaper than leaving a reader to wonder.

**Holding without adopting is reported, at Info.** `elector-holds-unadopted` names, per elector,
the resolutions its branch holds, took no part in, and its declaration does not reach. Info because
none of them is wrong: an elector who has absorbed a resolution and not declared it has done
nothing the protocol forbids. It is a prompt to decide, and it clears by declaring.

**And a resolution older than the seat is inherited, not abstained from.** An elector cannot have
sat out a settlement that predates their registration, so the report is bounded by the commit that
first put their branch in `electors.md`. Measured, this is not a rounding detail: it takes one
elector from 13 reported resolutions to 4.

One case the boundary decides that neither RFC anticipated. PROTOCOL allows registering an elector
*in the first resolution they participate in*, and the measured repository did exactly that once —
`ma/advocate`'s registration and the `electoral-purpose` settlement are **the same commit**. That
resolution's `tips:` do not name the new seat, so a naive reading calls it an abstention. It is not:
you cannot have sat out the resolution that seated you.

## What this proposal had to answer, and did not

The original treated scoped binding as the obviously-correct reading with no cost. Measurement says
it has one, and the RFC is not ratifiable until it is answered:

- **`merge main` binds you to everything on `main`.** An elector merging the baseline picks up every
  settlement, including resolutions they abstained from. Under scoped binding, is that merge now
  *wrong*? It cannot be — it is how the corpus stays shared, and step 3 requires it mid-loop. So
  either the declaration is what binds (and the merge does not), or scoped binding is unenforceable
  in a repository with a shared `main`. **Lean: the declaration binds.** Holding a commit is not the
  same as being measured against it, and separating those two is the whole content of RFC-0010.
- **Then what is an abstainer's baseline, concretely?** If they hold the settlement but have not
  declared it, they are measured against an older evolution while their working tree contains a newer
  corpus. That is a coherent state and an odd one, and the protocol should say plainly that it is
  expected rather than leaving a reader to wonder whether it is a defect.
- **Is a non-participant who holds the settlement worth reporting?** It is decidable — 9 of 9 today —
  and reporting it is not the same as calling it wrong. Lean: report it once the declaration exists,
  so that "bound without adopting" is visible; before then there is nothing to compare against and
  the finding would be every abstainer, every time.

## Migration & compatibility

Template-layer protocol commentary; no new schema — it reuses RFC-0010's baseline declaration. Reword
PROTOCOL's "the active baseline" to name participants, and add an adoption note. **29 resolutions and
9 already-bound abstainers exist**; the original's *"existing resolutions are unaffected (none
exist)"* was written before any did.

## Alternatives considered

- **Global binding by default.** Rejected as a *rule*, and it is what practice does. A subset
  imposing a baseline on non-participants contradicts minimal authority (VI) and epistemic equality
  (II). If a domain wants it, it should say so — see quorum below.
- **Rigpa is purely advisory.** Rejected: the participants who synthesized want a shared baseline;
  producing one is the point of resolving.
- **Quorum makes binding global.** Offered as an opt-in: a domain may add a quorum article so a
  resolution meeting quorum binds the whole sangha. Given that 9 of 9 partial resolutions already
  bound everyone, a domain adopting quorum would be *ratifying its own practice* rather than changing
  it — which is a legitimate thing for a domain to do and an argument for making the extension easy
  to reach for.
- **Leave it undecided.** Rejected, and this is the alternative measurement kills. Undecided did not
  produce caution; it produced global binding with no record.

## Open questions

- **Later conflict.** If an abstainer's un-adopted position conflicts with the participant baseline at
  a subsequent resolution, is that an ordinary tension? Lean: yes — it becomes an open-question node
  or a new synthesis like any other.
- **Stale baselines.** Does a long-abstaining elector's baseline expire? Lean: no forced expiry —
  divergence is normal (Article VI), and staleness is visible from the declaration.
- **Who counts as a participant?** *Decided: tips-read.* Measured, it is also the only available
  definition — nothing records who was notified, so "notified and declined" is indistinguishable
  from "never asked". The distinction cannot be drawn from the record as it stands, and inventing a
  `notified:` field to draw it would be recording an act nobody performs.
- **The vocabulary question this turned up.** *Decided: neither.* `merge main — …` is used 72 times
  and is outside the closed vocabulary, and the reason it does not need a verb is the decision
  above — that merge is not an adoption, so nothing about it wants naming. A baseline merge is
  written with git's own subject, which the commit check already exempts, and carries the
  `Baseline:` trailer that does the binding. The 72 existing commits stay as they are; no history
  is rewritten to satisfy a rule written after them.
