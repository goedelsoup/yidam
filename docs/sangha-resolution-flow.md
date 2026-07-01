# The sangha resolution flow

### When to resolve

Not every divergence warrants resolution. Appropriate moments:

- A shared question has been sufficiently explored across ≥2 `ma/*` branches
- An axiom is contested and dependent nodes cannot be trusted until it is settled
- A new phase of inquiry requires a common baseline

### Resolution procedure

1. **Read** — read the current tip of each participating `ma/*` branch
2. **Synthesize** — produce a corpus representing collective understanding
3. **Open tensions** — any disagreement that cannot be resolved becomes an open-question node; not silently collapsed
4. **Commit** — create the `rigpa/<evolution>` branch with a message naming what was resolved, which `ma/*` tips were read, what changed, what remains open
5. **Record** — write a resolution file to `sangha/resolutions/<evolution>.md`

### Elector registration

A participant becomes a recognized elector by:

1. Opening a `ma/<name>` branch with at least one committed position
2. Having an existing elector add them to `electors.md` on their own `ma/*` branch
3. Including the registration in the first resolution they participate in

The first elector registers themselves.
