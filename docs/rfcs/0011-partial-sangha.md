# RFC-0011 — Partial-sangha resolutions and participant-scoped binding

- **Status:** Draft
- **Track:** G4 (governance)
- **Relates to:** RFC-0010 (explicit baselines), RFC-0009 (participants = tips read),
  `CONSTITUTION.md` Articles II / VI, `sangha/PROTOCOL.md`, `docs/sangha-resolution-flow.md`
- **Versioning layers touched:** template (protocol commentary); reuses RFC-0010's baseline field
- **Downstream reference case:** none — sangha-layer, applies to every derived repo.

## Summary

A subset of electors can resolve while others abstain — the "≥2 `ma/*` branches explored" trigger
already implies subsets are the normal case — but the constitution never says whether the resulting
rigpa **binds the abstainers' baseline**. This RFC ratifies: a partial resolution is
baseline-of-record for its **participants only**; abstainers keep their prior baseline until they
**adopt**, and adoption is an **explicit act** — the baseline declaration from RFC-0010 supplies it
for free. Two RFCs, one field.

## Problem

**Subsets are normal.** Resolution triggers on "a shared question... sufficiently explored across
≥2 `ma/*` branches" ([`sangha-resolution-flow.md:8`](../sangha-resolution-flow.md#L8),
[`PROTOCOL.md:27`](../../sadhana/sangha/PROTOCOL.md#L27)); nothing requires the whole sangha. The
"**participating** `ma/*` branches" phrasing throughout
([`PROTOCOL.md:38`](../../sadhana/sangha/PROTOCOL.md#L38),
[`CONSTITUTION.md:55`](../../yidam/prelude/CONSTITUTION.md#L55)) presumes non-participants exist.

**Binding is unaddressed.** Does a rigpa produced by a subset become the baseline for electors who
did not participate? The documents cut both ways and never decide:

- *Against* global binding: Article VI — "the sangha exercises the minimum authority needed... Positions
  that do not conflict are inherited... Resolution focuses on genuine tensions, not on imposing
  uniformity" ([`CONSTITUTION.md:90-95`](../../yidam/prelude/CONSTITUTION.md#L90-L95)) — and "an
  elector's `ma/*` branch may diverge freely from `rigpa/*`... Divergence is normal and expected; it
  is not a violation" ([`:94-95`](../../yidam/prelude/CONSTITUTION.md#L94-L95)).
- *Toward* global binding: "The new `rigpa/<evolution>` is **the active baseline**"
  ([`PROTOCOL.md:87`](../../sadhana/sangha/PROTOCOL.md#L87)), read as a single global fact.

The contradiction is exactly the ambiguity to close.

## Proposal

**1 — Binding is participant-scoped.** A rigpa is the baseline-of-record only for the electors whose
tips it read (its `tips:` list / `synthesized-by`, per RFC-0009). Abstainers are **not** bound; their
declared baseline (RFC-0010) is unchanged by a resolution they were not part of. This is Article VI
applied literally: the sangha does not exercise authority over positions that were not in tension
with the resolution, and a subset cannot, under Article II, impose a baseline on the whole.

**2 — Adoption is an explicit act.** An abstainer adopts a partial resolution by re-declaring their
baseline to it (the RFC-0010 `Baseline:` trailer) and rebasing — deliberate, not automatic. Until
then they keep their prior baseline. This scopes PROTOCOL's "baseline update"
([`PROTOCOL.md:85-87`](../../sadhana/sangha/PROTOCOL.md#L85-L87)) to participants, and makes it
*voluntary* for everyone else.

**3 — Not bound ≠ invisible.** The rigpa is still a permanent, legible evolution for the whole
sangha: its provenance stands, anyone may read or cite it, and any elector may later adopt it or fork
from it (RFC-0010). "Not your baseline yet" is a statement about *measurement*, not *visibility*.

Together this makes "the active baseline" a **per-elector** fact — which evolution each branch
declares — rather than a global singleton, which is the only reading coherent with RFC-0010's forked
lineage. It also forecloses the failure mode where a subset silently rebases the collective, which
would breach Articles II and VI at once.

## Migration & compatibility

Template-layer protocol commentary; no new schema — it reuses RFC-0010's baseline declaration.
Reword PROTOCOL's "the active baseline" to "the active baseline **for participating electors**," and
add an adoption note. Existing resolutions are unaffected (none exist). A domain wanting whole-sangha
binding adds a **quorum extension** (below), an opt-in per Article's domain-extension mechanism
([`CONSTITUTION.md:99-107`](../../yidam/prelude/CONSTITUTION.md#L99-L107)).

## Alternatives considered

- **Global binding by default.** Rejected: a subset imposing a baseline on non-participants violates
  minimal authority (VI) and epistemic equality (II), and is incoherent under forked lineage
  (RFC-0010) where there is no single global baseline to impose.
- **Rigpa is purely advisory (binds no one).** Rejected: the participants who synthesized *want* a
  shared baseline — producing one is the point of resolving. Scoped binding gives them that without
  reaching the abstainers.
- **Quorum makes binding global.** Not rejected — *offered as an opt-in*: a domain may add a quorum
  article so a resolution meeting quorum binds the whole sangha
  ([`CONSTITUTION.md:105`](../../yidam/prelude/CONSTITUTION.md#L105) already names quorum as a valid
  extension). Global binding should be a declared domain choice, never the silent default.

## Open questions

- **Later conflict.** If an abstainer's un-adopted position conflicts with the participant baseline at
  a subsequent resolution, is that just an ordinary tension to resolve? Lean: yes — nothing special;
  it becomes an open-question node or a new synthesis like any other.
- **Stale baselines.** Does a long-abstaining elector's baseline ever expire or require
  acknowledgment? Lean: no forced expiry — divergence is normal (Article VI); staleness is visible
  from the declared baseline, which is enough.
- **Who counts as a participant?** Tips-read (RFC-0009's `tips`) is the crisp definition; an elector
  notified but whose tip was not read is a non-participant. Confirm this is the intended line, or
  whether "notified and declined" deserves a distinct recorded status.
