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
verbatim. The argument for it is upstream, in
[RFC-0028](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0028-kuten-layer.md),
"Article V and the kuten"; the rule lands here because it binds a repository that will never
read an RFC.

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

The **Read by** column names which surface reports a populated slot, in two tokens: `check` is
`yidam kuten check`, and `block` is the `AGENTS.md` declaration `yidam kuten` regenerates.
`none` means no tool reads it — the slot is a proposal a person weighs, and saying so is the
point. The column is guarded in both directions: a slot that claims a reader must actually
change that surface, and a slot that claims `none` must actually change neither. That is the
mechanical form of the failure this layer keeps finding in itself — a surface with no consumer.

| Slot | What it declares | Read by |
|---|---|---|
| `phases` | The valid phase types, and the share of commits that settle one | `check`, `block` |
| `vocabulary` | The subset of the closed verb list this practice uses, and the off-vocabulary share it expects | `check`, `block` |
| `classes` | The shape of corpus this practice accretes — nodes per commit, and node length | `check`, `block` |
| `dialogue` | What the bootstrap asks | `none` |
| `skills` | What the practice routes through | `none` |
| `clocks` | Proposed `[due]` intervals — a proposal the corpus's own config holds or declines | `none` |
| `thresholds` | The `[lint]`/`[propose]` values — `escalate_after`, `withdraw_uncited_after`. Named, and proposed by no kuten: one decides when a finding fails the build, the other licenses a drafted deletion, and a kuten reaches neither act | `none` |
| `policy` | Proposed severity overrides, which enter through the policy layer and are visible as overrides | `none` |
| `object` | The artifact outside the corpus, and the direction of the arrow between them | `block` |
| `rubric` | The criteria a contribution is scored by | `block` |
| `question_pressure` | What kind of question this corpus should be opening | `check`, `block` |

One of these — `thresholds` — is named here and populated by no profile, and it is named
rather than counted so the list cannot lose it by being reordered. A slot with no values says
which state a repository is in; a slot invented ahead of its evidence says nothing and is
believed anyway. `thresholds` is the one that stays empty on principle rather than on
evidence: a value under it would be a gate decided outside `policy:` and a deletion drafted
with no licence.

`rubric` is the one slot that names **criteria and no bands**, and the asymmetry is the point.
Every band in a profile was measured over eighteen corpora before it was written down. What a
*good* reading of a criterion is was never measured, so a band here would be a number believed
because it is written down — the failure this layer exists to name. The slot says which
criteria a contribution is read against; `yidam score <range>` says what each one computes and
reports a row each, with no overall number and no verdict.

## The kinds of question pressure

`question_pressure` names a kind rather than a band, and the kinds are these two and no others.
A profile naming anything else does not parse.

| Kind | What it means |
|---|---|
| `epistemic` | Questions about understanding — what the corpus does not yet know. Measured against the open questions the corpus holds, through the same predicate `yidam open-questions` uses, and never against `open:` commits: two of the six repositories that defined `inquiry` have written none while holding 27 and 15 open-tagged nodes |
| `coverage` | Questions about what a series does not yet span. **Reserved and unimplemented**: it needs a class to be able to declare what its instances cover, which is a class-contract change filed outside this layer. It parses, and it never diverges |

Whichever kind is declared, the slot **creates pressure toward a kind of question and authors
none**. That is not restraint, it is the licence: opening a question asserts nothing the work
did not already assert, which is exactly why `propose` may draft `open:` and may not draft
`establish:`. A slot that wrote would be reaching past the door it came through.

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

### Adopting one after genesis

A repository older than this layer holds no record, because the step that writes one runs at
genesis and runs once. Re-vendor the prelude, which is what brings this directory, and then
declare the profile:

```sh
YIDAM_REF=<template tag> mise run yidam-vendor-update
yidam kuten adopt inquiry
```

`adopt` reads the revision out of the vendored profile rather than asking for it. That is the
same rule the bootstrap states — *copied, not typed from memory* — and it is mechanical here
because the vintage rule above is only as good as the number recorded beside the name.

It also gives `AGENTS.md` the kuten section when that file has none, since the scaffold that
carries it into a new repository is consumed at genesis. And it refuses when a record already
exists: overwriting one in place would erase the discontinuity that `replay` marks and
`score` refuses to read across.
