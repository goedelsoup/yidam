### The `courts` source pack

**The template ships `courts` 0.1.0, for court opinions and dockets (#1344).**
`courtlistener:` reads CourtListener's clusters, opinions and dockets, such as `courtlistener:clusters/105221`.
`caselaw:` reads a case from the Caselaw Access Project's static volumes, such as `caselaw:us/347/0483-01`.
See [Source packs](source-packs.md).

**`courtlistener:` needs `COURTLISTENER_TOKEN`, and sends it only to `www.courtlistener.com`.**
A `caselaw:` case needs no token, so the pack is ready without one.

**What changes for you: nothing, until you pin it.**
Add `courts@^0.1` under `prelude_sources`, then run `mise run yidam-vendor-update`.
