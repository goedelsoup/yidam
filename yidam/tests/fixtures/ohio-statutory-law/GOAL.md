# Ohio Statutory Law — Assessment Criteria

## Domain summary

A knowledge graph for researching Ohio statutory law — how sections, definitions, court
decisions, and precedent chains connect across the Ohio Revised Code.

## Central question

How do statutory definitions propagate across the ORC, how do court decisions interpret
and extend them, and what does the resulting precedent network reveal about contested
legal concepts?

---

## Required ontology

These classes must appear:

- `statute-section` (or equivalent) — the atomic unit of the ORC. Without this, the
  corpus has no anchor in the actual text.
- `definition` (or `term`) — distinct from the section that contains it. This is the
  node type most likely to have many inbound edges from court decisions; collapsing it
  into sections loses the propagation structure.
- `court-decision` — the interpretive layer. Without decisions, the corpus is a
  document index, not a legal knowledge graph.
- `legal-concept` (or `doctrine`) — cross-chapter abstractions. Without these, the graph
  can only express what is in a single section, not how concepts travel.

Additionally strong:

- `chapter` — navigational hierarchy that groups sections; helps the graph remain
  traversable as it grows
- `precedent-chain` — makes the interpretive sequence explicit rather than implicit in
  the individual decision→decision edges

---

## Required edges

- `court-decision` →[interprets]→ `definition` — the core interpretive edge
- `court-decision` →[relies-on]→ `court-decision` — precedent must be a directed edge,
  not a property
- `definition` →[cross-references]→ `definition` — cross-chapter propagation
- `statute-section` →[defines]→ `definition` — provenance from text to term

If only one direction of edges exists between decisions (no overturning, only reliance),
the corpus will miss the dynamic of legal revision.

---

## Seed instance quality

Good seed instances:

- A statute section names its actual ORC number (§ 2901.01, not "a section in Title 29")
  and describes what it covers
- A definition names the term ("serious physical harm"), cites the section that defines
  it, and includes the operative language
- A court decision names the case (State v. Thompkins), year, court level, and the
  specific issue decided
- A legal concept like "mens rea" or "causation" links to at least two definitions and
  one court decision

Red flag: definitions named "Definition 1" or decisions named "Sample Case."

---

## Good bootstrap looks like

5–7 classes capturing the distinction between the statute hierarchy (sections, chapters)
and the interpretive layer (decisions, concepts, precedent chains). At least one
cross-chapter definition with inbound edges from multiple court decisions. The genesis
commit message names the jurisdiction and subject matter (e.g., "Ohio criminal law,
Title 29"), lists the class schema, and describes at least one precedent chain or
cross-chapter propagation.

---

## Red flags

- No court decisions — the corpus is a document index, not a legal knowledge graph
- Decisions as properties on definitions rather than first-class nodes — precedent chains
  cannot be traversed
- No cross-chapter definitions — misses the propagation structure that makes statutory
  law interesting to graph
- Statute hierarchy treated as the primary structure (chapter → section → subsection)
  with definitions as leaf nodes — inverts the knowledge priority
- Legal concept class absent — the graph cannot represent contested doctrines that span
  multiple statutes
