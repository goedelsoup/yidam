# RFC-0055 — The watch ledger lives on its own branch

- **Status:** Draft
- **Track:** I38
- **Relates to:**
  - #1500 (the watch commits an `unchanged` heartbeat on every run; the evidence this answers), parent epic #1513
  - #1073 (the watch scaffold, whose three rules this keeps)
  - #1497 / RFC-0052 and #1499 / RFC-0054 (the other bookkeeping commits the epic removes)
- **Versioning layers touched:** template only. `sadhana/github/workflows/watch.yml` writes its row to a `yidam/watch` branch instead of the default branch. The `watch` job in `sadhana/github/workflows/ci.yml` reads the ledger from that branch. No CLI change. No report or MCP contract change. No SDK change.
- **Downstream reference case:** matt-huffman, measured read-only on 2026-11-22 at `origin/main` (7fcec05). It is the only corpus on the measuring machine with an armed watch. Its workflow is the local original that #1073 generalised, pinned at v0.14.0, and it is not the template's `watch.yml` (§1.2).

## Summary

The watch scaffold appends one row to `.github/watch/heartbeats.tsv` every round and commits it
to the default branch. That ledger has to exist. #1073 is right that an unreachable round is
not an unchanged one, and the gate in `ci.yml` can tell them apart only by reading a row for
every round. But the row is liveness data, and the default branch's history is where a corpus
records what it knows. Every reader that counts commits there pays for it: practice, cohort,
kuten accretion, and the eye of anyone reading `git log`. In matt-huffman, 30 of the last 37
commits on `main` are `watch: <date> — unchanged`.

This RFC moves the ledger, unchanged in format, onto an orphan branch named `yidam/watch`. The
round still writes one row per round, commits it, and pushes it. The push goes to
`yidam/watch`, so the default branch gets no watch commits at all. The gate reads the same file
from that branch with the same `awk`, so silence and blindness are measured exactly as they
are today.

## Problem

### 1.1 What the template does

`watch.yml`'s round job runs `.github/watch/round` and appends one row to the ledger. The guard
fails unless the ledger is the only file that changed. Then it commits
`refresh: watch round <date> — <verdict>` to the default branch and pushes. It does this for
every verdict, `unchanged` included. That is deliberate. The `watch` job in `ci.yml` asks two
questions of the ledger. **Silence** asks whether any round ran in the last seven days.
**Sight** asks whether the last eight rounds were all `unreachable`. A round that leaves no row
cannot answer either one.

### 1.2 What it costs, measured

matt-huffman armed its watch on 2026-08-21. Its workflow is the one #1073 was extracted from:
the same three rules, the ledger at `crates/watch/data/heartbeats.tsv`, and a commit subject of
`watch: <date> — <outcome>`. On `origin/main` at 7fcec05:

| | Commits |
|---|---|
| All commits on `main` | 1000 |
| `watch:` commits | 93 |
| … `unchanged` | 60 |
| … `unreachable` | 28 |
| … `changed` | 5 |
| Commits in the last 30 days | 37 |
| … `watch: <date> — unchanged` | 30 |

Ledger rows: 60 `unchanged`, 28 `unreachable`, 6 `changed`. That is one more `changed` row
than commits, because one row was written by hand on a day the scheduled round also ran.

A quiet month is now mostly heartbeats. The corpus learned something on five of the seven
non-watch commits. Its history says it was busy on thirty-seven days.

The other six corpora in #1513's review have no armed watch, so this is one repository's cost.
It is the cost the template installs, though: any corpus that arms the scaffold today pays it
from the first night.

### 1.3 Why excluding them downstream does not work

Each reader could skip `watch:` or `refresh: watch round` subjects. Practice would need to,
and cohort, and kuten accretion, and `due`. So would every corpus-local reader, and every
person reading `git log --oneline`. A new reader would get it wrong by default. That is the
"stored copy of a derived fact" shape RFC-0053 removed from `used-by`, turned the other way
round: here, every reader must know to subtract the watch's commits. Bookkeeping belongs where
readers of knowledge do not have to look.

## Proposal

### 2.1 The ledger branch

The ledger lives at the same path, `.github/watch/heartbeats.tsv`, on an orphan branch named
`yidam/watch`. That branch holds the ledger and nothing else. Its history is one commit per
round, which is the ledger's own audit trail. It shares no ancestor with the default branch, so
no reader that walks the default branch's history ever reaches it.

A branch and not a custom ref (`refs/yidam/watch`, the option #1500 names): `git clone`
fetches every branch, so the ledger arrives with any clone as `origin/yidam/watch`. A custom
ref needs an explicit refspec on every fetch. A reader that forgot one would find no ledger,
and "no ledger" reads as "not armed", which is silence again.

`.github/watch/round` stays on the default branch. It is code, a person writes it, and it
changes through review like any other code.

### 2.2 Arming

Arming is still a decision a person makes. It is now two pushes instead of one commit:

```bash
git switch --orphan yidam/watch
printf '# armed: %s\ndate\toutcome\tdetail\n' "$(date -u +%F)" > .github/watch/heartbeats.tsv
git add .github/watch/heartbeats.tsv
git commit -m "establish: the watch ledger"
git push origin yidam/watch
git switch -
```

Then commit an executable `.github/watch/round` on the default branch and uncomment the
schedule. `watch.yml`'s header carries these commands in place of today's "commit two files".

`git switch --orphan` empties the index and the working tree, which is the point: the branch
must not carry a copy of the corpus. The guard in §2.3 enforces this every round.

### 2.3 The round

The round job checks out the default branch as today, for `.github/watch/round`. Then:

1. **The ledger worktree.** `git fetch --depth=1 origin yidam/watch`, then
   `git worktree add "$RUNNER_TEMP/ledger" FETCH_HEAD`. The worktree lives in `$RUNNER_TEMP`,
   outside the checkout, so the default branch's working tree is never the place a row is
   written.
2. **The armed check** becomes: the fetch succeeded, the ledger exists in the worktree, and
   `.github/watch/round` is executable.
3. **The round runs** exactly as today, and the row is built exactly as today.
4. **The row is appended** to the ledger in the worktree.
5. **The guard checks both trees.** The default branch's checkout must be clean, with nothing
   modified and nothing untracked. The ledger worktree must differ from its HEAD in the ledger
   alone. The first half is stronger than today's rule. Today the ledger is the one file the
   checkout may change. After this RFC there is no file the round may change there.
6. **The commit** is made in the ledger worktree and pushed to `yidam/watch`. The subject
   stays `refresh: watch round <date> — <verdict>`. On a non-fast-forward the step fetches,
   rebases and pushes once more. The `concurrency` group already serialises rounds, so a
   retry only covers a person who annotated the ledger by hand mid-round.
7. **The issue on `changed`** is opened exactly as today.

Nothing is pushed to the default branch. The job's `contents: write` permission is unchanged,
because it now pushes to a different branch.

### 2.4 The gate

The `watch` job in `ci.yml` is split into a fetch and a read:

- **`detect`** sets `watch=true` when `git ls-remote --exit-code --heads origin yidam/watch`
  succeeds. A ledger file on the checked-out branch no longer counts.
- **"Fetch the ledger"** runs `git fetch --depth=1 origin yidam/watch` and writes
  `git show FETCH_HEAD:.github/watch/heartbeats.tsv` to `$RUNNER_TEMP/heartbeats.tsv`.
- **"The ledger says the watch runs and sees"** keeps its `awk` program unchanged, and reads
  the file at `$LEDGER_FILE` instead of a path in the checkout.

The `awk` program is unchanged, so the meaning of every row and both windows is unchanged.
Each pull request's CI still reads the newest ledger, as it does today. A watch that went
quiet still turns every build red.

**A ledger left on the default branch fails the gate.** If `.github/watch/heartbeats.tsv` is
in the checkout, the job fails with the move in §3, whether or not the branch exists. Nothing
writes that file any more, so it would read as silent within a week anyway. Failing on the
first build instead says why.

### 2.5 Tests

`tests/suite/watch_workflow.rs` already runs the round step and the gate's `awk` program
against scripted rounds and fixture ledgers. It changes in four places:

- **The guard** gets fixtures for both trees. A clean checkout plus a ledger-only change
  passes. Any file the round leaves in the checkout fails, the ledger path included. A second
  file changed in the ledger worktree fails.
- **The gate** runs against `$LEDGER_FILE`. Every existing silence and sight fixture keeps its
  verdict, which pins that the `awk` program did not move.
- **A new test runs a round end to end** against a bare origin made in a temp dir. The origin
  has a default branch and a `yidam/watch` branch. The test asserts that the round's commit
  lands on `yidam/watch`, and that the default branch's ref does not move.
- **A new test checks the leftover-ledger refusal** in §2.4: a checkout carrying the ledger
  fails the gate with the move's commands in its output.

`the_gate_is_armed_by_the_ledger_both_workflows_name` keeps its point, that both workflows
name one ledger. It checks the branch name as well as the path.

## What this does not touch

- **The three rules.** The round writes only its ledger. Unreachable is not unchanged. A gate
  reads the ledger. Rule 1 gets stricter (§2.3 step 5).
- **The ledger format.** It keeps the same header, the `# armed:` line, the three outcomes and
  the hand-written comments.
- **`changed` issues.** They are opened the same way and say the same thing.
- **`due` and the corpus graph.** Neither reads the ledger today. The epic's note that `due`
  must keep reading it describes a corpus-local reader (§3.2). The template has none.

## Migration & compatibility

### 3.1 A repository on the template's watch

Both workflow files are copied by hand from `sadhana/github/workflows/`, as #1073's upgrade
note says, so they change together. Move the ledger in the same change:

```bash
git show HEAD:.github/watch/heartbeats.tsv > "$TMPDIR/heartbeats.tsv"
git switch --orphan yidam/watch
mkdir -p .github/watch && cp "$TMPDIR/heartbeats.tsv" .github/watch/heartbeats.tsv
git add .github/watch/heartbeats.tsv
git commit -m "establish: the watch ledger, moved off main"
git push origin yidam/watch
git switch -
git rm .github/watch/heartbeats.tsv
```

Then commit the deletion with the two new workflows. The rows move intact. The default branch
keeps its old watch commits, and history is not rewritten. Only new rounds go elsewhere.

The upgrade note in `.changes/upgrading/` carries these commands. No `yidam migrate` step is
added. The move is one branch push and one deletion, and #1073's own upgrade was a hand copy.

### 3.2 A corpus with its own watch

matt-huffman's watch predates the template's. Its ledger is
`crates/watch/data/heartbeats.tsv`, and its gate is a Rust crate that compiles the ledger in
with `include_str!` and runs in the gating crates job. Moving its ledger off `main` means the
crate reads it at test time from `git show origin/yidam/watch:…` instead. This RFC does not
make that change. It is the corpus's own code, and the template only owes it the pattern. The
upgrade note names the shape so the corpus can choose it.

The corpus's ledger also carries hand-written comments: waivers, a certificate decision, and
a day annotated with two rows. Those comments stay in the file. A person writes them on the
`yidam/watch` branch with an ordinary commit, as today, just on a different branch. That is
the real cost of the move, and §5 says why it is the right cost.

## Alternatives considered

### 4.1 Commit on a state change only

Commit a row only when the verdict differs from the last committed one, plus a keepalive row
every few days so silence stays measurable. matt-huffman's 30 heartbeats a month would fall to
about five. Rejected:

- It still commits liveness to the history of knowledge, just less often. The readers in §1.3
  still subtract it.
- The sight gate counts rounds. Under this scheme a row stands for a span, so the gate would
  have to become a span gate. Seven blind rounds would turn into seven blind days. That is
  defensible, but it changes the gate #1073 tested, to save commits that §2 removes outright.

### 4.2 Batch `unchanged` rows into one commit per period

Hold rows between commits somewhere: the Actions cache, or a workflow artifact. Rejected.
Both are evictable. A row lost to an eviction is a round that left no trace, which is the
blindness #1073 exists to make visible. Artifacts also expire after at most 90 days, and the
ledger is read for longer than that.

### 4.3 The run history as the ledger

The silence question is "did the job run", and GitHub Actions already records that. The gate
could call `gh run list --workflow watch.yml`. Rejected:

- The run history does not hold the verdict, so the sight question would still need a ledger.
- A gate that needs the GitHub API and a token is not runnable against a fixture.
  `watch_workflow.rs` runs every property here locally, and that matters more than it looks:
  each edge case it pins was a production failure first.

### 4.4 Teach the readers to skip watch commits

See §1.3. It fixes the readers that exist today and fails every one written later.

### 4.5 A custom ref outside `refs/heads`

`refs/yidam/watch` keeps the ledger out of the branch list. It also keeps it out of every
clone, which turns "not fetched" into "not armed" (§2.1). The branch list is a small price
next to that.

## 5. Why a person's annotations move too

The ledger is not only machine rows. matt-huffman's carries a waiver and a written account of
why the watch was blind for eleven days. Those are worth more in `git log` on `main` than the
heartbeats are. One could keep annotations on `main` and rounds on the branch. Then the ledger
would be split in two, and the gate would read whichever half it was given. It is better to
have one file and one place. Anyone who wants to know why the watch was blind is already
reading the ledger, and `git log origin/yidam/watch` is where its history now is.

## Expected effect, and the falsifier

After adoption, a corpus with an armed watch has no commits from the watch on its default
branch. On matt-huffman's last 30 days that is 30 fewer of 37, and the 7 left are all knowledge
or upkeep. The gate goes red on the same days it would have: same rows, same `awk`, same
windows.

**Falsified if** either of these holds after adoption:

- `git log <default> --since=<adoption> --format=%s | grep -c 'watch round'` is above zero;
- a watch whose rounds stop, or all come back `unreachable`, passes the gate past its window.

The end-to-end test in §2.5 checks the first. The existing silence and sight fixtures check
the second.

## Open questions

- **The branch name.** `yidam/watch` cannot coexist with a branch named exactly `yidam`, which
  git refuses as a directory/file conflict. No corpus on the measuring machine has one. Is the
  `yidam/` namespace worth reserving for template-owned branches in general?
- **Branch protection.** A ruleset that matches every branch would refuse the bot's push to
  `yidam/watch`. Today's push to the default branch is refused by any protection on it, so
  this is no worse, and probably better. Should the arming commands also create a ruleset
  exemption, or only mention one?
- **An arming command.** §2.2 is six commands a person runs once. A `yidam watch arm` would
  make them one. That is a CLI surface for a scaffold one corpus uses, so it is left out
  until a second corpus arms one.
