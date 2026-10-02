### Catalog locations gain `identifier`, and the catalog gains four types

**A catalog location can be `kind: identifier`, with a value of `scheme:local-id` (#1314).**
For example, `doi:10.1167/tvst.8.5.14` or `pmc:PMC6753881`.

`kind: doi` and `kind: pmc` are still reported by `catalog-location-malformed`.
The finding now names the fix: run `yidam migrate --dry-run locations`, then `yidam migrate locations`.
A value that is a URL is reported and left for you to rewrite.

**The catalog `type` set gains `statute`, `report`, `standard` and `document`.**
`yidam lint` now reports `catalog-type-unknown` at `Warn` for any other word.
`primary` is reported too, because it names a standing, not a form.
Nothing maps it for you: pick the type that fits each source.
