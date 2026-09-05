# inquiry

> A kuten declares what this corpus's practice is aimed at. It narrows and parameterizes the
> loop; it may not widen the model: it may not add a commit verb, add or alter a claim
> standing, contradict Articles I–VI, change the graph encoding, or loosen a gate except as a
> visible policy override. It asserts nothing the corpus holds — no node, no edge, no claim,
> no standing — and it binds nobody: divergence from it is a question for a person, not a
> defect. It speaks in this corpus's name from the decision record that adopted it, and it
> changes only by a superseding decision.

**Revision 1.** The declaration a tool reads is [kuten.yml](kuten.yml); this document is the
one a person reads. See [the layer](../README.md) for what a kuten is and what it may not do.

## What this practice is

A corpus that grows by opening questions and settling them. Understanding is committed as
nodes, nodes are linked as they land, and a bounded unit of work is a phase that settles onto
the baseline with a merge. That is what [IDENTITY.md](../../IDENTITY.md) has always said the
loop was for. Until now nothing wrote it down where a repository could be measured against it.

## Extracted, not invented

Every band below was measured before it was declared, over eighteen derived corpora holding
6,900 commits and 3,300 instance nodes, read-only. One cluster survived two controls, and
this profile is that cluster: **six repositories, six unrelated domains, 73 to 1,123
commits**, converging on four numbers.

| Slot | Band | What the six showed |
|---|---|---|
| `phases` | 13–26% | Commits settling a phase, among the nine repositories whose vendored prelude has the verb |
| `classes` | 0.50–1.11 | Instance nodes per commit, at matched maturity |
| `classes` | 35–62 lines | Median instance node length |
| `vocabulary` | 0% | Off-vocabulary commits — exactly zero in all six |

Two controls are what make those numbers a cluster rather than an artifact, and both are
mandatory in any repeat of the measurement.

**Vendored-prelude vintage.** A repository works from the prelude it vendored, not from
upstream's current one. Three candidate members had vendored a prelude with no `phase` verb
and no closed vocabulary. Their zero phase usage and their 43% and 73% "violations" are
properties of the template they hold, not of their practice.

**Repository maturity.** Nodes per commit halves over a repository's life — one corpus went
2.07 at commit 69, 0.98 at commit 250, 0.54 at its head. Comparing a 69-commit repository
against a 1,123-commit one manufactures a difference that is only age.

## What this profile does not declare

Three slots are named by the layer and left empty here, because the evidence for them is not
in yet and a value invented now would be believed later. A fourth is empty for a reason
evidence will never change, and it is listed after them.

- **`object`** — the artifact outside the corpus, and whether the arrow runs corpus → object
  or object → corpus. Object coupling is an axis crossing both shapes rather than a property
  of this one, and it is specified separately.
- **`rubric`** — the criteria a contribution is scored by. A rubric built alone would be one
  corpus's answer imposed on every other, which is the argument `escalate_after` already
  makes about compiling a threshold into a binary.
- **`question_pressure`** — what kind of question this corpus should be opening.

And **`thresholds`** — `[lint] escalate_after` and `[propose] withdraw_uncited_after` — which no
kuten populates, whatever it measures. `escalate_after` decides when a finding fails the build,
and `withdraw_uncited_after` licenses `propose` to draft a deletion. A kuten reaches neither act:
the first is a gate change and arrives as a visible policy override, the second is authorship and
arrives through `propose`'s own licence. The slot is named so that emptiness is a state a reader
can see, rather than a family the layer forgot.

`clocks` and `policy` are populated but are **proposals with values, not permissions with
blanks**. `.yidam/config.toml` was empty in seventeen of the eighteen corpora measured and
not one carried a policy override, so there is no measured interval to extract. The proposed
values are the ones yidam's own configuration documentation puts in a reader's hands; a
corpus holds them by writing them into its config, or declines them by not.

## How divergence is reported

`yidam kuten check` reads this declaration and the repository's own history, and reports
where the two disagree. It writes nothing, and it exits zero.

That is not leniency. Divergence from a kuten is a question for a person — *you declared
inquiry, and you have opened no questions in two hundred commits* — and a question is not a
defect. Anything that refuses arrives through the policy layer, where it is visible as an
override. And a metric a repository's vendored prelude could not have produced is reported as
vintage, never as divergence.
