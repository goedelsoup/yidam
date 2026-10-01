### `yidam record --fold` commits the consumption record

**The record `[serve] record` keeps can now reach the history (#1018).**
`yidam record --fold` counts the new lines of `.yidam/record/calls.jsonl` into `.yidam/consumption.json`.
It commits that file alone, as one `refresh:` commit on the current branch.
The commit is operational, so no proposal branch and no author are involved.

The file is never truncated. The fold remembers how many bytes it counted, and takes only complete lines.
A line a server writes during the fold is counted by the next one.
With nothing new to count, the fold writes no commit, so it can run on a clock.

`yidam record` now reads the committed counts and the file together.
Its JSON report gains `consumption.folded`, which is `0` in a corpus that has never folded.
Nothing needs to change in a corpus until you choose to fold.
