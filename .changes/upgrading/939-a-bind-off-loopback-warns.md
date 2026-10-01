### A bind off loopback warns at startup

**`serve --mcp --http --bind` with a non-loopback address now prints a warning (#939).**
The transport authenticates nobody. Anyone who reaches the port can call every read tool.
The warning names each served tool, and the two repairs.

**What changes for you: nothing, unless you watch stderr.** The server still starts and answers.
Bind `127.0.0.1`, or require a bearer token. A token silences the warning.
