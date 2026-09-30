### An edge `source:` is resolved

**A new Error check, `edge-source-unresolved` (#1067).** Before, lint checked only that a
`verified` edge wrote a `source:`. A value naming a renamed or nonexistent catalog entry passed.

Lint now resolves the value on every edge that writes one, whatever its tag. Three spellings
resolve: a catalog entry's file stem, a path written the way `target:` is, or a decision
record's `id:`. Every edge source in every derived corpus we measured already resolves. A
corpus that writes none has nothing to do.

If the check fires, the finding names the edge and the value as written. Rename the value to
the entry's current stem, or baseline the finding.
