### A date that does not exist is no longer read as a later day

**A `retrieved:` date must be a real `YYYY-MM-DD` day before a TTL counts from it (#1617).**

`2024-02-31` was read as 2 March, and `2024-2-3` as 3 February.
Neither is read now, so the entry has no date to measure against.
`lint` reports such an entry under a TTL as `catalog-expired`, and `doctor` and `due` count it.
The finding says nothing records when the entry was fetched, though the field is there.
Correct the date, with a month and day of two digits each.
`doctor` also warns on a vendored-prelude pin date of that kind, where it gave an age.
