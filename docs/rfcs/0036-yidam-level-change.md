# RFC-0036 — An yidam-level change, and the two shapes a derivation's report takes

- **Status:** Accepted
- **Track:** G6
- **Relates to:**
  - RFC-0024 (whose settlement — the repository's own rule decides, and an override is visible as one — is one of the three routes a derivation already has)
  - RFC-0028 (whose binding rule names the other two: a kuten narrows the loop, and a domain extension appended at genesis is where a constitutional departure goes)
  - RFC-0012 (extended by #293 and #295, the sibling E8 designs, whose venue reasoning — *not the constitution* — this RFC follows and then departs from, once, for a reason)
  - RFC-0004 (the pin, and drift over it; still Draft, and the reason staleness is reported rather than enforced)
- **Versioning layers touched:** template (one new prelude document, two in-place link edits that change no line count, one section added to `directories.md`, one scaffold reading-list entry, one issue template). **No tooling change, no SDK change, and nothing on disk in any corpus moves.**
- **Downstream reference case:** fifteen derived repositories with a vendored prelude and at least one commit, measured read-only on 2026-09-22.

## Summary

[Article I](../../yidam/prelude/CONSTITUTION.md) forecloses sangha resolution of the prelude —
correctly, and it should not be changed — and coins a term for the alternative it leaves open: *an
yidam-level change*. Nothing anywhere defined the term. This RFC defines it, adds the one report
shape the return path was missing, and states what evidence gathered across derivations obliges.

It builds **no new mechanism**, and it declines the obvious one. The reason is a measurement: the
three routes a derivation already has for departing from a prelude default are adopted by two,
zero and zero of fifteen repositories, and in the same fifteen there is not one recorded instance
of a substantive departure from a prelude norm. A fourth route would be the fifth surface in this
layer with no consumer. What is declined is named with the condition that would reverse it.

## Problem

### Three of the filing issue's premises move

#292 was written from the state of the tracker rather than from the state of the code, and reading
for it changes what is missing. Stated here rather than quietly built around, because the residual
gap is narrower and more specific than the issue's framing, and a design against the framing would
have built the wrong thing.

| #292 says | Measured, 2026-09-22 |
|---|---|
| "the entire mechanism is one maintainer reading derived repositories by hand" | Half true. **Discovery** is a maintainer reading. The **channel** is not: `directories.md` has stated a return path since the vendoring layer was written, and **46 issues** carry the `from:derived-repo` label |
| "It is not a forum" | For a defect it is one, and a busy one. For a misfit it is not: there is no form for one, and reading the 46 titles, none of them asks for a norm to be narrowed for one domain |
| "it does not scale past the number of repositories one person can read" | The reading half has been a command since #288. `yidam cohort` produces exactly the cross-derivation evidence the issue asks for, and it ran over fourteen repositories on 2026-09-18 |
| "Today it silently diverges" | **Not once, substantively.** All fifteen vendored constitutions are byte-identical to an upstream revision. The single `vendored-prelude-unedited` loss `cohort` reports is `cargo fmt --all` reaching into `domains/geodesics`, which was already filed upstream as #590 |
| "eight derived repositories" | Fifteen carry a vendored prelude and at least one commit |

What survives all of that is the sentence the issue leads with, and it is the real finding:
**Article I says what the forum is not, and nothing says what it is.**

### What was actually missing

Three things, and only three.

1. **A term with no definition.** `CONSTITUTION.md` and `GRAPH.md` both reach for *an yidam-level
   change* at the moment a rule has to move, and neither says what one is, who makes one, how it
   reaches a repository, or what a repository does while it waits.
2. **A misfit with no form.** The return path is shaped for a **defect** — *what is wrong, what it
   cost, your pin, your workaround*. A rule that is right in general and wrong for one domain is a
   different report asking for a different repair, and filed on that template it argues that a
   rule is broken when what the filer means is that it does not reach their case.
3. **An aggregate with no obligation.** `cohort` can now say a norm lost in eleven of thirteen.
   Nothing says what that costs anybody. It has happened once already — #584 measured a node-length
   norm exceeded by the median corpus in 11 of 17 — and it was answered well, informally, by one
   person who happened to be holding both halves.

## The measurement

Read-only, from committed state, on 2026-09-22. Members are ordered by authored commit count and
lettered, which is this repository's convention for downstream findings; the three that are public
are named so the aggregate has a falsifier.

| | Repository | Authored | Constitution vintage | Kuten | `.yidam/policy/` | Resolutions |
|---|---|---|---|---|---|---|
| A | `allen-county-ohio` | 1,449 | 2026-09-04 | adopted | — | 0 |
| B | *(private)* | 811 | 2026-09-04 | — | — | **58** |
| C | *(private)* | 709 | genesis | — | — | 0 |
| D | `ohio-education-funding` | 637 | 2026-09-04 | adopted | — | 0 |
| E–L | *(eight, private or unpublished)* | 247 … 73 | six at 2026-08-08, two at genesis | — | — | 0 |
| J | `ohio-budget` | 83 | genesis | — | — | 0 |
| M–O | *(three, private or unpublished)* | 69 … 8 | genesis | — | — | 0 |

Four readings, and the fourth is the one the design turns on.

**An amendment reaches a derivation slowly, and that is by design working.** The constitution has
been amended twice since the template's genesis commit — `35aee3f` on 2026-08-08, which made the
sangha opt-in, and `2fcbd19` on 2026-09-04, which bound Article V's three objects separately. The
current text is in force in **3 of 15**. Six still run the 2026-08-08 text and six still run the
genesis text, which has no scope note at all — so in six repositories a constitution that governs
collective resolution reads as binding on a repository that has never held a second position.

**The constitution is exercised in one repository of fifteen.** B has 58 `rigpa/*` refs; the other
fourteen have none. Three more carry a single `ma/*` ref and no resolution.

**All three sanctioned local departures are near-unused.** Two of fifteen have adopted a kuten.
**None** has a `.yidam/policy/` directory at all — the override layer RFC-0024 built has never had
a user. **None** carries a domain extension: every vendored constitution is byte-identical to an
upstream revision, so no genesis augmentation has ever appended an article.

**Nothing has diverged.** Which is the finding that decides what not to build.

> Dropping N and O — two template prototypes rather than domain corpora — leaves thirteen, moves
> the vintage split to 4 / 6 / 3, and changes no conclusion. They are counted because a population
> defined after the numbers are seen is not a population.

## Proposal

### 1. An yidam-level change, defined

**A commit to the prelude in the yidam template repository, decided there, and reaching a derived
repository by a re-vendor and by nothing else.** Five properties follow, and the full statement is
in [`guidelines/upstream.md`](../../yidam/prelude/guidelines/upstream.md): it is not a resolution;
it is decided upstream in the open on the issue that raised it; it reaches a repository only when
that repository fetches it; it arrives as one reviewable `vendor:` commit; and it is not announced.

Almost all of this is a description rather than a proposal — it is how the vendoring layer has
worked since it was written. What the document adds is that it is now written down in one place,
under the name the constitution already uses, in a file a derived repository has.

One property is worth stating as a decision rather than a description. **The process names one
maintainer and does not invent a body.** A quorum written into a document that has one participant
is a fiction, and the sibling designs in this epic were both settled by the repository owner on the
record. What makes the decision reviewable is that the issue, the commit and the release are
public — not that a count of signatures was met.

### 2. A misfit is not a defect

| | A **defect** | A **misfit** |
|---|---|---|
| What it is | A rule that is wrong everywhere — it contradicts itself, contradicts the tooling, or costs every derivation something | A rule that is right in general and wrong *here*, because it collides with a fact about this domain |
| What proves it | One repository is enough: a rule that contradicts its own tooling does so on every corpus | One repository is a case. What generalises it is the domain fact, not the count |
| What it asks for | A fix upstream, carried down by re-vendor | One of three answers, and the report says which |
| Template | `derived-repo-finding.md` (46 filed) | `prelude-misfit.md` (new; none filed) |

A misfit report asks for exactly one of three things, and a report that does not say which leaves
the judgement to somebody holding less of the evidence:

1. **Narrow the norm** — it keeps its force upstream and stops reaching the case. Cheapest to
   grant, and the right ask when a rule was written from evidence that did not include a domain
   like the filer's. #584 is the worked example and it predates the vocabulary: a node-length
   number defended in the rubric as a genesis snapshot, read by everyone else as a standard.
2. **Sanction a local departure** — the rule stands, and this repository is licensed to depart via
   a kuten, a `.yidam/policy/` override, or (at genesis) a domain extension.
3. **Hold the norm, and say why** — a refusal on the record. This is a result, not a non-answer:
   it stops the case being re-argued from scratch, and the reasoning is what the next derivation
   reads. A negative result about a norm is the only durable record that the norm was examined.

And a fourth thing a report must not become: **a local workaround nobody upstream sees.** Invisible
to every other derivation, discarded at the next re-vendor. `directories.md` already carries the
cost of that, in the episode where upstream added a verb citing a derived repository's use of it as
evidence of a gap, while that same repository spent four commits the same day removing the verb.

### 3. What cohort evidence obliges — record, do not gate

Two rules, binding the template repository:

- **A change to a prelude norm states the cohort reading it rests on** — the norm, the date of the
  run, and held/lost/vintage/unmeasurable over the occasions the question could be asked of. Where
  no reading was taken, the change says so. Unmeasured is a normal state and not a confession; it
  is the tri-state discipline `doctor`'s `skipped`, the MCP handshake's `stale`, and #295's
  `unrecorded` all follow.
- **A norm that loses a majority of its occasions must be answered** — amended, narrowed, or kept
  with the reason on the record. Answering is not changing: a rule that loses everywhere and is
  kept anyway is a defensible outcome, and an unexamined one is not.

**Neither gates**, following #295's settlement on `independence:` for the same class of reason.
Three specific ones here:

- **The instrument's population is a roster.** `cohort::NORMS` is hand-maintained and is not the
  set of norms the prelude states; a rule written upstream tomorrow is invisible to it until
  somebody adds a row. A gate over that measures the roster, not the practice.
- **The command has never run in CI**, and cannot: its input is a set of mostly-private
  repositories on one machine. A gate whose evidence exists in one place is a gate on one person.
- **A reading can be accurate and wrong.** The standing example is the norm withdrawn from `cohort`
  before it shipped — lost in twelve of thirteen, the strongest-looking result in the run, quoted
  accurately from a document that argues the opposite two sections further down.

These are recording rules in a prose document with no checker, and that is a real limit rather than
a phase. A rule with no enforcement decays in the direction nobody looks. What makes this one
cheaper to keep than to break is that the evidence it asks for is the output of a command that
already exists, pasted into the commit that rests on it.

## What this does not touch

**No tooling changes.** `yidam cohort` gains no verdict, no norm row and no flag; `doctor`,
`lint` and `policy check` are untouched; no schema, no report and no fixture moves. The two
recording rules are prose with no checker, which is stated as a limit in the proposal above and
not smuggled past as a phase.

**No new constraint on a resolution.** Article I already forecloses resolving the prelude, and
nothing here narrows what a sangha may do. The constitution edit resolves a term the article
itself coins; it adds no article and changes no article's force.

**Nothing in any corpus moves.** A derived repository that never re-vendors is unaffected, and one
that does gains three documents and loses nothing. No node, edge, front-matter key or convention a
corpus is written against changes.

**The 46 filed defects keep their channel.** `directories.md` states the return path and continues
to; this adds a distinction and a pointer beside it, and rewrites none of the words those issues
followed.

## Venue

The rule lands in `yidam/prelude/guidelines/upstream.md`. The argument lands here. That split is
the kuten layer's, stated in its own README — *the argument for it is upstream, in RFC-0028; the
rule lands here because it binds a repository that will never read an RFC* — and three facts force
it for this subject specifically:

- **A derived clone has no `docs/`.** #817 is the precedent: a remedy pointer that named
  `docs/configuration.md` pointed at a file the clone excludes. A process a derivation cannot read
  is not a process a derivation has.
- **Not a new constitutional article.** #293 and #295 both declined the constitution as a venue,
  and the measurement adds a sharper reason than precedent: the constitution is **dormant in
  fourteen of fifteen derivations**, by its own scope note. The one process that must bind a
  single-elector repository is the one that cannot live in the document that exempts it.
- **Not `directories.md` alone.** The return path lives there because re-vendoring does, and it
  stays there; 46 issues have followed those words and they are not being rewritten. What
  `directories.md` gains is the distinction and a pointer.

**Article I gets a link and not a sentence**, and the edit is in place: the line now reads
`an [yidam-level change](guidelines/upstream.md)` — a relative link, resolved against the installed
prelude rather than this tree — at the same line number, because twelve of this
repository's line citations point into `CONSTITUTION.md` below it and an inserted line would slide
every one. `GRAPH.md`'s use of the same term is linked the same way. Resolving a term a document
coins adds no constraint and changes no article's force, which is what makes it the one place this
RFC touches the constitution at all.

## Migration & compatibility

**Template layer.** Three new files and four edited ones, all under `yidam/prelude/`,
`sadhana/root/` and `.github/`. No `cli/v*`, SDK or bootstrap-protocol bump, and no parity
fixture changes.

**A derived repository adopts it by re-vendoring**, which is the whole of the mechanism this RFC
describes and therefore the only honest way to deliver it. Before that, the repository is pinned to
a prelude with no `upstream.md`, and the return path it does have — `directories.md` — still works;
the misfit template is on the template repository's issue tracker and is reachable by any filer
regardless of pin. **Nothing breaks** at any pin: the two CONSTITUTION.md and GRAPH.md edits are
relative links added at existing line numbers, and the new scaffold entry reaches a file that only
exists after the same re-vendor that adds the entry.

**No consumer must act.** There is no migration to perform, no file to write, and no state to
declare — which is the point of declining the mechanism below.

## Alternatives considered

**A fourth local route — a declared divergence, recorded in the repository and read back by
`cohort` as a fifth verdict.** It is the obvious design, it would answer #292's third question
cleanly, and the measurement says not to build it: the three existing routes have two, zero and
zero users across fifteen repositories, no derivation has substantively departed from a prelude
norm, and no misfit has ever been filed. A fourth route would be a mechanism with no case,
measured by an instrument with no subject, in a layer whose recurring failure is exactly that.

**The falsifier is a condition, not an intention.** Build it when either holds:

- a misfit report is filed that none of the three existing routes can express; or
- two derivations file a misfit against the same norm.

Either one produces the subject the mechanism is missing, and the second is also the first real
test of the aggregate rule above.

**A threshold that decides rather than obliges** — a norm losing N of M occasions is automatically
withdrawn, or a CI gate refuses a prelude-norm commit that cites no reading. Rejected for the three
reasons in the proposal, and for a fourth that only shows up as a rule: it would have withdrawn the
node-length norm in #584, where the right answer was to keep the number and correct what it was
being read as. A count is an input to a judgement and is not one.

**A distinct `cohort` verdict for a declared divergence** follows the same reasoning, and would be
wrong even with a subject. A declaration says a repository knows it departed; it does not remove
the occasion. Vintage is not divergence because the repository *could not* have held the rule; a
declaration is not that, and a norm that loses where it was declared has still lost. The evidence
about the norm is unchanged by the derivation's awareness of it.

## The prelude as a corpus

#292's fourth question — *is there a version of this that is itself a corpus?* — is filed as
suggestive and not yet an argument, and it stays that way, for a reason better than "not now".

The encoding objection is weak and should be set aside: files are nodes, links are edges, commits
are events, so the prelude is *trivially* encodable. The question is what would read it, and this
layer's own record says to answer that before building.

The real objection is that the interesting version is foreclosed by the article that raises the
question. A prelude-as-corpus whose disagreements were *resolved* would be a sangha resolving the
prelude, which is precisely what Article I forbids. The only version Article I permits is a corpus
with no sangha — a single-elector record of norms, positions, and the evidence behind them. That
is a description of the issue tracker with more ceremony.

What would make it an argument is a population: misfit reports, structured enough to be instances
under a class, in enough quantity that querying them beats reading them. That population is
currently zero, and the falsifier above is what would start it.

## Open questions

- **Nothing checks the recording rule.** A prelude-norm change that cites no cohort reading goes
  green. Whether that wants a check depends on how often norm changes happen — four constitutional
  amendments in the template's whole history suggests the population may be too small for a gate to
  be worth its own failure modes.
- **`cohort::NORMS` covers four norms.** The set of rules the prelude states is much larger, and
  which sentences are rules is a judgement no discovery closes. The aggregate rule is only as wide
  as that roster, and the roster's growth is unforced.
- **Re-vendor latency is measured once and is not tracked.** 3 of 15 on the current constitution is
  a snapshot. Whether that is a healthy pin-and-review discipline or a distribution problem needs
  the same number at two dates, and nothing takes it on a schedule.
