# Test harness and multi-agent architecture

Three agents participate in every bootstrap test run:

| Agent | Role | Model |
|-------|------|-------|
| **Bootstrap** | The thing under test — runs the bootstrap skill | Varies (test matrix dimension) |
| **Domain owner** | Simulates a human answering ontology questions | Fixed (Haiku — cheap, credible) |
| **Judge** | Reads repo state and scores against rubric | Fixed (Opus — stable scorer) |

The domain owner is intentionally constrained: seed concept hints are anchors, not definitions.

## Scenario schema

Scenarios drive test runs:

```yaml
id: <kebab-case>
domain: <one-line domain description>
central_question: <the question this repo exists to investigate>
seed_concepts:
  - name: <string>
    hint: <one-sentence anchor>
good_bootstrap_looks_like: <1–2 sentences describing a successful result>
```

## Snapshot path

```
tests/results/<id>/<model>/<YYYY-MM-DD>/
  structural.json
  quality.json
  snapshot.json
```
