### Source packs that pin a mutable identifier

**A scheme can pin an identifier that names a moving target (#1343).**
Declare `[scheme.<name>.pin]` with `mutable`, `read`, `media`, `value` and `pinned`.
`source add` asks the read once, and writes the version it answered into the identifier.
See [Source packs](source-packs.md#when-an-identifier-names-a-moving-target).

**`archive` 0.2.0 adds `github` and `wikipedia`.**
`github:<owner>/<repo>@<ref>/<path>` is pinned to the commit the ref names.
`wikipedia:<lang>/<title>` is pinned to the article's current revision.
A bare `wayback:<url>` is pinned to the capture the archive names.

**`catalog-fetch` refuses the mutable form, with how to pin it.**
It never asks the read.

**The MCP contract is now 0.30.0.**
`resolve_source` returns the pinned identifier in `locations`, and `answered` counts the read.

**What changes for you: re-pin `archive` to use the new schemes.**
`archive@^0.1` does not admit 0.2.0. Change it to `archive@^0.2`, then run `mise run yidam-vendor-update`.
Every `wayback:<timestamp>/<url>` location resolves as before.
