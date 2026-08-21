# Actions — Intervention

## Queries

- List all estimands defined by a given intervention: follow `defines` edges out
- Find the control intervention contrasted with a given treatment intervention: follow
  `contrasts-with` edges

## Transitions

- An intervention node may be annotated with a status indicating whether this intervention
  has been realized in any study design in the corpus (linked via `study-design`)

## Skills

- `identification-graph-checker` — given this intervention as the do-target, check whether
  identification criteria are satisfied in the current causal graph
