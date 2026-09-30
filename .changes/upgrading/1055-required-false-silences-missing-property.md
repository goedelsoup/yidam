### `required: false` silences `missing-property`

**An omission the class licensed is no longer a warning (#1055).** Before, `missing-property`
warned on every omitted property that was not `required: true`. A property marked
`required: false` warned the same as one that said nothing.

It is now silent on `required: false`. A property that says nothing still warns, and
`required: true` still gates. To accept a standing `missing-property` floor, write
`required: false` on the properties an instance may omit. No baseline is needed.
