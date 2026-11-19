### A `file` location can pin the commit it was read at

**A `file` location takes optional `commit`, `repo` and `symbol` keys (#1521).** An entry without them reads as before.

`commit` is the commit the file was read at, written in full. `value` is then a path from the repository's top level.
`repo` names a dependency in `.yidam/tonpa.toml`. Leave it out for this repository.
`symbol` names what was read. Its last segment must occur as a word in the file at `commit`.

Five checks resolve each pin, with one `git cat-file` per repository:

- `catalog-pin-unresolved` (error): the commit, the `repo` or the path is not there.
- `catalog-pin-ambiguous` (error): an abbreviated commit matches more than one commit.
- `catalog-pin-abbreviated` (info): the finding gives the full commit to write.
- `catalog-pin-symbol-missing` (warning): the symbol does not occur in the file.
- `catalog-pin-unchecked` (info): a shallow clone, or a dependency with no local checkout.

A binary older than this one ignores the new keys.
