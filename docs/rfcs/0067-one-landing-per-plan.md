# RFC-0067 — A plan lands once, as one commit per verb

- **Status:** Implemented
- **Track:** I50
- **Relates to:**
  - #1536 (a whole-manifest `yidam run` commits once per capability), parent epic #1513
  - #1497, which this was split from. It is now about receipt-only commits, and RFC-0052
    answered it with notes.
  - RFC-0026 §1, §2 and §5 (a run is a commit somebody can refuse, and how it writes)
  - RFC-0052 §2.2 and §2.5 (a re-run that reproduced its receipt writes a note, not a commit)
- **Versioning layers touched:**
  - **CLI.** `yidam run` and `yidam run <step>` land a plan's operational steps as one commit
    per contiguous run of one verb, and move the branch once (§1, §2). A plan that fails
    lands nothing on the branch (§4). There is no new flag.
  - **Receipts.** The format does not change. What `input.commit` names changes for a step
    that is not first in its group (§3).
  - **Reports.** No field is added. A step's `committed` names the commit that carries it,
    so steps in one group share one. `committed` keeps its meaning: how many steps landed.
  - **Prelude.** None. No file that `yidam-vendor-update` copies changes shape.
  - **Docs.** The run section of the CLI reference says what one invocation lands.
  - **The bump.** It is minor for the CLI. No authored file moves. An upgrade note is owed,
    because a repository's log changes shape and a failed plan no longer leaves its earlier
    steps landed (Migration).
- **Downstream reference cases:**
  - allen-county-ohio-democratic-party. Its evidence is quoted from #1497 and #1536, since
    no local checkout was read and nothing was fetched.

## Summary

A whole-manifest `yidam run` lands one commit for every step that ran, and moves the branch
once per step. The chain is deliberate: a step is materialized from the tree its upstream
wrote. The cost is one commit per capability, every time.

This RFC keeps the chain and drops the per-step ref move:

- Each step still runs against the tree its upstream produced. That tree is a commit object
  no ref points at (§1).
- Consecutive operational steps with one verb land as one commit. A calculator-only plan is
  one commit (§2).
- The branch moves once, through the compare-and-swap that exists now (§2).
- A receipt's `input.commit` names a commit that is on the branch (§3).
- A plan that fails lands nothing on the branch (§4).

## Problem

### 0.1 What was measured

The 2026-11-18 cross-corpus review recorded this (#1497, #1536):

- **allen-county-ohio-democratic-party** made 12 whole-manifest runs of 8 calculators. Each
  run landed 8 commits, one per calculator.
- Many of those were receipt-only. RFC-0052 has since made a reproduced receipt a note, so
  that part of the count is gone. What is left is a run where several calculators did change
  output. That run still lands one commit per calculator.

These numbers are from #1497 and were not re-measured for this RFC.

### 0.2 Why the commits exist

`plan_and_write` (`cmd/run/mod.rs`) walks the plan in order. For each step it calls
`head(root)` again, materializes that step's inputs from the new HEAD, invokes it, and calls
`commit`. `commit` builds the commit on a temporary index and moves the ref with one
`git update-ref <ref> <new> <expected>`.

The module header gives the reason. `after` means something only because *"the upstream's
output is in the tree the downstream is given because the upstream committed it a moment
earlier."*

So the property and the cost have one cause. A step that is invoked against what its
upstream wrote needs that write to exist as a tree first. The tree is what the property
needs. The ref move is not.

### 0.3 What already exists

- **`build_commit` writes objects only.** It returns the commit sha and moves no ref.
  `commit` is `build_commit` followed by the compare-and-swap.
- **`exec::materialize` reads from a commit sha**, not from a ref. It is handed `full`,
  which today is HEAD.
- **`freshness` and `outputs_already_committed` read at `parent`**, which is a sha.
- **Epistemic steps are ordered last** (`Manifest::plan`), and none may be an `after`
  dependency of another step (`check_graph`). So every operational step runs before the
  first epistemic one.

Nothing in the step loop needs the branch to have moved. It needs a sha to read from.

## Design

### 1. Each step runs against the tree its upstream produced

The loop keeps a **tip**: a commit sha, set to HEAD before the first step. Each step:

1. materializes its inputs from the tip;
2. reads freshness and `outputs_already_committed` at the tip;
3. is invoked, checked, and given a receipt, as now;
4. calls `build_commit` on the tip with `fold` off, and the result becomes the new tip.

No ref moves in this loop. No REGEN block is folded here. The group commit folds once (§2). The tip is a commit object that no ref points at. A downstream
step is therefore given exactly the tree its upstream wrote, which is the property the
module header states.

A step whose tree equals the tip's (`build_commit` returns `None`) leaves the tip where it
was, as it leaves HEAD now.

### 2. One commit per verb run, and one ref move

A **landing step** is an operational step whose `build_commit` in §1 changed the tip. A step
that was fresh, that wrote a note, or whose tree equalled the tip's is not one. It joins no
group. A fresh step never splits one. A stale step with another verb does, for the reason the
base gives below.

A **group** is a maximal run of consecutive landing steps in plan order that share one verb.
A plan of eight `compute` steps is one group. A plan of `compute`, `compute`, `reconcile`,
`compute` is three groups if all four land. If the `reconcile` was fresh, it is one group.

The **base** is the last group commit built so far, or HEAD before the plan if none is.
A group's commit is built before a stale step with another verb is invoked, or after the last
operational step. So the base is known when each step runs, which it must be, because
`YIDAM_INPUT_COMMIT` is exported before the step writes anything. That step is then checked out
again from the new base. If it writes a note instead of landing, the group it ended stays closed.

Plan order is not changed to make groups larger. It is the order the steps ran in, and a
commit's position in the log should say that.

At the end of each group, one commit is built:

- its tree is the tip's tree, with the REGEN blocks the group's combined landing made stale
  refreshed in it (RFC-0054 §2.2). This is one fold over every file the group wrote, against
  the base, so no `regen:` commit has to follow and the generators run once per group;
- its parent is the base;
- its author and committer follow RFC-0026 §5, as now.

The intermediate commits from §1 are not ancestors of it. They become unreachable, and
`git gc` collects them.

After the last group, the branch moves once:

```
git update-ref refs/heads/<branch> <last group commit> <HEAD before the plan>
```

This is the compare-and-swap `commit` uses now. If the branch moved during the plan, nothing
lands, and the error says `<branch> moved while the plan was running; nothing was landed`.

**The subject.** A group of one step keeps today's subject:

```
compute: seat-coverage — 2 files at c96a1dd
```

A group of several steps names the count:

```
compute: 8 steps — 23 files at c96a1dd
```

The short sha is the group commit's parent. `yidam vocabulary --check` accepts both. The
body lists each step with its files and their sha256. It says what each step read. The first
step ran against the parent. A later step ran against the parent and what the steps above it
in the body wrote. Then comes the REGEN paragraph when the fold refreshed anything, and the
operational paragraph that `closing` writes now.

**Notes.** A step that reproduced its receipt writes a note under RFC-0052 §2.2. The note is
attached to that step's receipt blob at the tip. Only that step writes its receipt, so it is
the blob the branch holds after the landing. The note records the base as its input commit,
which is on the branch after the landing. It never records the tip, which is not kept.

Notes are written once every operational step has succeeded, after the branch move if
there is one. A plan in which every stale step reproduced its receipt builds no group
commit and moves no ref. It still writes its notes, after checking that the branch is
still at HEAD before the plan. If the branch moved, or a step failed, it writes no notes.

**Epistemic steps** run after the branch moves, as they run after the operational steps now.
They are materialized from the new HEAD, and each lands on `propose/<head>` as now. They are
never part of a group.

### 3. `input.commit` names a commit on the branch

A step's receipt names `input.commit`. `YIDAM_INPUT_COMMIT` exports the same value.

Under this RFC, it is the base when the step runs, which is the parent of the step's group
commit. That commit is on the branch.

- For the first step of a group, that is exactly the tree the step was given.
- For a later step in the group, the step was also given what the earlier steps wrote. Those
  files are in the receipt's `input.files` with their digests, so the record of what was
  read is complete.

The intermediate commit is not named, because it is not kept. Keeping it would need a ref
per step, which is the clutter this RFC removes.

`input_state`, which freshness compares, is computed from the files read. It does not
change. RFC-0052 §2.6's stamp check compares an output against `input.commit`, and it still
does, because the exported value and the recorded value are the same.

### 4. A plan that fails lands nothing

Today a step that fails leaves every step before it landed. Under this RFC, the branch moves
only after the last operational step succeeds. A failure in any operational step:

- moves no ref and writes no note;
- names the step that failed;
- says that nothing was landed.

Re-running recovers the work of a calculator that is a function of its inputs. It reproduces
its output on the next run, and the next run lands it with the rest. A step that declares
`ageing_days` is not such a function. Its next run may compute a different answer, so its
earlier answer is lost, not recovered.

The cost is a wider window. Today a branch that moves during a run loses the step in
flight. Under this RFC it loses every step the plan had finished, because the one
compare-and-swap comes at the end. A long plan on a busy branch pays this.

The pre-pass already holds this rule for a step this binary does not invoke. This extends it
to a step that fails while it runs.

An epistemic step runs after the branch moved. Its failure leaves the operational landing in
place, and the error says so, as `require_room`'s context does now.

## What this does not do

- **No change on a cluster.** `cluster/step.rs` runs one step per pod and calls
  `build_commit` itself. A workflow's landing is a separate question.
- **No change to what a single step lands.** `yidam run <step>` with no stale upstream is a
  plan of one step, and lands what it lands now.
- **No reordering.** Groups follow plan order, so a mixed plan can land more commits than it
  has verbs.
- **No new verb.** A mixed plan is not given a run-level verb. Each commit keeps one verb
  from the closed vocabulary.

## Alternatives considered

- **One commit per plan, with a run-level verb.** This gives the fewest commits. It was
  rejected because the vocabulary is closed, and a new verb would be a vocabulary change for
  bookkeeping.
- **One commit per verb, with the plan regrouped.** A topological sort that keeps
  same-verb steps together where `after` allows would land fewer commits. It was rejected
  because the log would stop showing the order the steps ran in.
- **Name the intermediate commit in `input.commit`, and keep it under a ref.** This makes
  `input.commit` exactly the tree the step saw. It needs a ref per step, such as
  `refs/yidam/runs/…`, and something to prune them. The receipt's input digests already
  record what was read.
- **Land what completed before a failure.** This keeps today's partial progress. It was
  rejected because a plan could again leave the corpus half-advanced. The next run recovers
  a calculator's work. It does not recover an ageing step's earlier answer, and §4 counts
  that as a cost.
- **An opt-in `--batch` flag.** It was rejected because it keeps two landing behaviours.
  The new one is worse in one case, the wider window in §4. That case is not worth a second
  behaviour to maintain.

## Migration

- **No authored file changes.** Existing receipts load unchanged, and the next run that
  changes them writes the same format.
- **The log changes shape.** A plan that landed eight `compute:` commits now lands one. A
  tool that finds a step's commit by its subject (`: <step> `) finds it only for a group of
  one. The body names every step.
- **A failed plan lands nothing.** A repository that relied on a partial landing re-runs
  after the fix. An ageing step that had finished runs again, and a branch that moves
  during a plan discards every finished step.

The upgrade note says these three things.

## Falsifier

This is wrong if any of these is observed after it ships:

- a whole-manifest run in which every landing step is `compute`, which lands more than one
  commit on the branch;
- a run in which every stale step reproduced its receipt and the branch did not move, which
  writes no notes;
- a note whose recorded input commit is not an ancestor of the branch after the run;
- a group commit whose REGEN blocks are stale for the files it wrote;
- a dependent step whose materialized inputs lack what its `after` upstream wrote in the
  same plan;
- a run that moves the branch more than once;
- a run in which the branch moved during the plan, which lands anything;
- a failed operational step that leaves any commit or note from the same run;
- a receipt whose `input.commit` is not an ancestor of the branch after the landing;
- an epistemic commit on the branch, or an epistemic step in a group.

## Unresolved

- **Whether a group should cap its size.** A plan of many steps lands one commit with a long
  body. No corpus has a plan large enough to show whether that hurts review.
