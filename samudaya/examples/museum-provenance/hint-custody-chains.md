---
kind: hint
---

# Custody events chain, and the chain is what has gaps

Consider modelling the sequence explicitly — each custody event naming the one it follows —
rather than letting date order imply it.

Ordering by date makes a gap invisible: two events six years apart look exactly like two
events six days apart, and the whole point of this corpus is to see the six years. An
explicit chain has a place to say *nothing is known between these two*, which a sorted list
does not.
