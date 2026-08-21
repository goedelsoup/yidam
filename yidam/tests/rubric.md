# Evaluation Rubric

Criteria used to evaluate a bootstrap result. Structural checks run automatically via the
Rust harness; quality checks are assessed by the [judge agent](judge.md).

---

## Structural checks (automated)

These are pass/fail. Any failure is a blocking regression.

A **node** is an instance file — `.yidam/corpus/<class>/<instance>.yml`. A `<class>.ont.yml`
at the top of the corpus is a class definition, not a node of itself, and links are the
entries of an instance's `links:` list resolved against the file that declares them.

The **genesis sequence** is what step 8 of the bootstrap skill writes and step 9 refuses to
begin without: a `genesis:` root (`overlay:` in existing-repo mode), then a `consume:` commit
per transient layer, then `vendor:`. S4 asked for exactly one commit until protocol 0.2.0,
which no correct run has ever produced.

These rows are the source. `check.rs` reports each check under the sentence written here and
a test pins the two together, so a check whose description drifts from its row fails the
build; `docs/quality-rubric.md` is pinned the same way. The harness runs no check this table
does not state, and states no check the harness does not run.

Every node-scoped check (S2, S3, S7) fails when the corpus walk finds no instances. It is not
a violation-free corpus and must not report as one — see [check.rs](harness/yidam-harness/src/check.rs)
for what went wrong when it did.

| ID | Check |
|---|---|
| `S1` | The corpus holds ≥1 class definition and ≥2 instance nodes |
| `S2` | Every instance node declares ≥1 link |
| `S3` | No orphan instance nodes (zero in AND zero out links) |
| `S4` | The history is the genesis sequence, and holds nothing else |
| `S5` | The genesis commit message is ≥3 lines |
| `S6` | The `.yidam/` scaffold exists (`catalog`, `corpus`, `decisions`, `skills`) |
| `S7` | No instance node exceeds 40 lines |

---

## Quality checks (judge-assessed)

Each criterion is scored `pass` / `marginal` / `fail`. See [judge.md](judge.md) for scoring
guidance and the band anchors.

This table is the source, not a description of one: the harness parses these rows and holds
the judge to exactly the IDs it finds. A criterion added here is a criterion the judge must
score; a verdict that omits one, invents one, or gives a band with no evidence is rejected.

| ID | Criterion |
|---|---|
| `Q1` | Bootstrap asked ≥2 clarifying questions before scaffolding |
| `Q2` | Corpus nodes are scoped to one concept each |
| `Q3` | Corpus node content is substantive and domain-specific |
| `Q4` | Edges reflect real conceptual relationships (not directory citations) |
| `Q5` | Genesis commit message names domain, describes ontology, notes ≥1 edge |
| `Q6` | Seed nodes are at a consistent level of abstraction |
| `Q7` | Ontology matches `good_bootstrap_looks_like` from the scenario |
| `Q8` | Edges assert only relationships the domain supports |

---

## Regression thresholds

A run is a **regression** against a prior snapshot if:
- Any structural check changes from pass → fail
- Any quality criterion drops by ≥1 band (pass → marginal, or marginal → fail)
- The corpus node count decreases
