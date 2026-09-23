# commitments

What one seat currently holds, what it has withdrawn, and which of its own positions argued
each. One file per seat, resident on that seat's own branch.

`.yidam/sangha/commitments/<elector>.md` on `ma/<elector>`, and nowhere else. It is not
transported, it does not reach the baseline, and no resolution reads it. See *Why this stays on
the branch* below — the reason is not incidental.

## The problem it answers

An elector is a seat, and a seat is a `ma/<name>` branch plus its row in
[electors.md](../electors.md). An *agent* elector is a seat whose occupant arrives with no memory of
having sat there before. Every act it performs on that branch is performed cold.

`CONSTITUTION.md` Article VI licenses an elector's branch to diverge freely from `rigpa/*` after a
resolution, and says divergence is normal and expected. That is right, and it presumes something
the mechanism does not supply: that the thing doing the diverging **carries a position between
acts**. A human elector does. A `ma/*` branch maintained by a succession of cold instances is a
random walk that looks like deliberation, and it satisfies every constitutional check the
repository has. Article V decides what a *resolution* may synthesize from the tips in front of it.
Nothing asks whether one seat, read across time, is still arguing the same case.

[positions/](../positions/README.md) already states the norm:

> An elector who changes their mind writes a new position for the new question and says in it
> which earlier ground of theirs did not survive.

That norm has no artifact and no check, and a norm whose alternative leaves no trace is
indistinguishable from the absence of one. Measured on 2026-09-22 in the one repository that has
run this protocol: of 71 positions, **17 link another elector's position and 4 link one of the
author's own**. The loop is built to make electors read each other, and it does. Reading
themselves is the direction nothing supports.

## The shape

Exactly two sections, and every item under either one links the position that argued it:

```markdown
# Commitments: ma/advocate

## What this seat holds

- The advocate is adversarial about the *frame*, not the wording, and collapsing it into the
  auditor would lose that — [registration](../positions/advocate-registration.md)
- A payload budget stated per document is inert where it was set —
  [payload-budget](../positions/advocate-payload-budget.md)

## What this seat has withdrawn

- The per-document ceiling, withdrawn on reading the auditor's filing; the way I got it wrong is
  the failure mode this seat is named for — [payload-budget](../positions/advocate-payload-budget.md)
```

**The position link is the item's identity, and the prose is not.** This is Article V's rule about
what a check may decide, applied to the same problem one layer out. Whether two sentences state
the same commitment is a judgement; whether a position this seat filed is named under `holds`,
named under `withdrawn`, or named under neither is set membership, and a check can settle it. So
the wording of an item may be revised freely at any time, and the link may not disappear.

Nothing else is prescribed. Order, grouping, headings below the two, how much of the argument is
restated — all of it is the elector's. A commitments file is an **index of a seat's own
positions**, not a second place to make the case; the case is in the position, which is where the
loop can answer it.

## Why this stays on the branch

Transporting it would make it an argument on the baseline, and it is not one. Three consequences
follow, and the third is the one that decides it:

- **A position is contestable and a commitments file is not.** Step 2 carries positions onto the
  baseline so other electors can answer them. There is nothing here for another elector to answer
  — the arguments are in the positions this file links, and they are already carried.
- **Editing it is not a synthesis risk.** A transported file is one somebody could improve in
  carriage, which Article V forbids and `positions/` warns about at length. A file that never
  leaves its branch cannot be improved by anyone but its holder.
- **A resolution must not read it.** Article V confines synthesis to what participating positions
  held. A seat's index of its own grounds is not a position; a resolution that reached for it
  would be synthesizing from something no elector filed as an argument. Keeping it off the
  baseline is what makes that mistake unavailable rather than merely forbidden.

## What the checks decide

`yidam lint` reports four things, and what they share is that each is decidable from refs without
reading an argument:

| check | severity | fires when |
|---|---|---|
| `elector-commitments-absent` | Info | the seat has filed at least one position and its branch tip carries no commitments file |
| `elector-commitments-malformed` | Error | the file exists and does not carry both headings |
| `elector-position-unindexed` | Info | a position this seat filed is named in neither section |
| `elector-commitment-vanished` | Error | a position was named under `holds` at one commit and is named in neither section at a later one |

`elector-commitment-vanished` is the one with teeth, and it is the narrowest possible reading of
Article III's *tensions that could not be resolved must not be silently discarded*, applied to one
elector across time rather than to a resolution across electors. A ground may be held. It may be
withdrawn, which is the interesting case and the one the loop was built to produce. It may not
quietly stop existing.

**Malformed gates for one reason: otherwise the file disarms the rest.** A commitments file with
no headings parses to two empty sections, and empty sections lose nothing — every vanished ground
would read as clean. The same is true of deleting the file, which is why the deletion of a
commitments file that held grounds is reported as those grounds vanishing rather than as the file
going absent.

### What is deliberately not checked

**Whether the seat engaged the argument it reversed.** [#294](https://github.com/goedelsoup/yidam/issues/294)
asks for a check that catches a position reversing a commitment *without engaging the argument
that established it*, and engagement is a judgement of exactly the kind Article V's commentary
refuses to delegate to a checker. A seat can name a position under `withdrawn` and write one
contentless line, and nothing here will say so. What the checks guarantee is narrower and is the
part that can be guaranteed: the reversal is **on the record, in the seat's own hand, next to the
ground it replaced**, where the next elector and the next occupant of this seat can both read it.

**Whether the commitments are true, coherent, or well-chosen.** That is Article II's business and
it belongs in the loop.

## What this has not yet been observed to catch

Stated plainly, because the case for building this is prospective and not diagnostic. The one
repository that has run this protocol ran it across **14 days** — 2026-08-10 to 2026-08-23, 30
resolutions and 71 positions — and has filed nothing since. A seat drifting across months is a
failure that corpus has not had time to exhibit. What it *did* exhibit is the shape: `advocate`
wrote a registration position stating what the seat is for, and four days later argued from it by
name. The practice reached for this artifact once, unprompted, and then had no place to keep it.

If a quarter from now the commitments files in this repository are stale — written at
registration, never revised, and never contradicted by a position — that is the finding, and the
honest response is to withdraw the mechanism rather than to require it harder.
