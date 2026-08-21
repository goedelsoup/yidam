# Actions — Confounding

## Queries

- Find which identification strategies are threatened by this confounding structure:
  follow `threatens` edges out
- Find which assumptions address this confounder: follow `blocked-by` edges out (reverse
  of `blocks` from assumption)
- Find which study designs can rule out this confounder: follow `ruled-out-by` implied edges

## Transitions

- A confounding instance may be updated from `measurability: latent` to `measurability: observed`
  when a proxy variable is identified, triggering reassessment of identification strategies

## Skills

- `identification-graph-checker` — given this confounder as a node in a DAG, determine
  whether any adjustment set blocks all backdoor paths from treatment to outcome
- `sensitivity-bounds` — quantify how strong this unmeasured confounder would need to be
  to produce the observed treatment-outcome association spuriously
