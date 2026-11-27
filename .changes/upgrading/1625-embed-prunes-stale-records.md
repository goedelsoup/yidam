### embed removes the records of nodes that are gone

**`yidam embed` now removes every record under `.yidam/embeddings/` that the run did not write (#1625).**

A node that was deleted, renamed or moved to another class used to keep its old record.
`index-build` then indexed it, and `retrieve` could return a node the corpus no longer has.
The next `embed` removes those records and says how many it removed.
Run `yidam embed` and then `yidam index-build` once to clear an index built before this.
`embed --no-catalog` now removes the catalog records an earlier run wrote.
A node that fails to parse keeps its last record, as before.
`embed --dry-run` still writes and removes nothing.
