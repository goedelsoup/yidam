### `catalog-fetch --archive` reads the capture it adds

**Each added `wayback:` location now gets its own artifact record in the same commit (#1589).**

The report's archived row gains `sha256` and `matches`.
`matches` says whether the capture's bytes equal the source's.
A mismatch never fails the run; an unreadable capture is added with `matches` null.
A digest is now skipped only when held from the same location.
A held record with no `from:` still blocks the digest from any location.
See [`catalog-fetch`](cli-reference.md).
