### `regen` and `vault push` author their own commits

**`yidam regen --commit` and `yidam vault push --commit` write the `regen:`, `index:` and `bundle:` commits (#1215).**
Each commits only the tracked files it changed, under its own author, and commits nothing when nothing changed.
It refuses a file that already has uncommitted edits.

The index workflow now runs `yidam vault push --index --commit` and pushes. Its `Commit the lock` step is now `Push the lock commit`.
Take the new `.github/workflows/index.yml` on upgrade. `index-build` and `bundle` still commit nothing, because their output is gitignored.
