# Manufacturing Technology — Assessment Criteria

## Domain summary

A knowledge graph for a CNC precision manufacturing operation — connecting part
specifications, workpieces, operations, setups, machines, tooling, measurements, and
non-conformances to trace where process variation originates and how it propagates.

## Central question

When a workpiece fails inspection, what chain of process decisions and equipment states
produced that outcome — and which machines, setups, or tool conditions are responsible
for systematic variation across multiple lots?

---

## Required ontology

These classes must appear:

- `part` — the specification. Must be separate from `workpiece` so that a single drawing
  can have many physical instances; without this, the graph cannot compare variation
  across lots of the same design.
- `workpiece` — the physical instance. This is what moves through the shop, gets measured,
  and generates non-conformances. If part and workpiece are collapsed, the graph loses the
  ability to trace specific lots.
- `setup` — the configuration event. Must be separate from `operation` because the same
  operation can produce different results depending on how it was set up (tooling, offsets,
  machine). Without setup, the causal chain from process to outcome is missing its most
  actionable node.
- `machine` — must be a first-class node. Systemic variation (spindle wear, thermal drift,
  out-of-calibration states) is machine-level, not setup-level or operation-level. If
  machine is only a property on setup, the graph cannot answer "which machine is producing
  systematic non-conformances."
- `measurement` — the empirical evidence. Must be separate from `non-conformance` because
  many measurements are in-tolerance; the non-conformance is a derived state, not the
  measurement itself.
- `non-conformance` — the quality event. If this is only a property on `measurement`, the
  disposition decision has nowhere to attach.

Optional but strong:

- `tool-instance` — specific insert tracking is what enables tool wear as a root cause;
  without it, setup→measurement correlation is possible but not tool-condition correlation
- `disposition` — the decision that closes a non-conformance; needed if the corpus will
  track quality outcomes (use-as-is, rework, scrap rates)

---

## Required edges

- `workpiece` →[instantiates]→ `part` — connects physical instance to specification
- `workpiece` →[produced-by]→ `setup` — the core provenance edge
- `setup` →[performed-on]→ `machine` — connects setup to equipment state
- `measurement` →[inspects]→ `workpiece` — ties measurement result to what was measured
- `non-conformance` →[triggered-by]→ `measurement` — causal derivation

If setup and machine are connected but machine has no calibration or condition state, the
graph can record which machine but cannot explain *why* that machine produced the result.

---

## Seed instance quality

Good seed instances:

- A part names a real part number (or a realistic stand-in like "P/N 44210-A Rev.C"),
  its material class, and its most demanding tolerance
- A workpiece names a lot number, links to the part drawing, names the machine it was
  produced on (via setup), and has a production date
- A setup names the machine, the operation, the lot it was running, and at least one
  tooling or offset configuration detail
- A machine names the specific asset (Mazak VTC-300C, Asset #M-047), its current
  calibration status, and the operations it is qualified to run
- A non-conformance names the specific out-of-tolerance feature, the actual vs. nominal
  measurement, and the workpiece it was found on

Red flag: workpieces named "Part Instance 1," machines named "CNC Machine A," setups
described as "a manufacturing step."

---

## Good bootstrap looks like

7–9 classes covering the full causal chain from specification through physical production
to quality outcome. At least two workpieces from the same part (enabling lot comparison),
at least one machine linked to multiple setups (enabling machine-level variation analysis),
and at least one non-conformance with a traceable chain back through measurement → setup →
machine. The genesis commit message names the industry (aerospace, automotive), the
tolerance regime, and the systemic variation question — not a generic description of
manufacturing.

---

## Red flags

- Part and workpiece collapsed — cannot distinguish specification from physical instance
  or compare multiple lots
- Setup absent — the causal chain jumps from operation directly to measurement, losing
  the configuration that explains variation
- Machine as a property on setup rather than a first-class node — cannot aggregate
  non-conformances by machine or track calibration history
- Non-conformance as a property on measurement rather than its own node — disposition
  has nowhere to attach; non-conformances cannot be queried as a class
- Measurement and non-conformance collapsed — every measurement appears as a failure;
  in-tolerance results cannot be represented
- No connector opportunities identified for ERP, CMM output, or tool crib logs — the user
  explicitly named these as the data sources; missing them means the corpus will be
  manually maintained indefinitely
- Genesis commit describes "a manufacturing corpus" without naming a part family, machine
  type, or quality question
