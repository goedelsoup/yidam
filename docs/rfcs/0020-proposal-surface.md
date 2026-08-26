# RFC-0020 — Proposing what a finding already says (`yidam propose`)

- **Status:** Draft
- **Track:** I15
- **Relates to:** RFC-0019 (whose movement questions this carries into commits), RFC-0008
  (the strict reading of Article V this depends on), RFC-0009 (the resolution authority this
  must stay below), RFC-0001 (the report contract the run emits on), RFC-0003 (the light
  binary this must run in), RFC-0015 (the epistemic log these commits join)
- **Versioning layers touched:** tooling (`yidam` CLI) / template (`prelude/GRAPH.md` gains a
  ref namespace and one rule; `.yidam/config.toml` gains one declaration) — **no parity-surface
  change and no MCP contract change**; see [What this does not touch](#what-this-does-not-touch)
- **Downstream reference case:** Project BOSC (watermark-directory)
- **Parent epic:** #252 (E4) — this RFC precedes #269, and the proposal halves of #270 and
  #271 are built against it

## Summary

Every gate this repository ships names a task and hands it to a human who may never come
back. `docs/post-genesis-measurement.md` measured which norms survive that handoff and which
do not, and the rule it landed on is the reason E4 exists: **a norm holds when something
echoes it back inside the act, and decays after it.**

The obvious conclusion — *let the tool make the edit* — is the wrong one, and checking #269's
four proposal rows against the code is what shows it. Of the four, **one survives as written,
one has to become a different verb, and two propose acts the corpus has no finding for and no
authority to take.** The corrections are this RFC's design.

What survives is a smaller command than #269 imagines and a better-founded one. `propose`
does not fix findings. It **carries** them: a finding already phrased as a question becomes an
`open:` commit, a node the corpus itself declared over-collected becomes a `withdraw:` commit,
and a question `propose` opened becomes a `close:` commit once its finding is gone. Three
acts, each of which asserts nothing that was not already asserted by the finding or by the
corpus's own declarations.

The test that decides all three is not new. `prelude/GRAPH.md` already licenses exactly one
epistemic commit written outside a resolution event, and licenses it on exactly this ground:

> It is carriage and not synthesis, which is what makes it legal outside a resolution event:
> Article V confines synthesis to resolutions, and copying a file verbatim introduces no node,
> edge or claim that its author did not hold.
> — [`GRAPH.md:386-389`](../../yidam/prelude/GRAPH.md#L386-L389), on `transport`

`propose` inherits that test and this RFC makes it mechanical: **a proposal's commit body must
contain the finding's own words, verbatim.** That is the constitutional rule expressed as
something a golden fixture can check.

## Problem

### The finding a human never comes back for

The measurement is already in the repository and is not re-argued here. The commit vocabulary
held across 201 commits in one derived repository because the verb is chosen *during* the act
and `lint --commits` echoes it back. Orphan discipline lost, in the same repositories under
the same prelude, because nothing echoes.

A report is not reviewable. A branch of commits is: it can be merged, amended, or deleted, and
each of those is a decision that leaves a trace.

### The four proposals, checked against the code

#### 1. `orphan-in` past its residence threshold — the only row that survives, and not in the form given

#269 proposes: *link it from the node the ontology says should cite it, or propose deletion.*
Three corrections, each measured.

**(a) The ontology names a class, not a node.** `ClassEdge` carries `relationship`, `target`
and `direction` ([`checks.rs:101-110`](../../yidam/cli/src/cmd/lint/checks.rs#L101-L110)), and
`target` is *"the class at the other end"*. So for an orphaned node the ontology narrows the
candidates to a set of classes, and every instance of those classes is equally licensed.

Measured on the worked example. `examples/streamflow` holds eight nodes — four `concept`, two
`gage`, two `reach` — and three classes declare an edge whose target is `concept`:
`concept.refines`, `gage.sources-from`, `reach.exhibits`. For an orphaned `concept`, the
licensed candidates are therefore **the other three concepts, both gages and both reaches:
seven candidate edges in an eight-node corpus.** The ontology has narrowed the choice to *any
node but this one.*

Picking one is choosing which node asserts a relationship it does not currently assert. That
is authoring an edge, and an edge is a claim.

**(b) In the worked example the check cannot fire at all.** `is_source_class` reads only the
class's *own* edge list, and is true when that list is non-empty and holds no `direction: in`
entry ([`checks.rs:125-132`](../../yidam/cli/src/cmd/lint/checks.rs#L125-L132)). Every class in
`examples/streamflow` declares outbound edges only, so all three derive as source classes and
every instance is exempt.

Verified by construction. Removing all three inbound links to
`.yidam/corpus/concept/low-flow.yml` and running `lint` reports **`0 finding(s)`**. Appending a
single `direction: in` declaration to `concept.ont.yml` and re-running the same corpus reports:

```text
INFO [orphan-in] Node nothing points to — 1 finding(s)
  .yidam/corpus/concept/low-flow.yml: nothing links to this node — uncited since 2026-08-26, 2 commit(s)
```

The corpus did not change between those two runs; one line of the ontology did. `gage` already
declared `sources-from → concept, direction: out` — the same relationship, stated from the
authoring end — and nothing reads it that way. This is E1's own thesis one field short of
where it stopped: *the ontology already knows things the tooling never asks it.* It is a
defect in #255's derivation rather than in this RFC's scope, and is filed as **#336**; but
`propose`'s flagship row depends on a check that is silent in the corpus yidam ships to teach
people what good looks like, and that has to be said here.

**(c) `orphan-in` is the only check that carries an age.** `Violation::age` is `None` for every
finding except those `orphan_in_dated` decorates
([`mod.rs:289-311`](../../yidam/cli/src/cmd/lint/mod.rs#L289-L311),
[`model.rs:58`](../../yidam/cli/src/cmd/lint/model.rs#L58)). "Past its residence threshold" is
therefore well-defined for exactly one check today. That is not a problem to fix here — it is a
bound on how much of the corpus `propose` can speak about, and the command should say so rather
than imply generality it does not have.

#### 2. A claim whose catalog source moved — the finding does not exist, and its nearest relative refuses the retag on purpose

#269 proposes: *retag, with the demotion recorded in the message.*

**Nothing detects a catalog source moving.** A catalog entry records `location` (a list of
`url` / `url_template` values), `obtained:` and an optional `used-by:` list. There is no fetch,
no hash of what was fetched, and no TTL — the TTL is #271, unbuilt. The three catalog checks
(`catalog-uncited`, `catalog-unobtained-but-cited`, `catalog-used-by-drift`) check the
catalog's own bookkeeping against the corpus's citations. None of them looks upstream, and
`doctor` is constitutionally barred from doing so.

**And the built machinery for this exact case deliberately does not retag.** E3 shipped it one
epic ago for dependency citations: `citations::moved` compares two surveys and emits a
`Movement` whose payload is a *question* — `/// Phrased as a question, deliberately. The answer
is a person's.` ([`citations.rs:626-641`](../../yidam/cli/src/cmd/lint/citations.rs#L626-L641)).
The renderer is explicit about the temptation it is refusing
([`citations.rs:747-772`](../../yidam/cli/src/cmd/lint/citations.rs#L747-L772)):

```text
3 question(s) opened by this update — nothing was changed, and no claim was re-tagged:
```

> **Questions, and a sentence saying nothing was changed.** The temptation on a report like
> this is a summary line that reads like a verdict — *3 claims weakened* — and that is precisely
> the synthesis this must not perform.

Row 2 as written would reverse that, for the weaker case: E3 could at least *see* the far side
move, and still declined to say what it meant. A catalog source cannot even be seen to move.

The correction is not to abandon the row but to notice that E3 already produced the right
artifact and stopped one step short of making it durable. **A `Movement.question` is exactly
what an `open:` commit records.** Row 2 becomes an `open:`, not a `revise:`, and #270's demotion
and #271's expiry both collapse into the same act.

#### 3. A question whose answer already landed — deciding that is the resolution event

`claims::is_open_question` finds open questions
([`claims.rs:587-593`](../../yidam/cli/src/claims.rs#L587-L593)). Nothing finds answers, and
nothing could: an open question in this corpus is a paragraph of prose tagged `[open]`, and
deciding that some later paragraph answers it is a judgment about content.

`.yidam/decisions/` records decisions, but matching a decision to a question is the same
judgment wearing a filename. Article V is unambiguous that this belongs to a resolution event,
and `cmd/sangha.rs` already treats the read-only limit as constitutional rather than
convenient ([`sangha.rs:14-16`](../../yidam/cli/src/cmd/sangha.rs#L14-L16)).

The row does not survive as stated. What survives is a strictly narrower thing that is not a
judgment at all, and it is the closing half of this design: **`propose` may close a question
`propose` itself opened, when the finding that prompted it no longer holds.** It never closes a
human's question. See [The three acts](#the-three-acts).

#### 4. An oversized node — no finding, and a split authors nodes

There is no oversized-node check. `[lint]` in `.yidam/config.toml` declares exactly one field,
`escalate_after` ([`config.rs:25-43`](../../yidam/cli/src/config.rs#L25-L43)), and no check in
`checks.rs` measures a node's length.

Even given one, a split names two nodes that did not exist, divides the original's claims
between them, and decides which edges follow which half. Every one of those is authoring. The
row is out of scope, and a length check — if a corpus wants one — is an ordinary lint finding
whose proposal, under this design, would be an `open:` like any other.

### What the corrections have in common

Rows 1(a), 2 and 4 all fail the same way: the proposed act **asserts something the finding did
not**. An edge asserts a relationship. A retag asserts a standing. A split asserts two nodes
and a partition of claims. Row 3 fails one step further out — the act would assert that a
question is settled, which is the definition of a resolution.

The acts that survive are the ones that assert nothing new: recording a question, retracting
what nothing came back for, and retiring a question the tool itself raised.

## Design

### The test: carriage, not composition

A proposal is legal iff its content is already present in the finding, or in a declaration the
corpus wrote. This is `transport`'s licence, and it is the same licence for the same reason.

Made mechanical: **the commit body must contain the finding's `detail` string verbatim.** A
generated message that paraphrases has composed; one that quotes has carried. The rule is
checkable by a golden fixture, it is the thing to test rather than prose quality, and it fails
closed — a proposal whose finding has no quotable detail is not drafted.

### The three acts

| Act | Verb | What licenses it |
|---|---|---|
| Record the question a finding already phrased | `open` | the finding exists |
| Withdraw a node the corpus declared over-collected | `withdraw` | `propose.withdraw_uncited_after`, declared by the corpus |
| Retire a question `propose` opened, whose finding is gone | `close` | the tool's own marker, and the finding's absence |

Nothing else. No edges, no retags, no new nodes, no splits, no merges.

#### `open:` — where the question goes

An open question in this corpus is **a paragraph inside an existing node, tagged `[open]`** —
not a node of its own. `examples/streamflow` demonstrates it in
`concept/instream-flow-right.yml`, whose third paragraph is a live question tagged `[open]`
under a node carrying `[verified]` and `[inference]` paragraphs above it.

That is where a proposed question goes: appended to the node the finding is about. It creates
no node, so it chooses no class and satisfies no class contract; it is visible to
`open-questions`, to `claims`, to the MCP claim tools and to `status`; and it sits against the
thing it is a question about.

The appended paragraph identifies itself, in its own text, as generated and at what commit:

```yaml
description: |
  … the node's authored prose, untouched …

  Opened by `yidam propose` at 8d35441 — nothing links to this node — uncited since
  2025-11-02, 214 commit(s). The ontology licenses an inbound edge from `concept`,
  `gage` or `reach` and names no node, so none is drawn here. [open]
```

Two properties follow from the marker and both are load-bearing. A reader can tell generated
prose from authored prose without consulting the log — the principle `authorship.rs` already
argues for regions, applied at the paragraph. And a later run can find its own paragraphs
again, which is what makes `close:` possible.

**`claim_tag` is not touched.** A node carrying `claim_tag: verified` that gains an `[open]`
paragraph arguably ought to be demoted under the rule that *a derived assertion travels only as
far as the weakest claim beneath it* (`agent-conduct.md`). That rule is a norm and not a check,
retagging is composition, and the commit says in as many words that it was not done — the same
sentence `render_movements` prints for the same reason.

#### `withdraw:` — and what licenses a deletion

An orphan can be linked or deleted, and choosing between them is a judgment. What makes the
deletion carriage rather than judgment is that the corpus made the choice first, in a
declaration of its own:

```toml
[lint]
escalate_after = 100

[propose]
# Corpus-touching commits an uncited node may hold before `propose` drafts its withdrawal.
withdraw_uncited_after = 400
```

Same shape as `escalate_after` and for the same argued reason: **declared by the corpus, never
hard-coded, and absent means never.** A number compiled into the binary would be one
repository's judgement arriving as a proposed deletion in another that never agreed to it.
Absent — which is every corpus by default — `propose` drafts questions and nothing else.

Note that `escalate_after` cannot double for this. It declares when a finding becomes a build
failure, which is a statement about the gate; it is not a statement that the node should go.
Two thresholds because they are two claims.

A withdrawal deletes the node and, in the same commit, drops it from any catalog `used-by:`
list that names it — otherwise the proposal would trade one finding for a
`catalog-used-by-drift`. Nothing points at the node, by construction, so no edge is orphaned.

#### `close:` — the only closure that is not a judgment

A `close:` proposal removes a paragraph `propose` wrote, and only when the finding that
produced it no longer holds. The marker is the identity; the finding's absence is the licence.
It never touches a paragraph a person wrote, and it never decides that a question was answered
— only that the observation which raised it is no longer true.

This is what keeps the branch from accumulating: a corpus that fixes an orphan gets the
question retired on the next run, in a commit that says what changed.

### The branch

**`propose/<head>`**, where `<head>` is `git rev-parse --short HEAD`.

The namespace is deliberately not Tibetan. `docs/git-branch-model.md` states that the
distinction between `ma/` and `rigpa/` *"is ontological, not procedural"* — `ma/` is a voice
moving toward recognition, `rigpa/` is recognition. A proposal is neither. It is a draft
awaiting a person, and giving it a name from that vocabulary would be the first move toward
treating it as a standing it does not have. A plain English name says what it is.

Naming the branch after HEAD rather than after a date follows the same argument the residence
clock does: a date is a function of when you ran the command, and a commit is a function of the
repository. The branch names the corpus state it was computed against, so two runs at the same
HEAD address the same branch, and a stale proposal branch is visibly stale.

**Nothing merges itself.** The branch is reviewed as commits and rejected by deleting it.
`propose` never checks it out, never fast-forwards anything onto it, and refuses to write over
an existing `propose/<head>` without `--force`.

### Writing without touching the working tree

Plumbing against a temporary index, so a `propose` run in a dirty working tree is safe and
changes nothing a person can see:

```
GIT_INDEX_FILE=<temp>  git read-tree HEAD
                       git hash-object -w --stdin        # the new blob
                       git update-index --add --cacheinfo 100644,<sha>,<path>
                       git update-index --force-remove <path>     # a withdrawal
                       git write-tree
                       git commit-tree <tree> -p <parent> -F -    # the message on stdin
                       git update-ref refs/heads/propose/<head> <sha>
```

Every command here is one this repository already shells out to git for. The one thing the
plumbing buys that a checkout would not is the guarantee in `--help`: the `*` marker says a
command *rewrites files in the repository it is run against*, and this one does not — it writes
objects and one ref. That distinction is worth keeping true, so `propose` carries the marker
(it writes to the repository) with its long help stating what it does not touch.

### Who authored it

git distinguishes author from committer, and the distinction is exactly the one needed here:

```
Author:    yidam propose <propose@yidam>
Committer: <the identity that ran the command>
```

The tool drafted it; a person ran it; a person will merge it or delete it. That is a true
record without inventing an attestation mechanism — RFC-0012's elector identity work is open
(#274) and a proposal is not an elector's position, so it must not borrow one's name.

### The commit message

The interesting surface, per #269, and the requirement is that it be testimony while being
generated. A generated message reads as a changelog when it describes what the commit did to
the files. It reads as testimony when it says what was observed and what follows — and the tool
has exactly that available, because a `Check` already carries a `rationale` written to explain
why anyone should care, and a `Violation` already carries a `detail` written to describe one
failure.

```text
withdraw: low-flow — collected, and uncited for 214 corpus commits

nothing links to this node — uncited since 2025-11-02, 214 commit(s)

`concept` declares an inbound `sources-from` edge, so this is not a source class
holding: it is a node this corpus collected and did not use. `.yidam/config.toml`
declares `withdraw_uncited_after = 400`, and this finding is past it.

Nothing replaces it. If that is wrong, the fix is an edge, and not this commit.

Finding: orphan-in .yidam/corpus/concept/low-flow.yml
Proposed-from: 8d35441
```

Four things about the shape:

- **The second paragraph is the finding, verbatim.** That is the carriage test, in the place a
  reader reads first.
- **The subject is a subject line, not a diff summary** — `withdraw: <node> — <what held>`, in
  the register GRAPH.md's own examples use.
- **`withdraw` is honoured on its own terms.** GRAPH.md requires a withdrawal to *"say what
  replaces it, or that nothing does"*, so the message says it.
- **Two trailers**, so `log` and `lint` can filter proposals without parsing prose, and so a
  reviewer can find the run that produced a commit.

An `open:` commit takes the same shape with the question in the subject and the appended
paragraph quoted in the body.

### Idempotence, and what a re-run means

Re-running at the same HEAD against the same corpus produces the same set of proposals in the
same order — findings are already emitted in a stable order and the branch is derived from
HEAD. It does not produce byte-identical commits, because the committer date is real and
`propose` will not falsify it. So: a re-run against an existing `propose/<head>` refuses,
naming the branch, and `--force` replaces it.

### Reporting

`propose` emits on the report contract like every other command: `--json` gives the proposals,
their findings, the commits written and the branch. The text form prints what was drafted and
one sentence that is not optional —

```text
4 proposal(s) on propose/8d35441 — nothing was merged, and no claim was re-tagged.
Review them as commits: `git log --reverse main..propose/8d35441`.
Reject them by deleting the branch.
```

— because the failure mode of a command that writes epistemic commits is a reader who assumes
they landed.

## What this does not touch

- **The parity surface.** No SDK gains a function. `propose` is a CLI surface over findings the
  CLI already computes, exactly as `lint` is.
- **The MCP contract.** No tool is added. An agent that could call `propose` over MCP would be
  a tool proposing to a tool, and the whole design turns on a person reading the branch.
- **The node model.** No field is added to an instance. A proposed question is a paragraph, and
  a paragraph is already what an open question is.
- **`doctor`.** It writes nothing and does no network, and keeps both.
- **Synthesis.** `cmd/sangha.rs` stays read-only and Article V is unchanged.

## Open questions

- **Does the ontology ever narrow to one candidate?** If exactly one instance of exactly one
  class could license an inbound edge, proposing that edge is arguably carriage. It is left out
  of this RFC because the case is rare, the rule is easy to state and hard to keep true as a
  corpus grows, and the cost of getting it wrong is a tool authoring a claim. Worth measuring
  against a real corpus before deciding.
- **Should a withdrawal carry the node's text into its own commit body?** It would make the
  branch self-contained for review — a reviewer would not need to check out the parent to read
  what is being deleted. It would also put an entire authored node into a message, which is
  not what a message is for.
- **What retires a stale `propose/<head>` branch?** Nothing, currently. A corpus that runs
  `propose` weekly accumulates one branch per HEAD it ran at. A `--prune` that deletes proposal
  branches whose HEAD is an ancestor of the current one is the obvious answer and is not
  specified here.
- **`withdraw_uncited_after` has no measured default to recommend.** `escalate_after` has the
  same problem and answers it by having no default at all. That is the right answer for now,
  and it means the deletion half of this design is off in every corpus until someone turns it
  on — which is a fair description of how much confidence it has earned.

## Consequences for E4's other children

- **#270** — part 1 (`[verified]` with no citation) is an ordinary lint check and lands
  independently of this RFC. Part 2 (demotion when a source moves) becomes an `open:` proposal
  carrying the question, not a retag. #270 says *"nothing here may ever propose a promotion"*;
  this design says nothing may propose a retag in either direction, which is stronger and
  removes the need for the asymmetry.
- **#271** — the TTL and its report are independent. An expired entry's proposal is an `open:`,
  which is what #271 already asks for (*"open a question rather than refreshing silently"*).
- **#23** — unaffected. Its findings would be proposal-eligible on the same terms as any other,
  once it produces findings.
