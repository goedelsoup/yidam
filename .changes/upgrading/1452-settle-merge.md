### `phase settle --merge` writes the phase merge, and `max_open_proposals` is read locally

`yidam phase settle --merge` writes the merge that `settle` drafts. It is authored by `yidam phase`, with you as committer.
It refuses a phase that is not ready, or a baseline that has moved on. It also refuses a checkout that is not the commit.
`--summary "<text>"` replaces the drafted tally in the merge subject.
Plain `yidam phase settle` is unchanged and still commits nothing.

`[cluster] max_open_proposals` is now read by `yidam propose` and `yidam run`, against local `propose/*` branches.
At the cap, neither opens a new proposal branch. A corpus without the key sees no change.
See [cli-reference.md](cli-reference.md) and [configuration.md](configuration.md).
