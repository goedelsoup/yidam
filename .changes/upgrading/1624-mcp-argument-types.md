### An MCP tool argument of the wrong type is refused

**A `tools/call` argument whose JSON type its tool's `inputSchema` does not declare is now a tool error (#1624).**

It used to be read as absent, so `"k": "20"`, `20.0` and `-1` each answered with the default `k`.
`propose` with `"dry_run": "true"` used to write the branch a dry run exists to avoid.
The error names the argument and its type: `invalid argument: k must be a non-negative integer, got "20"`.
An absent argument, or one sent as `null`, still takes its default.
A client that sent a count as a string or a float now sends it as an integer.
