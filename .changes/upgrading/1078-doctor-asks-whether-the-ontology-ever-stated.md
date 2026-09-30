### `doctor` asks whether the ontology ever stated its contract

**A new `contract` check (#1078).** It warns when no property in the ontology says `required:`.
It also warns when no class with edges says `edge_policy:`. Either way, `missing-property` or
`unlicensed-edge` can warn there and never gate.

Writing the key is the answer, whatever its value. `required: false` counts, and so does
`edge_policy: characteristic`. One class answering answers for the whole ontology.

It is a `warn`, so `doctor` still exits 0. **`doctor --strict` does not.** Of 16 derived
ontologies measured, 14 warn.

`doctor --only <id>` reports only the named checks. `yidam-vendor-update` runs
`doctor --only contract` after it re-vendors. A binary that predates the flag prints nothing
there, so the first re-vendor after an upgrade may stay quiet.
