# Section 5 notes: efficiency analysis and implications

Open — needs cost model development and empirical grounding.

## Key claims to support

1. Traversal cost under blind scan = O(k) in candidates evaluated, where k ≫ 1 for
   non-trivial corpora. Under anchored traversal, cost = O(depth × branching factor),
   where depth is bounded by ontology structure.
2. As corpus size N grows, blind scan cost grows; anchored traversal cost does not.
3. Focused candidate sets improve answer quality by reducing noise in the context window.
4. At aggregate scale (millions of AI interactions), the reduction in tokens per goal
   translates to meaningful energy reduction.

## Cost model to develop

```
blind_scan_cost(N, k, t_per_candidate)     = k × t_per_candidate
anchored_lookup_cost(depth, branch, t_hop) = depth × branch × t_hop
focused_scan_cost(N, C, k, t_per_candidate) = min(C, k) × t_per_candidate
```

Where:
- N = corpus size (nodes)
- k = top-k retrieval parameter (RAG default: 5–20)
- C = class-constrained candidate count
- depth = hops from anchor to relevant node (empirically ≤ 4 for structured domains)
- branch = branching factor of relevant path (≤ 5–10 for typed edges)
- t_per_candidate = tokens to evaluate one candidate (embed + read + reason over)

For the sustainability argument: token cost → FLOPs → energy. Published estimates from
model providers (cost-per-million-tokens) and energy-per-FLOP estimates can bridge these.

## What to research

- Published energy estimates for LLM inference (tokens → Wh/MWh)
  - AI energy consumption papers (Patterson et al., Lottick et al., Strubell et al.)
  - Anthropic, OpenAI, Google published efficiency reports if any
- RAG pipeline token consumption measurements at scale
  - Any published studies of production RAG cost
- Scaling law literature: does retrieval cost scale differently than generation cost?
- KGQA benchmarks with efficiency measurements (not just accuracy)

## The quality argument

Reducing candidate set size is not only about cost — it is also about quality. A focused
candidate set drawn from semantically appropriate, ontologically typed nodes contains less
irrelevant material. In a long-context window, irrelevant material is a source of
distraction (attention dilution) and hallucination risk (confounders that superficially
resemble the target). Anchored traversal reduces this risk structurally.

This is an argument that the efficiency gain is not purchased at a quality cost — it may
actually improve quality. If empirical evidence exists (KGQA with type filtering vs.
without), cite it.

## The sustainability frame

This paper was motivated in part by the observation that resource consumption driven by AI
is a growing systemic concern. The efficiency argument should not be buried in section 5 —
it should be named in the abstract and introduction. The framing: structural approaches to
reducing per-goal cost are underexplored relative to hardware efficiency gains. Ontological
anchoring is one such structural approach, and it operates at a layer that hardware
improvements do not reach.

Relevant prior framing: the Green AI movement (Schwartz et al. 2020), "efficiency" as a
first-class ML metric.

## Open questions

- Is there empirical work directly comparing token costs of SPARQL / graph traversal vs.
  embedding-based retrieval on matched goal sets?
- How does the cost model change for multi-agent systems where multiple agents traverse
  the same corpus? (Graph traversal allows shared state; scan does not.)
- What are the maintenance costs of the ontology layer? (If maintaining ontological
  alignment is expensive, the efficiency gains must be weighed against that cost.)
