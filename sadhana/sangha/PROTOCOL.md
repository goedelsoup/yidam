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

## Who may execute one

**Any recognized elector, whether or not they called it.** Calling was specified here from the
start; executing was not, and the executor holds the one pen Articles III–V are written to bound —
it decides what the collective understanding *is*, which tensions become open questions, and what
the rigpa records.

A standing designated-synthesizer role would be exactly the privilege Article II forbids: no
elector's position is privileged "by identity, seniority, or the model that produced it." So
execution authority is universal, and it is bounded by *what* the executor may do rather than by
*who* they are. Holding the pen is not a tiebreaker. If the executor's own position is in tension
with another's, that tension becomes an open-question node under step 5 like any other — never a
claim the executor settles in their own favour because they were the one typing.

The caller and the executor may differ, and often should: the elector who noticed a tension is not
always the one best placed to synthesize it. Only the executor is recorded, in `synthesized-by`
below.

## Resolution procedure

Steps 1–3 are a loop. An elector who reads another's position and has something to say
writes it and transports it, and the round runs again. The loop ends when a round adds
nothing — not after a fixed number of passes, and not when somebody runs out of patience.

**One round is a complete cycle.** If every elector states a position, nobody has anything
to add on reading the others, and the tension is clear, the loop has terminated correctly
after one pass. The loop is not a quota.

1. **State positions** — Each participating elector writes their position on the question
   to `positions/<elector>-<question>.md` and commits it to their own `ma/*` branch with
   `open:` — or `revise:`, when answering a round already held. A position that contests
   another links to it. This step is not optional and it is not ceremony; see
   [positions/](positions/README.md) for why the branch tip alone cannot stand in for it.

2. **Transport** — Carry each new position onto the baseline, **unmodified**:

   ```
   git switch main
   git checkout ma/<elector> -- .yidam/sangha/positions/<elector>-<question>.md
   git commit -m "transport: <whose position, and what it says>"
   ```

   The path is from the repository root, which is where git runs — the bare
   `positions/…` used elsewhere in this file is relative to the sangha directory holding
   it.

   Record in the message which ref it was read from — `ma/<elector>@<short-hash>` — so the
   carriage is auditable against the branch it came from.

   **Do not edit what you carry.** Article V confines synthesis to resolution events. A
   verbatim copy introduces nothing its author did not hold, which is exactly why this is
   legal outside one; a `transport` commit that improves the argument it carries is a
   resolution performed by one elector who has read nobody. If the position is wrong,
   answer it in step 3 under your own name.

   Without this step a position sits on a branch no one else is on. A derived repository
   ran twenty resolutions before adding it and found the cost: four corpus nodes citing
   positions that resolved for their author and for nobody else, and two resolutions
   standing on the baseline whose arguments were not.

3. **Read and answer** — Each elector brings the baseline into their own branch and reads
   what the others have filed:

   ```
   git switch ma/<elector>
   git merge --no-ff --no-edit main
   ```

   No `-m` here, deliberately. Git writes `Merge branch 'main' into ma/<elector>`, and a
   merge subject git generated is exempt from the closed vocabulary — nobody chose that
   verb, so nothing can be said about the choice. This merge is not `adopt`: that verb is
   for taking a *settled* baseline after a resolution, and nothing is settled mid-loop.

   Identify where positions agree, where they diverge, and where one has no position. An
   elector with something to say — a concession, a refutation, a ground of their own they
   are withdrawing — returns to step 1 and the round runs again.

   **This is the step the protocol used to lack, and it is where the work happens.** In the
   repository that first ran it, the loop produced commits no single pass could:
   *"I withdraw the per-document ceiling, and the way I got it wrong is the failure mode
   this seat is named for"*; *"proposal 9 is inert where I set it and gameable where it
   would bind, so it is withdrawn"*. Both are electors dismantling their own proposals,
   which is only reachable if they can read each other before the resolution rather than
   through it.

4. **Synthesize** — When a round adds nothing, produce a corpus representing collective
   understanding at the current tips. Per Article V (Scope Fidelity), introduce only nodes
   and edges present in at least one elector's position. Do not add content from outside
   the positions.

   **A node or an edge here is decided rather than judged**, and `yidam lint` decides it —
   `resolution-scope-unheld` reads the `tips:` this record names, resolves each one, and asks
   whether the node stood in the corpus there or was named in a position filed there. Article
   V's third object, a *claim*, is deliberately not checked: a node and an edge carry an
   identity of their own and a claim does not, so that judgement stays with the synthesizer
   under Article II. See the commentary under Article V in
   [CONSTITUTION.md](../.vendor/prelude/CONSTITUTION.md).

   The rule this most often catches is the one the commentary states: **a class is not its
   instances.** A position arguing that a class should exist does not hold the instances of
   it, so a resolution adopting the class and seating its first instances in the same commit
   is introducing nodes no elector held. In the repository that has run this protocol that is
   three of twenty-nine resolutions, and every one of them is that mistake.

5. **Open tensions** — Any genuine disagreement that cannot be synthesized without
   choosing one elector's position over another must become an open-question node
   in the corpus. Title the node as the question. Do not silently collapse divergent
   positions into a single claim.

   An open-question node is the one thing Article V lets a resolution introduce that no
   elector held, and **it is licensed by this record and nothing else** — name the node under
   `What remains open` below. Nothing in the corpus model marks such a node; there is no class
   for it and no field. Tying the exception to the record is not a workaround for the missing
   marker but the right place for it, because what makes the node legal is the resolution
   saying the question is still open.

6. **Commit** — Create the `rigpa/<evolution>` branch and commit the synthesis with the
   `resolve:` verb. The commit message must include:
   - What domain question was resolved
   - Which `ma/*` tips were read (branch name + short hash)
   - How many rounds the loop ran, and what the last one changed
   - What changed in the collective understanding
   - What open questions remain, if any

   ```
   git switch -c rigpa/<evolution>
   git add <the corpus files step 4 wrote>
   git commit -m "resolve: <what was settled, and what it cost>"
   ```

7. **Record** — Add a file to `resolutions/<evolution>.md` using the format below.

## When to stop

A round that adds nothing ends the loop. Three things that are *not* reasons to stop:

- **Agreement.** Electors agreeing early is a result and it terminates the loop, but
  agreement reached because nobody read the others is not agreement.
- **A fixed count.** Two rounds is not more rigorous than one, and four is not more
  rigorous than two. The question is whether the last round moved anything.
- **Impatience.** A tension that is still moving is a tension that is not ready. If the
  loop cannot converge, that is itself the finding: record the disagreement as an open
  question under step 5 rather than resolving past it.

## Resolution record format

```markdown
---
evolution: <name matching rigpa/<evolution> branch>
date: <YYYY-MM-DD>
synthesized-by: ma/<elector>
rounds: <how many times the loop ran>
tips:
  - ma/<elector>@<short-hash>
  - ...
positions:
  - positions/<elector>-<question>.md
  - ...
---

## What was resolved

...

## What changed

...

## What remains open

...
```

`synthesized-by` names the elector who executed the resolution — one seat, or a list where a
synthesis was genuinely joint. **It is a record, not a rank.** Article II governs weight and grants
the executor none: being named here is no standing, no tiebreak, and no priority in any later
resolution. Article III governs record, and the human or agent who did the reading is part of the
ancestry it demands. The record already names which tips were read; omitting who read them left the
most consequential actor in the event the one actor the provenance omits.

It is also the field that makes a seat legible at all. In the repository that has run this protocol,
all 126 commits across three elector branches carry one git author — the operator's. Nothing in git
distinguishes the auditor's position from the owner's, and until this field exists nothing in the
record does either.

`rounds` and `positions` are what make Article III (Provenance) checkable rather than
asserted. Ancestry is not only which commits were read; it is which claims were contested
and by whom, and a record that names its positions can be audited by someone who was not
there. `rounds: 1` is a fine number — see [When to stop](#when-to-stop).

## Annotating an open item after the fact

A resolution is a dated record of an event, and its `What remains open` is a statement
about the world on that date. The world moves. **A resolution written last month that still
reads *"unproposed by any elector"* about something proposed the next day is the same defect
this repository gates for in its corpus** — a sentence that was true when written and reads
as current.

Until there was a convention for it there was no way to say so, because no resolution had
ever been annotated and inventing the practice inside one commit would have been a
governance act dressed as a correction.

**The original text is never edited.** An item that has moved gets a dated annotation
appended beneath it, as a block quote:

```markdown
> **Moved YYYY-MM-DD.** What has since happened, and where it now lives.
```

Three constraints, and the third is the one that makes this safe:

1. **The annotation is additive.** The sentence above it stays exactly as written. A dated
   record that gets rewritten is no longer a record of what was decided.
2. **It carries its own date**, which is the date of the movement and not of the resolution.
3. **An annotation records movement and never outcome.** It may say a question was proposed,
   retrieved, measured, or superseded. It may not say it was *settled* — **an open item is
   closed only by a later resolution**, because closing one is synthesis and Article V puts
   synthesis in resolution events.

The third constraint is not about annotations being wrong. It is about what an annotation
structurally *is*: a place where one elector, in one commit, having read no `ma/*` tip and
having transported nothing, could perform a resolution in a file the protocol never routes
through one. An annotation that decides something is a resolution written in the wrong
place.

Annotate under `What remains open` and nowhere else. Annotating `What was resolved` would
edit history rather than extend it.

**What this does not catch: an open item that moved and was never annotated.** That is the
same shape one layer up, it is not solved here, and saying so is cheaper than a check that
would have to know what the world is doing.

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
