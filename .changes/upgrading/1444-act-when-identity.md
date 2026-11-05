### New derivations may write proposals, where an identity allows it

New derivations ship `[serve] act = "when-identity"` in `.yidam/config.toml`.
`serve --mcp` then serves `propose` and `cycle` wherever a git author identity resolves.
Without one, the server still starts, with the read tools only.
It says why at startup and in every refusal of `propose` or `cycle`.
An HTTP bind beyond loopback is treated the same way.

An existing repository keeps what its file says.
Write `act = "when-identity"` under `[serve]` to adopt the new default.
`act = true` still refuses to start where a condition fails.

The MCP contract is now 0.32.0.
