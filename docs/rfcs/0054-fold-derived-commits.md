# RFC-0054 — A derived update rides in the commit that caused it

- **Status:** Draft
- **Track:** I38
- **Relates to:**
  - #1499 (derived updates are their own commits; the evidence this answers), parent epic #1513
  - #1498 / RFC-0053 (removed `reconcile:` instead of folding it, which settles one of #1499's three kinds)
  - #1452 (`phase settle --merge`, whose licence §2.1 rewords)
  - #1465 and #1392 (the practice document's checkpoints, and why a merge is never one)
  - #1215 (`regen --commit`, beside which `--fold` sits)
- **Versioning layers touched:** tooling (`phase settle --merge` writes one commit instead of two; `regen` gains `--fold`; local `yidam run` folds its refresh; the practice generator's population moves to strict ancestry). Template: `prelude/PHASES.md`'s settle sequence. Report contract stays at 1: `settle`'s `merged.regen` field is kept and is always null after this. No MCP contract change. No SDK change.
- **Downstream reference cases:** city-engine, measured read-only on 2026-11-21 at `origin/main` (c5236c66). ohio-education-funding was measured at ab0cdb32 the same day. allen-county-ohio-democratic-party's counts are quoted from #1499, because no clone of it is on the measuring machine.

## Summary

A phase in a derived repository is a branch of work closed by a `--no-ff` merge. In
city-engine a phase averages 9.4 commits, and 1.8 of them are `regen:`. Those commits refresh
REGEN blocks, the generated sections of README, PRACTICE.md and the corpus and catalog
indexes. Nobody chose their content. It is a function of the commit before them, and they
exist only because the commit before them did not carry it.

This RFC makes the commit that changes the tree carry the refresh, in three places:

1. `phase settle --merge` writes the refreshed blocks into the merge, not into a `regen:`
   commit before it (§2.1).
2. `yidam run` writes them into the `compute:` commit it already authors (§2.2).
3. `yidam regen --fold` amends them into HEAD for work a person or an agent committed (§2.3).

The practice generator reads git history, so folding into an authored commit makes its block
name the commit carrying it. §2.4 moves its population to the carrier's strict ancestors, so
the fold converges.

`scaffold:` commits are not folded. §4 says why, and leaves the cheaper half open.

## Problem

### 1.1 What the commits are

Each `phase` merge's commits, `m^1..m^2`, were read on city-engine's `origin/main`. All 448
first-parent merges are phase merges.

| | per phase |
|---|---|
| commits, the merge included | 9.4 mean, 9 median |
| `regen:` | 2.1 in October, 1.7 in November |
| `scaffold:` | 0.9 |
| `compute:` (`yidam run`) | 0.7 |
| non-ceremony (not regen, scaffold, refresh, reconcile or practice) | 4 median; 17 phases have exactly 1 |

The commit before the merge is `regen:` in 447 of 448 phases. 675 of the 819 `regen:`
commits inside phases are authored by `yidam regen`, so they came from `regen --commit`, and
144 are a person's. Their subjects are `regen: refresh practice` (349), `regen: refresh N
generators' blocks` (232), and the rest name two or three generators. They touch PRACTICE.md
818 times, README.md 462 times, `.yidam/corpus/README.md` 458 times and
`.yidam/catalog/README.md` 348 times.

The commit before each `regen:` says what it refreshes after:

| before the `regen:` | count |
|---|---|
| `compute:`, a `yidam run` landing | 306 |
| a person's or agent's own work (`OTHER`, `revise:`, `establish:`, `docs:`) | 489 |
| `refresh:` | 16 |

So the shape is: work, then `regen`, then `compute`, then `regen`, then merge. The most
common phase, 85 of them, is `scaffold OTHER refresh revise OTHER regen compute regen`.

None of city-engine's merges were written by `settle --merge`. All 448 are authored by a
person, and no subject reads `refreshed to settle`. The refresh before the merge is the
hand workflow's, and §2.1 has to serve both routes.

ohio-education-funding shows the other kind. It has no `regen:` and no `scaffold:` inside a
phase. Its ceremony was 37 `reconcile:` commits, which RFC-0053 has since removed. #1499
counts allen-county-ohio-democratic-party at about 69% ceremony, 1,274 of 1,853 commits, with
a median of 2 non-ceremony commits per phase and 86 of 246 phases holding exactly one.

### 1.2 Why each is its own commit today

Each one is a separate commit for a reason that was true when it was written.

- **`settle --merge`.** The module doc says `git merge --no-ff` refuses a dirty index, so the
  refresh has to be committed on the phase branch first. But `write_merge` does not run
  `git merge`. It builds the merge with `commit-tree` from the tip's tree. The constraint is
  the hand route's, inherited by the tool. What the tool actually holds to is its licence:
  the merge's tree is the tip's tree, *"and nothing was resolved"*.
- **`yidam run`.** A `compute:` commit lands what the step wrote, and `regen` was never asked
  about it. Nothing refused the fold. Nothing did it.
- **A person's commit.** `regen --commit` (#1215) authors a separate commit on purpose. A
  person's commit is theirs, and a tool that rewrote it would be putting words under their
  name. That argument is sound for content. §2.3 argues that it does not reach a block that
  is regenerated, never authored.
- **The practice block.** It is history-derived. Its fixed point is that the commit carrying
  it is a `regen:` commit, and `regen:` commits are outside the population it measures. Fold
  it into an authored commit and that commit is in the population. If it lands on a
  checkpoint, the block names the sha of the commit that carries it, and amending changes
  that sha. So the gate could never pass. This is the one real obstacle, and §2.4 removes it.

## Proposal

### 2.1 `settle --merge` writes the blocks into the merge

`write_merge` builds the merge's tree from the index, not from the tip:

1. `settle` refreshes and stages the stale blocks, as it does now.
2. `--merge` runs `git write-tree` over that index. The checkout check already refuses
   anything staged that `settle` did not stage.
3. It writes the merge with that tree and the parents it has today: the baseline, then the
   tip.

There is no `regen:` commit on the phase branch. The phase branch is deleted after the merge,
as it is now.

The licence is reworded, not weakened. It reads: *the merge introduces nothing but what regen
derives from the tip.* That is checked, not asserted, on two counts:

- **Only staged block files differ.** The diff from the tip's tree to the merge's tree lists
  only the files `settle` staged. `foreign` is computed already. This holds it on the other
  side.
- **The blocks are current.** `regen --check` over the merge's tree passes. `settle`'s second
  check already asks this of the staged index, and the index is the merge's tree.

The ancestor refusal stays, and it is what makes the second count sufficient. The baseline is
an ancestor of the tip, so every block the merge carries is a function of a history that
contains no commit the phase did not see.

`settle` without `--merge` stages the blocks and moves no ref, as
`settle_authors_no_commit_and_moves_no_ref` pins. Its printed sequence changes from "commit
the `regen:`, then merge" to:

```
yidam regen --fold
git switch <baseline> && git merge --no-ff phase/<slug>
```

On the hand route the blocks ride in the phase's last commit, not the merge, because
`git merge` will not take them. That is the route city-engine uses, and it removes the same
commit.

The `merged.regen` field in `settle --format json` is kept, and is always null. A field with
nothing to report is a report-contract change at 1 that no consumer breaks on.

### 2.2 `yidam run` folds its refresh into the `compute:` commit

`run` writes the step's outputs to the working tree and commits them under a pathspec through
`operational::author` (`Writer::WorkingTree`). Before that commit it asks the same two
questions `settle` asks: which blocks are stale with the outputs written, and are they still
stale after a quiet refresh? It refreshes them and adds their files to the pathspec. The
subject is unchanged. The body gets one line per refreshed block, in the shape
`regen --commit`'s body already uses.

`require_clean` is asked about the block files as well as the outputs, before anything is
written. That is the same refusal for the same reason: afterwards a hand edit and a refreshed
block are the same diff. When a block file is dirty, `run` lands the step without the fold
and prints `regen --fold` as the next step. It does not refuse the step, because a step that
computed correctly should not be lost to a README somebody is editing.

A shallow clone cannot generate a block (`require_whole_history`). There `run` lands without
the fold and says so. That covers `Writer::Detached` on a cluster today. Whether the lander
should fold is §6's first question.

Both commits are operational and tool-authored. Nothing about the `compute:` commit's licence
changes. Its body already says nothing was synthesized, and a regenerated block is not
synthesis.

### 2.3 `yidam regen --fold` amends HEAD

`--fold` refreshes every stale block and amends HEAD with exactly those files. The author,
author date and subject are kept. One trailer is appended, `Regen: <generators>`, naming the
generators whose blocks moved. The committer is whoever ran it.

It refuses, and writes nothing, when:

- **HEAD is reachable from any ref but the current branch.** That covers remote-tracking refs,
  so a pushed commit is never rewritten. It also covers another local branch, so an amend
  never forks a history somebody else is standing on. The remedy is `regen --commit`, which
  is still there.
- **HEAD is a merge.** `settle --merge` is how a merge gets its blocks, and an amended merge
  is a tree nobody checked.
- **A block file differs from HEAD.** This is `require_clean` over the same paths, for the
  reason §2.2 gives.
- **Nothing is stale.** It says so and exits 0, the way `--commit` does.

**Why amending a person's commit is licensed.** The amend changes only what is inside REGEN
markers. Those regions are regenerated, never authored: a hand edit inside one is
exactly what `regen --check` fails. So the amend cannot put anything under the author's name
that the author could have written. It moves the commit's tree to the one the gate would have
demanded anyway.

The same holds for an epistemic commit. A `revise:` or `establish:` whose README block is
refreshed has had no claim, node or tag changed. RFC-0026's line runs between operational
and epistemic *content*, and a block is neither. The trailer is there so a reader of the
commit can see which part the tool wrote.

**Why not a pre-commit hook.** #1499 offers a hook from the scaffold as one form. It would
run the same refresh, and it is rejected as the mechanism for three reasons:

- **A hook is per-clone.** `.git/hooks` is not versioned, and `core.hooksPath` has to be set
  in every clone, which a fresh CI checkout does not do.
- **Agents skip it.** `--no-verify` is one flag away, and an agent that hits a slow hook
  learns to pass it.
- **It runs on every commit.** That includes the commits a refresh does not need, and
  practice reads the whole history each time.

`--fold` is what a hook would call. A repository that wants the hook can write one line. The
scaffold does not install it.

### 2.4 The practice block measures the history behind its carrier

Today the practice population is the authored, non-`regen:` commits reachable from HEAD. The
fixed point is that the carrier is a `regen:` commit, so the carrier is never in the
population.

This RFC moves the population to **the commits reachable from the carrier's parents**: the
carrier's strict ancestors. The block in commit C is a function of `C^@`. That holds whether
C is a `regen:` commit, a `compute:`, a person's `revise:` with a folded block, or a merge.
So:

- **The fold converges.** C's own sha is never a sampled point in C's own block, so amending
  C cannot make the block stale.
- **The `regen:` exclusion stays.** It still keeps the separate route from counting its own
  ceremony, and dropping it would move every existing document.
- **A merge can carry it.** A merge's strict ancestors are both parents. A merge is still
  never a sampled point (#1392). But the reason #1392 gives, that no `regen:` can follow a
  merge, is gone. With §2.1, the merge carries the block itself.

The document lags its carrier by one commit. That is the only reading that can be committed
in the commit it describes, and `kuten check`, which writes nothing, still reads the newest.

Every existing PRACTICE.md block is stale by at most one checkpoint after this change. The
first fold or `regen --commit` after an upgrade refreshes it. §5 says so in the upgrade note.

No other generator reads commit history. `status` reads the tree. `genesis` reads the first
commit, which no amend can change. The implementation keeps that true with a test, not with
this sentence: each generator runs against a commit and its amend, and their blocks must be
byte-identical.

## What this does not touch

- **`regen --commit`.** It still writes a separate `regen:` commit. That is the right tool
  after a push, and for a block that went stale on main without any phase.
- **The CI gate.** `regen --check` asks the same question of the same tree.
- **`scaffold:` commits.** §4.
- **`refresh:` commits.** In city-engine 528 of these sit inside phases, as hand-written tonpa
  re-pins and probe re-reads. They record a decision to re-read something. They are not
  derived from the commit before them, so there is nothing to fold.

## Migration & compatibility

- **There is no `migrate` operation.** History is not rewritten. Old `regen:` commits stay
  where they are, and the new shape starts at the next phase.
- **An upgrade note under `.changes/upgrading/`** names the new printed `settle` sequence,
  `regen --fold`, and the one-commit staleness of PRACTICE.md from §2.4.
- **`PHASES.md`'s closing sequence** in the prelude changes to §2.1's two lines.
- **The parity fixtures carry no practice block.** No SDK golden moves. The CLI's practice
  goldens are regenerated for §2.4.

## Alternatives considered

- **Squash the phase on merge.** One commit per phase is the smallest number, but it
  destroys the per-commit epistemic record PHASES.md is built on. `log`, `practice` and
  `replay` all read individual verbs.
- **Fold the regen into the phase's last authored commit inside `settle --merge`.** This
  is the same effect as §2.1. But it rewrites a person's commit as a side effect of a command
  that says it merges. The merge is the tool's own commit, so the refresh belongs there.
- **Don't commit REGEN blocks; generate them at read time.** This would end the ceremony
  entirely. But a README on a forge is read without running anything, and #647's contract
  exists because the committed form is the one people read.
- **`git notes` for the derived blocks.** Notes are not fetched by default and are not shown
  on a forge. That is the record-on-a-ref problem in §4 again.

## 4. Why `scaffold:` is not folded

There are 391 `scaffold:` commits in city-engine's phases, almost all of them `phase start`'s
one per phase. Two different things block a fold:

- **`phase start`.** The record has to be on a ref, because `yidam phases` reads
  remote-tracking refs (RFC-0028 §3). It must also not move the checkout, because the module
  inherits RFC-0026 §5's properties from `propose`. The only commit `start` could fold into
  is the phase's first authored one, which does not exist yet and is somebody else's to
  write. Staging the record for that commit would mean switching the checkout, which `start`
  avoids on purpose.
- **`phase run`'s per-step `scaffold:`.** The record names the sha of the step's landing, so
  it cannot ride in that landing. That is the same self-reference §2.4 removes from practice.
  city-engine has none of these, because its `compute:` commits come from `yidam run`
  directly.

So `phase start`'s scaffold stays as one structural commit per phase, constant and readable.
§6 keeps the second case open, because RFC-0053's move would answer it: derive the sha, don't
store it.

## Expected effect, and the falsifier

On city-engine's 448 phases, every one of the 819 `regen:` commits follows a commit this RFC
covers:

- 306 follow a `compute:`, which §2.2 folds.
- 489 follow authored work, which §2.3 folds. When that work is a phase's last commit,
  §2.1 folds it as well.
- 16 follow a `refresh:`, which is an unpushed non-merge like any other, so §2.3 folds it.

The 447 closing `regen:` commits are among these, not added to them. Removing all 819 takes
the phase from 9.4 commits to about 7.6. Inside phases, the ceremony share, counting
`regen`, `scaffold` and `refresh`, falls from 1,738 of 3,763 commits (46%) to 919 of 2,944
(31%).

The falsifier is #1499's own. On the first corpus to adopt this, measure commits per phase
and non-ceremony commits per phase with the classifier used in §1.1. Then compare against
allen-county-ohio-democratic-party's median of 2 non-ceremony commits in about 6 total. If
`regen:` commits inside phases stay above 0.5 per phase after 50 phases, the fold is not
being reached, and the printed sequences are the thing to fix.

## Open questions

1. **Should the cluster lander fold?** A pod's clone may be shallow, and the lander holds the
   only ref credential. Folding there means the lander runs `regen` over a whole-history
   clone before it lands. That is a new duty for the one component that is meant to decide
   and not compute.
2. **Can `phase run`'s step record derive the landing's sha?** It could find the commit that
   introduced the step's entry, the way RFC-0053 derives `used-by` from the citations. If it
   can, the per-step `scaffold:` folds into the step's landing, and only `phase start`'s
   stays.
3. **Should `--fold` be the default printed remedy of `regen --check`?** It cannot be right
   after a push. `--check` could name whichever of `--fold` and `--commit` HEAD's reachability
   allows.
