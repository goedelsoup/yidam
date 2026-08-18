# positions

One elector's stated position on one question, written down before a resolution reads it.

`positions/<elector>-<question>.md` — the elector is the branch name without `ma/`, the
question is the `rigpa/<evolution>` name the resolution will carry. A resolution over three
electors and one question has up to three files here, and they are committed to each
elector's own `ma/*` branch, not to the baseline.

## Why these exist when the branch tip is already the position

The graph model says an elector's position lives in git refs: `refs/heads/ma/<elector>` is
their working position and the resolution reads its tip. That is true about the *corpus* —
which nodes they hold, which edges, which claims at which tag. It is not true about the
**argument**, and the argument is what a resolution actually turns on.

A branch tip records a conclusion. It does not record why the elector reached it, what they
conceded, which of their own earlier grounds they withdrew, or what they think the other
position gets wrong. That reasoning exists — it is what the elector did — and without a
file it exists only in whatever session produced the commits. Then the resolution merges,
the divergence disappears into the merge base, and the argument is gone.

Two constitutional articles are unsatisfiable in that state:

- **Article III (Provenance)** requires that a resolution preserve the ancestry of
  synthesized knowledge. Ancestry is not only which commits were read; it is which claims
  were contested and by whom.
- **Article IV (Legibility)** requires that a resolution be describable — what was resolved,
  what changed, what remains open. A resolution written from branch tips alone can state
  what changed. It cannot state what was *argued*, because nothing wrote it down.

A resolution record in `resolutions/` is the synthesis. These files are the inputs it
synthesized. Keeping both is what makes the record auditable by someone who was not there.

## What a position file holds

No fixed schema — this is argument, and a form would flatten it. In practice:

- A heading naming the elector, their branch, and what this position contests
- What the elector concedes, stated first and without hedging. A position that concedes
  nothing is usually one that has not read the other side.
- The grounds they are withdrawing, if any, and what replaced them
- The case itself, with edges into the corpus nodes it turns on
- What the position does *not* claim

Concession is the part worth protecting. The most useful position files in practice are the
ones where an elector dismantles a bar they themselves wrote — and that only becomes visible
across resolutions if each position is a durable file rather than a session.

## Lifecycle

Positions are permanent. A superseded position is not deleted or edited into agreement: it
is the record of what was held, and the next resolution's record links back to it. An
elector who changes their mind writes a new position for the new question and says in it
which earlier ground of theirs did not survive.
