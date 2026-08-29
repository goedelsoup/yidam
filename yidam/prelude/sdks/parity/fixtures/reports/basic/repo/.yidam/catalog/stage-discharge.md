---
name: Stage–discharge relation
description: A rating-curve derivation, obtained and drawn on by two nodes.
type: paper
used-by:
  - mixing-zone.yml
  - low-flow.yml
---

The second entry, and the one with citations. `gauge-record.md` pins the arm where no node
draws on a source; this pins the arm where two do — and its `used-by` list is deliberately
wrong in **both** directions, so `catalog-used-by-drift` and the `drift` field in
`catalog-audit` are each exercised on a real report rather than only in a unit test.

It claims `mixing-zone.yml`, which cites nothing, and omits `tailwater.yml`, which cites it.
