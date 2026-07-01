# Section 1 notes: the traversal problem

Open — needs literature research.

## Key claims to support

1. Current AI goal-resolution systems use scan as their primary navigation mode (RAG, web
   search agents, long-context models).
2. The cost of scan scales with information-space size, not goal complexity.
3. For goals with determinate answers in structured knowledge, this is wasteful by
   construction — the waste is load-bearing, not incidental.

## What to research

- Published token/cost measurements for RAG pipelines on structured-domain queries
- Papers comparing long-context vs. structured-retrieval cost on matched tasks
- Empirical work on how many documents RAG systems evaluate before converging on an answer
  ("retrieval depth" studies if such exist)
- Cost breakdowns for frontier model API usage by goal type

## Candidate sources

- RAGAS, ARES, RECALL benchmarks — may have per-query retrieval statistics
- LlamaIndex, LangChain documentation and benchmark reports
- Papers on "retrieval-augmented generation" efficiency (search: "RAG efficiency" "RAG cost"
  "token consumption retrieval")
- GPT-4 / Claude cost calculator papers / analyses from practitioners

## Framing note

The argument should not attack RAG as a technology — it is well-suited for open-domain
synthesis tasks. The argument is narrower: for goals that are anchorable to structured
knowledge, RAG's scan mode is a mismatch, and the mismatch has measurable cost. The paper
should be explicit that this is a claim about a subset of goals, not all goals.

## Open questions

- Is there published work specifically comparing structured graph lookup vs. embedding
  retrieval on knowledge graph question answering (KGQA)? This could be the closest
  existing empirical comparison.
- "Retrieval cost" in the literature often means time or FLOPS, not tokens. How do we
  bridge to token cost as the unit of comparison?
