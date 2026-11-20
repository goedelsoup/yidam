### A symbol pin can be measured at the symbol

**A new non-default feature, `symbol-pins`, measures a pinned read that names a `symbol` by that item (#1524).**

Built with it, the read is due only when the named item's text changes.
An edit elsewhere in the file no longer makes it due.
Rust, Python and TypeScript files are parsed; every other file is read at file grain.
The released binary does not carry the feature, so the same reads come due as before.
The catalog clock's line for a symbol pin now names the symbol and the grain it read at.
A script that matches that line exactly needs updating.
