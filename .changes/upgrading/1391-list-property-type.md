### `type: list` is now a checked type

**A property declared `type: list` is now read (#1391, RFC-0050).** Before, `list` was a coined type and nothing checked it.
Its value must now be a non-empty list.
A single value not wrapped in a list is reported by `property-type`.

**`items:` and `fields:` say what each item is.** `fields:` declares a record, closed to keys it does not name.
A misspelled key in a record is now an error.
See [configuration](configuration.md) for the forms.

**A tag inside a property is no longer the node's standing (#1382).** Standing is read from top-level `type: claim` properties only.
To grade each entry, declare the property `type: list` with a `claim` field.
`claim-property-undeclared` now suggests exactly that.

**The warehouse contract no longer refuses a `list` property.** It gets no column, as a `claim` does.
