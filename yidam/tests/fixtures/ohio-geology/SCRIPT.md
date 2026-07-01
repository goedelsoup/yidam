# Ohio Geology — Bootstrap Script

Derived from yidam-proto-002b (score: 7/10). Domain: the stratigraphic, glacial, and
ecological geology of Ohio — how rock units, landforms, ecological systems, and economic
deposits form a connected web of action and counteraction.

---

## Initial User Message

> Start here. The theme is geology of Ohio — I want to know all about how locations and
> rocks and sediment layers and glacial cuts intersect and relate with the broader
> eco/econo-geology. The action/counteraction relationship between these things is what's
> interesting.

---

## Prelude Internalized Checkpoint

> **Prelude internalized.** Graph model: files are nodes, markdown links are directed edges,
> every commit is a knowledge event — epistemic or operational. Key constraints I'll honor:
> no orphan nodes; commit messages describe what was understood, not what changed; new
> classes must be confirmed before any files are written. Directory layout: ontology class
> definitions in `.yidam/corpus/<class>.ont.yml`; instance nodes in
> `.yidam/corpus/<class>/<instance>.yml`.

---

## Ontology Discovery Dialogue

**Bootstrap:** Before I scaffold anything, I want to understand the domain. A few
orienting questions:

1. What is the central question this repository exists to investigate? Is it the
   stratigraphic record as such, or the *causal web* — how geological history produces
   present-day ecological and economic conditions?
2. When you say "eco/econo-geology," are those two separate lenses on the same geology, or
   does the relationship *between* them matter — e.g., how an ecological system depends on
   or is disrupted by an economic one?
3. What is the right scale of location? Are you interested in the whole state, specific
   physiographic regions, or site-level features?
4. What feels irreducible to you — the things you'd have to understand before anything else
   makes sense?

**User:** 1. All / 2. It's the web that's interesting — action/counteraction / 3. All
three / 4. These look GREAT.

**Bootstrap:** Good. Let me push on the "action/counteraction" framing. You seem to be
pointing at something like: *a glacial advance deposits till that suppresses bedrock
quarrying; a quarry dewatering dewaters a fen that depends on carbonate groundwater.* Is
that the kind of relationship you want to capture?

Also — should the graph include *time* as a first-class node type? Geologic events are
inherently temporal, and the Pleistocene glaciations span a different order of magnitude
than an ORC permit cycle.

**User:** Yes, exactly that kind of relationship. Time should be there — events need
temporal anchoring.

**Bootstrap:** Two more candidates surfaced by that framing:

- **Administrative unit** — county, ODNR district, watershed boundary. Things that
  constrain what can happen to a site regardless of what the geology permits.
- **Geologic event** — glacial advance, marine transgression, local extinction. The
  *happenings* that link one state to another.

Are those first-class nodes, or do they decompose into something more fundamental?

**User:** Both are first-class.

---

## Confirmed Sketch

**Nodes**

| Node | What it is |
|------|------------|
| `stratigraphic-unit` | A named rock body — formation, member, or bed — defined by lithology, age, and extent |
| `physiographic-region` | A landscape province defined by shared geologic structure and landform character |
| `geologic-event` | A discrete happening that changed the stratigraphic or ecological record |
| `ecological-system` | A community of organisms defined by a shared substrate, hydrology, and disturbance regime |
| `resource-deposit` | An economically significant accumulation of mineral, aggregate, or energy resource |
| `site` | A named location — outcrop, quarry, nature preserve, monitoring well — where multiple node types intersect |
| `administrative-unit` | A jurisdictional boundary that shapes what may happen to a site or deposit |
| `time-interval` | A named span of geologic or human time used to anchor events and formations |

**Edges**

```
stratigraphic-unit →[overlies]→ stratigraphic-unit
stratigraphic-unit →[hosts]→ resource-deposit
stratigraphic-unit →[outcrops-in]→ physiographic-region
geologic-event →[deposited]→ stratigraphic-unit
geologic-event →[enabled]→ ecological-system
geologic-event →[occurred-during]→ time-interval
physiographic-region →[contains]→ site
ecological-system →[modifies]→ resource-deposit
resource-deposit →[modifies]→ ecological-system
administrative-unit →[governs]→ site
```
