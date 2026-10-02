### The `scholarly` and `archive` source packs

**The template ships its first two source packs (#1320).**
`scholarly` reads papers by `doi:`, `pmc:` and `arxiv:`, through Crossref, Europe PMC and arXiv.
`archive` reads Wayback Machine snapshots and Internet Archive items.
See [Source packs](source-packs.md).

**One `source add --fetch doi:…` reaches a paper's open-access full text.**
The DOI leads to a `europepmc:` location, and that one to `pmc:` when Europe PMC holds the text.
`catalog-extract` then reads the full text as plain text, without the reference list.

**`scholarly` requires `YIDAM_CONTACT`.**
Every request it makes names that contact, and it waits three seconds between requests to one host.

**What changes for you: nothing, until you pin a pack.**
Add `scholarly@^0.1` or `archive@^0.1` under `prelude_sources`, then run `mise run yidam-vendor-update`.
