# Scenarios

Each scenario seeds the domain owner agent with enough context to answer the bootstrap's
interrogation loop believably, without pre-cooking the ontology.

## Schema

```yaml
---
id: <kebab-case, stable — renaming breaks result history>
domain: <one-line domain description>
central_question: <the question this repo exists to investigate>
seed_concepts:
  - name: <concept name>
    hint: <one sentence — enough for the domain owner to elaborate on, not a definition>
good_bootstrap_looks_like: <1-2 sentences describing what a successful bootstrap produces>
---
```

Hints should be genuinely sparse. The domain owner uses them as anchors, not scripts.
If a hint answers the question a good bootstrap agent would ask, it is too detailed.

For the harness-side contract (Rust struct, validation rules, snapshot paths), see
[tests/harness/SCENARIO.md](../harness/SCENARIO.md).
