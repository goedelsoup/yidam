# sangha

*Community.* The collective of agents and humans who maintain and evolve the knowledge graph.

`sangha/` encodes the protocol by which individual positions become collective understanding.
It does not contain knowledge directly — knowledge lives in the corpus. Sangha is the
mechanism by which multiple participants resolve their positions into a shared graph.

All resolution events are governed by the [prelude constitution](../prelude/CONSTITUTION.md).
Domain-specific procedure is defined in this repo's `sangha/PROTOCOL.md`, but the
constitutional articles are invariant — `PROTOCOL.md` may not contradict them.

## The ref path store

Sangha's state lives in git refs, not in files:

| Ref namespace | Lifetime | What it represents |
|---|---|---|
| `refs/heads/rigpa/<evolution>` | Long-term | A settled evolution — named, stable collective understanding |
| `refs/heads/ma/<elector>` | Short-term + influence | An elector's current position — in flux, pending resolution |

### rigpa — *pristine awareness*

A `rigpa/<evolution>` branch marks a point where the collective has resolved individual
positions into shared understanding. Each evolution is named for what it represents
(e.g., `rigpa/causal-closure`, `rigpa/v2-ontology`). Rigpa branches are stable checkpoints;
they do not diverge freely. A new rigpa branch supersedes the previous one and becomes the
common baseline from which elector branches diverge again.

### ma — *voice, position*

Each elector — a human contributor or an agent — maintains a `ma/<name>` branch as their
working position. They commit changes to corpus and catalog here: new nodes, revised
understandings, added edges. The branch reflects their current state without requiring it
to be globally settled. Elector branches are expected to diverge from each other and from
the current rigpa baseline.

## Resolution

Resolution is the act of synthesizing `ma/*` positions into a new `rigpa/<evolution>`.
It is a deliberate knowledge event — more like a synthesis commit than a mechanical merge.

A resolution event:

1. Reads the tips of all participating `ma/<elector>` branches
2. Identifies agreement, tension, and gap across positions
3. Synthesizes a corpus that represents collective understanding — resolving conflicting
   nodes, preserving distinct valid perspectives where they genuinely coexist
4. Commits the synthesis as a new `rigpa/<evolution>` branch; the message names what was
   resolved, what tensions were found, and what remains open
5. Elector branches continue from the new rigpa baseline, diverging again as inquiry proceeds

## When to resolve

Resolution is not automatic. Appropriate moments:

- A shared question has been sufficiently explored across positions
- An axiom is contested and must be settled before dependent nodes can be trusted
- A new phase of inquiry requires a common baseline

Not every divergence warrants resolution. Positions should diverge freely during active
inquiry; resolve when convergence serves the work, not on a schedule.

## What sangha/ contains

Files here are protocol documents only — not knowledge:

- `PROTOCOL.md` — the resolution algorithm for this repo's sangha (varies by domain)
- `resolutions/` — records of past resolution events: what was resolved, from which tips, what remained open
- `electors.md` — the current recognized participants: agents and humans with `ma/*` branches
