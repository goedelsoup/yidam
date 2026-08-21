# Actions — Estimator

## Queries

- Find which estimand this estimator targets: follow `recovers` edges out
- Find which assumptions this estimator requires: follow `valid-under` edges out
- Find which identification strategy this estimator operationalizes: follow `operationalizes`
  implied edges

## Transitions

- An estimator instance may receive additional `valid-under` links as the set of required
  conditions is refined (e.g., overlap assumption for IPW, first-stage strength for 2SLS)

## Skills

- `sensitivity-bounds` — compute the sensitivity of this estimator's output to violations
  of its key assumption
- `assumption-audit` — trace all assumptions this estimator requires and assess the
  corpus evidence for each
