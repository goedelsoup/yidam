# Actions — Identification

## Queries

- Find which assumptions a strategy requires: follow `requires` edges out
- Find which estimand a strategy targets: follow `targets` edges out
- Find which confounders threaten this strategy: follow `threatened-by` edges in
- Find which study designs make this strategy available: follow `enabled-by` edges in

## Transitions

- Identification status may be updated when new assumptions are established or when
  confounding structures are discovered that threaten the strategy

## Skills

- `identification-check` — evaluate whether the current corpus evidence supports the
  assumptions required by this identification strategy
- `identification-graph-checker` — check whether the graphical criterion for this strategy
  is satisfied in the current causal DAG
