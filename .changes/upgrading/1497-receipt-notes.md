### A re-run that reproduces its receipt writes a note, not a commit

**`yidam run` no longer commits a receipt-only change when a re-run writes the outputs it already committed (#1497, RFC-0052).**

It appends a line to a note on the committed receipt, in `refs/notes/yidam-runs`, and the branch does not move.
A step whose input moved and whose answer did not is then fresh on the next run.
The JSON run report gains `noted` on each step. It is null when no note was written.

Notes are not shared by default. To share them, fetch the ref, merge with `cat_sort_uniq`, and push it.
`yidam doctor` warns when a clone holds notes no remote fetches. A new check, `run-output-stamps-input-commit`, warns about an output that contains its own input commit.
