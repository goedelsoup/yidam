# Quality rubric

This is the evaluation framework for bootstrap runs. It defines the quality bar the system
must help users achieve.

### Structural checks (pass/fail)

| ID | Check |
|----|-------|
| S1 | `corpus/` exists and contains ≥2 `.md` files |
| S2 | Each corpus node has ≥1 outgoing markdown link |
| S3 | No corpus node has zero incoming AND zero outgoing links (no orphans) |
| S4 | Exactly 1 git commit exists (the genesis commit) |
| S5 | The genesis commit message is ≥3 lines |
| S6 | `agents/`, `skills/`, and `catalog/` stub directories exist |
| S7 | No corpus node exceeds 40 lines |

### Quality checks (scored `pass` / `marginal` / `fail`)

| ID | Criterion |
|----|-----------|
| Q1 | Bootstrap asked ≥2 clarifying questions before scaffolding |
| Q2 | Corpus nodes are scoped to one concept each |
| Q3 | Corpus node content is substantive and domain-specific |
| Q4 | Edges reflect real conceptual relationships (not directory citations) |
| Q5 | Genesis commit message names domain, describes ontology, notes ≥1 edge |
| Q6 | Seed nodes are at a consistent level of abstraction |
| Q7 | Ontology matches the domain's stated `good_bootstrap_looks_like` |

### Regression thresholds

A run is a regression if:
- Any structural check changes from pass → fail
- Any quality criterion drops by ≥1 band (pass → marginal, or marginal → fail)
- The corpus node count decreases
