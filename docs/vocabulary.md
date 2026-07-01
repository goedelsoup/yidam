# Vocabulary

These terms carry specific meaning throughout the system and should be treated as design
tokens in their own right.

| Term | Meaning |
|------|---------|
| **corpus** | The living knowledge graph — all domain nodes and their edges |
| **catalog** | Provenance layer — one node per external data source |
| **node** | A single file; one concept, relation, artifact, or open question |
| **edge** | A markdown link between two nodes — directional |
| **corpus node** | An authored or generated domain knowledge claim |
| **catalog node** | A source descriptor (dataset, paper, API) |
| **agent** | A participant (human or AI) who commits to the graph |
| **elector** | A recognized sangha participant; maintains a `ma/*` branch |
| **sangha** | The collective of all participants; the governance layer |
| **rigpa** | *Clear seeing* — a settled collective understanding; a named branch `rigpa/<evolution>` |
| **ma** | *Voice, position* — one elector's working branch `ma/<name>` |
| **samudaya** | *Arising* — pre-bootstrap seed material; consumed at genesis |
| **sadhana** | The scaffold template layer; also consumed at genesis |
| **genesis commit** | The first commit in a derived repo; names domain, seeds ontology |
| **phase** | A bounded unit of agent inquiry: Investigation, Extraction, Synthesis, or Assessment |
| **connector** | An external-facing async adapter that fetches data into the corpus |
| **calculator** | A pure, deterministic domain computation |
| **prelude** | Inherited yidam infrastructure: identity, graph model, constitution, conduct norms |
| **BFO** | Basic Formal Ontology — foundational alignment organized around the continuant/occurrent axis |
| **UFO** | Unified Foundational Ontology — foundational alignment organized around kinds, roles, and relators |
| **foundational type** | The BFO or UFO type assigned to an ontology class; encoded in `foundational_type:` in `.ont.yml` |

### Claim confidence markers

Inline tags that annotate epistemic status within a corpus node:

| Marker | Meaning |
|--------|---------|
| `[verified]` | Supported by a committed primary source |
| `[inference]` | A reasonable conclusion from verified facts; not directly witnessed |
| `[open]` | A live question; unknown, contested, or under investigation |
