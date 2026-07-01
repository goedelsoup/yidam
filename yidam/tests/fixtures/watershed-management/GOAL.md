# Watershed Management — Assessment Criteria

## Domain summary

A knowledge graph for a specific watershed — tracking stream reaches, discharge permits,
land use, monitoring data, and regulatory standards to assess whether current permit loads
are consistent with the river's assimilative capacity under low-flow conditions.

## Central question

Are current discharge permits, individually and in aggregate, consistent with the
watershed's actual assimilative capacity under low-flow conditions — and if not, which
reaches, which permits, and which land uses are responsible?

---

## Required ontology

These classes must appear:

- `reach` — the fundamental spatial unit of the stream network. Without it, nothing is
  anchored in a specific physical place.
- `discharge-point` — the physical outfall. Must be separate from `permit` because a
  single point can have a permit history (renewals, modifications, violations).
- `permit` — the legal instrument. Must be separate from discharge point because the
  question is about permit math, not just physical geography.
- `regulatory-limit` — the standard against which permits are measured. If this is only
  a property on permits rather than a first-class node, the corpus cannot represent the
  case where *multiple permits in aggregate* exceed a single limit applied to a reach.
- `monitoring-station` — the empirical grounding. Without this, the corpus cannot
  connect model predictions to measurement.

Optional but strong:

- `land-use-parcel` — makes non-point source load explicit and traversable; without it
  the corpus can only model permitted point-source discharges

---

## Required edges

- `reach` →[flows-into]→ `reach` — the network topology. Without this edge type, the
  graph cannot propagate load downstream or identify cumulative exceedances.
- `discharge-point` →[discharges-into]→ `reach` — connects legal instrument to physical
  location
- `regulatory-limit` →[governs]→ `permit` AND →[applies-to]→ `reach` — both directions
  needed to answer the cumulative question

A corpus with reach nodes but no `flows-into` edges between them is a list, not a network.

---

## Seed instance quality

Good seed instances:

- A reach names a specific segment (e.g., "Little Miami R. — Spring Valley to Corwin"),
  gives approximate length, catchment area, and the downstream reach it flows into
- A permit names the NPDES permit number, the permittee, the authorized daily load for
  at least one constituent, and the discharge point it authorizes
- A regulatory limit names the specific standard (Ohio EPA 7Q10 low-flow standard for
  the Little Miami), the numeric threshold, and the constituent it limits
- A monitoring station names its USGS gage number, the reach it monitors, and the
  period of record

Red flag: instances named "Reach 1," "Permit A," "Example Monitoring Station."

---

## Good bootstrap looks like

5–6 well-linked classes capturing the stream network (reach topology), the regulatory
layer (permit, discharge point, regulatory limit), and the empirical layer (monitoring
station, optionally land use). At least two reaches linked by `flows-into`, at least one
permit linked through a discharge point to a reach, and at least one regulatory limit
linked to both a permit and a reach. The genesis commit message names the specific
watershed, states the central question, and identifies which permit or reach is the
initial focus.

---

## Red flags

- Reach class present but no `flows-into` edges — the network topology is not represented
- Permit and discharge point collapsed into one class — cannot track permit history
  separately from physical location
- Regulatory limit as a property rather than a node — cannot represent aggregate
  exceedance across multiple permits
- No monitoring station — corpus is model-only with no empirical ground truth
- Generic instances not anchored to the named watershed (Little Miami or equivalent)
- Genesis commit treats this as a database schema rather than a knowledge event
