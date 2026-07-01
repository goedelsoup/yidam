# Manufacturing Technology — Bootstrap Script

Novel domain. A knowledge graph for a precision manufacturing operation — tracking part
designs, manufacturing processes, machine configurations, quality measurements, and
non-conformances to understand where process variation originates and how it propagates
through the production system.

---

## Initial User Message

> I want a corpus for a CNC machining operation. We make high-tolerance aerospace
> components. The thing I actually care about is: when a part fails inspection, what in
> the process produced that outcome? We have part numbers, operations, setups, tooling,
> measurements, and non-conformances — but they exist in separate systems (ERP, CMM
> output, tool crib logs) and nothing connects them. I want to build the graph that
> connects them.

---

## Prelude Internalized Checkpoint

> **Prelude internalized.** Graph model: files are nodes, markdown links are directed
> edges, git history is the knowledge record. Key constraints: every node must link to
> at least one other; nodes must be specific enough to be wrong; commit messages describe
> what was understood, not what was changed. Directory layout: class definitions in
> `.yidam/corpus/<class>.ont.yml`; instances in `.yidam/corpus/<class>/<instance>.yml`.

---

## Ontology Discovery Dialogue

**Bootstrap:** The problem you're describing is fundamentally a provenance problem: a
measurement outcome needs to trace back through a chain of decisions and configurations
to explain *why* it came out the way it did. Let me probe the key nodes before naming
any classes.

1. Is **part** the right primary unit — as in, the part drawing or specification — or is
   it the **workpiece** (a specific physical instance of a part moving through the shop)?
   They're different: the spec is stable; the workpiece has a lot number, a traveler, and
   a history.
2. An **operation** in an ERP is usually a step in a routing (e.g., "Turn OD, .001 tol,
   Mazak VTC-300"). Is a *setup* a node separate from the operation, or is it a
   configuration event that happens within an operation?
3. **Tooling** — is a tool an instance (this specific insert, serial number 00142) or a
   type (CNMG 432 grade IC807)?
4. When you say non-conformance — is that a failed inspection, a disposition decision,
   or both?

**User:** Part is the drawing/spec; workpiece is the physical instance. Setup is separate
— a setup is a specific configuration on a specific machine for a specific lot, and the
same operation might require different setups for different materials. Tool instance
matters more than tool type because we track insert wear. Non-conformance is the
inspection finding; disposition is a separate decision that follows.

**Bootstrap:** That gives us a very clean causal chain:
- Part spec → operation routing → setup → workpiece → measurement → non-conformance → disposition

One more question: **machine** — is it a node? A setup only happens on a specific
machine, and machine-level variables (spindle wear, thermal drift) are often the source
of systemic variation.

**User:** Yes, absolutely. Machine is a node. That's actually where most of our systematic
issues come from — a machine goes out of calibration and it shows up in twenty lots before
anyone catches it.

---

## Confirmed Sketch

**Nodes**

| Node | What it is |
|------|------------|
| `part` | A part drawing or specification — stable, version-controlled, defines nominal geometry and tolerances |
| `workpiece` | A specific physical instance of a part — lot number, material cert, traveler; what actually moves through the shop |
| `operation` | A defined step in a routing — process type, tolerance requirements, machine type requirement |
| `setup` | A specific configuration on a specific machine for a specific lot — tooling, fixturing, offsets, coolant |
| `machine` | A specific piece of production equipment — type, serial number, calibration status, spindle hours |
| `tool-instance` | A specific cutting tool or insert — grade, geometry, accumulated cuts, wear measurement |
| `measurement` | A recorded inspection result — feature measured, nominal, actual, deviation, instrument |
| `non-conformance` | A documented out-of-tolerance finding — feature, severity, linked measurement |
| `disposition` | A resolution decision for a non-conformance — use-as-is, rework, scrap, supplier return |

**Edges**

```
workpiece →[instantiates]→ part
workpiece →[produced-by]→ setup
setup →[executes]→ operation
setup →[performed-on]→ machine
setup →[used]→ tool-instance
measurement →[inspects]→ workpiece
measurement →[references]→ operation
non-conformance →[triggered-by]→ measurement
non-conformance →[found-on]→ workpiece
disposition →[resolves]→ non-conformance
machine →[within-calibration-for]→ operation
```
