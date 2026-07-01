# Section 4 notes: yidam as case study

Material drawn directly from the repository. This is the most grounded section — no
external literature required. Every claim here is traceable to a design document or source
file in this repo.

---

## Design choice → traversal reduction mapping

### 1. Corpus as navigable graph

Source: `yidam/prelude/GRAPH.md`, `docs/what-yidam-is.md`

> Every **file** is a knowledge node.
> Every **markdown link** (`[label](path)`) is a directional edge.

In a conventional RAG corpus, the agent issues a query and receives top-k documents by
embedding similarity. The agent does not know what a retrieved document is connected to; it
reads what is returned and infers structure from content.

In a yidam corpus, an agent reading a concept node sees its outgoing edges inline. To find
related knowledge, the agent follows the link — it does not issue a new retrieval query. The
traversal is structural, not statistical. The path from a starting concept to a related
source is: load node → read outgoing edges → load neighbor. No embedding comparison; no
candidate ranking.

This is full lookup mode when the relevant neighbor is one or two hops away. It degrades to
focused scan only when the agent needs to explore beyond the locally linked neighborhood.

### 2. Catalog nodes: source lookup by concept

Source: `docs/information-architecture.md`

> An edge from a corpus node to a catalog node reads as "this concept draws on this source."
> Catalog nodes do not contain derived knowledge — only enough to locate and characterize
> the source.

In a scan-based system, finding the primary source for a concept requires either knowing
the source in advance (lookup) or searching for it (scan). In the yidam corpus, the edge
from concept node to catalog node *is* the lookup. The agent does not search for sources —
it follows the provenance edge.

This design decision also prevents provenance drift: in a flat retrieval corpus, the
connection between a claim and its source is implicit (embedded in the document text or
inferred). In yidam, it is structural. The cost of provenance retrieval is one edge
traversal, not a separate retrieval operation.

### 3. The semantic index as acknowledged fallback

Source: `docs/domain-computer.md`

> A vector index over corpus embeddings enables semantic retrieval. The index is not the
> corpus — it is a derived representation. Maintaining a fresh index reduces token consumption
> by letting agents retrieve only relevant nodes rather than loading the full corpus.

This is the paper's traversal-cost argument stated directly by the system's design
documentation. The index exists because loading the full corpus is expensive. The index
reduces that expense by enabling focused retrieval rather than full-corpus scan. It is
explicitly described as a fallback derived representation, not the primary navigation mode.

The semantic index is the "focused scan" regime in this system. The primary navigation mode
is edge traversal (full lookup); the index is invoked when a goal has no direct edge path
to relevant nodes.

### 4. Prelude domain functions as pure lookup

Source: `yidam/prelude/domains/README.md`, domain directories

The prelude ships pure, deterministic, cross-language functions for:

| Domain | Representative functions |
|--------|--------------------------|
| causal | `ate`, `confounding_score` |
| graph-metrics | `degree_centrality`, `density` |
| graph-topology | `clustering_coefficient`, `connected_components` |
| information-theory | `entropy`, `kl_divergence` |
| similarity | `cosine`, `jaccard`, `edit_distance` |
| statistics | `mean`, `variance`, `pearson_correlation`, `z_score` |
| finance | `present_value`, `future_value`, `sharpe_ratio`, `simple_interest` |
| economics | `gdp_expenditure`, `opportunity_cost`, `price_elasticity` |
| energy | `kinetic_energy`, `potential_energy`, `power`, `efficiency` |
| geodesics | `haversine_km`, `bearing_deg`, `central_angle_deg` |
| hydrology | `manning_velocity`, `rational_product`, `return_period` |
| set-theory | `union`, `intersection`, `difference`, `is_subset` |
| group-theory | `modular_add`, `modular_mul`, `additive_order` |
| trade | `tariff_revenue`, `terms_of_trade`, `trade_balance`, `revealed_comparative_advantage` |

For an agent working in a domain covered by this library, domain math is a function call —
not a retrieval operation. The agent does not need to search for "how to compute entropy" or
"what is the formula for haversine distance." It invokes the function. This is the
purest case of lookup replacing scan.

The parity discipline (Rust as reference, TypeScript and Python verified against it by
fixture) ensures the lookup is correct across the languages an agent might use.

### 5. Commit classification preserves traversal structure over time

Source: `yidam/prelude/GRAPH.md`, `docs/what-yidam-is.md`

> **Epistemic commits** add or revise understanding.
> **Operational commits** advance the corpus through pipeline work.

A corpus that grows without classification becomes harder to traverse over time because
the history of a concept — how it arose, what it displaced, what prompted revision — is
buried in undifferentiated commit noise. The distinction between epistemic and operational
commits is a structural choice that keeps the knowledge history navigable.

For the traversal paper: this is evidence that the design explicitly models and maintains
the navigability of the knowledge graph over time, not just at a point in time. A graph
that is navigable today but becomes opaque as it grows is not a solution to the traversal
problem — it is a deferral.

### 6. Sangha and rigpa: traversal across collective positions

Source: `yidam/prelude/GRAPH.md`, `yidam/prelude/SCRIPTURE.md`

> `refs/heads/ma/<elector>` — individual positions
> `refs/heads/rigpa/<evolution>` — settled collective understanding

When multiple agents contribute to a corpus, the risk is divergence: the same concept is
represented differently by different contributors, and an agent traversing the graph
encounters contradictory nodes without a clear resolution path. The sangha resolution
protocol produces `rigpa` branches — settled, named states of collective understanding.

For the traversal paper: this is a governance mechanism that preserves graph coherence
under multi-agent contribution. Coherence is a precondition for the traversal model — a
graph where the same concept is represented at multiple incompatible nodes requires scan
to reconcile, defeating the lookup advantage. The rigpa mechanism prevents this by making
collective synthesis a first-class operation.

---

## What this section needs from outside the repo

- A concrete worked example: take a user goal (e.g., "what is the causal effect of
  intervention X in this domain?"), trace the path an agent would follow using yidam's
  edge structure, and count the traversal steps. Contrast with a RAG baseline over the
  same corpus (count embedding queries + documents returned + tokens consumed).
- If a bootstrapped derived repo exists, use it. If not, a synthetic example using the
  fixture corpus is sufficient for illustrative purposes.
