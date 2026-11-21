---
name: Stage–discharge relation
description: A rating-curve derivation, obtained and drawn on by two nodes.
type: paper
artifacts:
  - sha256: 419d61a7674cb5452c1bc6ba3f2f77cb45efb05bd2fc9ae0167c84ed586020a0
    bytes: 182391
    media_type: application/pdf
    retrieved: 2026-08-22
    vault: none
---

The second entry, and the one with citations. `gauge-record.md` pins the arm where no node
draws on a source; this pins the arm where two do. It does not list them: an entry does not
store what cites it, and `catalog-audit` reports both from their citations.

Its prose names low-flow.yml without linking it. That is the reference `rename` reports and
leaves, so the rename golden pins the arm where a mention is not rewritten.

It is also the entry that carries an **artifact record**, so the goldens exercise a populated
`Artifacts` cell rather than only the empty one `gauge-record.md` gives them. The record is
well-formed and routed to `vault: none` — the local cache and nowhere else — so it trips
neither `catalog-artifact-malformed` nor `catalog-artifact-unroutable`, in keeping with this
fixture not being a corpus that trips every check. The digest is `sha256("stage-discharge")`,
which is a real digest of a known input rather than a plausible-looking string; nothing
verifies it, because no check here reads bytes.
