# Ontology-Anchored Path Resolution: Reducing Traversal Cost in AI Goal Systems

**Status:** early draft — outline settled, case study grounded, literature threads open

---

## Thesis

AI systems currently resolve user goals through broad path traversal: vector similarity
search, web retrieval, and long-context scan over candidate corpora. The cost of this
traversal — measured in tokens, API calls, latency, and energy — scales with the size of
the information space, not the complexity of the goal. When a goal can be mapped to a concept
already represented in a public ontology or structured corpus network, the traversal collapses
to a directed graph walk whose depth is bounded by the ontology's structure, not by the
corpus size. **Ontological anchoring of user intent converts O(n) scan into O(depth) lookup**,
and where full lookup is impossible, ontological context produces focus-bounded scan that is
one to two orders of magnitude cheaper than blind scan. This paper formalizes the claim,
analyzes the efficiency gains, and demonstrates the principle through the yidam architecture —
a git-native knowledge graph system in which agents navigate structured corpus networks by
explicit typed edges rather than by retrieval.

---

## Paper sections

| # | Title | Status |
|---|-------|--------|
| 1 | [The traversal problem](notes/traversal-cost.md) | open |
| 2 | [Ontology and corpus networks as maps](notes/ontology-maps.md) | open |
| 3 | [Focused scan as the fallback regime](notes/focused-scan.md) | open |
| 4 | [System architecture: yidam as case study](notes/yidam-case.md) | drafted |
| 5 | [Efficiency analysis and implications](notes/efficiency.md) | open |

Full section-by-section structure with key claims: [outline.md](outline.md)

---

## What this paper is not

- Not a systems paper proposing a new retrieval algorithm
- Not a benchmark paper — though it now carries one. `yidam bench` measures the §5 claim
  against a committed goal set, and `yidam bench --scaling` measures it over generated
  corpora. The generated corpora are circular by construction: the slope follows from the
  degree distribution chosen for them. Their parameters are derived from a real corpus,
  committed as configuration, and reported in every run, which makes the result arguable
  rather than neutral — see §5 of [outline.md](outline.md).
- Not an argument against embedding-based retrieval — retrieval is the right fallback;
  the paper argues it should be a fallback, not the default

## Key terms

**Path traversal** — the process by which an agent moves from a user's stated goal to a set
of relevant knowledge artifacts. Encompasses retrieval, search, graph walk, and long-context
reasoning over candidate material.

**Ontological anchor** — a mapping from a user's goal (expressed in natural language) to a
node in a formal ontology or structured corpus. Once anchored, traversal proceeds along typed
edges rather than by similarity.

**Focused scan** — a retrieval operation whose candidate set is constrained by ontological
context to a subset of the full corpus; contrasted with blind scan (no ontological context)
and full lookup (traversal replaces retrieval entirely).

**Corpus network** — a connected body of knowledge nodes with typed, directional edges,
maintained with provenance. Distinguished from a document corpus by the presence of explicit
structure.
