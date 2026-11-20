### A coverage universe can be a registry kept as code

**`[[due.coverage]]` reads TOML, and a pinned source needs no fetch (#1525).**

`select` walks a TOML file with the path syntax it uses for JSON.
A file is TOML when its name ends in `.toml`.
A source with a pinned `file` location is read from that file at the pin's `commit`.
Nothing is fetched or cached for it.
When the file changes after the read, a zero gap reads as stale coverage.
Existing universes read as before.
