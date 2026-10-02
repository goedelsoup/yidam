### `search_sources` and `resolve_source` over MCP

**`serve --mcp` answers two source-pack questions, and writes nothing (#1319).**
`search_sources` is `yidam source search`. `resolve_source` is `yidam source add --dry-run`.
Both sit at a new `sources` tier, true where the corpus enables a pack.
The MCP contract is now 0.28.0. See [MCP server](mcp-server.md#asking-a-source-pack-and-writing-nothing).

**`[serve] offline_sources = true` answers both from the packs' recorded fixtures.**
Each answer says which route it took, in `answered`.

**What changes for you: nothing, until you enable a pack.**
A client that pins the contract version sees 0.28.0, with `sources` in the capability block.
