### A renamed node keeps its ages

**`yidam rename` writes `moved-from:` into the node it moves (#1180).** Before, a rename
restarted two counts of commits. One is how long nothing has cited a node, which `orphan-in`
escalates on. The other is how long a question has stood `[open]`, which `due` reads. A renamed
overdue question dropped out of `due` entirely, and its clock read Ok.

The new line names the old path the way a `target:` does, for example
`moved-from: ../concept/low-flow.yml`. Both counts now carry across the move. A move that also
adds a citation or answers the question still ends that count. F2 in an editor writes the same
line. A second rename replaces it.

A move made without the command still restarts both counts. To keep them, add the line by hand
in the commit that moves the file. Moves made before this release are not backfilled. The
`rename` JSON report gains a `moved_from` object.
