# Section 3 notes: focused scan as the fallback regime

Open — needs formal development and some empirical support.

## Key claims to support

1. The lookup/scan distinction is a spectrum, not a binary. Partial ontological context
   narrows the scan space even when full lookup is impossible.
2. Ontological class membership and relationship type constraints reduce a corpus from N
   candidates to C candidates, where C/N is small for structured domains.
3. Uncertainty in anchoring degrades gracefully: widen from class to subtree, or from
   direct edge to 2-hop neighborhood. Does not require fallback to blind scan.

## What to formally develop

The C/N ratio argument: if a corpus has N nodes and an ontological class covers C of them,
then a class-constrained retrieval query has C candidates rather than N. The token cost of
evaluating those candidates scales with C, not N. For the argument to land, we need:

- A characterization of typical C/N values for representative ontologies and domains
- A demonstration that class-constrained retrieval preserves answer quality (the relevant
  node is not systematically excluded by class filtering)

## What to research

- Class-size distributions in Wikidata, DBpedia, domain ontologies
- SPARQL filtering vs. embedding retrieval: candidate set size comparison on KGQA tasks
- Work on "type-constrained entity retrieval" — filtering candidates by ontological type
  before scoring
- Papers on uncertainty-aware entity linking and ontological grounding

## The graceful degradation argument

This is the section's most important original contribution. The paper needs to argue that
the lookup → focused scan → blind scan spectrum is controllable, not an abrupt fallback.
Mechanisms:

- Anchor confidence < threshold → widen to parent class (still focused, larger C)
- No anchor found → use ontological relationship type as a filter on embedding retrieval
  (focused scan using ontological constraint on the retrieval query)
- No ontological context at all → blind scan (the standard RAG baseline)

Each step in this sequence is cheaper than the next fallback. The system never has to
jump straight to blind scan unless all ontological context is absent.

## Open questions

- Is there a principled way to set the anchor confidence threshold for when to widen?
- What is the quality cost of focused scan relative to blind scan on standard KGQA
  benchmarks? (Are there tasks where ontological filtering systematically excludes the
  answer?)
- How does this interact with multi-hop goals that cross ontological class boundaries?
