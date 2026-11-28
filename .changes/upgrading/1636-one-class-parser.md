### A class file's stem names its class, and a class file that does not parse is an error

**`parse_class` now returns an error for a class file that does not parse (#1636).**

The Rust SDK is 0.15.0, and the TypeScript and Python SDKs are 0.12.0.
In Rust, `parse_class` returns a `Result`, and the error is a `ClassParseError`.
In TypeScript and Python, it throws or raises a `ClassParseError`.
A document whose top level is not a mapping is also an error.
An empty document still reads as a class that declares nothing.
The file's stem names the class in all three SDKs.
A `class:` field inside the document is no longer read.
The CLI already used the stem, so its class names do not change.
`yidam schema` no longer writes an open schema for a class file that does not parse.
It skips that file and names it on stderr.
`yidam migrate` refuses to plan against a class file that does not parse.
The MCP `licensed_edges` tool refuses one too.
`yidam lint` already reports such a file as `malformed-yaml`.
