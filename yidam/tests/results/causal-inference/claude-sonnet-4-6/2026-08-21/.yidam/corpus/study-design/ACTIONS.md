# Actions — Study Design

## Queries

- Find which assumptions this design supports: follow `supports` edges out
- Find which identification strategies this design enables: follow `enables` edges out
- Find which confounders this design rules out by construction: follow `rules-out` implied edges

## Transitions

- A study design instance may receive additional `supports` links as domain analysis
  establishes which assumptions the design's features bear on

## Skills

- `identification-check` — given this study design, evaluate which identification
  strategies are available and which assumptions need justification from domain knowledge
  rather than design features
