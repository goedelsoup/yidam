### `cycle` can now say what moved since the last round

`yidam cycle --record` keeps this round under `.yidam/loop/<branch>.json`.
`yidam cycle --since-last` reports what arrived and what left since that record.
It covers findings, due clocks, stale steps, refs in flight and local proposals.
Each branch keeps its own record, and a detached HEAD keeps `HEAD.json`.

The directory writes its own `.gitignore`, so nothing in it is ever staged.
No migration is needed.

With no record, `since_last` is absent and `since_last_unread` says why.
Never read that as nothing having moved.

`yidam cycle --brief` prints a short report within a token budget.
The default budget is 500 tokens, and `--budget` changes it.
It refuses `--format json`.

Without these flags, `cycle` prints exactly what it printed before.
