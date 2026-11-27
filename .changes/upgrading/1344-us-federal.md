### The `us-federal` source pack

**The template ships `us-federal` 0.2.0, for federal law and statistics (#1344).**
`ecfr:` reads a CFR section as it stood on a date, such as `ecfr:2024-01-01/40/60.1`.
`govinfo:` reads a GovInfo package's record, and names its PDF as a `govinfo-pdf:` location.
`uscode:` reads one US Code title, pinned to the release point that was read.
`fec:`, `fec-filing:`, `bls:`, `census:` and `census-api:` read FEC, BLS and Census records.
`eia:`, `irs:`, `irs-990:`, `nces:` and `ncei:` read an EIA, IRS, NCES or NCEI file by its path (#1608).
See [Source packs](source-packs.md).

**The FEC and the Census Data API need a key.**
Set `FEC_API_KEY` and `CENSUS_API_KEY`. Each is sent only to its own API host.

**What changes for you: nothing, until you pin it.**
Add `us-federal@^0.2` under `prelude_sources`, then run `mise run yidam-vendor-update`.
