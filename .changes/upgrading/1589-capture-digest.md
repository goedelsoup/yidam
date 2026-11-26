### `catalog-fetch --archive` reads the capture it adds

**Each added `wayback:` location now gets its own artifact record in the same commit (#1589).**

The report's archived row gains `sha256` and `matches`.
`matches` says whether the capture's bytes equal the source's.
A mismatch never fails the run; an unreadable capture is added with `matches` null.
A listed capture with no record from its location is read on the next run.
That covers captures added before this release, and reads that failed.
`matches` is null for a paginated source, whose capture is one page.
A digest is now skipped only when held from the same location.
A held record with no `from:` still blocks the digest from any location.
The report and the commit body say when that happens.
The `superseded` clock now counts a digest per location.
A location whose bytes change to another location's is a new edition.
See [`catalog-fetch`](cli-reference.md).
