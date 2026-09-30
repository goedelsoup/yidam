### A new version of a source names the nodes that read the old one

**`yidam catalog-fetch` names every node citing an entry when it records a new version (#1200).**
A new version is a digest from a location the entry already held a digest from. The report and
the `refresh:` commit both list the nodes. The JSON row gains a `superseded` list.

**`yidam due` gains a fifth clock, `superseded`.** It lists each node citing a source that has
changed since the node was last committed. Set `[due] superseded_after` in days to let it come
due. A commit touching the node discharges it. A new text reading of a PDF is not a new version.

Nothing needs changing. The clock reads `undeclared` until the key is set, and still reports
what it measured.
