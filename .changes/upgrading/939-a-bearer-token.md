### `serve --mcp --http` can require a bearer token

**Set `YIDAM_SERVE_TOKEN` or pass `--token-file`, and every request needs the token (#939).**
A request without it gets `401` on every path. Nothing changes until one is set.

**What changes for you: check the environment you serve from.** An empty `YIDAM_SERVE_TOKEN` now refuses to start.
So does setting the variable and `--token-file` together.

A token does not lift the loopback rule for `[serve] act`. See [mcp-server.md](mcp-server.md#a-bearer-token).
