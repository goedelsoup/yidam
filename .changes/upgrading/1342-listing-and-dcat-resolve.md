### Source packs that read a listing or a catalog

**A scheme can resolve through a page that lists its files (#1342).**
Write `resolve = { listing = "…", match = '…' }`, or `resolve = { dcat = "<host>" }` for a DCAT-US `data.json`.
Use one where a release date or a fresh path in the address defeats a template.
See [Source packs](source-packs.md#when-no-template-reaches-the-file).

**`source add` writes the file it picked into the identifier.**
The location reads `scheme:local-id@<pin>`, and `catalog-fetch` follows only the pin.
An identifier in such a scheme without a pin is skipped, with a message saying how to pin it.

**A `resolve` without `template` now parses.**
A pack with no `template`, `listing` or `dcat` still fails `source check`, now with that message.

**The MCP contract is now 0.29.0.**
`resolve_source` returns the pinned identifier in `locations`, and `answered` counts a page read.

**What changes for you: nothing.**
Every template pack resolves as before.
