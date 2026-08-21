# Actions — Estimand

## Queries

- Find which identification strategies can target this estimand: follow `targeted-by` edges in
- Find which estimators can recover this estimand: follow `estimated-by` edges in
- Find the interventions that define this estimand: follow `defined-by` edges in

## Transitions

- An estimand node may be updated with an `identification_status` property as identification
  strategies are analyzed
- An estimand may receive a `decomposes-into` link when a more refined sub-estimand is added

## Skills

- `identification-graph-checker` — check whether this estimand is identified under a given
  set of assumptions and causal graph
- `sensitivity-bounds` — compute how sensitive an estimate of this estimand is to
  unmeasured confounding
