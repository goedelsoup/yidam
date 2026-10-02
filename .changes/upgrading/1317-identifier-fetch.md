### `catalog-fetch` follows identifiers through source packs

**`catalog-fetch` now fetches a `kind: identifier` location through the pack declaring its scheme (#1317).**
An identifier no enabled pack declares is skipped, as before.
See [Fetching an identifier](cli-reference.md#fetching-an-identifier).

**Every request now names the tool in its User-Agent.**
Set `YIDAM_CONTACT` to an email address or URL to add it there.
A pack with `contact = "required"` fetches nothing without it.

**A refused request now fails the run.**
A non-success status is recorded under `refused` and not retried.
Before, it stopped the whole run with an error.

**`[transport] auth` now names where each credential goes.**
Write `{ env = "VAR", header = "Name" }` or `{ env = "VAR", query = "param" }`.
`source check` now reports a bare variable name as an error.

**`--archive` adds the nearest Wayback capture as a pinned location.**
It needs an enabled pack declaring the `wayback` scheme.
