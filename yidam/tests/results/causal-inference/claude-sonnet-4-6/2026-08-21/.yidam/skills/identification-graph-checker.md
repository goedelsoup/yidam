---
name: identification-graph-checker
description: Given a causal DAG and a target estimand, determine whether a backdoor, front-door, or IV criterion is satisfied and return the minimal sufficient adjustment set
---

# Skill: identification-graph-checker

Checks identification of a causal estimand in a stated causal graph. Given a DAG (as an
adjacency specification or a list of edges) and a target (treatment variable, outcome
variable), determines whether any standard identification criterion is satisfied and
returns what is needed to identify the estimand.

## What it computes

**Backdoor criterion check** — Is there a set X of measured variables that satisfies the
backdoor criterion relative to (T, Y)? If so, returns the minimal sufficient adjustment
sets and confirms that conditional ignorability holds for each.

**Front-door criterion check** — Is there a set M of measured mediators satisfying the
front-door criterion? (Applicable when T → M → Y and all backdoor paths from T to Y pass
through unmeasured U that does not affect M except through T.)

**IV criterion check** — Given a candidate instrument Z, does Z satisfy relevance,
exclusion restriction, and exogeneity relative to the stated graph? Returns identification
status and the identified estimand (LATE for binary Z and D).

**d-separation queries** — Is node A d-separated from node B given set C in the stated
graph?

## Reads from corpus

- `confounding` — confounding instances define the U nodes in the graph
- `identification` — identification strategy nodes describe which criteria to check
- `assumption` — assumption nodes describe which conditions are claimed to hold

## Returns

- Identification status: `identified`, `partially-identified`, or `not-identified`
- For `identified`: the identifying functional and the minimal sufficient adjustment set
- For `not-identified`: which paths remain unblocked and what additional assumptions would
  close them

## Implementation status

**Stub** — implement in `crates/identification-graph-checker/` as a pure calculator.
Input format: edge list (source, target, observed/unobserved flag) plus query (T, Y, and
optionally Z). Output: JSON with identification status and supporting path analysis.
Candidate libraries: `dagitty` (R, via subprocess) or a native Rust d-separation implementation.
