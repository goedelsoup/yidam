# Sangha Protocol

The resolution algorithm for this repository's sangha. Read alongside
[CONSTITUTION.md](../prelude/CONSTITUTION.md), which defines invariant constraints
that this protocol may not override.

This file is domain-specific — derived repositories should adapt it to their
inquiry style, quorum needs, and communication norms.

## Electors

Recognized electors are listed in [electors.md](electors.md). An elector is any
human or agent maintaining a `ma/<name>` branch in this repository.

A participant becomes a recognized elector by:

1. Opening a `ma/<name>` branch with at least one committed position
2. Having an existing elector add them to `electors.md` on their own `ma/*` branch
3. Including the elector registration in the first resolution they participate in

The first elector registers themselves.

## Calling a resolution

Any elector may call a resolution by:

- Identifying a question or tension that has been explored across ≥2 `ma/*` branches
- Naming the `rigpa/<evolution>` branch for the synthesis (use the settled question as the name)
- Notifying participating electors before beginning

Not every divergence warrants resolution. Call one when:
- A shared question is sufficiently explored and the positions want synthesis
- An axiom is contested and dependent nodes cannot be trusted until it is settled
- A new phase of inquiry requires a common baseline

## Resolution procedure

1. **Read** — Read the current tip of each participating `ma/*` branch. Identify
   where positions agree, where they diverge, and where one has no position.

2. **Synthesize** — Produce a corpus representing collective understanding at those
   tips. Per Article V (Scope Fidelity), introduce only nodes and edges present in
   at least one elector's position. Do not add content from outside the positions.

3. **Open tensions** — Any genuine disagreement that cannot be synthesized without
   choosing one elector's position over another must become an open-question node
   in the corpus. Title the node as the question. Do not silently collapse divergent
   positions into a single claim.

4. **Commit** — Create the `rigpa/<evolution>` branch and commit the synthesis.
   The commit message must include:
   - What domain question was resolved
   - Which `ma/*` tips were read (branch name + short hash)
   - What changed in the collective understanding
   - What open questions remain, if any

5. **Record** — Add a file to `resolutions/<evolution>.md` using the format below.

## Resolution record format

```markdown
---
evolution: <name matching rigpa/<evolution> branch>
date: <YYYY-MM-DD>
tips:
  - ma/<elector>@<short-hash>
  - ...
---

## What was resolved

...

## What changed

...

## What remains open

...
```

## Baseline update

After resolution, participating electors rebase their `ma/*` branches onto the new
`rigpa/<evolution>` tip. Prior rigpa branches are not deleted — they remain as
provenance. The new `rigpa/<evolution>` is the active baseline.
