# Paper outline

Each section states its key claim, the evidence or argument needed to support it, and open
research questions. Sections are written sequentially in `drafts/`; notes in `notes/` feed
each section.

---

## Section 1 — The traversal problem

**Key claim:** Current AI goal-resolution systems are structurally committed to broad-scan
traversal, making their cost a function of information-space size rather than goal complexity.
This is not a product shortcut — it is a consequence of treating knowledge as an unstructured
retrieval corpus.

**Argument structure:**

1. Define path traversal: the sequence of retrieval and reasoning operations required to move
   from a user goal to a useful response. Path length = number of hops; node evaluation cost
   = tokens consumed per candidate considered.
2. Characterize current systems: RAG pipelines retrieve top-k by embedding similarity; web
   search agents issue multiple queries and scan result pages; long-context models ingest broad
   context and rely on attention to "find" the answer inside it. All three are scan modes.
3. Show the cost structure: token consumption grows with k (RAG), number of results (web),
   or context length (long-context). Goal complexity has weak or no influence on this cost.
4. State the inefficiency: for goals that have determinate answers locatable in structured
   knowledge, scan is wasteful. The waste is not incidental — it is load-bearing in current
   architectures.

**Evidence needed:**
- Published token/cost measurements for RAG pipelines on structured-domain queries
- Long-context vs. structured-retrieval cost comparisons
- Any empirical work on "how many documents does a RAG system read before it finds the answer"

**Open questions:**
- What fraction of real user goals have determinate answers vs. requiring broad synthesis?
- Is there a principled way to classify goals by their "anchorability" prior to traversal?

---

## Section 2 — Ontology and corpus networks as maps

**Key claim:** Public ontology networks and structured corpus networks encode the concept
topology of a domain. A user goal matched to a concept node in such a network gains a
starting point, typed neighbor relationships, and a traversal direction — converting the
search problem into a graph walk with bounded depth.

**Argument structure:**

1. Describe what public ontologies provide: concept identity (stable IRI/ID), labeled
   relationships (`is-a`, `part-of`, `causes`, `measured-by`, etc.), and cross-ontology
   alignment (BFO, OWL, SKOS). These are maps, not just indexes.
2. Ontological anchoring: the act of mapping a natural-language goal expression to a concept
   node. Once anchored, the agent knows which relationship types are semantically appropriate
   to follow given the goal type (e.g., a "find causes of X" goal follows `causes`/`caused-by`
   edges; a "find instances of X" goal follows `instance-of` edges).
3. Depth bound: in well-maintained ontologies, the shortest path between any two
   domain-relevant concepts is empirically short (cite concrete examples: Wikidata, BFO
   hierarchies, MeSH). The claim is not that all paths are short — it is that the *relevant*
   path is short once the anchor is known.
4. Corpus networks extend ontologies with instance knowledge: authored nodes for specific
   concepts, provenance edges to primary sources, typed relationships derived from domain
   semantics. An ontology gives the map; a corpus network populates it with findings.

**Evidence needed:**
- Wikidata and DBpedia path-length distributions for domain-relevant concept pairs
- BFO and domain-ontology coverage surveys (how many concepts in a given domain are
  represented)
- Examples of entity linking / ontological grounding work in the literature (SPARQL, EL)
- Comparison of SPARQL-style structured retrieval vs. embedding retrieval on matched tasks

**Open questions:**
- How good are current LLMs at performing ontological anchoring without explicit ontology
  access? (relevant to how much external tooling is required)
- What domains have insufficient public ontology coverage to support this approach?

---

## Section 3 — Focused scan as the fallback regime

**Key claim:** Full ontological lookup is not always possible. For goals that cannot be
anchored, or where the corpus network has gaps, ontological context still narrows the scan
space by constraining which candidate nodes are semantically relevant — producing focused
scan that is substantially cheaper than blind scan without requiring a complete lookup.

**Argument structure:**

1. The spectrum: full lookup → focused scan → blind scan. These are not categories to choose
   between; they are modes that a well-designed system applies based on available context.
2. Focused scan mechanics: ontological class membership constrains the candidate set (only
   nodes of the relevant class are retrieved); relationship type constraints narrow further
   (only nodes reachable via the appropriate relationship type). These constraints can
   reduce a corpus from tens of thousands of nodes to dozens without retrieval.
3. Quantify the narrowing: if a corpus has N nodes and an ontological class covers C nodes,
   focused scan is C/N of blind scan's cost. For structured domains, C/N is typically small.
4. Uncertainty handling: when anchor confidence is low, the system can broaden the candidate
   set gracefully — widening from a single class to a class subtree, or from a direct edge
   to a 2-hop neighborhood. Uncertainty does not force a reversion to blind scan.

**Evidence needed:**
- Class-size distributions in representative ontologies and corpus networks
- Empirical measurements of candidate-set reduction from class filtering in SPARQL / graph
  query systems
- Any work on uncertainty-aware ontological anchoring

**Open questions:**
- How is the transition between lookup and focused scan operationalized at runtime?
- What is the quality cost of focused scan relative to full blind scan (precision/recall)?

---

## Section 4 — System architecture: yidam as case study

**Key claim:** The yidam architecture is a working instantiation of the ontology-anchored
traversal model. Its design choices — git-as-knowledge-graph, typed markdown edges, catalog
provenance, semantic index as fallback, prelude domain functions as pure lookup — each
correspond to a specific reduction in traversal cost.

**Status:** drafted in [notes/yidam-case.md](notes/yidam-case.md)

**Argument structure:**

1. The corpus as a navigable graph, not a retrieval corpus: every file is a node; every
   markdown link is a typed directional edge. Agents traverse edges rather than issuing
   retrieval queries for local knowledge.
2. Catalog nodes: source lookup by concept rather than by keyword. A corpus node carries
   edges to catalog nodes; the agent finds the primary source by following the edge, not by
   searching.
3. The semantic index as explicit fallback: the yidam design explicitly names the index as a
   mechanism to "reduce token consumption by letting agents retrieve only relevant nodes
   rather than loading the full corpus." This is an acknowledgment of the scan problem and a
   deliberate design to bound it.
4. Prelude domain functions as pure lookup: causal, graph-metrics, information-theory,
   finance, similarity, statistics, geodesics, energy, hydrology, set-theory, trade,
   group-theory, economics functions are all pure, deterministic, cross-language. Domain math
   is a lookup, not a scan.
5. Commit classification (epistemic vs. operational): the graph's provenance structure is
   preserved, making it possible to traverse the history of a concept — not just its current
   state — without scanning the full commit log.

**Evidence needed:**
- Concrete traversal examples from a bootstrapped yidam-derived repo
- Token-count comparison: agent navigating by edges vs. agent using blind semantic search
  over the same corpus

---

## Section 5 — Efficiency analysis and implications

**Key claim:** Ontological anchoring and corpus-network traversal reduce tokens-per-goal,
API calls per goal, wall-clock latency, and energy per useful outcome by factors that scale
with corpus size. The gains compound as corpora grow: blind scan cost grows linearly with
corpus size; anchored traversal cost grows with the depth of the relevant path, which is
structurally bounded.

**Argument structure:**

1. Cost model: tokens per goal = (candidates evaluated) × (tokens per candidate). Under
   blind scan, candidates evaluated ≈ k (top-k retrieval) or document count (full scan).
   Under anchored lookup, candidates evaluated ≈ depth × branching factor of relevant path.
2. Scaling behavior: as corpus size N → ∞, blind scan cost → ∞; anchored traversal cost
   → O(depth × branching) which is bounded by ontology structure, not corpus size.
3. Quality dimension: anchored traversal can improve answer quality by reducing noise. A
   focused candidate set drawn from semantically appropriate nodes contains less irrelevant
   material than a top-k embedding retrieval, reducing the chance of the model attending to
   confounders.
4. Environmental implication: fewer tokens per useful outcome = less GPU compute = less
   energy. At the scale of millions of daily AI interactions, the aggregate reduction is
   meaningful. This is not the primary claim but is a consequence of the primary claim.
5. The sustainability argument: resource consumption driven by AI is increasingly a systems
   concern. Structural approaches to reducing per-goal cost — as opposed to hardware
   efficiency alone — are underexplored. Ontological anchoring is one such structural approach.

**Evidence needed:**
- Token cost estimates for representative goals under scan vs. lookup regimes
- Energy-per-token estimates (publicly available from model provider cost data)
- Literature on scaling laws and retrieval efficiency
- Any work on "semantic efficiency" in LLM pipelines

**Open questions:**
- Is there a class of goals for which anchored traversal is reliably worse than blind scan?
- How does the approach interact with goals that span multiple domains (multi-hop across
  ontology boundaries)?
