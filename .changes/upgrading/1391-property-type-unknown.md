### A coined property type is now reported

**`property-type-unknown` warns on a `type:` that nothing reads (#1391).** Its values were never checked.
Known types are `string`, `text`, `ref`, `date`, `number`, `claim`, `quotation` and `list`.
For entries that are records, declare a `list` with `fields:`.
The finding is a warning and does not gate.
