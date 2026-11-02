### `index-build` embeds each record alone

**`index-build` now embeds one record per call (#1428).** Each record is embedded the way a query is.
Before, records went in batches of 256. Each vector then depended on the records batched with it.

**Rebuild your index with `yidam index-build`.** An old index still works, but its vectors sit slightly apart from its queries.
On one corpus, a record used as a query kept 92% of its top ten.

**A shared remote index should be rebuilt by every corpus that pushes to it.** Then all its vectors come from one build.

**The build is also faster and smaller.** One corpus took 23 seconds instead of 43, and 0.2 GB instead of 12 GB.
`[index] batch_size` restores batching, but its vectors drift from the queries again. See [configuration](configuration.md).
