### An MCP argument outside its enum is refused

**A `tools/call` string argument outside the values its `inputSchema` lists is now a tool error (#1690).**

Only `claims` declares one: `standing` takes `verified`, `inference` or `open`.
It used to filter on whatever it got, so `"verifed"` answered with an empty list.
That looked the same as a corpus with no such claims.
The error names the argument and the values it takes: `invalid argument: standing must be one of verified, inference, open, got "verifed"`.
An absent `standing`, or one sent as `null`, still returns every claim.
A client that sent a misspelled standing now gets an error instead of an empty list.
