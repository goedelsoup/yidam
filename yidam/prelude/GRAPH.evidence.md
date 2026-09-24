# Knowledge Graph Model — evidence

Why the rules in [GRAPH.md](GRAPH.md) say what they say: the incident that produced each one,
the measurement that set its threshold, the failure it was built against.

This file is **not** part of the recurring read. Read the section you need when a rule
surprises you, when you are about to argue with one, or when you need to know how far a number
can be pushed. Each section is reached by the `[why]` link beside its rule.

## propose-namespace

`propose/<head>` is deliberately named in plain English while the others are not. The
distinction between `ma/` and `rigpa/` is ontological rather than procedural — `ma/` is a voice
moving toward recognition, `rigpa/` is recognition — and a proposal is neither. It is a draft
awaiting a person, and a name from that vocabulary would be the first move toward treating it
as a standing it does not have.

It is named after the commit it was computed against, for the reason the residence clock counts
commits: a date is a function of when you ran the command, and a commit is a function of the
repository. See [commits-not-days](#commits-not-days).

## quote-the-question-label

A bare `?` followed by a space is YAML's explicit-key indicator, so the unquoted form is not a
node with a funny title — it does not parse, and `serde_yaml` and npm `yaml` refuse it at the
same column. `yidam lint` reports it as `malformed-yaml`, which is an error and gates; what it
does *not* do is reach every reader first, and a report run over an unparseable node shows it
with an **empty label and no properties** rather than announcing the problem.

Every instance in every corpus that uses this marker quotes it, and a declaration that showed
the bare form would be handing out the one spelling that silently loses the marker.

`yidam open-questions` has read the `?` for as long as the report has existed; it is written
down in the rules because no document had ever said so, and a convention only the tool knows is
not a convention.

## open-question-marker-gates-nothing

The marker exists so that a question is legible standing alone — outside whatever record opened
it, and to a reader who has not read that record.

Adoption is accordingly thin, and is the reason this is a declaration rather than a rule: **10
nodes, in 2 of 16 derived corpora.**

## question-node-versus-open-claim

The difference is the whole of why the marker is worth writing down. **1,665 of those same
2,770 nodes, 60.1%, carry an open claim somewhere**, which is what a corpus that tags its
evidence looks like rather than a corpus made of questions.

One corpus declares a claim property named `attestation_standing`; its `technique` node titled
*Blast Beat* reads `open` there, because how well the technique is attested is genuinely
unsettled. That is the field working exactly as designed, on a node that is plainly not a
question.

## date-precision

What `property-type` catches in a date field is prose, and `1985` is not prose: it is the
precision the fact is actually known to, and demanding a month and a day there does not make
the record more accurate, it makes it invented.

## silence-is-not-a-contract

Reading either silence as *and therefore none are permitted* would flood every corpus whose
ontology is not filled in — which is the corpus with the least reason to trust its graph.

## both-ends-are-read

Reporting a source class's instances as orphans reports the ontology working. In one derived
repository that was 17 of 35 `orphan-in` findings.

This is stated because the derivation once read only a class's own list, looking for a
`direction: in` entry. That treated a class's silence about inbound edges as a positive
declaration that nothing points at it, while the ontology said elsewhere that something does —
the inverse of the over-read in [silence-is-not-a-contract](#silence-is-not-a-contract). It was
measured on the worked example, where all three classes derived as source classes and
`orphan-in` could not fire anywhere in the corpus.

## edge-policy

**This is the part that was got wrong.** Reading a non-empty `edges:` as *and no others may*
put 210 errors on a derived corpus that coins precise single-use verbs on purpose — 107
distinct relationships across 18 classes, none of them a defect.

The three-value table is the result: `exhaustive` for a class that closed its vocabulary and
asked for the gate, `characteristic` for one whose `edges:` names what it is *defined by*,
absent for the typo case, which is real and worth seeing but was never a contract anybody
wrote.

## edge-tags-reported-beside

A node's claims are measured over its text, and an edge is in no node's text. So a tagged edge
is counted beside the node figure and not added to it.

A link's tag is the *edge's* claim throughout: the node's own counters skip the `links:` block,
so writing `claim_tag: "[open]"` on a link no longer reads as an `[open]` in the node's prose.

## edge-claims-two-keys

`edge-verified-unsourced` needs no declaration because its population is empty in a corpus that
tags no edges — writing `claim_tag: verified` is itself the opt-in. `edge-untagged` cannot be
that: its population is every edge.

`structural:` verbs are authored by many classes at once, which is why this is declared
corpus-wide rather than per class — the argument `universal.yml` already makes about a property
every class carries. See [universal-properties](#universal-properties).

Recording which verbs are bookkeeping is a fact about a vocabulary; `required: true` is a
request for a gate over every other edge in the corpus, and one must not arrive as a side
effect of the other.

## edge-standing-unheld

An edge asserting `verified` between two nodes this corpus grades `[open]` claims more about the
relationship than the corpus claims about what it relates.

A node's standing is read from a `type: claim` property rather than the weakest marker in its
prose because a synthesis node carries all three tags by design and is meant to.

It is one-directional, like every check in this family. An `open` edge between two `verified`
nodes is not a defect: that is a corpus saying it knows both things and not that they are
related, which is what the vocabulary is for. And it never gates — both sides of the comparison
are opted into separately, so a corpus can adopt edge tags on a graph whose nodes were graded
years earlier and inherit findings it did not create.

## prose-keys

`description` was the only key anything read as prose, and corpora write more than one. Closing
the node schema over the top level rejected **117 nodes of 117** in one derived repository —
`summary`, `findings`, `revisions`, `unfilled` — and a projecting consumer 199 of 199.

None of those keys reached the parsed node, so the checks that read a *field* saw a fraction of
what the node says while the ones that scan the *file* saw all of it. On one corpus the two
answer **118 lines against 21**.

Union rather than override, because two declarations that a key holds prose agree — unlike two
declarations of one property's `type`, which contradict each other.

## prose-in-a-property

A fifth of what corpora write is not at the top level at all. Measured over sixteen corpora and
2,763 nodes: **1,895 of them — 68.6% — hold a block scalar nested inside another key**, and 83%
of that sits under `properties:`. A `method`, a `verbatim`, a `location_description` is the node
saying something, and nothing that read prose could see it.

**What changes when you flag one.** On one public corpus, flagging 14 property declarations took
`node-too-long` from 58 findings to 94 — which is the length the nodes always were. On that same
corpus the flag adds 10.4% more prose and changes the text of 380 nodes of 694, which is why
`yidam index-build` must be re-run.

## retrievable-not-prose

`prose:` is the right question for `node-too-long` and for `missing-description`. For `embed` it
is nearly the right question and not the same one.

`examples/streamflow`'s gage declares `parameter: "00060"` and `units: cubic feet per second`.
Neither string was in the node's vector, so a query for the units could not reach the node that
writes the phrase, and a query for the parameter code could not reach the node the catalog entry
for that source is *organised around*.

Flagging them `prose: true` would have fixed the embedding and broken two reports along the way:
`node-too-long` would count `00060` toward the node's length, and `missing-description` would
accept a node that says nothing but `00060` as a node with something said about it.

Prose is composed once either way, so flagging a `prose: true` property `retrievable` as well is
redundant rather than wrong.

## claim-counter-unchanged

`count_in_node` and `has_open_claim` always read the whole file, so they always saw every prose
field. What the `prose:` declaration closes is the gap between them and the field readers, which
is where the two numbers in [prose-keys](#prose-keys) came from.

## carried-findings

`yidam propose` used to open a question by splicing a paragraph into the `description:` block and
identifying it later by searching the prose for the sentence it had written. Four things follow
from the record form, and each was a defect of the prose form:

| | Prose paragraph | Record |
|---|---|---|
| an author rewords the question | unclosable — `close:` matched the sentence | closed by `id` |
| the corpus counts its open questions | the paragraph ended in `[open]` and was counted as one | counted as nothing the corpus claims |
| *which questions are open here, from which check, since when* | scan the prose | read the records |
| a node whose `description:` is a plain scalar | refused — a paragraph would reformat a line somebody wrote | recorded; the prose is not touched |

**Under `yidam:` and not at the top level**, because the obvious key is taken: one derived corpus
writes a top-level `findings:` holding its own research prose.

The separation from the corpus's own claims is what lets a carried question be closed when the
check stops reporting it.

## universal-properties

Two shapes were measured that no per-class declaration handles.

A `name:` is apparatus that applies to every class — declaring it per class would be sixteen
copies of one decision, and a seventeenth class would silently not have it. A `pattern:` is a
self-describing family: a fiscal-year snapshot recurs, so it is not a typo, but declaring each by
name would mean editing an ontology every July to permit next year's.

Between them these were 29 of one derived corpus's 29 `undeclared-property` findings.

Anchor a pattern: an unanchored one licenses every name that merely contains a match, and
`undeclared-property` is what catches the next real typo.

## not-a-property-side-edge-policy

The corpus that needed `universal.yml` measured its property vocabulary at 94% declared against
its relationships at 68%, and wants the property gate. What it lacked was a way to say that two
specific shapes are apparatus rather than schema.

## required-absent-means-false

The check could not gate before the field existed: without it, an omission contradicted nothing,
and a node carrying no `claim_tag` is a real state rather than a defect.

Every corpus written before this field existed was written under a schema where the question
could not be asked, so defaulting to `true` would gate every class in every derived repository on
a declaration nobody made.

## schema-no-stricter

A schema that stayed strict where the gate had relaxed would underline a field the build is happy
with, which is the same drift arriving from the direction nobody watches.

That symmetry is the point. A consumer that rejected what the gate accepts would fail a build on
a file that looked fine everywhere else, and the ontology would get the blame.

## changing-a-class

An ontology is only as good as its ability to change, and once the contract gates, editing a
class definition in place breaks the corpus that adopted it: add a property and every instance
trips `missing-property`, retype one and they trip `property-type`, re-target an edge and they
trip `edge-target-class`. That is a strong incentive to leave a definition wrong.

**A retype is refused rather than guessed.** `type: string` → `type: date` over a value reading
`last spring` has no mechanical conversion, and writing it back unchanged while reporting success
would leave the corpus in a state its own gate rejects.

The migration record carries the mechanical half and `.yidam/decisions/` the argument, because a
record that also had to carry the reasoning would make `decisions-log` a list of two different
kinds of thing.

## commits-not-days

The level cannot say the thing worth knowing. A node uncited for five commits is a sweep in
progress and entirely healthy. A node uncited for two hundred is over-collection.

A day count is a function of when you ran the report, so the same corpus answers differently
tomorrow and nothing can pin or cache it. A commit count is a function of `HEAD`. Only commits
that touched `.yidam/corpus` are counted: a commit that changed nothing here could not have
cited anything, so counting it would inflate every age in a repository that also holds code.

A node cited on the day it was authored and orphaned two hundred commits later dates from the
orphaning.

## reachability-per-class

A source class at 12 of 12 is the model working. The same figure on a class declaring an inbound
edge is the only one of the three that is a finding. A class that declared no edges is not being
scored at all — and saying so beats printing nothing, which reads as a pass.

Excluding source classes from the percentage column is the difference between the 22% and the 7%
the same corpus reads at, and is why the column says what it is a percentage *of*.

## escalate-after

The commit checks must stay at Warn because history cannot be rewritten to fix a verb, so a gate
on immutable state could only ever be noise.

The right number depends on how fast this corpus is meant to consume what it collects, so a value
compiled into the binary would be one repository's judgement arriving as a build failure in
another that never agreed to it. Declaring it in `.yidam/config.toml` keeps the argument for the
number in the repository that has to live with it.

**It reaches the checks that date their findings, and today that is `orphan-in` alone.** A
finding about an immutable event has no clock, so no threshold can act on it. Most of the report
is in that position, which means arming this number changes less than its prose suggests — and a
corpus reading the prose alone has declined it on the strength of a risk that did not apply.

## which-checks-escalate

Every check that ran is in that array, including the ones that found nothing — which is the case
that matters, because the question is asked before adopting rather than after.

## baseline-expiry

`since` is carried forward so that re-running `--bless` records new debt without forgiving old
debt a second time.

Raising `expire_after` and saying in the commit message why this corpus needs longer than it said
it did puts that argument in the repository, in a diff somebody reviews.

## two-clocks

The commits rule is about *corpus state*: how long a node has gone uncited is a fact about this
repository, so the repository's clock is the honest one.

A source's staleness is not a fact about this repository — a statute does not become stale
because you committed, and a gauge record does not stay fresh because you did not. So that one
report does answer differently tomorrow, and that is what a TTL is for.

## distinct-by-style-is-not-enough

A reader can eyeball the difference between an epistemic and an operational commit; a tool
cannot, and neither can a reader six months and four hundred commits later. Keeping them distinct
is what preserves the log's readability as a knowledge record.

## the-verb-stands-alone

`vendor(yidam): …` costs twice: the lint reports it as outside the vocabulary, and classification
falls through to Epistemic, silently filing an operational commit as a change in understanding.

## the-open-verb

Measured across eighteen derived repositories: of 57 `open:` commits, **41 touch no corpus node
at all, and all 41 are in the one repository that has run the sangha protocol** — 40 of them
writing a `sangha/positions/*.md`. There, opening is opening a *position*, the first move of a
deliberation, and that is the verb's majority use by a wide margin. The other 16 are spread
across six repositories and do touch a corpus node.

The old gloss led with *a question opened*, and the measurement does not support leading with it:
**not one of the ten `?`-marked question nodes in any corpus was introduced by an `open:`
commit.** They arrive under `establish` (7), `assess` (2) and `genesis` (1).

Opening a question and marking a node as one are, in practice, unrelated acts, and a corpus
should not be told to expect the verb where the corpora do not use it. Both senses are still the
same shape at different scales — something has been put in play and not settled.

## the-collective-verbs

The vocabulary had none of `transport`, `resolve` or `adopt` until derived repositories needed
them: the constitution makes resolution events first-class and the rules give them a ref
namespace, but nothing named the commits that perform one.

`transport` is legal outside a resolution event because Article V confines synthesis to
resolutions, and copying a file verbatim introduces no node, edge or claim that its author did
not hold. That constraint is the whole of the verb.

Without it a position is committed to a branch nobody else is on. A derived repository ran twenty
resolutions that way and found what it costs: four corpus nodes citing position files that
resolved for their author and for nobody else, and two resolutions standing on the baseline whose
arguments were not. It coined this verb itself, used it twenty-six times, and never wrote it down
anywhere but its commit bodies.

`adopt` is the routine follow-on, which happens once per elector per resolution and is the single
most repeated act in a collective repository.

## a-tool-writes-three-verbs

`yidam propose` is the only tool that writes an epistemic commit. A proposal records a question a
finding already phrased, withdraws a node this corpus declared over-collected in its own
`.yidam/config.toml`, or retires a question `propose` itself opened once the finding is gone.

The boundary is worth stating because it is easy to cross by accident. RFC-0020 carries the
argument, including why three of the four acts originally proposed for the surface are not there.

## why-the-vocabulary-is-closed

An open vocabulary decays into one verb per commit. A repository derived from this template ran a
hundred commits with roughly sixty distinct leading words — `lift`, `grain`, `void`, `worst`,
`viewport` — each individually evocative and collectively useless: nothing could recover which
commits changed what was known and which merely moved bytes.

## a-step-names-its-verb

`yidam lint --commits` is Warn severity and correctly so, since history cannot be rewritten to
fix a verb; that also means it reports drift only after the drift is permanent.

A derived repository put four consecutive resolution commits on the wrong verb and the finding sat
in a warning nobody read. Its own remedy was to record the verb at the step that writes the
commit, which is the only place the next one can be caught.

## naming-the-kind-is-not-the-verb

The kind is derived from the verb rather than the other way around — `classify_commit` takes the
verb and returns the kind.

This template prescribed four commits that way, two of them in consecutive sentences, and every
one had an obvious right verb nobody had written down.
`no_step_names_a_commit_kind_instead_of_its_verb` cannot see a step that says "commit this" with
no qualification at all, and no check can.

## merge-commits

Exemption is detected by parent count rather than by subject text, so every form git generates is
covered — including `Merge <ref>`, which the prefix test used to miss and which is what a derived
repository produced ten times.

## the-sangha-link-is-absolute

Bootstrap creates `.yidam/sangha/` only under `governance: collective`, and single-elector is the
default — the case the skill tells the agent to take when the user is unsure. A relative
`../../sangha/README.md` is correct in a collective repository and points at nothing in every
other one, where it becomes a permanent `unauthored-prose-link` the reader cannot act on.
