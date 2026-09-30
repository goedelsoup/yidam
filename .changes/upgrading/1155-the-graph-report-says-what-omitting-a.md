### The graph report says what omitting a property costs

**A new field, `omission`, on each class property in `yidam graph --format json` (#1155).**
Its value is `gates`, `reported` or `licensed`: the verdict `missing-property` gives when an
instance omits the property. Before, the report carried only `required`, one bool. A property
marked `required: false` and one that said nothing both arrived as `false`. The web
editor labelled a licensed omission **reported**.

`required` is unchanged. The web editor reads `omission` and labels a licensed property
**allowed**. Against a binary older than the field, it labels a `false` property **does not
gate** rather than guessing. Nothing changes for a corpus.
