# The sangha resolution flow

## When to resolve

Not every divergence warrants resolution. Appropriate moments:

- A shared question has been sufficiently explored across ≥2 `ma/*` branches
- An axiom is contested and dependent nodes cannot be trusted until it is settled
- A new phase of inquiry requires a common baseline

## Resolution procedure

Steps 1–3 are a **loop**, and it ends when a round adds nothing.

1. **State positions** — each participating elector writes their position to `sangha/positions/<elector>-<question>.md` on their own `ma/*` branch, with `open:` or, when answering a round already held, `revise:`
2. **Transport** — carry each new position onto the baseline **unmodified**, with `transport:`, naming the `ma/<elector>@<hash>` it was read from. A position on its author's branch is one nobody else can answer and no corpus node can cite
3. **Read and answer** — each elector merges the baseline into their own branch and reads what the others filed. An elector with a concession, a refutation, or a ground of their own to withdraw returns to step 1, and the round runs again
4. **Synthesize** — when a round adds nothing, produce a corpus representing collective understanding at the current tips
5. **Open tensions** — any disagreement that cannot be resolved becomes an open-question node; not silently collapsed
6. **Commit** — create the `rigpa/<evolution>` branch with `resolve:`, naming what was resolved, which `ma/*` tips were read, how many rounds ran, what changed, what remains open
7. **Record** — write a resolution file to `sangha/resolutions/<evolution>.md`

**One round is a complete cycle.** The loop is not a quota: if everyone states a position and nobody has anything to add on reading the others, it has terminated correctly after one pass. What ends it is a round that moves nothing — not a fixed count, and not impatience. A tension still moving is a tension not ready; if the loop cannot converge, record the disagreement as an open question rather than resolving past it.

Why the loop rather than a single pass: the step where electors read each other *before* the resolution is where positions get answered on their merits. In the first repository to run it that way, two electors withdrew their own proposals mid-loop — which a single pass cannot reach, because nothing puts one elector's argument in front of another until the synthesis has already happened.

See [PROTOCOL.md](https://github.com/goedelsoup/yidam/blob/main/sadhana/sangha/PROTOCOL.md) for the git mechanics of each step.

## Elector registration

A participant becomes a recognized elector by:

1. Opening a `ma/<name>` branch with at least one committed position
2. Having an existing elector add them to `electors.md` on their own `ma/*` branch
3. Including the registration in the first resolution they participate in

The first elector registers themselves.
