# RFC-0052 — A re-run that reproduces its outputs is recorded, not committed (receipt re-validation)

- **Status:** Implemented
- **Track:** I38
- **Relates to:**
  - RFC-0026 (the orchestrator layer: the receipt, its input state, and the lander that holds the only credential for a ref)
  - #1162 (a proposal that reproduces itself lands nothing, the precedent for a run that writes no commit)
  - #1497 (the evidence this answers), #1536 (one commit per whole-manifest run, kept separate), parent epic #1513
- **Versioning layers touched:** template (`guidelines/directories.md` says what `YIDAM_INPUT_COMMIT` is for, and what it is not for) / tooling (`run`'s freshness, `doctor`'s standing, one doctor check). No receipt format change: the receipt a run commits is byte-for-byte what it commits today. No MCP change. No SDK parity change.
- **Downstream reference cases:** allen-county-ohio-democratic-party (pinned v0.16.0), whose receipt-only commits #1497 counts, and city-engine, measured read-only on 2026-11-20 at its v0.15.0 pin.
- **Amended 2026-11-27 (#1598, #1600):** §2.7 lets a step declare an output whose consumer
  requires the input commit. `doctor` reports that output as declared rather than warning, and
  `run` judges its reproduction with the commit masked. One manifest key, `stamps_input_commit`.

## Summary

A step re-runs when its input state moves. When it produces the same bytes it produced last
time, `yidam run` still commits, because the receipt changed: it names a new input state and a
new input commit. In allen-county-ohio-democratic-party, 24 of 52 commits since its v0.16 re-pin
are that commit and nothing else.

Skipping the commit is not enough on its own. Freshness reads the committed receipt, so a step
whose re-validation was never committed is stale forever, and `run` would re-invoke it on every
call. This RFC records the re-validation in a git note attached to the committed receipt. The
note lists each input state that is known to reproduce that receipt's outputs. Freshness accepts
a state the note lists as well as the one the receipt names. The note's own commit carries the
date that `ageing_days` measures, so the receipt still carries no clock.

A second finding limits what the first can do. city-engine's calculator writes
`$YIDAM_INPUT_COMMIT` into its own output. That output is never byte-identical to the last one,
so no rule keyed on identical bytes would ever fire for it. 279 of its 353 `compute:` commits
change nothing but that stamp. The RFC adds a doctor check for an output that stamps its own
input commit.

## Problem

### 1.1 Why a step re-runs, and why a re-run commits

[`Receipt::input_state`](../../yidam/cli/src/cmd/run/receipt.rs#L251) hashes
what a step declares and reads: its kind, verb and run, the manifest and config digests, its
declared reads and the digest of each file they match, the resolved graph, the script, and its
declared writes. It does not hash `input.commit`. A head that moves without touching anything
the step reads leaves the input state where it was, [`freshness`](../../yidam/cli/src/cmd/run/mod.rs#L878) reports the step fresh, and nothing
runs. That part works.

A step that declares a broad read is a different case. allen-county-ohio-democratic-party's
`seat-coverage` reads `crates/local-seat-targets/src/lib.rs`, so an edit to that file moves its
input state. The step runs again and writes the same `.yidam/computed/seat-coverage.yml`. Its
receipt does change, in three fields: `input_state`, `input.commit`, and the digest of the one
read. [`commit`](../../yidam/cli/src/cmd/run/mod.rs#L1662) decides from the tree
alone, and the tree differs by the receipt. So `b634a8f compute: seat-coverage — 2 files at
c96a1dd` lands, changing one file under `.yidam/runs/`.

The run already knows. It sets [`unchanged_outputs`](../../yidam/cli/src/cmd/run/mod.rs#L542) from
[`outputs_already_committed`](../../yidam/cli/src/cmd/run/mod.rs#L1350), which
compares object ids. That function's own doc says the commit is written anyway, *"so the check is
recorded whether or not the answer moved."* The commit is the record of having looked. It is
the only record there is.

### 1.2 Why the commit cannot simply be skipped

`freshness` compares the current input state with the one in the receipt at the parent commit.
If a run that reproduced its outputs wrote no commit, that comparison would fail on the next
call, the step would run again, reproduce its outputs again, and write nothing again. `doctor`'s
[`standing`](../../yidam/cli/src/cmd/run/mod.rs#L1005) reads the same receipt and
would report the step stale for as long as nobody changed its outputs. Skipping the commit trades
one bookkeeping commit for an invocation that never ends and a doctor finding that is never
true.

The re-validation therefore needs a record. The question this RFC answers is where that record
lives, if not in the branch's history.

### 1.3 Ageing

A step with `ageing_days` re-runs once its receipt is old enough, even with no input change
([`aged`](../../yidam/cli/src/cmd/run/mod.rs#L1218)). The age is the committer date
of the commit that last touched the receipt ([`receipt_age`](../../yidam/cli/src/cmd/run/mod.rs#L940)). The receipt carries no date of its own,
and the module doc's "No clock" section says why
([`receipt.rs:18-24`](../../yidam/cli/src/cmd/run/receipt.rs#L18-L24)): *"a second date written
into the file would be the same fact recorded twice and the only copy anyone could forge."* An aged re-run that reproduces its
outputs commits a receipt naming the new head. That commit is the only thing that resets the
clock. A record outside history has to carry a date that `aged` can read, and that date must not
be a field a person can write.

### 1.4 An output that stamps its own input commit

The step's environment carries [`YIDAM_INPUT_COMMIT`](../../yidam/cli/src/cmd/run/exec.rs#L470), the commit the step read. city-engine's
`engine-state` calculator writes it into the output it produces, as `input_commit:`. Measured
over the whole of city-engine's history on 2026-11-20:

| Measure (city-engine, v0.15.0 pin, 5,048 commits) | Count |
|---|---:|
| `compute:` commits, 2026-10-19 to 2026-11-18, all from `engine-state` | 353 |
| … that change only `.yidam/runs/` (receipt-only) | 0 |
| … whose only change under `.yidam/computed/` is an `input_commit:` line | 279 |
| … that change a value | 74 |

So city-engine shows none of #1497's receipt-only commits, and the reason is that its output
changes on every run. 79% of its `compute:` commits record that the calculator looked again and
found the same answer. A fix keyed on byte-identical outputs would not change any of them.
cellular-automata's 117 recomputes after its cli/v0.24.0 re-vendor, none of which changed a
value, are the same shape. Nothing yidam reads needs the stamp in the output. The receipt beside
the output already records `input.commit`, and the commit that lands them both is itself dated
and addressed. A consumer of the corpus may read it there, which is why §2.6 warns rather than
fails.

## Proposal

### 2.1 The record: a note on the committed receipt

A re-validation is written to the notes ref `refs/notes/yidam-runs`. The note is attached to the
object id of the committed receipt blob, the one at `<parent>:.yidam/runs/<step>.yml`. It holds
one line per entry:

```text
format_version 1
reproduced 9f2c… c96a1dd…
```

Each `reproduced` line names the input state that was run, then the head it was run at. The
format is lines, not YAML, so that two copies of a note merge by sorting their lines and
dropping duplicates (§2.4). `format_version` sorts ahead of `reproduced`, so a merged note still
opens with it.

A note is attached to the blob, not to a commit, for three reasons:

- **It survives commits that do not touch the receipt.** A blob's id is its content. A later
  commit that leaves the receipt alone leaves the same blob at the head, and the note is still
  found.
- **It dies with the receipt.** The next run that changes an output commits a new receipt, which
  is a new blob with no note. The old note is still in the notes ref and no longer read by
  anything. No code has to invalidate it.
- **It cannot vouch for the wrong step.** The receipt names its step, so two steps never share a
  blob.

A note lives in its own ref, so the branch's history is unchanged and `git log` on the branch
shows only runs that changed an output. Every write to the notes ref is a commit with a committer
date, written by git, which is the date §2.3 reads.

### 2.2 What `run` does

A step that is not fresh runs. It reproduced the committed receipt when all three of these hold:

- the parent holds a receipt for the step;
- `outputs_already_committed` is true, so every file it produced is the parent's bytes;
- the receipt it would write equals the parent's receipt in every field outside `input_state`
  and `input`.

The third test is the one that matters. Identical bytes are not enough. A receipt also records
what produced the output and which files it wrote: `format_version`, `outputs`, `version`,
`model`, `config` and `image_digest`. None of these is in the input state. After a CLI upgrade,
a gluon step's `version` moves while its bytes stay the same. A step that wrote three files and
now writes two passes `outputs_already_committed`, because both files match, but its `outputs`
list changed. In either case the committed receipt would no longer be true, so the run commits
it, as now. The fields this test leaves out are the ones a re-validation is expected to move:
the input state, the head, and the digests of what was read.

When the step reproduced the receipt, and the route is the branch (not a proposal, §2.5), `run`:

1. writes no commit to the branch;
2. appends a `reproduced` line to the note on the parent's receipt blob, as one commit
   to `refs/notes/yidam-runs`, with the same compare-and-swap on the notes ref that `commit` uses
   on the branch;
3. reports the step as `Ran`, `unchanged_outputs: true`, `committed: None`, and names the note.

The summary line counts it as a step that ran with no commit written, which is the shape #1162
gave a proposal that reproduced itself.

When any of the three fails, nothing changes: the outputs and the receipt land as one commit, as
now. That includes a parent with outputs but no receipt, where there is no blob to attach a note
to.

### 2.3 What freshness reads

`freshness` and `standing` read the note on the committed receipt blob, if there is one.

- **Input state.** The step is fresh on input if the current state is the receipt's, as now, or
  is one the note lists.
- **Age.** `aged` measures from the later of two dates: the committer date of the commit that last
  touched the receipt (as now), and the committer date of the notes commit that last appended to
  this note. An aged re-run that reproduces its outputs appends an entry for the current state,
  whether or not that state is already listed. That notes commit is what resets the clock.

Both dates are written by git, on a commit. Neither is a field in a file. The receipt's "No clock"
section stays true, and its last sentence becomes the general rule rather than a special case: a
re-run against an unchanged corpus lands no commit, and neither does a re-run against a changed
one that produces the same bytes.

### 2.4 A clone without the note

`git fetch` and `git push` do not carry `refs/notes/*` unless asked. A clone that lacks the note
reads the committed receipt alone. It sees the step stale, re-runs it, reproduces the outputs,
and writes its own note. Being wrong in that direction costs one invocation. It never claims
something is fresh that is not, which is the direction `freshness` already chooses for a clock it
cannot read. `doctor` on such a clone reports the step stale, which is today's answer.

`yidam run` pushes nothing today, and does not start pushing here. Sharing the notes ref between
clones and CI is something the corpus sets up. Two clones that each re-run a step write two notes
commits on two histories, so the ref diverges as soon as it is shared. Fetching straight into
`refs/notes/yidam-runs` cannot work. A forced refspec replaces the local ref and loses the local
entries. An unforced one is refused as non-fast-forward. So the corpus fetches into a ref of its
own and merges:

```sh
git fetch origin refs/notes/yidam-runs:refs/notes-remote/origin/yidam-runs
git notes --ref=yidam-runs merge -s cat_sort_uniq refs/notes-remote/origin/yidam-runs
git push origin refs/notes/yidam-runs
```

`cat_sort_uniq` joins the two copies of a note, sorts the lines and drops repeated ones. This is
why §2.1 writes a note as lines. A push that is refused because the remote moved is the same
fetch and merge, then the push again. Merging keeps every entry from both sides, and an entry
only ever says that a state reproduced a receipt, so the union is as true as either side.

The merge is itself a notes commit, dated when the merge ran, not when the run did. `aged`
therefore reads the date from the latest notes commit that is not a merge (`git log --no-merges`).
The remote's own commits arrive as the merge's parents, with their own dates.

A missing note leaves no trace, so `doctor` cannot tell that a clone lacks one. What it can see is
a local `refs/notes/yidam-runs` that no remote fetch refspec reads from, and it warns about that
once per repository. See Open questions.

### 2.5 The proposal route

A proposal-route step has two cases.

- **No pending proposal.** The parent is HEAD. Today a step whose outputs match HEAD opens a
  proposal that changes only its receipt, and a person has to merge a change with nothing in it
  to review. Under this RFC the step writes a note on HEAD's receipt instead, as §2.2 does, and
  opens no proposal.
- **A pending proposal.** The parent is the proposal's tip. #1162 already lands nothing when the
  tree is the one proposed, and this RFC leaves that alone. The pending proposal is the answer
  until a person acts on it, and a note on its receipt would vouch for a tree the branch does not
  hold.

### 2.5.1 The cluster path

`cluster step` decides freshness through the same `plan_and_write` dry run, so it reads the note
wherever the pod's clone has one. But a pod builds its commit with `build_commit` and hands it to
the lander, and the bundle a pod receives carries no notes. Until the lander question in Open
questions is settled, a cluster run behaves as it does today. It re-runs a step that a local run
would have found fresh by its note, and it lands the receipt-only commit.

### 2.6 A doctor check: an output that stamps its own input commit

A new doctor check, `run-output-stamps-input-commit`, reads each step's committed receipt and its
committed outputs. It reports `warn`, never `fail`, for an output whose bytes contain the
receipt's full `input.commit`.
Its rationale is §1.4: the stamp makes every run's output new, it hides whether the answer moved,
and the receipt already records it.

`YIDAM_INPUT_COMMIT` is still exported, because a step may need the commit
to read history or to name what it measured. `guidelines/directories.md` gains one sentence on what
the variable is for, and on not echoing it into an output.

### 2.7 An output whose consumer requires the commit

§1.4 assumed that nothing needs the stamp inside the output. Six corpora show otherwise:
traffic-design, travel-demand, land-use-activity, civic-infrastructure, urban-land-markets and
urban-environment-health. Each writes `.yidam/computed/engine-pack.json` for city-engine, and
city-engine's pack schema requires `corpus_commit`, "the corpus commit every span was read at".
The engine pins each corpus at that commit, and a test holds the pin equal to the field. The pack
is copied out of the corpus, and the receipt does not travel with it. So the commit has to be
inside the artifact.

Measured read-only on 2026-11-27, 75 of the 181 commits that touched `engine-pack.json` across
the six change nothing but `corpus_commit`. That share ranges from 19% in
urban-environment-health to 57% in travel-demand. In each repository §2.6 warns on every run,
and nothing the corpus can write clears the warning.

A step may therefore name the outputs that carry the commit by contract:

```toml
[capability.engine-pack]
writes = [".yidam/computed/engine-pack.json", ".yidam/computed/engine-pack-omissions.yml"]
stamps_input_commit = [".yidam/computed/engine-pack.json"]
```

Each entry is a repository-relative path, not a glob, and `writes` must cover it. A manifest
naming a path that no `writes` glob covers fails to load, with the path named.

**What `doctor` does.** §2.6 still reads every output. A declared output that contains the commit
is reported as declared, in the `ok` detail, and is not warned on. An undeclared one is warned
on as before. So the declaration clears the warning for exactly the outputs it names. The
companion omissions file above still warns if it carries the commit, which is the case
land-use-activity and traffic-design already fixed by hand.

**What `run` does.** §2.2's first test compares the produced bytes with the parent's. For a
declared output, the comparison is made after every occurrence of this run's full input commit is
replaced with the full `input.commit` of the parent's receipt. When the replaced bytes equal the
committed ones, and the receipt built from them reproduces the committed receipt, the run
reproduced its receipt. It writes a note and no commit, as §2.2 says, and the committed output
keeps the commit it already carries.

That commit is still true for the consumer. The output says its spans were read at the commit it
names. The note says a later commit's inputs gave the same answer. The consumer's pin moves only
when an answer moves, which is the reproducibility #1600 asks for.

When the replaced bytes differ, the output is committed as produced, carrying this run's commit.
Only the full 40-character commit is masked. A step that also writes an abbreviation reproduces
nothing, and commits as it does today.

**What it does not do.** It does not change `YIDAM_INPUT_COMMIT`, which is still this run's
commit. It does not change the receipt format: the receipt of a committed run records the
output's digest as produced. It does not change freshness, which still reads input states and
never reads outputs.

## What this does not touch

- **The receipt format.** No field is added, and `FORMAT_VERSION` stays 2. A run that changes an
  output commits exactly the receipt it commits today.
- **The input state.** It still hashes every declared read. The receipt is still honest that the
  inputs moved. What changes is where the run records that they moved without effect.
- **Batching.** One commit per whole-manifest run is #1536. The two compose. A batched run whose
  every step reproduced its outputs would write notes and no commit.
- **The cluster lander.** RFC-0026 §7 separates building a commit from moving a ref. How the
  lander is granted the notes ref is an open question, not decided here (§2.5.1).
- **Gather receipts.** `.yidam/runs/gather/<name>/<peer>.yml` records what a peer answered. A
  gather is not a manifest step and `freshness` never reads its receipts. It keeps its own rule
  for writing nothing when the branch already holds the answer.

## Migration & compatibility

- **No migration.** Existing receipts are read as they are. A corpus's first run after the
  upgrade behaves as now, until a step reproduces its outputs, which then writes the first note.
- **An older binary.** It ignores the notes ref. It sees a step whose re-validation is only in a
  note as stale, re-runs it, and commits the receipt-only change it would have committed anyway.
  Nothing it reads is broken, and the newer binary reads that commit as an ordinary receipt.
- **The run report** gains a way to name the note. That is a report field, with the report
  contract's handshake and its own golden, and an upgrade note when it is implemented.
- **`stamps_input_commit`** is a new manifest key, and the manifest denies unknown keys. A binary
  older than the amendment fails to load a manifest that declares it, so a corpus declares it only
  after re-vendoring. A manifest without it loads as before, and every output is judged as §2.6
  says.
- **History already written** is left alone. The receipt-only and stamp-only commits a corpus has
  are its record, and nothing rewrites them.

## Alternatives considered

- **Keep committing (the status quo).** The commit is an accurate record. But in the corpora that
  run calculators it is now the largest kind of bookkeeping commit, and it buries the commits
  where an answer moved.
- **Freshness from output digests.** #1497's second suggestion. Freshness has to be decided before
  the step runs, so that `run` knows whether to invoke it and `doctor` can say whether it is owed.
  Output digests are only known afterwards. Reading them would mean running every step to decide
  whether to run it.
- **A local record under the git directory** (`.git/yidam/revalidated`). Simpler than a note, but
  it is not an object, so it cannot be fetched. Its date would be a field in the file or the
  file's mtime: the second copy of a date that the receipt's "No clock" section refuses. Two
  worktrees of one repository would also need a shared path, where a ref is already shared. A
  notes ref has none of these problems, and where it is not shared it behaves the same way (§2.4).
- **No record at all.** Skip the commit and let the step re-run on every call. §1.2 explains why
  that fails: `doctor` would never report the step fresh again, and `run` would never stop
  invoking it.
- **Rewrite the commit away.** Amend or squash receipt-only commits on the branch. That rewrites
  shared history, and the record would still be a commit.
- **Have the consumer read the receipt** (#1598's second option). city-engine would take the
  commit from `.yidam/runs/engine-pack.yml` and not from the pack. That keeps §2.6 unamended. But
  the pack is copied out of the corpus without its receipt, the consumer is not the corpus's to
  change, and six corpora would each be waiting on one repository they do not own.
- **Make the check declinable under `[doctor.declined]`**, which takes only offer ids today.
  Declining clears the warning for every output of the corpus, including the ones that should not
  carry the commit. It also leaves `run` committing a
  stamp-only change on every re-run.
- **Export a source commit** (#1600's second option): the newest commit that touched the step's
  declared reads, as a second variable a step may write. An output carrying it reproduces while
  the reads are unchanged, with no masking. But a `compute:` usually runs right after the commit
  that changed its reads, so the source commit is usually the input commit, and §2.6 could not
  tell the two apart without a declaration anyway. Computing it also means a history walk over
  every declared glob, the manifest, the config and the script, which the input state already
  summarizes without one. The declaration gives the consumer the same stable pin.
- **Stop exporting `YIDAM_INPUT_COMMIT`.** This would fix §1.4 by force, and break a step that
  needs the commit for a reason other than stamping. The doctor check names the stamp and leaves
  the choice with the corpus.

## Open questions

- **Sharing the notes ref.** Should `doctor` (or `init`) offer the fetch and push refspecs, or
  should the corpus's CI add them? A CI job that runs steps without the notes ref re-runs each
  reproduced step once per job, which is the cost §2.4 accepts.
- **The lander.** Under RFC-0026 §7 a step pod builds a commit and a lander moves the ref. The note
  is a second ref. Either the lander holds that credential as well, or a pod returns the note
  entry for the lander to append.
- **Pruning.** A note grows by one entry per reproduced run, and stops growing when an output
  changes. Is that small enough to leave unbounded, or should a note keep only its latest entries?
  A merged note is sorted, not in the order its entries were written, so "latest" would have to
  be read from the dates of the commits each entry names.
- **The doctor check's reach.** Matching the full 40-character commit is exact but misses a short
  stamp. Is a short sha worth the false matches?
