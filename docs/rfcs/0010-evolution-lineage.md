# RFC-0010 — Evolution lineage: what a position is measured against

- **Status:** Draft
- **Track:** G3 (governance)
- **Relates to:** RFC-0009 (resolution record schema), RFC-0011 (partial-sangha binding),
  `CONSTITUTION.md` Article III, `sangha/PROTOCOL.md`, `prelude/GRAPH.md`, `sangha/README.md`
- **Versioning layers touched:** template (protocol) + bootstrap protocol (branch convention)
- **Downstream reference case:** one derived repository, 29 resolutions — measured, see below.

> **Amended 2026-09-04.** This RFC was written on the argument that *forks are conceivable and
> unrepresentable*, and proposed a `supersedes:` field to represent them. Measurement against the
> one repository that has run a sangha says no fork exists and the lineage is a line. It also says
> the fallback the RFC leaned on — *"today the answer is inferred by merge-base... with one
> baseline that works"* — **is false**: merge-base gives every elector the same answer and it is
> four to six resolutions stale. So the motivation inverts. `supersedes:` is withdrawn; the
> baseline declaration, which the original filed third and treated as support for the fork case,
> is the whole of what remains and it stands on its own evidence.

## Summary

An elector's position is measured against some evolution, and **nothing states which one.** The
answer is currently inferred from git's merge-base, which — measured — returns a stale evolution for
every elector in the one corpus running a sangha. This RFC proposes one thing: **an explicit
baseline declaration on `ma/*` branches**, so "what is this position measured against" is a stated
fact rather than an inference that does not work.

## What was measured

One derived repository: 29 resolutions, 3 registered electors, 126 commits across the elector
branches, read-only.

**1 — The lineage is a line.** All 29 records land on `main`. Their 28 distinct record-adding commits
are **totally ordered** — 0 incomparable pairs. There is no fork, and there never has been.

**2 — The six "concurrent" rigpa branches are not a fork.** Six `rigpa/*` tips are contained by no
other `rigpa/*` tip, which reads as six concurrent heads. It is an artifact: all six of their
*records* are on `main`, and only their *branch tips* were left unmerged. The branches are
workspaces, not lines of inquiry.

**3 — Merge-base does not answer the question.** Asked "which evolution does this branch diverge
from", merge-base against the `rigpa/*` tips returns `rigpa/challenger-filings` for **all three**
electors — four to six resolutions behind the settlement each of them actually holds, and identical
across three branches that are 38, 42 and 46 commits apart. Their own commit subjects name later
baselines (*"the baseline through claims-that-travel"*, *"through property-claims"*). The inference
and the record disagree, and the inference loses.

**4 — Adoption does not happen the way the protocol says.** PROTOCOL prescribes
`git merge --no-ff -m "adopt: the baseline after <evolution>" rigpa/<evolution>`. Across 126 elector
commits there is **one** `adopt:` commit and **72** `merge main` commits. Electors adopt by merging
the baseline branch, not the evolution branch — which is why merge-base against `rigpa/*` is stale:
it is measuring against refs nobody merges.

## Problem

**A position's baseline is unstated and unrecoverable.** Article III's ethos is that history states
its own structure, and this is the one structural fact about an elector's branch that is left to
inference — an inference that measurement shows returning the wrong evolution for every branch it was
asked about. RFC-0011 needs this same fact to say who a partial resolution binds; today neither RFC
can name it.

**The prose presumes a singular baseline.** "A new rigpa branch supersedes the previous one and
becomes the common baseline" (`sangha/README.md`); "The new `rigpa/<evolution>` is the active
baseline" (`PROTOCOL.md`, *Baseline update*). That reading is *correct today* — the lineage is a
line — but it is correct by accident rather than by declaration, and it is not the baseline any
elector is actually measured against, because the tip they hold came from `main`.

## Proposal

**1 — Declare the baseline on `ma/*`.** An elector states which evolution their branch is measured
against, rather than leaving it to merge-base. Recommended mechanism: a
`Baseline: rigpa/<evolution>@<short-hash>` trailer on the branch's working commits — git-native, no
new file, greppable, updated by an explicit act.

The declaration is a stated fact — *this position is measured against that evolution* — and its value
is that it can be **checked against what the branch actually holds**. That check is what measurement
argues for: a declared baseline whose evolution the branch does not contain is a finding, and today
there is no field for such a finding to be about.

**2 — Linearity is the observed default and stays undeclared.** The original proposed making
linearity an opt-in domain constraint, on the theory that forks would otherwise arrive silently. With
0 forks in 29 resolutions the constraint has nothing to constrain, and a per-domain toggle nobody
sets is a surface with no consumer. If a fork ever appears, the baseline declaration is what makes it
legible — each branch says which line it is on — and *that* is the moment to argue about representing
parentage in the record.

## Withdrawn

**`supersedes:` in the resolution record.** It was proposed to represent a fork, and no fork exists.
With a linear lineage the parent of any evolution is the previous record on `main`, which the commit
order already states and which nothing can disagree with — so the field would restate a fact it
cannot contradict, in a record nobody would have reason to read it from. Adding it now would be a
schema change to every derived repository's bootstrap protocol in exchange for a value that is always
derivable. Reopen it when there is a second line to name.

## Migration & compatibility

Template (protocol) plus a bootstrap-protocol touch (the branch convention). Purely additive: a
branch with no declaration is the state every branch is in today, and the honest report for it is
"undeclared" rather than a guess. **29 resolutions and 126 elector commits exist** in at least one
derived repository — the original's *"existing resolutions are unaffected (none exist)"* was written
before any did, and no migration may assume an empty sangha.

## Alternatives considered

- **Merge-base inference (status quo).** Rejected on measurement, not on principle: it returns a
  stale evolution for all three electors, because they adopt through `main` and it reads `rigpa/*`.
- **A tracked `.yidam/sangha/baseline` file.** Enforceable, but it adds a mutable file to a
  refs-first design and makes the baseline a working-tree fact rather than a property of the commits
  that carry it. Lean: trailer, with a check; revisit if omission proves common.
- **Derive the baseline from the last `merge main` subject.** The electors are already writing it
  there in prose — *"the baseline through property-claims"* — but that is a convention nobody
  declared, parsed out of free text, and outside the closed commit vocabulary (see RFC-0011). It is
  evidence that the fact wants stating, not a mechanism for stating it.

## Open questions

- **Who updates the declaration, and when.** Adoption is a merge; the trailer is on a commit. A
  merge commit can carry it, which makes adopting and declaring one act. Confirm that is the shape.
- **Staleness.** A declared baseline the branch does not contain is a clear finding. A declared
  baseline that is *behind* the current head is normal — divergence is expected (Article VI) — so
  staleness is reportable and not an error. What, if anything, escalates it?
- **What a merge-through-`main` declares.** An elector who merges `main` picks up every settlement on
  it, not one evolution. Does the declaration then name the latest evolution on `main`, or the one
  the elector means to be measured against? These differ, and the second is the useful one.
