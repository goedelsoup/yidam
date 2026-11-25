### A git worktree reads the main checkout's index

**`retrieve` in a worktree answers by vector search (#1517).** `.yidam/index/` is gitignored,
so a `git worktree add` checkout had no index and answered every `retrieve` by keyword. A
worktree with no index of its own now reads the main worktree's. It does so only when that
index was built at the branch's HEAD or an ancestor of it. No file you author moves.

An index built on a commit the branch does not contain is refused, and `retrieve` degrades
under `no_index` as before. This happens when the main checkout rebuilds after the baseline
moves. `yidam doctor` names the refused commit. Merge the baseline, or run `yidam index-build`
in the worktree. A worktree's own index always wins.

**The MCP contract is 0.37.0.** The `corpus` capability block and every `retrieve` response
carry `index_source`: `local`, `shared`, `remote`, or null. On `retrieve` it is null whenever
`degraded` is true. `index-status --format json` gains `index_source` and `index_path`.

`export`, `bundle` and `index-push` still package only a worktree's own index.
