# Section 2 notes: ontology and corpus networks as maps

Open — needs literature research.

## Key claims to support

1. Public ontologies encode concept topology: typed identity, typed relationships, cross-
   ontology alignment. They are maps, not indexes.
2. Ontological anchoring (NL goal → concept node) gives the agent a starting point, typed
   neighbors, and a traversal direction.
3. Relevant path depth in well-maintained ontologies is empirically short (≤ 3–4 hops for
   domain-related concept pairs).

## What to research

- Wikidata and DBpedia shortest-path statistics for domain-relevant concept pairs
- BFO adoption and coverage surveys: how many concepts in representative domains (causal
  inference, hydrology, economics) are represented in BFO-aligned ontologies
- Entity linking and ontological grounding literature: how well do NL goal expressions
  map to ontology nodes (precision, recall, confidence)
- SPARQL-based structured retrieval vs. embedding retrieval on KGQA benchmarks
- OWL / SKOS / RDF ecosystem overview — what public structured knowledge is actually
  available and how complete is it

## Candidate sources

- Wikidata Statistics (wikistats.wmcloud.org) — concept counts, edge counts by type
- "Knowledge Graph Question Answering" (KGQA) survey papers
- SPARQL over Wikidata: published query complexity and result quality studies
- BFO documentation and adoption papers (Arp, Smith et al.)
- Entity linking papers: BLINK, GENRE, ELMo-EL, ReFinED
- Papers on ontology-augmented RAG (a growing area — search "ontology RAG" "KG-RAG")

## Key distinction to establish

Ontology ≠ knowledge base ≠ embedding index. An ontology provides relational structure
(what relates to what, and how). A knowledge base populates that structure with instances.
An embedding index enables similarity search over a flat representation. The paper's
contribution is in the first and second categories — the relational structure is what makes
directed traversal possible.

## Open questions

- How well do current LLMs perform ontological anchoring zero-shot (without explicit
  ontology access)? If LLMs already "know" the structure of Wikidata implicitly, the
  external ontology requirement is lower.
- What is the coverage gap: what fraction of real user goals in a given domain have
  anchorable concepts vs. requiring open-ended synthesis?
