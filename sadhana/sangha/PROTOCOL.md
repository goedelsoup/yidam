# Sangha Protocol

The resolution algorithm for this repository's sangha. Read alongside
[CONSTITUTION.md](../.vendor/prelude/CONSTITUTION.md), which defines invariant constraints
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

1. **State positions** — Each participating elector writes their position on the question
   to `positions/<elector>-<question>.md` and commits it to their own `ma/*` branch. A
   position that contests another links to it. This step is not optional and it is not
   ceremony; see [positions/](positions/README.md) for why the branch tip alone cannot
   stand in for it.

2. **Read** — Read the current tip of each participating `ma/*` branch, and the positions
   stated on it. Identify where positions agree, where they diverge, and where one has no
   position.

3. **Synthesize** — Produce a corpus representing collective understanding at those
   tips. Per Article V (Scope Fidelity), introduce only nodes and edges present in
   at least one elector's position. Do not add content from outside the positions.

4. **Open tensions** — Any genuine disagreement that cannot be synthesized without
   choosing one elector's position over another must become an open-question node
   in the corpus. Title the node as the question. Do not silently collapse divergent
   positions into a single claim.

5. **Commit** — Create the `rigpa/<evolution>` branch and commit the synthesis with the
   `resolve:` verb. The commit message must include:
   - What domain question was resolved
   - Which `ma/*` tips were read (branch name + short hash)
   - What changed in the collective understanding
   - What open questions remain, if any

   ```
   git switch -c rigpa/<evolution>
   git add <the corpus files step 3 wrote>
   git commit -m "resolve: <what was settled, and what it cost>"
   ```

6. **Record** — Add a file to `resolutions/<evolution>.md` using the format below.

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

After resolution, each participating elector takes the new `rigpa/<evolution>` tip into
their own branch — by **merge**, with the `adopt:` verb:

```
git switch ma/<elector>
git merge --no-ff -m "adopt: the baseline after <evolution>" rigpa/<evolution>
```

**Merge, not rebase.** This instruction read "rebase their `ma/*` branches onto the new
tip" for the whole of this template's early life, and it contradicted Article III one
document away: *do not rewrite `ma/*` branches after a resolution; let the history stand as
provenance.* A rebase rewrites every commit it moves, which discards exactly the ancestry
the article exists to preserve — the record of what each elector held before the resolution
and when. A merge keeps both sides and makes the adoption itself a dated event. The
contradiction was found in a derived repository that had already resolved it the right way
by merging 23 times without being told to.

Prior rigpa branches are not deleted — they remain as provenance. The new
`rigpa/<evolution>` is the active baseline.
