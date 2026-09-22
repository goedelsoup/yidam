---
evolution: silt-budget
date: 2026-01-02
independence: distinct-seats
tips:
  - ma/hydrologist@ccccccc
  - ma/dredger@ddddddd
---

## What was resolved

Kept in the fixture on purpose, and it is three findings rather than one. It names no
`synthesized-by`, which is every record written before that field existed; one of its tips
is `ma/dredger`, which `electors.md` does not register; and it states `independence:
distinct-seats` over two tips that name no commit in this repository, so the registry cannot
be read at either of them and the derived answer is `unrecorded`. A golden that only ever saw
a well-formed record would not notice the day any of the three stopped reporting.

The third reaches an arm nothing else in this fixture does: the one a thin clone produces.
`independence:` is derived from the registry **at the tips the record names**, and a checkout
without those commits gets `unrecorded` rather than a different answer read off HEAD. No clone
of this fixture has `ccccccc`, so this is what every consumer of the report should expect to
see when the tips are out of reach — a stated value, an underived one, and the reason said
out loud rather than a silent pass.
