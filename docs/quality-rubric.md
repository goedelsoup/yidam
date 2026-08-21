# Quality rubric

This is the evaluation framework for bootstrap runs. It defines the quality bar the system
must help users achieve.

This page is a copy. [`yidam/tests/rubric.md`](https://github.com/goedelsoup/yidam/blob/main/yidam/tests/rubric.md)
is the rubric the harness implements, and it is the source when they disagree — but they
cannot disagree for long: `the_docs_copy_of_the_rubric_states_the_same_criteria` fails the
build when these tables drift from it. It caught Q7 already, worded one way here and another
there since both were written.

### Structural checks (pass/fail)

| ID | Check |
|---|---|
| `S1` | The corpus holds ≥1 class definition and ≥2 instance nodes |
| `S2` | Every instance node declares ≥1 link |
| `S3` | No orphan instance nodes (zero in AND zero out links) |
| `S4` | The history is the genesis sequence, and holds nothing else |
| `S5` | The genesis commit message is ≥3 lines |
| `S6` | The `.yidam/` scaffold exists (`catalog`, `corpus`, `decisions`, `skills`) |
| `S7` | No instance node exceeds 40 lines |

### Quality checks (scored `pass` / `marginal` / `fail`)

| ID | Criterion |
|----|-----------|
| Q1 | Bootstrap asked ≥2 clarifying questions before scaffolding |
| Q2 | Corpus nodes are scoped to one concept each |
| Q3 | Corpus node content is substantive and domain-specific |
| Q4 | Edges reflect real conceptual relationships (not directory citations) |
| Q5 | Genesis commit message names domain, describes ontology, notes ≥1 edge |
| Q6 | Seed nodes are at a consistent level of abstraction |
| Q7 | Ontology matches `good_bootstrap_looks_like` from the scenario |

### Regression thresholds

A run is a regression if:
- Any structural check changes from pass → fail
- Any quality criterion drops by ≥1 band (pass → marginal, or marginal → fail)
- The corpus node count decreases
