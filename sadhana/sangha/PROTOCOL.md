# Sangha Protocol

The resolution algorithm for this repository's sangha. Read alongside
[CONSTITUTION.md](../.vendor/prelude/CONSTITUTION.md), which defines invariant constraints
that this protocol may not override.

This file is domain-specific — derived repositories should adapt it to their
inquiry style, quorum needs, and communication norms.

## Electors

Recognized electors are listed in [electors.md](electors.md). An elector is any
human or agent maintaining a `ma/<name>` branch in this repository.

**An elector is a seat**, and that is the whole of the definition: a `ma/<name>` branch together
with its row in [electors.md](electors.md). An *agent* elector is a seat whose `Kind` is `agent`.
A model, an operator, a harness and a run are **occupants** of a seat — recorded on it, never the
elector itself.

This is what every mechanism here already points at. `synthesized-by:` names a seat. The
allowed-signers file generated from the registry uses the seat's branch as the principal, so what
a verified signature establishes is that a commit came from whoever holds *that seat's* key. And
`independence:` compares what the registry records about seats. Naming the unit makes those one
answer instead of three that happen to agree.

It grants nothing. Article II governs weight and Article III governs record; a seat is a place a
position is held from, not a standing that privileges one.

A participant becomes a recognized elector by:

1. Opening a `ma/<name>` branch with at least one committed position
2. Having an existing elector add them to `electors.md` on their own `ma/*` branch
3. Including the elector registration in the first resolution they participate in

The first elector registers themselves.

**Step 3 is where a seat becomes accountable to somebody.** The resolution that admits it is a
committed record naming its executor and the tips it read, so *who answers for this seat* has an
answer in the repository rather than in a convention about who ran what. A seat records that
resolution in `electors.md`'s `Seated` column. The bootstrap seat leaves it blank — nobody
admitted it — which is a normal state and not an omission.

### What a registration records

An agent elector records what it is at registration — model, version, and a hash of its
operative configuration — in the columns [electors.md](electors.md) describes. A material
change is recorded as an update, so the ancestry of a position includes the state of the
agent that held it; a model upgrade is material, and the config hash is what makes
"material" detectable rather than a matter of opinion.

A seat may also bind an SSH public key, and binding one is a declaration: `yidam lint`
generates the allowed-signers file from `electors.md` and verifies that seat's branch tip
against the key its own row carries. A seat that binds none is not in violation of
anything — it has said its commits are unverifiable, which is the state every seat is in
until somebody decides otherwise. Recording any of this grants a seat nothing; Article II
governs weight and Article III governs record, and `electors.md` says so at the top.

### What a seat carries between acts

A registration says what a seat *is*. It does not say what the seat currently **holds**, and
between two acts of the same seat nothing did.

`CONSTITUTION.md` Article VI licenses an elector's branch to diverge freely from `rigpa/*`, and
that licence presumes a standing position the elector carries forward. A human elector carries one.
An agent elector is a seat whose occupant arrives with no memory of having sat there before, and a
`ma/*` branch maintained by a succession of cold instances is a random walk that looks like
deliberation — while satisfying every constitutional check this repository has, because Article V
decides what a *resolution* may synthesize from the tips in front of it and nothing reads one seat
across time.

So a seat keeps **[`commitments/<elector>.md`](commitments/README.md) on its own branch**: two
sections, `## What this seat holds` and `## What this seat has withdrawn`, each item linking the
position that argued it. It is an index and not an argument, which is why it has a shape where a
position does not, and why it is **not transported** — there is nothing in it for another elector
to answer, and a resolution that reached for it would be synthesizing from something no elector
filed as a position.

The link is the item's identity. Wording may be revised at any time; a position that was named
under `holds` may move to `withdrawn`, and may not quietly stop being named at all.
`elector-commitment-vanished` gates on exactly that and on nothing more — whether the withdrawal
*engaged* the argument it reverses is a judgement, and Article V's commentary leaves that kind with
the elector rather than with a checker.

## Calling a resolution

Any elector may call a resolution by:

- Identifying a question or tension that has been explored across ≥2 `ma/*` branches
- Naming the `rigpa/<evolution>` branch for the synthesis (use the settled question as the name)
- Notifying participating electors before beginning

Not every divergence warrants resolution. Call one when:
- A shared question is sufficiently explored and the positions want synthesis
- An axiom is contested and dependent nodes cannot be trusted until it is settled
- A new phase of inquiry requires a common baseline

### What the trigger cannot see

The trigger fires on structure. Two branches is two branches, and nothing above asks whether
they are two **positions**.

`CONSTITUTION.md` Article II says no elector's position is privileged "by identity, seniority,
or the model that produced it". That clause does real work — it stops a position being
*discounted* for its provenance, which is the protection an agent elector needs to be an equal
participant at all. It does not establish that three positions are three positions.
**Non-privilege is not independence.** Three instances of one model, at one temperature, given
one prompt over one corpus, are one position wearing three hats. They will agree, their
agreement carries no information, and a resolution synthesized from them satisfies Article V
perfectly — every claim traces to a participating position — while meaning nothing. This is a
failure that cannot occur in a sangha of humans and is the default in a sangha of agents unless
something prevents it.

**Decided 2026-09-17: the protocol records the condition; it does not gate on it.** There is no
diversity bar on calling a resolution and nothing below refuses one. What the record gains is
`independence:`, which states what the registry distinguishes among the seats a resolution read.
A resolution among seats the registry cannot tell apart is legal, and says so.

Two reasons the bar was declined rather than overlooked.

The first is that it would be unenforceable in the only direction that matters. Every signal this
protocol can read — branch name, registry row, signing key, git author, `synthesized-by` — is set
by one operator, and an operator who wants three distinguishable seats can produce three of each
without producing three minds. [electors.md](electors.md) states the general form against
measurement: *"It is not evidence that two seats are two minds."* A gate over signals like those
refuses the honest registry and passes the careless one.

The second is that the loop converges regardless. Step 3 is where the work happens, and the
commits it has produced — *"I withdraw the per-document ceiling"*, *"proposal 9 is inert where I
set it and gameable where it would bind, so it is withdrawn"* — are electors dismantling their
own proposals. One configuration writing a position, reading that position transported onto the
baseline, and answering it is a materially different act from writing one document, and it is the
act that produced those. Refusing it on a purity argument would forbid a practice the record
shows working. What the record must not do is call the result a synthesis of positions when it
was not, and that is what the field is for.

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

   **If this position changes a ground the seat was standing on, update
   [`commitments/<elector>.md`](commitments/README.md) in the same commit** — move the item to
   `## What this seat has withdrawn` and link the position that retired it. The commitments file
   stays on the branch; only the position is transported in step 2.

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

   Read your own commitments file before you answer. It is the only thing in this repository
   that tells a cold occupant of this seat what the seat was arguing before it arrived, and the
   concessions this loop is built to produce are the ones that land in it.

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
   in the corpus. Title the node as the question, and **mark it: a label beginning with
   `?`**, quoted, as `GRAPH.md` declares. Do not silently collapse divergent positions into
   a single claim.

   An open-question node is the one thing Article V lets a resolution introduce that no
   elector held, and **it is licensed by this record and nothing else** — name the node under
   `What remains open` below. The `?` does not license it and is not checked here: it makes
   the node legible to a reader who never opens this record, which is a different job from
   making it legal. What makes it legal is the resolution saying the question is still open,
   so the record stays the authority and the marker stays a courtesy to whoever reads the
   corpus next.

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
independence: <distinct-seats | shared-configuration | unrecorded>
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

### `independence` — what the registry distinguishes, not what minds exist

**Read the field name together with its values and never on its own.** No registry can establish
independence — see [What the trigger cannot see](#what-the-trigger-cannot-see) — and this field
does not claim to. What it records is narrower and checkable: whether
[electors.md](electors.md) distinguishes each participating seat from every other. That is the
strongest thing available, and it is the one a later reader can hold the record to.

A closed vocabulary of three:

- **`distinct-seats`** — every participating seat differs from every other in something the
  registry records. Human seats differ by being different people. Agent seats differ when no two
  carry the same `Model`, `Version` and `Config`.
- **`shared-configuration`** — two or more participating agent seats carry the same `Model`,
  `Version` and `Config`. The resolution read fewer positions than it read tips.
- **`unrecorded`** — a participating agent seat leaves `Model`, `Version` or `Config` blank, so
  the question cannot be answered from the registry at all. **A consumer MUST NOT read this as
  `distinct-seats`, and MUST NOT read it as `shared-configuration` either.** Every attestation
  column is optional by `electors.md`'s own rule — *"a registry that fills none of them in is
  read exactly as it was before they existed"* — so a blank column is a normal state and not a
  confession. *Cannot tell* gets a word of its own, which is the same discipline `doctor`'s
  `skipped` verdict and the MCP handshake's tri-state `stale` already follow.

**Derived, since 2026-09-22.** The record already names its `tips:`, each tip names a seat, and
the registry carries the columns — so the value is a function of things already written down
rather than a judgement, and `yidam lint`'s `resolution-independence-mismatch` computes it. The
synthesizer still writes the field; what changed is that a stated value which disagrees with the
registry is now visible instead of being a defect nothing could see.

Both questions that check needed answered are answered, and both answers constrain anyone
changing it.

**The registry is read at the tips, never at HEAD — and each seat at its own tip.** A seat's row
is mutable, *"a material change is recorded as an update"*, so the two readings disagree for any
record old enough to matter. Reading HEAD would re-judge settled history every time somebody bumps
a model: a resolution correct the day it was written would go red on a commit that has nothing to
do with it. Reading each seat's row out of the blob at its own `ma/<elector>@<hash>` is what *"the
state of the agent that held it"* means literally, and it is the only reading under which a settled
record's derived value never changes again. **A tip this clone does not carry yields `unrecorded`,
and there is no fallback to HEAD** — a shallow or single-branch checkout does not have the `ma/*`
commits, and an answer that depended on how the repository was fetched would be worse than no
answer. `unrecorded` is already the word for *the registry does not say*.

**A disagreement is a finding, not a gate.** It reports at Info and carries the derived value in
the finding, the arrangement `elector-baseline-undeclared` already uses, because every record
written before this field existed carries no value at all — a gate would be red on the whole
corpus the day it armed, which is how a gate gets switched off rather than answered. One silence
is deliberate: a record that states nothing, whose seats derive `unrecorded`, is passed over.
There is nothing to write down, and *"add `independence: unrecorded`"* is a request to record
*we do not know*, which is already what absence means.

**What `shared-configuration` costs, which is the entire point of writing it down.** A resolution
carrying it **is not a synthesis of positions and must not describe itself as one.** It is one
position, reviewed under several branch names, and `What was resolved` should read that way.
Article II is untouched — nothing here privileges or discounts anybody's position, and such a
resolution binds exactly as any other does. What is constrained is only what the record may claim
about its own ancestry, which is Article III's business and not Article II's.

**And it is a budget.** Deliberation among humans is free to the repository; deliberation among N
agents is not. If `shared-configuration` is the honest label for N instances of one setup, then N
is a cost with no return, and *how few positions can legitimately resolve this* becomes a question
with a second, independent reason to be answered well. That is
[RFC-0011](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0011-partial-sangha.md)'s
partial sangha, which stops being a convenience for when people are unavailable and becomes the
ordinary case.

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

**What electors actually do is merge `main`, and that is worth knowing before you follow the
recipe above.** Measured across 126 commits on the three elector branches of the repository that
has run this protocol: **one** `adopt:` commit, and **72** `merge main` commits. The settlement
lands on the baseline branch, so merging `main` picks it up along with everything else settled
since; merging `rigpa/<evolution>` picks up that evolution and nothing after it.

Three consequences, none of them yet decided — see
[RFC-0010](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0010-evolution-lineage.md) and
[RFC-0011](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0011-partial-sangha.md), which
is where the decision belongs rather than here:

1. **Merge-base cannot tell you which evolution a branch is measured against.** Asked, it returns
   the same evolution for all three electors — four to six resolutions stale — because it reads
   `rigpa/*` refs that nobody merges.
2. **Merging `main` adopts resolutions you did not participate in.** All nine partial resolutions
   in that repository are held by the elector who sat them out, absorbed this way, with nothing
   recording that it happened.
3. **`merge main — …` is outside the closed commit vocabulary.** An authored merge subject is not
   the git-generated form the commit check exempts, so `main..ma/<elector>` reports thirty of them
   — and the default range never walks an elector branch, so a workflow used seventy-two times is
   invisible to the gate.

None of that makes merging `main` wrong. Step 3 requires it mid-loop, and a corpus with a shared
baseline is the point. It does mean that *which evolution this position is measured against* is a
fact the branch does not currently state, and the recipe above is not the one producing the
history.

### Declaring the baseline

Say it, rather than leaving it to be inferred. Put a trailer on the commit that adopts:

```
Baseline: rigpa/<evolution>@<short-hash>
```

The most recent one on a `ma/*` branch is the branch's declaration, and `yidam lint` reads it:
`elector-baseline-unmet` gates when a branch declares an evolution no record carries, or one whose
settlement the branch does not contain. **A baseline that is merely old is not a finding** —
divergence from the baseline is what an elector's branch is for, and Article VI says so.

### What binds, and what does not

**A resolution binds the electors whose tips it read. The declaration binds everyone else, and the
merge binds nobody.**

*Holding* a settlement and *being measured against* it are two facts. Merging `main` brings you
every settlement on it, including resolutions you took no part in — that is what arrived, not what
you stand on, and it cannot be the act that binds without making the ordinary workflow illegal.
Step 3 requires that merge mid-loop.

So a partial resolution is baseline-of-record for its participants, and for everyone else only once
they declare it. Article VI applied literally: the sangha exercises the minimum authority needed,
and a subset does not move a position that was not in tension with it.

Three consequences worth stating plainly, because each is a state a reader might otherwise mistake
for a defect:

- **Holding a settlement you have not declared is expected.** Every branch is in that state today.
  Your position is measured against your last declaration; your tree carries whatever `main` has
  settled. `elector-holds-unadopted` reports the gap at Info — it is a prompt to decide, and it
  clears by declaring.
- **A baseline several resolutions old is not stale.** Divergence is normal and expected; it is not
  a violation.
- **A resolution older than your seat is inherited, not abstained from.** You cannot have sat out a
  settlement that predates your registration, including the resolution that registered you.

Write the merge with git's own subject — which the commit check exempts — and put the declaration
in a trailer:

```
git switch ma/<elector>
git merge --no-ff --no-edit main
git commit --amend --no-edit --trailer "Baseline: rigpa/<evolution>@<short-hash>"
```

The merge is not an adoption and wants no verb of its own; the trailer is the adoption, and it is
what a later reader can check. See
[RFC-0011](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0011-partial-sangha.md).

Until a branch declares one, `elector-baseline-undeclared` reports at Info **and names the
evolution the branch holds through** — which is derivable from the settlements, and is what to
write in the trailer.
