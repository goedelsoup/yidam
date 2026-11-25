### A corpus gate can check what is staged

**`graph-check`, `lint` and `regen --check` take `--staged` (#1519).** Each one checks the tree
the next commit would hold, which is the index, not the working tree. Unstaged edits and
untracked files are left out. The run happens in a scratch worktree of HEAD under the git
directory. Every check that reads history still reads it. The report names the repository,
never the scratch tree. No file you author moves.

A staged run needs a commit at HEAD and an index with no unmerged paths. `lint --staged` does
not combine with `--commits`, `--bless` or `--init-baseline`.

If a pre-commit script exports the index with `git checkout-index` before running the gates,
replace the export with `--staged`. The configuration page gives a reference script.

**`yidam doctor` counts a local hook as a gate.** Its `ci` check used to read only
`.github/workflows/`. It now also reads `pre-commit` and `pre-push` in the hooks directory git
uses, `core.hooksPath` included. A repository gated only by such a hook now passes `ci`. The
check names the hook and the checkouts it binds. A hook that is not executable is named
and does not count.
