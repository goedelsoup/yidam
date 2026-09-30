### A class may declare its interval

**`interval:` names an instance's start and end (#1201).** Write `start:` and `end:`, each a
`date` property the class declares. Add `exclusive_over:` with a relationship from `edges:`.
No two instances may then link one target by it at once.

A new `interval-overlap` lint check is an error for each overlapping pair, and for an end before
its start. Intervals are half-open. They compare as `query` orders dates, at the precision both
sides share. So `1893` and `1893-06-01` meet rather than overlap. An absent end is still open.

Nothing changes until a class declares `interval:`. A declaration with a misspelled key, or no
`end:`, is a malformed class.
