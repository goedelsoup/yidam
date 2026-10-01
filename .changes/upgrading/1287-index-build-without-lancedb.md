### `index-build` moves into `vector-read` and needs no protoc

**`index-build` no longer writes a LanceDB table, so building an index needs no protoc (#1287).**
Nothing ever read that table. Every reader decodes `corpus.arrow`, which `index-build` now writes directly.

`index-build` is now in the `vector-read` build. `index` is kept as an alias for it.
An install with `--features index` or `--features full` still builds, without protoc.

A `vector-read` build now lists `index` in `yidam --version` and the report's `features`.
A client that read `index` as "this build needs protoc" should read it as "this build can make an index".

An index built before this upgrade is still read. Its `corpus.lance/` directory is no longer used.
The next `index-build` removes it.
