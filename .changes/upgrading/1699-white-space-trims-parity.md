### The TypeScript and Python SDKs trim embed text and references the way the CLI does

**`compose_embed_text` and `parse_reference` now give the same answer in all three SDKs (#1699).**

The Rust SDK and the CLI are unchanged.
In TypeScript and Python, text is now trimmed by the Unicode `White_Space` set that Rust's `str::trim` uses.
So U+0085 counts as whitespace, and U+FEFF and U+001C to U+001F do not.
The prose reader in each SDK follows the same rule.
A node or reference holding these characters may now embed or parse differently from either SDK.
Most nodes and references hold none of them and read exactly as before.
