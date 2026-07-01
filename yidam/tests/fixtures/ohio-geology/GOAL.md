# Ohio Geology — Assessment Criteria

## Domain summary

The stratigraphic, glacial, and ecological geology of Ohio — how rock units, landforms,
ecological systems, and economic deposits form a web of action and counteraction across
geologic and human time.

## Central question

How do Ohio's stratigraphic record, glacial history, physiographic regions, ecological
systems, and resource deposits interact — and how do those interactions shape what is
possible, constrained, or endangered in the present?

---

## Required ontology

At minimum, these five classes must appear (by any reasonable naming):

- `stratigraphic-unit` — the record layer: formations, members, beds. Without this,
  the geological dimension collapses.
- `geologic-event` — the happenings: glacial advances, marine transgressions. Without this,
  the corpus has no mechanism for change — it can describe states but not transitions.
- `ecological-system` — the living layer that both responds to and modifies geology.
  Essential for the eco/econo web.
- `resource-deposit` — the economic layer: aggregate, petroleum, mineral. Essential for the
  counteraction direction of the web.
- `site` — the grounding node: where multiple class types intersect at a named location.
  Without site nodes, the graph has classes but no specific claims.

These are additionally strong (present in the reference run, worth noting if absent):

- `physiographic-region` — groups sites and units by shared landscape character
- `administrative-unit` — the jurisdictional constraint layer
- `time-interval` — anchors events and units in named spans of geologic or human time

---

## Required edges

At least one edge in each direction of the action/counteraction web must be present:

- Something →[produces / deposits / enables]→ something else (geology or event generating
  an ecological or economic condition)
- Something →[modifies / disrupts / constrains]→ something else in the reverse direction
  (ecological or economic system affecting geological access or condition)

The most diagnostic pair:
- `ecological-system` →[modifies]→ `resource-deposit` (fen recharges carbonate aquifer;
  quarry dewatering desiccates fen)
- `resource-deposit` →[modifies]→ `ecological-system` (same pair, opposite direction)

A corpus that only has one direction of edges is missing the central argument of the domain.

---

## Seed instance quality

Good seed instances for this domain are *specific enough to be wrong*:

- A stratigraphic unit names its formation (Columbus Limestone, not "a limestone layer"),
  gives an age range, lithology, and depositional environment
- A geologic event names a specific glacial stage (Wisconsin glaciation, Devonian marine
  transgression), not a generic "glaciation"
- An ecological system names a specific community type (calcareous fen, oak savanna) with
  a specific substrate dependency
- A resource deposit names a specific commodity and a named site or region

Vague instances ("a rock unit," "an ecological community") are red flags.

---

## Good bootstrap looks like

7–8 well-linked corpus classes with at least one concrete instance per class. The seed
instances demonstrate the action/counteraction web with at least two bidirectional edges
between identifiable pairs. The genesis commit message names the domain, lists the classes,
and describes at least one cross-domain relationship (e.g., how glaciation enabled a
specific ecological community or how quarrying threatens a fen).

---

## Red flags

- Only geological classes, no ecological or economic ones — the web was not built
- Only unidirectional edges (everything flows one way) — action without counteraction
- Instances named "Example Limestone Formation" or "Sample Ecological System" — placeholders
- Time included as a property on other nodes rather than as a first-class class with its
  own instances — events cannot be anchored
- Genesis commit message lists filenames rather than describing what the corpus knows
- `site` class absent — no specific location anchors the graph to real claims
