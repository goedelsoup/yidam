# Actions — Assumption

## Queries

- Find all identification strategies requiring this assumption: follow `required-by` edges
  to identification nodes
- Find all estimators requiring this assumption: follow `required-by` edges to estimator nodes
- Find which study designs support or undermine this assumption: follow `supported-by` edges

## Transitions

- An assumption's `testability` property should be updated when a new auxiliary test or
  falsification approach is added to the corpus
- An assumption may receive an `implies` link when a logical entailment to another
  assumption is confirmed

## Skills

- `assumption-audit` — traces which estimators and identification strategies depend on this
  assumption and evaluates the supporting evidence in the current corpus
- `sensitivity-bounds` — for assumptions like ignorability, computes how large a violation
  would need to be to overturn a finding
