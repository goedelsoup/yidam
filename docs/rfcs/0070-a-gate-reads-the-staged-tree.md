# RFC-0070 — A corpus gate can read what is staged

- **Status:** Draft
- **Track:** I53
- **Relates to:**
  - #1519 (no `--staged` mode for graph-check and lint, so pre-commit gates export the index by
    hand)
  - #693 and RFC-0028's correction, which record that this project ships no hook
  - RFC-0004 (*conformance, not hooks*) and RFC-0016 (the report envelope)
- **Versioning layers touched:**
  - **CLI.** `graph-check`, `lint` and `regen --check` each gain `--staged` (§1, §3). `doctor`'s
    `ci` check counts a local hook that runs a corpus gate (§5).
  - **Reports.** None. A staged run reports in the shape a working-tree run does. Its
    envelope's `root` names the repository, not the scratch tree (§2).
  - **Tree.** None. Nothing is written into the repository. The scratch tree lives under the
    git directory and is removed before the command exits.
  - **MCP contract.** None. No tool runs a gate.
  - **Prelude.** None. No hook is shipped and nothing `yidam-vendor-update` copies changes.
  - **Docs.** The CLI reference documents the flag. The configuration page gives a reference
    pre-commit hook (§4).
  - **The bump.** Minor for the CLI, since a flag is added. No authored file moves, so no
    upgrade note is owed.
- **Downstream reference cases:**
  - The corpus quoted in #1519, on 0.26.0 (c694ab1). Its pre-commit gate exports the index
    with `git checkout-index --all --prefix=…` and runs the gates there. `doctor`'s `ci` check
    warns on every run, because the corpus is gated only by local hooks.

## Summary

`graph-check`, `lint` and `regen --check` read the working tree. A commit is made from the
index. A pre-commit hook that runs a gate as written therefore checks the wrong tree:
- an unstaged fix hides a staged break;
- an unstaged break fails a clean commit.

This RFC adds `--staged` to the three gates `doctor` already counts as corpus gates. A staged
run checks the tree the commit would hold, inside a scratch checkout of HEAD, and reports it
as if it had run in the repository. `doctor` stops warning about a repository whose only gate
is a local hook. It names that hook and says what a local hook does not cover.

## Problem

### 0.1 What was measured

The corpus in #1519 runs both gates from a pre-commit hook. Its hook exports the index to a
temporary directory with `git checkout-index` and runs `graph-check` and `lint` against that
directory. Without the export, the gate's answer depends on what the person has not staged.

The export is a workaround with a cost. The temporary directory has no repository, so every
`lint` check that reads git history has nothing to read. Those checks include the commit
checks, pin checks, lineage and TTL. Every path in the report also names the temporary
directory, not the repository.

`doctor` in that corpus warns `no .github/workflows/` on every run. The corpus does gate its
commits. The `ci` check reads only workflows, so the gate it has is invisible to it.

### 0.2 What already exists

- **The three corpus gates are named once.** `doctor.rs::CI_GATES` lists `yidam graph-check`,
  `yidam regen --check` and `yidam lint`. `gates_in` reads shell text for them, following
  `mise run` into the inherited gate tasks and the repository's own tasks.
- **A scratch checkout is already made.** `regen::fold_for` checks a commit out with
  `git worktree add --detach` under the git directory. It sets `core.hooksPath=/dev/null` for
  that one call, so the person's `post-checkout` does not run. It removes the tree and prunes
  the worktree afterwards. It also clears a leftover from an interrupted run first.
- **The index is one tree away.** `git write-tree` writes the index as a tree object and
  prints its hash. It refuses an index with unmerged paths.

### 0.3 What does not exist

- No gate can read the index.
- `doctor`'s `ci` check reads `.github/workflows/` and nothing else.

## Design

### 1. The flag

`graph-check --staged`, `lint --staged` and `regen --check --staged` each check the tree the
next commit would hold. That is the index: tracked files as they are staged. Unstaged edits and
untracked files are left out.

- `lint --staged` conflicts with `--commits`, `--bless` and `--init-baseline`. `--commits`
  reads commits, not a tree. The other two write a baseline, and a staged run has nowhere to
  write it that would survive.
- `regen --staged` requires `--check`. Plain `regen` writes the files it regenerates, and a
  staged run would write them into a tree that is about to be removed.
- `--staged` with `--warn`, `--explain` or `--format` behaves as it does without `--staged`.

### 2. How a staged run reads the index

1. `git write-tree` in the repository writes the index as a tree. An index with unmerged
   paths is refused, with a message naming the merge in progress.
2. A scratch worktree is added at HEAD, detached, under the git directory, with
   `core.hooksPath=/dev/null` for that call. This follows `regen::fold_for`.
3. In the scratch worktree, `git read-tree -u --reset <tree>` replaces its index and its files
   with the staged tree.
4. The gate runs with its root at the scratch worktree. When `--root` names a corpus below the
   repository's top level, the root is the same subdirectory of the scratch tree.
5. The worktree is removed and pruned on every exit path, failure included.

The scratch worktree shares the repository's object store and refs. A history check sees the
commits it sees in the repository, and HEAD is the same commit. The staged changes show as the
scratch tree's difference from HEAD. A check that reads uncommitted changes therefore reads
exactly what is about to be committed.

**What a staged run reports.** It reports in the shape a working-tree run does, with one
difference: every path is named relative to the repository, not the scratch tree. The
envelope's `root` is the repository's root, and the text output names no scratch path.

**No commit yet.** A repository whose HEAD is unborn has no commit to check out, and a staged
run refuses it: *nothing is committed yet, so there is no HEAD to stage against; run the gate
without `--staged`.* A genesis commit is made by the bootstrap, which runs the gates itself.

**Files a gate reads but git does not track.** Gitignored state under `.yidam/` (the index,
cycle records) is absent from the scratch tree. No corpus gate reads it. A gate that later
does will see an absence there, which is the truth about what the commit holds.

### 3. Which gates

`--staged` goes on every command in `CI_GATES`, and a test holds the two lists together. A gate
added to `CI_GATES` without `--staged` fails that test. The test reads `CI_GATES`, so the list
is written once.

### 4. Hooks

This project still ships no hook (RFC-0004, RFC-0028's correction). The configuration page
gives a reference pre-commit hook that a repository can copy:

```sh
#!/bin/sh
# .githooks/pre-commit — enable with: git config core.hooksPath .githooks
set -e
yidam graph-check --staged
yidam lint --staged
yidam regen --check --staged
```

No installer is added. A hook is the person's, and the repository's conformance is what CI
checks. The hook only runs the gate earlier.

### 5. `doctor` counts a local hook

The `ci` check reads two places: the workflows, as now, and the hooks git would run. The hooks
directory is `git rev-parse --git-path hooks`, which follows `core.hooksPath`. Of the files
there, it reads `pre-commit` and `pre-push` with the same `gates_in` the workflows go through.

- **A workflow runs a gate.** A pass, as now. A hook gate is named beside it.
- **No workflow runs a gate, and a hook does.** A pass, naming the hook. For example: *runs
  yidam lint --staged in the pre-commit hook at `.githooks/pre-commit`, which binds only the
  checkouts that set `core.hooksPath`.*
- **Neither does.** The warning stands as now.

A `.sample` file is never read. A hook that git would not run is not a gate: one that is not
executable on a platform that checks the bit is skipped. That case is named in the
check's detail, so a person who wrote one is not left guessing.

A decline recorded for `ci` still declines, unchanged.

## Alternatives

### Export the index with no repository

`git checkout-index --all --prefix=<tmp>/` is what #1519's hook does now. Every check that
reads git history would have to be skipped, or would fail, in a tree with no repository. The
scratch worktree costs one `worktree add` and keeps all of them.

### Read blobs from the index in memory

Teaching the corpus loader to read through `git cat-file` would make no checkout at all. It
touches every file read in three commands, and each new check would have to remember to read
through it. Rejected for the size of the change and the ease of breaking it.

### `yidam hooks install`

A command that writes a pre-commit hook would reverse the documented stance. It would also add
a command to every registration a command trips, for a script of four lines. Rejected. The
reference hook in §4 is copied once.

### Count a hook only when its directory is tracked

A tracked `core.hooksPath` directory is one every clone can opt into. A hook under `.git/hooks`
binds one checkout. The difference is real, but `doctor` describes the checkout it runs in, and
in that checkout either hook gates the commit. The message says what a local hook does not
reach, so neither is mistaken for CI.

## Testing

- A suite test stages a broken link, leaves a fix unstaged, and asserts that `graph-check
  --staged` fails while `graph-check` passes. A second test does the reverse.
- The same pair runs for `lint --staged` and `regen --check --staged`.
- A test runs a staged gate in a repository with commits and asserts that no output names the
  scratch path, text or JSON. It also asserts that `git worktree list` holds only the
  repository afterwards, including after a failing gate.
- A test asserts that a staged run refuses an unborn HEAD and an index with unmerged paths.
- A test holds `CI_GATES` to the commands that accept `--staged` (§3).
- `doctor` tests:
  - a `core.hooksPath` hook running a gate passes `ci` and names the hook;
  - a `.sample` file does not pass;
  - a hook that is not executable does not pass;
  - a workflow gate still passes, with the hook named beside it.

## Unresolved

- Whether `vocabulary --check` belongs on the same flag. It already reads the staged paths, as
  RFC-0028's correction records, so it needs none.
