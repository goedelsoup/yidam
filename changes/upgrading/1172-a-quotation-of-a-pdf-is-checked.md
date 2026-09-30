### A quotation of a PDF is checked against a text reading

**New command, `yidam catalog-extract` (#1172).** It takes a text reading of each PDF artifact.
It files the reading in the vault cache and records it under the PDF as `text:`. It commits the
change as `extract:`.

**Lint compares a quotation of a PDF with that reading.** Before, every such quotation was
`quotation-unchecked`. Now a span missing from the reading is `quotation-span-drift`, an error.
Run `catalog-extract` on the machine that holds the PDFs, then run `yidam lint`.

**Nothing changes until you run it.** A PDF with no reading stays `quotation-unchecked`. An older
binary ignores the `text:` key.
