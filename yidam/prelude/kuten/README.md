# Kuten — the form a practice takes

A yidam repository declares what it is **about** and never what its work is **for**. The
domain layer is genuinely parameterized — the ontology dialogue, `.ont.yml`, `edge_policy`,
the selectable domain libraries. The telos layer had exactly one value, stated once in
[IDENTITY.md](../IDENTITY.md) as *sustained inquiry*, and then assumed by the four phase
types, the four clocks, the genesis rubric and the bootstrap's central question.

A **kuten** is that assumption written down where a corpus can hold it, name its revision,
and be measured against it.

*kuten* (སྐུ་རྟེན, *sku rten*) — the support in which a form is present; also the medium
through which an oracle speaks. Read it as **"the form a practice takes"**, never as
"support": `prelude` and `vault` already occupy the substrate reading.

## The binding rule

This is the rule itself, and every profile document under this directory opens with it
verbatim. The argument for it is upstream, in RFC-0028, "Article V and the kuten"; the rule
lands here because it binds a repository that will never read an RFC.

> A kuten declares what this corpus's practice is aimed at. It narrows and parameterizes the
> loop; it may not widen the model: it may not add a commit verb, add or alter a claim
> standing, contradict Articles I–VI, change the graph encoding, or loosen a gate except as a
> visible policy override. It asserts nothing the corpus holds — no node, no edge, no claim,
> no standing — and it binds nobody: divergence from it is a question for a person, not a
> defect. It speaks in this corpus's name from the decision record that adopted it, and it
> changes only by a superseding decision.

## The five prohibitions

Each one is mechanical, and each is guarded upstream against every profile in this
directory. A profile that trips a guard does not ship.

| Prohibited | Because | Instead |
|---|---|---|
| Add a commit verb | The closed vocabulary in [GRAPH.md](../GRAPH.md) is what makes `log --epistemic` decidable, and `classify_commit` is a parity function pinned by fixtures in three SDKs | Declare a **subset** and gloss it |
| Add or alter a claim standing | Article V reads the standings as a total order when it licenses lowering a claim at resolution | Nothing. This is constitutional |
| Contradict Articles I–VI | Article I — the prelude is not subject to resolution, and a kuten is vendored prelude | A domain extension appended at genesis |
| Change the graph encoding | Files are nodes, links are edges, commits are events. This is the premise, not a policy | Nothing |
| Loosen a gate quietly | A local rule may be more permissive and may not be silent | Declare it under `policy:`, where `policy check`, `lint` and `doctor` all surface it |

## What a profile is made of

A profile is a directory here holding two files. `kuten.yml` is the declaration a tool
reads; `KUTEN.md` is the document a person reads, and it opens with the binding rule.

| Slot | What it declares |
|---|---|
| `phases` | The valid phase types, and the share of commits that settle one |
| `vocabulary` | The subset of the closed verb list this practice uses, and the off-vocabulary share it expects |
| `classes` | The shape of corpus this practice accretes — nodes per commit, and node length |
| `dialogue` | What the bootstrap asks |
| `skills` | What the practice routes through |
| `clocks` | Proposed `[due]` intervals — a proposal the corpus's own config holds or declines |
| `thresholds` | The `[lint]`/`[propose]` values — `escalate_after`, `withdraw_uncited_after`. Named, and proposed by no kuten: one decides when a finding fails the build, the other licenses a drafted deletion, and a kuten reaches neither act |
| `policy` | Proposed severity overrides, which enter through the policy layer and are visible as overrides |
| `object` | The artifact outside the corpus, and the direction of the arrow between them |
| `rubric` | The criteria a contribution is scored by |
| `question_pressure` | What kind of question this corpus should be opening |

Four of these — `thresholds`, `object`, `rubric` and `question_pressure` — are named here and
populated by no profile yet, and they are named rather than counted so the list cannot lose one
by being reordered. A slot with no values says which state a repository is in; a slot invented
ahead of its evidence says nothing and is believed anyway. `thresholds` is the one that stays
empty on principle rather than on evidence: the other three await a measurement, and a value
under this one would be a gate decided outside `policy:` and a deletion drafted with no licence.

## Holding one

A kuten is vendored at genesis like the rest of the prelude, and the selection is recorded
in `.yidam/decisions/kuten.yml` together with the revision that was vendored:

```yaml
kuten: inquiry
revision: 1
```

**Every consumer reads the vendored kuten, never upstream's current one.** That is the whole
of the vintage rule: a repository whose vendored `GRAPH.md` has no `phase` verb has not
stopped running phases — it never could, and reporting that as divergence measures the
template rather than the practice.

**A repository holding no kuten is a supported state.** `yidam kuten check` says so and exits
zero. A kuten changes after genesis by a `decide:` commit carrying a superseding decision
record, and a comparison spanning two revisions is annotated rather than quietly made.
