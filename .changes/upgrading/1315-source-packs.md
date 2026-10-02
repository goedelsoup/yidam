### Source packs, `prelude_sources`, and `yidam source check`

**A source pack names a family of sources by identifier, such as `doi:` or `pmc:` (#1315).**
It is a directory holding `pack.toml` and `entry.md`, with optional `transforms/` and `fixtures/`.
See [Source packs](cli-reference.md#source-packs) for the format.

**`yidam source check` holds every pack and every pin to that format, offline.**
It exits nonzero on an error, so a repository that writes its own pack runs it in CI.

**`prelude_sources` in `.yidam/decisions/proposals.yml` pins the packs a re-vendor copies.**
Write `<pack>@<range>`, or `<pack>@<range> from <repo>@<commit>` for another corpus's pack.
`mise run yidam-vendor-update` copies each into `.yidam/.vendor/sources/`.
It refuses a pin the pack's version does not meet, and then changes nothing.

**What changes for you: nothing, until you pin a pack.**
A repository with no `prelude_sources` vendors no packs, and `source check` passes.
The template ships no packs yet.
