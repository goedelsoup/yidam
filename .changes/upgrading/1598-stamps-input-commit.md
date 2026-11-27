### A step can declare the outputs that carry its input commit

**`stamps_input_commit` names an output whose consumer requires `$YIDAM_INPUT_COMMIT` (#1598, #1600).**

A re-run compares that output with the commit masked, so an unchanged answer writes a note, not a commit.
`yidam doctor` reports such an output as declared and no longer warns about it.
It warns about a declared output that no longer carries the commit.
Each entry is a path, not a glob, and a `writes` glob must cover it.
An older `yidam` refuses a manifest that uses the key, so update the pinned binary first.
See [RFC-0052 §2.7](rfcs/0052-receipt-revalidation.md).
