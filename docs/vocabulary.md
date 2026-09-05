# Vocabulary

These terms carry specific meaning throughout the system and should be treated as design
tokens in their own right. One term carries one meaning; where English gives a word several
senses, each sense is named separately below and the docs use the qualified form. The
[style guide](style-guide.md) states the rule.

## The graph

| Term | Meaning |
|------|---------|
| **corpus** | The living knowledge graph — all domain nodes and their edges |
| **catalog** | Provenance layer — one node per external data source |
| **node** | A single file; one concept, relation, artifact node, or open question |
| **edge** | A markdown link between two nodes — directional |
| **corpus node** | An authored or generated domain knowledge claim |
| **catalog node** | A source descriptor (dataset, paper, API) |
| **artifact node** | A corpus node describing a thing produced or found in the domain. Distinct from an **artifact**, which is bytes in a vault |
| **claim** | A single assertion inside a node, carrying an inline confidence marker |
| **gate** | The checks a corpus must pass before a commit counts — `graph-check`, `lint`, and the commit-vocabulary rule. What makes an edge a commitment rather than a hyperlink |
| **traversal** | Walking edges from a starting node to answer a query, as opposed to retrieving by similarity |
| **drift** | Divergence between the corpus and something derived from it — a stale index, an aged-out catalog source, a regenerated block no longer matching its source |

## The ontology

| Term | Meaning |
|------|---------|
| **ontology** | The corpus's schema layer: what kinds of things exist in this domain and how they may connect. Declared in `.ont.yml` files, confirmed during the bootstrap dialogue |
| **class** | One kind of thing, defined in `<class>.ont.yml`. Declares the properties its instances carry and the edges they may bear |
| **instance** | One node of a class, stored at `<class>/<instance>.yml` |
| **BFO** | Basic Formal Ontology — foundational alignment organized around the continuant/occurrent axis |
| **UFO** | Unified Foundational Ontology — foundational alignment organized around kinds, roles, and relators |
| **foundational type** | The BFO or UFO type assigned to an ontology class; encoded in `foundational_type:` in `.ont.yml`, carrying `ontology:`, `type:`, and an optional `iri:` |
| **alignment IRI** | The `iri:` inside `foundational_type:` — the identifier that type has in BFO or gUFO. Optional; `export-rdf` emits it as `skos:exactMatch`. Replaces the retired `bfo_anchor:` |

## Governance

| Term | Meaning |
|------|---------|
| **agent** | A participant (human or AI) who commits to the graph |
| **elector** | A recognized sangha participant; maintains a `ma/*` branch |
| **sangha** | The collective of all participants; the governance layer |
| **position** | One elector's current understanding, carried on their `ma/<elector>` branch. Positions are expected to diverge |
| **rigpa** | *Clear seeing* — a settled collective understanding; a named branch `rigpa/<evolution>` |
| **ma** | *Voice, position* — one elector's working branch `ma/<name>` |
| **evolution** | One settled synthesis of positions, named and carried on a `rigpa/<evolution>` branch. The unit a position is measured against |
| **resolution** | The act of synthesizing divergent positions into an evolution, under the constitution |

## Layers and lifecycle

| Term | Meaning |
|------|---------|
| **prelude** | Inherited yidam infrastructure: identity, graph model, constitution, conduct norms |
| **samudaya** | *Arising* — pre-bootstrap seed material; consumed at genesis |
| **sadhana** | The scaffold template layer; also consumed at genesis |
| **genesis commit** | The first commit in a derived repo; names domain, seeds ontology |
| **phase** | A bounded unit of agent inquiry: Investigation, Extraction, Synthesis, or Assessment |
| **derivation** | A repository bootstrapped from this template. Its corpus is its own; the prelude is inherited |

## The domain computer

| Term | Meaning |
|------|---------|
| **connector** | An external-facing async adapter that fetches data into the corpus |
| **calculator** | A pure, deterministic domain computation |
| **pack** | A context bundle built to a token budget from a query's answer, reporting what did not fit |

## Storage and distribution

| Term | Meaning |
|------|---------|
| **artifact** | Bytes too large, too derived, or too licensed to live in git. Held in a vault, referenced by a committed pointer. Distinct from an **artifact node** |
| **vault** | The store an artifact lives in. Git holds the record of which bytes and which vault; the vault holds the bytes |
| **store** | The backing service a vault is configured against — S3, a local directory, or another supported target |
| **bundle** | A `.yiz` archive publishing a corpus for another repository to consume |
| **knowledge artifact** | A derived repository as a whole — the sense used in "living knowledge artifacts" |

## Claim confidence markers

Inline tags that annotate epistemic status within a corpus node:

| Marker | Meaning |
|--------|---------|
| `[verified]` | Supported by a committed primary source |
| `[inference]` | A reasonable conclusion from verified facts; not directly witnessed |
| `[open]` | A live question; unknown, contested, or under investigation |
