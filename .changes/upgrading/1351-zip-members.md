### A file inside a zip is read as a reading

**A location can list `members:`, paths inside the zip it names (#1351).**
`catalog-extract` unpacks each into the cache and records it under the zip's `readings:`.
Each such reading carries `member:` and `by: unzip`.
See [Read a file inside a zip](source-packs.md#read-a-file-inside-a-zip).

**Lint refuses a member path that leaves the archive.**
`catalog-location-malformed` names a path with `..`, a leading `/`, a drive letter or a backslash.

**What changes for you: nothing.**
An entry without `members:` reads as before.
