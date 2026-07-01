# Watershed Management — Bootstrap Script

Novel domain. A knowledge graph for a specific watershed system — tracking stream reaches,
discharge permits, land use, flow conditions, and regulatory status with the goal of
understanding cumulative hydrological impact.

---

## Initial User Message

> I want to build a corpus for a specific watershed — let's say the Little Miami River
> basin. The question I care about is whether current discharge permits are consistent
> with the river's actual assimilative capacity under low-flow conditions. That means I
> need to track reaches, permitted discharge points, land use upstream, and flow monitoring
> data — and I need them to connect to each other, not just sit in separate tables.

---

## Prelude Internalized Checkpoint

> **Prelude internalized.** Graph model: files are nodes, markdown links are directed
> edges, commit history is the knowledge record. Key constraints: every node must connect
> to at least one other; commit messages describe what was understood; no speculation
> without source. Directory layout: class definitions in `.yidam/corpus/<class>.ont.yml`;
> instances in `.yidam/corpus/<class>/<instance>.yml`.

---

## Ontology Discovery Dialogue

**Bootstrap:** This is a clearly bounded domain. The central tension seems to be:
*permitted load* vs. *actual capacity* — and you need the physical river state (reaches,
flow) to anchor both sides. Let me probe the key concepts:

1. Is a **reach** the fundamental unit — a stretch of river with roughly uniform
   characteristics — or do you work at the watershed/sub-watershed level?
2. A **permit** is a legal instrument, but what matters is its *load* — the mass of
   pollutant per day it authorizes. Is "permit" a node, or is "discharge point" (the
   physical location) the node and the permit is a property?
3. You mentioned land use upstream. Is land use a node type, or is it a property of a
   sub-watershed that shapes modeled load?
4. What are the measurement nodes — flow gages, water quality sampling stations?

**User:** Reach is the fundamental unit. Permit and discharge point are separate — the
permit is a legal instrument with its own history; the discharge point is physical.
Land use is a node because it changes over time and I want to track it. Monitoring
stations are separate nodes — they produce data that validates or challenges permit math.

**Bootstrap:** Good. One more: **regulatory limit** — the numeric standard a permit must
not exceed (e.g., 7Q10 low-flow standard, total nitrogen limit). Is that a property of
the permit, or a first-class node that multiple permits reference?

**User:** First-class — the same standard governs multiple permits and I want to see when
a set of permits collectively exceeds it even if each is individually compliant.

---

## Confirmed Sketch

**Nodes**

| Node | What it is |
|------|------------|
| `reach` | A defined segment of stream with roughly uniform hydrological character; the primary spatial unit |
| `discharge-point` | A physical outfall where a permitted discharge enters the stream system |
| `permit` | A legal instrument authorizing a specific discharge load; has history (issuance, renewals, violations) |
| `land-use-parcel` | A defined area of land with a classified use that contributes runoff load to a reach |
| `monitoring-station` | A flow gage or water quality sampler that produces measurements used to validate permit math |
| `regulatory-limit` | A numeric water quality standard (e.g., 7Q10 low-flow threshold, nutrient limit) that governs one or more permits |

**Edges**

```
reach →[flows-into]→ reach
discharge-point →[discharges-into]→ reach
permit →[authorizes]→ discharge-point
land-use-parcel →[drains-to]→ reach
monitoring-station →[monitors]→ reach
regulatory-limit →[governs]→ permit
regulatory-limit →[applies-to]→ reach
```
