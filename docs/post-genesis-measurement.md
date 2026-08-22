# Post-genesis measurement

yidam instruments the moment a repository is born and almost nothing after it. The harness
scores the genesis commit against [rubric](../yidam/tests/rubric.md) checks S1–S7 and Q1–Q8,
and `yidam lint` reports the corpus's present state. Neither records how a corpus changes
across its life, so no question of the form *is this repository getting better or worse?* can
currently be answered.

This document records what three live derived repositories say when that question is asked by
hand, and what it would take to stop asking it by hand.

**On naming.** Two of the three repositories read here are not public. Following the
convention this repository already uses for downstream findings, they are lettered rather than
named; identity is not what carries the argument, and graph shape is.

## The repositories read

Measured 2026-08-22, read-only. Historical states were reconstructed with
`git archive <sha> | tar -x` into a scratch directory, so nothing touched the working trees,
and each repository's own `.yidam/bin/yidam` produced its figures.

| | A | B | C |
|---|---|---|---|
| Commits | 695 | 201 | 363 |
| Instance nodes | 99 | 96 | — |
| Open questions | 72 | 86 | — |
| Claims `v`/`i`/`o` | 456 / 105 / 227 | 109 / 288 / 171 | — |
| Vector index | not initialized | not initialized | — |
| `graph-check` | clean | clean | never run |
| `lint` findings | 113 (1 error, baselined) | 35 (all INFO) | never run |

C has no `.yidam/bin/yidam` installed and has therefore never run a gate against itself. That
absence is the reason it is missing from every row that requires one, and it is a finding: the
per-repository install introduced by `improvement(template): install yidam per repository, not
per machine` is not self-enforcing, and a derived repository can run for 363 commits without
anyone noticing there is nothing to run.

## Reachability is a per-class property, not a corpus rate

Sampling the `orphan-in` check — nodes nothing points at — across each repository's history.
These are the figures as first measured, with every uncited node counted:

| Instance nodes | ~31 | ~45 | ~67 | ~96 |
|---|---|---|---|---|
| **B** | 22% | 26% | 31% | 36% |

| Instance nodes | ~47 | ~88 | ~90 | ~99 |
|---|---|---|---|---|
| **A** | 0% | 0% | 0% | 0% |

B's rate rises monotonically across its whole life. A holds zero across 696 commits. The
obvious reading — that one repository is decaying and the other is not — does not survive
decomposition.

**These numbers overstate B, and the section below is why.** Counting only the nodes
something was meant to point at, `yidam replay` puts the same repository at 7% → 18%: half
the level, the same monotonic shape, and the finding intact. The corrected series is the one
to quote.

In B, **every** `person` node is orphaned, and **every** `boundary-case` node. They are not
neglected; they are richly out-linked. A person node carries edges reading `played on`,
`played in`, `carried`, `advanced` — four outbound relationships and no inbound one. In B's
ontology edges flow outward from people: a person carries a technique into a band into a
scene. `person` is a **source class**, and having no in-edges is what it is *for*.

A has no source classes. Its ontology is a citation network, in which the class analogous to
B's `person` is a target of many edges rather than a source of them. Most of the level
difference between these two repositories is ontology shape, and none of it is discipline.

### The ontology already said so

The class definitions declare this, and the check did not read them. An edge is documented
from both ends: `person.ont.yml` declares `played in` with `direction: out`, and
`band.ont.yml` declares the same relationship with `direction: in`. So a source class is one
that declares edges and none of them inbound — no new field, no new syntax, and a declaration
that cannot fall out of step with the ontology because it *is* the ontology.

Derived that way, the prediction is exact in both repositories:

| | Classes deriving as source classes | Classes 100% orphaned in practice |
|---|---|---|
| **A** | none | none |
| **B** | `boundary-case`, `person` | `boundary-case`, `person` |

Exempting them takes B from 35 findings to 18 — and the 18 are the asymmetry worth reading,
where a class's *other* instances are cited and these are not.

One trap sits inside this. A class that declares **no** `edges:` list has said nothing about
its shape, and reading that silence as a declaration would exempt every instance in a corpus
whose ontology is not filled in — switching the check off exactly where the graph is least
trustworthy. A source class must declare edges *and* declare none of them inbound.

### What survives decomposition

The rise is real. Decomposed by class, it has a single cause:

| B, at commit ordinal | 50 | 125 | 200 |
|---|---|---|---|
| `recording` | 1 | 1 | **13** |
| `person` | 3 | 5 | 12 |
| `boundary-case` | 3 | 4 | 5 |
| other | 1 | 4 | 5 |

Twelve uncited `recording` nodes arrived in one stretch of roughly seventy-five commits.
Reading that range of the log, it is a breadth sweep behaving exactly as
[GRAPH.md](../yidam/prelude/GRAPH.md) says a sweep should: `scope:` commits widening the net
and reporting what it caught, `establish:` commits landing what the obtained sources yielded.
The corpus grew evidence faster than its analytical layer consumed it. The `recording` nodes
that *are* cited are the canonical ones the definitions and boundary cases reference.

**The measurable quantity is residence time, not level.** A node uncited for five commits is a
sweep in progress and entirely healthy. A node uncited for two hundred is over-collection. A
flat percentage cannot distinguish them, because it sums three unrelated situations: source
classes (structural), evaluation inputs (deliberate), and uncited breadth (the only signal).

A corpus-wide orphan rate is therefore not a metric worth recording. Per class, against an
expectation the class declares, and aged, is.

## Only error severity gates

`lint` reports at three severities and one of them stops a build. Everything found in either
instrumented repository sits below it:

- **B** — 35 findings, all `INFO [orphan-in]`. No `lint-baseline.yml` exists, so the ratchet
  was never installed.
- **A** — 113 findings across three checks, of which exactly one is error severity. Its
  baseline file holds exactly that one entry.

Both repositories report `graph-check` clean and `lint … no regression` while a third of one
corpus is unreachable by traversal. The ratchet is not failing; it is nearly empty, because
the signal lies outside the severity it governs.

GRAPH.md argues for Warn on the commit checks and the argument is correct — *"history cannot
be rewritten to fix a verb"*, so a gate on immutable state can only ever be noise. The
generalisation is what went wrong: corpus state is not immutable. An orphaned node can be
linked or deleted today, which makes it eligible for a gate in a way a committed verb is not.

## Phase refs and the status line

`yidam status` reports **26 active phase(s)** for A. Not one of them is a phase.

`active_phase_count` reads `ma/*` and `rigpa/*` refs, local and remote-tracking, deduped. For
A that is three elector positions and twenty-three evolutions.
[PHASES.md](../yidam/prelude/PHASES.md) defines a phase in a different namespace entirely —
*"One phase, one branch — `phase/<name>`"* — and the counter never reads it. The number is
wrong three ways at once:

- **The phase namespace is not counted.** A holds twenty-seven `phase/*` refs. The status line
  sees none of them.
- **Settled evolutions are counted as active.** All twenty-three `rigpa/*` refs are merged
  ancestors of the baseline; their resolutions are over.
- **Elector positions are counted as phases.** All three `ma/*` refs are correctly unmerged. A
  standing position is *meant* to diverge from the baseline — it is not a bounded
  investigation and has no settlement to await. Counting it as active work is a category
  error, not a staleness one.

Testing the namespace PHASES.md does define: twenty-six of A's twenty-seven `phase/*` refs are
merged ancestors and exactly one is in flight. The true count is 1 against a reported 26. That
these two numbers coincide is chance, and it is the kind of coincidence that makes a wrong
reading look confirmed — the first pass of this document asserted that the reported 26 *was*
the twenty-six settled phase branches, and it was not.

The norm itself is stated and worked: PHASES.md gives `git merge --no-ff -m "phase: …"`
followed by `git branch -d phase/<name>`, and predicts the failure — *"A merged branch left
behind is not a record of anything: the commits and the merge are the record, and the ref is
indistinguishable from a phase still in progress."* It lost twenty-six times out of
twenty-seven.

The case for deleting is that the merge commit already carries the phase and its outcome, and
that sixty-nine branches make `git branch` useless for its purpose. The case for keeping is
that a ref is a cheap handle onto a phase's commits, that deletion is effectively irreversible
once the reflog expires, and that `ma/*` demonstrates the model already has long-lived refs
that are not stale by virtue of persisting.

**The disagreement is about the status line, not the refs.** A counter that read `phase/*` and
separated merged from unmerged would report `1 active` for A, and the leftover refs would cost
nothing. Correcting the report is the cheaper change, removes the only measurable harm, and
does not require settling a question this evidence cannot settle.

## The commit vocabulary holds

Recorded because it is the control case for everything below, and because a negative result
about coverage is the only durable record that coverage was checked.

- **B** — zero commit findings across 201 commits.
- **C** — drifted badly early, with several verbs outside the vocabulary and roughly
  fifty-five subjects carrying a `phase(scope)` suffix. Every one predates both the
  scope-suffix rule and C's own re-vendor of the prelude. It self-corrected on contact with
  the newer document.
- **A** — its findings are almost entirely hand-written three-parent merge subjects beginning
  `merge`, which is in no vocabulary list. The repository adopted GRAPH.md's advice to write
  its own merge subjects but not the worked example's verb; `resolve`, `adopt` and `phase` all
  exist and all fit.

## Prose holds a norm inside the act and loses a step after completion

The same prelude produced both a control and a treatment in the same repositories.

The commit vocabulary held. The verb is chosen *during* the act of committing, and something
echoes it back — `lint --commits` reports drift, even at Warn severity.

Branch deletion failed, twenty-six times out of twenty-seven, against a rule at least as
explicit and better argued. Deletion is a cleanup step *after* the satisfying part is
finished, nothing echoes it back, and `status` actively rewards leaving the ref by counting it
as active work.

That is the rule the evidence supports, and it predicts which norms need a mechanism stronger
than careful writing. Of the two stronger mechanisms:

- **A CLI subcommand** makes a sequence atomic, so the cleanup cannot be the part that gets
  skipped. Its failure mode is discovery — a subcommand nobody knows to run is *worse* than
  prose, because prose is vendored into the agent's context and a subcommand is not.
- **A gate** forces compliance, and the severity discussion above gives its boundary: gate
  reversible state, never immutable state. Refs and corpus content are reversible; history is
  not.

## Where each problem belongs

| Problem | Mechanism | Reasoning |
|---|---|---|
| Phase counting | Read `phase/*`; separate merged from unmerged | The counter reads the wrong namespace entirely, and `ma/*` is not a phase. Deletion stays optional once the number is right. |
| Orphan residence | Measure first; no gate | Nothing can be thresholded before it is decomposed. Source classes fall out of `edges[].direction`; age comes from replaying the graph, and `orphan-in` now dates every finding. |
| The series itself | `yidam replay` | Reconstructing it by hand is what found all of this, and what got the level wrong. A tool that costs 360ms should not be a script somebody rewrites. |
| Commit vocabulary | Leave it | It is working, and it is the control case that makes the argument above legible. |
| Merge subjects | Prose — one worked example | A norm exercised during the act, which is where prose works. |
| Unused index | A scope decision | Roughly 1,250 commits of revealed preference. Decide whether it earns its place rather than letting neglect decide. |
| Lived-repo evaluation | Replay before generate | Three repositories already hold 1,259 commits of real practice; replay costs seconds, generation costs $2.16 per genesis. |

## On extending the harness

The harness generates history: it runs a real bootstrap and scores the result. Extending it
past genesis means running phases, at a multiple of the existing cost — the stored baseline
run took $2.16 and 825 seconds for a single genesis.

Every figure in this document was produced by the opposite technique. **Replay** reconstructs
metrics from history that already exists, costs seconds and no model tokens, and caught a
defect a generated scenario would have had to be lucky to reproduce. The two answer different
questions — replay cannot test a prelude change nobody has lived with yet — but for everything
replay does cover it is strictly cheaper, and three derived repositories are a larger corpus
of real practice than the harness will generate for a long time.

`yidam replay` is that technique, as a command rather than a throwaway script. It folds over
the same walk that dates `orphan-in` findings, so a row here and a finding there cannot
disagree about what the graph looked like on a given day:

```
Date         Commit    Nodes   Orphans   Share
2026-08-21   a0ab68b      26         2      7%
2026-08-21   8be6603      31         1      3%
2026-08-22   f3e3217      70        10     14%
2026-08-22   73cfe0f      98        18     18%

Uncited at HEAD, by class
  recording                13 of 20
  session                  2 of 4
```

Its first act was to correct this document. The by-hand series counted every uncited node,
including the classes nothing was meant to point at; the command counts only the rest, and
the same repository reads 7% → 18% rather than 22% → 36%. Half the level, the same shape,
the finding intact — which is the argument for measuring by tool rather than by hand made
against the document that made it.

## What is not established

- **C contributes only a commit log.** With no CLI installed there are no `status`,
  `graph-check` or `lint` figures for it.
- **"Source class" was first read off instance link direction**, and described here as
  undeclared. That was wrong in an instructive way: the class definitions declare it, in
  `edges[].direction`, and had all along. The remedy was not a new field but a check that
  reads the ontology already written.
- **A's zero was attributed to graph shape.** Shape clearly explains most of it, but A also
  runs phases and a three-elector sangha where B runs neither, and the resolution protocol may
  force linking in ways this reading did not isolate.
- **The harness baseline was read as stored**, not re-run.
- **Skill convergence was weaker than predicted.** The expectation was that independent
  repositories would invent the same generic skills; they mostly did not, and their skills are
  genuinely domain-specific. The one duplicate in spirit is provenance auditing, which one
  repository built and the one that needs it most did not.
