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
this profile is that cluster: **six repositories, six unrelated domains, 73 to 1,312
commits**, converging on four numbers.

Re-fitted **2026-09-06** (#644). Each band is the observed range over the six, quoted to two
decimal places and rounded **outward**. The six measurements it was fitted from are recorded
in `kuten.yml` under `measured.members`.

| Slot | Band | Observed | What the six showed |
|---|---|---|---|
| `phases` | 12–27% | 12.50–26.23% | Commits settling a phase, among the nine repositories whose vendored prelude has the verb |
| `classes` | 0.50–1.12 | 0.5061–1.1096 | Instance nodes per commit, at matched maturity |
| `classes` | 35–62 lines | 35–62 | Median instance node length |
| `vocabulary` | 0–2% | 0–1.64% | Off-vocabulary commits: four of them, in two of the six |

### What the re-fit corrected

A band is a measurement. The precision it is quoted at is a choice, and A0 made that choice
without recording it. The first two rows above were published as *13–26%* and *0%* — the same
measurements, rounded inward. That put the two repositories whose numbers set those endpoints
outside the bands they had defined, which by this layer's own test is a wrong extraction and
not a divergent repository.

The correction is the rule rather than the numbers: round outward, record the measurements
beside the band, and have a guard read them back. The zero had a second cause worth stating.
Two of the four off-vocabulary commits carry a valid verb with a `(scope)` suffix that
[GRAPH.md](../../GRAPH.md) forbids, written at genesis under a prelude that had not yet closed
the list. A script that strips the suffix before matching reads this population as exactly
zero, and that is the one way to reach the number A0 published.

Two controls are what make those numbers a cluster rather than an artifact, and both are
mandatory in any repeat of the measurement.

**Vendored-prelude vintage.** A repository works from the prelude it vendored, not from
upstream's current one. Three candidate members had vendored a prelude with no `phase` verb
and no closed vocabulary. Their zero phase usage and their 43% and 73% "violations" are
properties of the template they hold, not of their practice.

**Repository maturity.** Nodes per commit halves over a repository's life — one corpus went
2.07 at commit 69, 0.98 at commit 250, 0.54 at its head. Comparing a 69-commit repository
against a 1,123-commit one manufactures a difference that is only age.

## The direction of the arrow

`object.direction` says which way the arrow between this corpus and the artifact outside it
runs. `inquiry` proposes **`authored`**: the corpus is written in git, [GRAPH.md](../../GRAPH.md)'s
premise holds — the files are the data, `git log` is the audit trail — and every
history-derived surface applies.

The other declared state is **`projected`**: the corpus is regenerated from the object by the
repository's own tooling, so the arrow runs object → corpus. It is endorsed rather than ruled
out. The largest repository the layer was measured over reached it deliberately, gitignoring
its corpus and mirroring 744 files back from its own data; a model with no word for that
defines its largest real instance as misuse. Under `projected`, `replay`, `--at`,
`log --epistemic` and the residence clocks report *not applicable by declaration* rather than
answering nothing, and `doctor` names the state on the kuten line.

**The slot declares no paths, and that is settled rather than pending.** This profile is one
upstream-authored file serving six repositories with six different object shapes, and the only
thing a corpus writes is its decision record — the kuten it adopted and the revision. There is
no channel by which a corpus hands paths to it, and paths are a fact about a repository rather
than about a practice. The live register is `[object] paths` in `.yidam/config.toml`, on the
same division the clocks are held to: the kuten proposes values, never holds live ones.

## What kind of question this corpus should be opening

`question_pressure.kind` is **`epistemic`** here: questions about understanding, which is what
`inquiry` is named for. `yidam kuten check` reads it against the open questions this corpus
holds and asks about the gap — *you declared inquiry, and you have opened none* — and that is
the whole of what the slot does. **It creates pressure toward a kind of question. It does not
author one**, and a test asserts that the check writes no file.

It is measured over the corpus's own open questions rather than over `open:` commits, through
the same predicate `yidam open-questions` uses. Two of the six repositories that defined this
profile have written no `open:` commit at all while holding 27 and 15 open-tagged corpus
files; a rule that read commits would report divergence against the very corpora it was
extracted from.

`coverage` is the other kind the layer names, and it is **reserved and unimplemented**. A
corpus completing a series can only be pressed to open coverage questions once a class can
declare what its instances span, and that is a class-contract change filed outside this layer
(#578). It parses, and it never diverges.

## What a contribution is read against

`rubric.criteria` names what `yidam score <range>` reads a session's work against. `inquiry`
declares three, and the declaration is **criteria only — no bands**.

| Criterion | What it reads | What the windows showed |
|---|---|---|
| `register` | Of the commits whose verb the vocabulary carries, how many are epistemic | n=63, 0.00 / 0.50 / 1.00; undefined in 9 of 72 |
| `landing` | Of the nodes the range added and left standing, how many something points at | n=41, 0.00 / 0.67 / 1.00 |
| `questions` | How many of those nodes are open questions | n=41, median 0.64, full spread |

The bands above this section were measured before they were written down. **Nothing measured
what a good reading of a criterion is**, so declaring one would be the number this profile's
own header says none of the others is. What was measured is that each of the three
discriminates across 10-commit windows in the derived corpora, which is what makes it a
criterion rather than a slogan.

Three things were measured and rejected, and each is recorded so nobody proposes it twice.
**Out-degree** — *did new nodes enter the graph reachable* — is an error-severity gate that 0
of 2,736 nodes across sixteen corpora trip, so scoring it would measure the gate. **The
presence of an `open:` commit** appears in 2 of 72 windows, and a criterion 97% of ranges fail
is not a criterion. **The naive epistemic share** is inverted: `classify_commit` is total, so a
corpus writing conventional commits with no recognized verb reads 1.00, and `register`
computes over the recognized subset for exactly that reason.

`score` reports a row per criterion with the evidence it came from, and no overall number: a
single score over a range of commits names somebody's session. It exits zero however it reads.

## What this profile does not declare

**`thresholds`** — `[lint] escalate_after` and `[propose] withdraw_uncited_after` — which no
kuten populates, whatever it measures. `escalate_after` decides when a finding fails the build,
and `withdraw_uncited_after` licenses `propose` to draft a deletion. A kuten reaches neither act:
the first is a gate change and arrives as a visible policy override, the second is authorship and
arrives through `propose`'s own licence. The slot is named so that emptiness is a state a reader
can see, rather than a family the layer forgot.

`clocks` and `policy` are populated but are **proposals with values, not permissions with
blanks**. The proposed values are the ones yidam's own configuration documentation puts in a
reader's hands; a corpus holds them by writing them into its config, or declines them by not.

They were chosen rather than measured, and the reason first given — that there was no measured
interval to extract — was wrong. Re-reading the same eighteen corpora on 2026-09-06 found
`catalog.ttl_days` declared 182 times, 165 of them inside this cluster, in the per-entry form
the first pass did not read. For the three `[due]` keys the population was never eighteen:
two corpora held a yidam new enough to name those keys, and one of the two set values. No
policy override exists anywhere, which does stand.

So the scalars stay, as a documented fallback and not as evidence, and the profile beside this
document records what would retire each one. A proposal that cannot be wrong is a preference
wearing a measurement's clothes; this one can now be wrong, and the rule for it is written
down in advance.

## How divergence is reported

`yidam kuten check` reads this declaration and the repository's own history, and reports
where the two disagree. It writes nothing, and it exits zero.

That is not leniency. Divergence from a kuten is a question for a person — *you declared
inquiry, and you have opened no questions in two hundred commits* — and a question is not a
defect. Anything that refuses arrives through the policy layer, where it is visible as an
override. And a metric a repository's vendored prelude could not have produced is reported as
vintage, never as divergence.
