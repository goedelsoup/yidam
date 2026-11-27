### The TypeScript and Python SDKs classify commits the way the CLI does

**`classify_commit` now gives the same verb, subject and kind in all three SDKs (#1666).**

The Rust SDK and the CLI are unchanged.
In TypeScript and Python, a commit's first line now ends only at a newline.
The line, verb and subject are trimmed by the Unicode `White_Space` set that Rust's `str::trim` uses.
So U+0085 counts as whitespace, and U+FEFF and U+001C to U+001F do not.
A subject holding U+2028, a form feed or a lone carriage return is no longer cut short in Python.
A commit message holding these characters may now get a different kind from either SDK.
Most commit messages hold none of them and classify exactly as before.
