### The MCP `open_questions` tool lists asks

`open_questions` over MCP now returns `asks` beside `open_questions` (#1486).
It is the same list `open-questions --format json` gives.
An ask is an open question to a dependency, kept under `.yidam/asks/<package>/`.
It is never counted among `open_questions`.

The MCP contract is now 0.33.0, and `asks` is required.
A server that has no asks sends `[]`.
Treat a missing `asks` as an older server, not as an empty list.
