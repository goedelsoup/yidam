### The TypeScript and Python SDKs read markers the way the CLI does

**`parse_markers`, `scan_markers` and `update_regen` now give the same answer in all three SDKs (#1620).**

The Rust SDK and the CLI are unchanged.
In TypeScript and Python, a line now ends only at a newline.
Whitespace is now the Unicode `White_Space` set that Rust's `str::trim` uses.
So U+0085 counts as whitespace, and U+FEFF and U+001C to U+001F do not.
A TEMPLATE line that does not close is now scanned for an inline REGEN block.
A file holding these characters may now report different markers from either SDK.
Most files hold none of them and read exactly as before.
