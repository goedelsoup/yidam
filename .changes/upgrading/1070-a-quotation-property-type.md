### A `quotation` property type

**A property may be declared `type: quotation` (#1070, RFC-0046).** Its value names a catalog
entry in `of:` and holds the copied words in `span:`. Add `sha256:` when the entry holds more
than one artifact. `quotation-span-drift` fails when the entry's cached bytes lack the words.
`quotation-unresolved` fails when `of:` or the pin names nothing. Where the bytes are not in
this machine's vault cache, or are not text, `quotation-unchecked` reports it at Info. A CI
runner with no cache sees only Info findings.

Nothing changes until a class declares the type. A corpus that coined `quotation` for itself
is now checked against this shape. `yidam rename` rewrites `of:` with the entry.

A quotation's `of:` is a citation of its entry (#1174). `catalog-uncited` and
`verified-unsourced` count it, as they count an edge `source:`.
