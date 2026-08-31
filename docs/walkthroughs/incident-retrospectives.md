# Incident retrospectives walkthrough

*An on-call team can hold a review document for every incident it has ever had and still not be
able to say which contributing factors keep coming back, or which fixes nobody ever checked.*

The corpus is [`examples/incidents/`](../../examples/incidents/README.md) — thirteen instances
across four classes, and the only example here that ships a **history**. Every transcript is
output from a run against it.

## The work, before a corpus

The retrospective process produces one document per incident, with a fixed section set:
summary, impact, timeline, contributing factors, action items. Each is written by somebody who
has just spent a week inside that incident, and each is good at what it is for.

Two questions are asked afterwards, and the documents answer neither.

**Which factors recur?** Any one review names three or four factors and moves on. A factor that
appears in three reviews is a finding none of the three reviewers could have made, because
making it requires having read the other two. In practice nobody has.

**Which remediations were never confirmed?** A review produces action items. Whether they
shipped is in the change log. Whether they *worked* is usually nowhere at all — and this is the
harder half, because **the absence of a verification is not a document.** There is nothing to
file, nothing to search for, and no place a reader would notice it missing.

The usual instrument is a spreadsheet of action items, which answers the shipped question and
not the worked one, and which nobody updates after the quarter ends.

## The ontology dialogue

**What are the irreducible kinds?** A `service` — something with an owner and a pager. An
`incident` — one declared event. A `contributing-factor` — something a review concluded
contributed. A `remediation` — a change made in response.

**What relates them?** An incident `affected` services and `has-factor` contributing factors. A
factor is `addressed-by` remediations. Factors are many-to-many with incidents in both
directions, which is the whole point: one incident has several, and one factor recurs across
several.

**The class that was rejected: `root-cause`.**

Review documents have a root-cause section, so the model was following its source material.
[`decisions/root-cause-is-not-a-class`](../../examples/incidents/.yidam/decisions/root-cause-is-not-a-class.yml)
refuses it:

> A root cause is a choice about how to tell the story, made under time pressure, by one
> reviewer. It is often defensible and it is never a property of the incident. Giving it a
> class or a privileged edge would make the ontology assert, of every incident carrying one,
> that the cause has been found — which is exactly the claim a good review is careful not to
> make.

The March incident is the case: a cache stampede and unbounded retries were both present, and
the stampede is survivable until the retries multiply it. Which is "the" cause is answerable
only by choosing a counter-factual, and the corpus has no business encoding one reviewer's
choice as structure.

**Naming the class `contributing-factor` is itself the commitment.** The name is what a person
sees at the moment of authoring, and it is what makes writing a second factor feel like
completing the record rather than hedging.

## The corpus

```console
$ yidam graph-check
Checked 13 instances across 4 classes — all clean.

$ yidam lint
lint: 0 finding(s), no errors
```

## Claims, honestly tagged

The interesting tag here is on a **remediation**, and it is about the remediation *working* —
never about it having merged.
[`decisions/a-remediations-tag-is-about-working`](../../examples/incidents/.yidam/decisions/a-remediations-tag-is-about-working.yml)
argues it:

> The interesting failure of a post-mortem process is not that remediations are not written or
> not merged. It is that nobody goes back. Under the merged reading that failure is invisible,
> because the field every remediation would carry is the one nobody has to earn.

All three remediations here merged. They sit at three different tiers:

| Remediation | Tier | Why |
|---|---|---|
| `request-coalescing` | `[verified]` | Two later expiry events of comparable size, no origin saturation. Somebody looked, under conditions that could have shown it failing |
| `retry-budget` | `[inference]` | Merged and assumed. No incident since — which is consistent with the budget holding and equally consistent with nothing having tested it |
| `load-shed` | `[open]` | **Nobody checked.** Not a failed verification, an absent one |

That middle row is the discipline. The absence of a recurrence is not confirmation, and
promoting it to `[verified]` because time has passed is the most common way this corpus would
degrade.

## The catalog, and what it does not answer

One source: the organisation's own incident record.

**It records what was declared, not what happened.** An incident nobody declared is not in it.
So a factor's recurrence count is a count across *reviewed* incidents, and it measures the
system only to the extent that declaring is consistent. That is why the SEV3 in July is in the
corpus at all — and why it carries an `[open]` claim saying the consistency is unestablished.

**Action items are not remediations.** An action item is a proposal. Whether it shipped is in
the change log; whether it worked is nowhere, which is precisely the gap the remediation tag
has to be authored to fill rather than imported.

**It has no memory across incidents.** Two reviews concluding the same factor produce two
paragraphs in two documents and no link. That absence is the whole reason for the corpus.

## The question a folder cannot answer

**Which factors recur.**

```console
$ yidam query 'contributing-factor~"Retries with no budget" <-has-factor- incident' --select label,properties.occurred
3 result(s)
  SEV1 — checkout saturation during a cache expiry  ()  properties.occurred=2026-03-14
  SEV2 — session store evicting under sustained load  ()  properties.occurred=2026-05-02
  SEV1 — a tier-3 service took down a tier-1 path  ()  properties.occurred=2026-06-08
2 step(s), 3 edge(s) walked, 6 of 13 node(s) read, ~74 token(s)
```

One factor, three incidents, four months, three reviewers. No single review document contains
this, and no reading of any one of them produces it.

**Which remediations nobody checked.** This one takes a property predicate rather than a
traversal — the answer is a set defined by a tag, not by a path:

```console
$ yidam query 'remediation[claim_tag=open]' --select label,properties.shipped
1 result(s)
  Shed load at the queue boundary  ()  properties.shipped=2026-06-11
1 step(s), 0 edge(s) walked, 3 of 13 node(s) read, ~19 token(s)

$ yidam query 'remediation[claim_tag=inference]' --select label,properties.shipped
1 result(s)
  A total retry budget per request  ()  properties.shipped=2026-05-20
```

Both shipped. Neither has been confirmed, and they are unconfirmed in two different ways that
a spreadsheet column marked "done" cannot hold apart.

## The shape over time

A retrospective process decaying is not an event. It is a slope, and a snapshot cannot show
one. `yidam replay` reconstructs corpus health at every commit that touched the corpus:

```console
$ yidam replay
Date         Commit    Nodes   Uncited   Uncited%
──────────   ───────   ─────   ───────   ────────
2026-03-20   1b41018       3         3       100%
2026-03-21   37247f3       4         2        50%
2026-03-24   1a10394       6         2        33%
2026-04-30   9bbf282       7         2        28%
2026-05-06   f74ecaa       8         1        12%
2026-05-21   809c868       9         1        11%
2026-06-12   2f75520      11         0         0%
2026-06-15   387857f      12         0         0%
2026-07-22   209bd08      13         0         0%

Series excludes source classes — the classes whose instances nothing is meant to
point at. The breakdown below counts every uncited node, those included.

Uncited at HEAD, by class, against what the class declares
  incident                   4 of 4    uncited by design — the ontology holding
```

The corpus opens with three services nothing points at, because the services were registered
before any incident was reviewed. Each review connects more of it. The last row is the state
the gates check, and it is the only row a snapshot would ever have shown you.

Read the last block too. `incident` is **4 of 4 uncited, and that is the ontology holding** —
nothing is meant to point at an incident, so counting incidents as orphans would report a
correct model as a failing one. A corpus-wide orphan number sums the classes where being
uncited is a finding with the classes where it is the design.

This section is why this example ships
[`history.toml`](../../examples/incidents/history.toml) and the others do not. The commits use
the corpus vocabulary rather than conventional commits, so the split is real:

```console
$ yidam log --epistemic
7f280153  [E]  decide: contributing factors, and what a remediation's tag is about
387857fb  [E]  establish: load shedding merged; nobody has checked it
809c868b  [E]  establish: a retry budget, merged and not yet confirmed
9bbf2822  [E]  establish: coalescing shipped, and held through two expiry events
1a103946  [E]  establish: the March review's two carried factors
1b41018d  [E]  genesis: incidents — the retrospective corpus

10 commit(s): 6 epistemic, 4 operational — showing epistemic
```

*The review changed what we believe* is `establish:`. *The timeline was pulled from the
incident record* is `extract:`. Four of the ten commits here are the pipeline moving and six
are the understanding changing, and the two are not the same activity.

## What this example does not show

**It does not model the timeline.** Detection, mitigation, resolution and the gaps between them
are the substance of an incident and are per-incident by nature. Nothing this corpus asks needs
them, and a class for timeline entries would be the `observation` class streamflow rejected,
one domain over.

**It does not measure anything.** No time-to-detect, no error budget, no frequency trend. Those
are computed over incidents and would need a calculator; the corpus holds what the reviews
concluded, not the metrics the platform emits.

**It does not model ownership or process.** No teams, no on-call rotation, no action-item
assignees. A real retrospective process runs on those, and the questions here happen not to
need them — which is a scoping choice and not a claim that they do not matter.

**Its replay is nine rows.** A real corpus has hundreds of commits, and the slope this one
shows is a corpus filling in rather than a process decaying. The shape of the report is real;
the arc it happens to describe is the arc of a small example being written.

**Nothing here is real.** Invented services, invented incidents, real conventions. See the
corpus README.
