### `catalog-location-malformed` checks identifiers against your packs

**Once you enable a source pack, identifier locations are held to it (#1328).**
`yidam lint` reports a scheme that no enabled pack declares.
It also reports a local id that the scheme's `pattern` refuses.

When a template pack declares the scheme, the finding names that pack.
For example, `wayback:` names `archive`. Pin it under `prelude_sources` to clear the finding.

**What changes for you: nothing, until you pin a pack.**
A corpus with no packs is checked for the `scheme:local-id` shape alone, as before.
