### `vault gc` sweeps what an interrupted write left

**`yidam vault gc` reports stale `.part` and `.packing` files in the cache, and `--yes` removes them (#1658).**

An interrupted fetch, put or push leaves one of these, and it can be as large as the artifact.
Before this, no command reported them or reclaimed the space.
Each file is named for the process that wrote it.
A file whose process is still running is a write in progress, and `gc` leaves it alone.
Only names a cache writer produces are swept; any other file in the cache is untouched.
Without `--yes`, `gc` still reports and removes nothing.
