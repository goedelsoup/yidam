### The `socrata` source pack

**The template ships `socrata` 0.1.0, for open-data portals (#1344).**
`socrata:` reads a dataset on any Socrata portal, such as `socrata:data.cdc.gov/bi63-dtpu`.
Its metadata names the rows as a `socrata-rows:` location, read page by page to the last row.
`arcgis:` reads every feature of an ArcGIS REST layer, such as `arcgis:sampleserver6.arcgisonline.com/arcgis/rest/services/Census/MapServer/0`.
See [Source packs](source-packs.md).

**A portal keeps no earlier version of a dataset.**
What was read is what the digest records, so the pack sets `ttl_days` to 90.

**What changes for you: nothing, until you pin it.**
Add `socrata@^0.1` under `prelude_sources`, then run `mise run yidam-vendor-update`.
