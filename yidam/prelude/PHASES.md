# Analysis Phases

After genesis, agent inquiry has shape. A **phase** is a named unit of work — a bounded
investigation with a declared input state, a body of agent activity, and a set of committed
outputs. Phases make the corpus's growth legible in the git log and give agents a unit of
accountability beyond the individual commit.

## Anatomy of a phase

| Element | What it is |
|---|---|
| **Name** | A short, declarative label: what question or task this phase addresses |
| **Input state** | The corpus state (branch tip, node set, open questions) at phase start |
| **Agent work** | Retrieval, calculation, synthesis, or assessment — using the domain computer |
| **Outputs** | New or revised nodes, new edges, resolved or opened questions |
| **Commit pattern** | One or more commits forming a coherent unit; the final commit names the phase |

## Phase types

**Investigation** — An agent explores a question using connectors and calculators. It reads
the corpus, queries external sources, and produces findings. Output: new corpus nodes, new
catalog edges, updated open-question nodes. The investigation → distill → commit cycle is
the standard pattern.

**Extraction** — Structured data is pulled from a primary source and committed as corpus
nodes. Output: validated authored or generated nodes linked to catalog entries. Extraction
phases are often automated; the commit is operational but the node is permanent.

**Synthesis** — Existing nodes are linked or merged across inquiry threads. Output: new
edge-bearing nodes, synthesis notes, resolved tensions. A synthesis phase is a first-class
knowledge contribution — it can be the primary output of a branch.

**Assessment** — Competing hypotheses are evaluated against evidence. Output: hypothesis
nodes updated with evidence cells, open questions narrowed or closed. Assessment phases
often follow a period of investigation and extraction.

## Phase discipline

- **One phase, one branch.** Open a branch when a phase begins; settle its outputs onto the
  baseline when they are ready. Branches carry in-progress phases; the baseline — an elector's
  `ma/<name>` position, resolved collectively into a `rigpa/<evolution>` (see [GRAPH.md](GRAPH.md))
  — carries settled knowledge.

- **Commit legibly within the phase.** Each commit in a phase should be a legible step.
  The final commit should name the phase and summarize what it produced — this is the event
  that future readers will use to understand why the corpus changed.

- **Bound phases.** A phase that never produces commits is an open inquiry thread, not a
  settled phase. If a phase stalls, mark it: open a question node naming what is blocking it
  and return to the baseline. Do not let branches accumulate without outputs.

- **Do not mix phase types in one commit.** An extraction commit and a synthesis commit
  have different semantics and different validation requirements. Keeping them separate
  preserves the graph's epistemic structure.

## Phases and the domain computer

The domain computer (crates/, packages/) provides the capabilities phases run on:

- **Connectors** are called during investigation and extraction phases to retrieve external
  data. They are side-effecting and may fail; their outputs are cached and committed as
  catalog entries or generated corpus nodes.

- **Calculators** are called during investigation and assessment phases to transform domain
  models. They are pure and deterministic; their outputs are committed as findings or
  updated nodes.

The agent orchestrates both. It does not implement retrieval or calculation — it directs
which connectors and calculators to invoke, and synthesizes their outputs into corpus nodes.

## Phases and sangha

Each elector's `ma/<name>` branch represents an active phase or set of phases. When
positions are ready to be resolved into collective understanding, the sangha reads the
phase outputs across `ma/*` branches and synthesizes them into a `rigpa/<evolution>`. A
rigpa branch is the settled state produced by resolving a set of completed phases.
