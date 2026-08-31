---
kind: hint
---

# Custody events chain, and the chain is what has gaps

Consider modelling the sequence explicitly — each custody event naming the one it follows —
rather than letting date order imply it.

Ordering by date makes a gap invisible: two events six years apart look exactly like two
events six days apart, and the whole point of this corpus is to see the six years.

An explicit chain makes the gap a **query** — a custody event with nothing following it, or
two that do not join — which is what lets the gap be derived rather than asserted. A sorted
list cannot express the question at all, and a corpus that cannot ask it ends up storing the
answer.
