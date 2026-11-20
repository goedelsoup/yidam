### A coverage universe can report the nodes whose keys it dropped

**`[[due.coverage]]` takes `orphans` and `except` (#1526).**

With `orphans = true`, the coverage clock also lists each instance whose key no member names.
It respects `scope` and `matches`, and a stale or empty list reads no orphans.
`except` leaves out instances kept on purpose, by a property or by `label`.
Each instance it leaves out is counted in the detail.
A declaration without `orphans` reports exactly what it did.
