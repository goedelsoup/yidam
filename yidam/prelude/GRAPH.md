# Knowledge Graph Model

This repository's knowledge graph lives in git. No external store is required.

## Encoding

| Git primitive | Knowledge meaning |
|---|---|
| File | Knowledge node — a concept, document, artifact, or relation |
| Commit | Knowledge event — an addition, revision, or synthesis |
| Commit message | Event description — what changed and why |
| Link (`[label](path)`) | Edge — an explicit relationship from one node to another |
| Branch | Parallel inquiry thread — speculative or in-progress |
| Merge commit | Synthesis — two threads of knowledge joined |
| Tag | Stable checkpoint — a named state of the graph |
| `refs/heads/phase/<name>` | A bounded phase of inquiry in progress |
| `refs/heads/rigpa/<evolution>` | *Collective mode only* — a settled, named collective understanding |
| `refs/heads/ma/<elector>` | *Collective mode only* — one elector's current working position |
| `refs/heads/propose/<head>` | A branch of proposed commits `yidam propose` drafted, awaiting review |

`rigpa/` and `ma/` exist only in repositories bootstrapped as `governance: collective`. In a
single-elector repository — the common case — the baseline is `main` and inquiry runs on
`phase/<name>` branches. See [PHASES.md](PHASES.md).

`propose/<head>` is deliberately named in plain English while the others are not. The
distinction between `ma/` and `rigpa/` is ontological rather than procedural — `ma/` is a
voice moving toward recognition, `rigpa/` is recognition — and a proposal is neither. It is a
draft awaiting a person, and a name from that vocabulary would be the first move toward
treating it as a standing it does not have. It is named after the commit it was computed
against, for the reason the residence clock counts commits: a date is a function of when you
ran the command, and a commit is a function of the repository.

## Nodes

Files are nodes. Their content is the node's value; their name and path are the node's
identity within the graph.

Nodes should be **small and focused** — one concept, one artifact, one relationship.
Large files are a sign that decomposition is needed.

Node types are distinguished by directory, not by filename:

| Directory | Node type | What it represents |
|---|---|---|
| `corpus/` | Knowledge node | A concept, relation, artifact, or open question in the domain |
| `catalog/` | Source node | A data source — dataset, paper, API, external knowledge base |

Corpus nodes represent derived knowledge; catalog nodes represent its provenance. An edge
from a corpus node to a catalog node reads as "this concept draws on this source." Catalog
nodes do not contain derived knowledge — only enough to locate and characterize the source.

## Edges

Edges are explicit markdown references: `[label](path)`. An agent reading the graph can
follow edges to traverse related knowledge.

Edges are **directional** — the file containing the reference is the source node, the
referenced file is the target. Bidirectional relationships require a reference in both files.

## The class contract

`<class>.ont.yml` is not documentation. It declares, per class, which properties an instance
carries and which relationships it may enter into, and `yidam lint` checks every instance
against it:

| Check | Finding | Gates |
|---|---|---|
| `undeclared-property` | a property the class never declared | yes |
| `property-type` | a value contradicting the declared `type` | yes |
| `unlicensed-edge` | a relationship the class does not declare | only under `edge_policy: exhaustive` |
| `edge-target-class` | an edge resolving to a node of the wrong class | yes |
| `missing-property` | a declared property the instance omits | only where the class says `required: true` |

The fourth is the one no other check could produce. `dangling-edge` catches an edge to
nothing; an edge to the *wrong* thing resolves, traverses, and exports, and is simply false.

**A `date` is accepted at the precision it is known to** — `YYYY`, `YYYY-MM`, or
`YYYY-MM-DD`. What `property-type` catches in a date field is prose, and `1985` is not
prose: it is the precision the fact is actually known to, and demanding a month and a day
there does not make the record more accurate, it makes it invented. `last spring` and
`[open] No date.` still fail.

Two rules keep this honest, and both are about not over-reading the ontology.

**Silence is not a contract**, read one field at a time. A class with no `properties:` has
said nothing about properties and none are checked; a class with no `edges:` has said
nothing about edges and none are licensed. Reading either silence as *and therefore none
are permitted* would flood every corpus whose ontology is not filled in — which is the
corpus with the least reason to trust its graph. The same rule decides which classes are
source classes for `orphan-in`.

### Which classes are source classes, and which end declares it

A **source class** is one the ontology says nothing points at. Its instances have no inbound
edges by design, so reporting them as orphans reports the ontology working — in one derived
repository that was 17 of 35 `orphan-in` findings.

**Both ends of the edge are read.** `gage` declaring

```yaml
edges:
  - relationship: sources-from
    target: concept
    direction: out
```

is a statement that gages point at concepts, so `concept` is a class something is meant to
point at — whether or not `concept.ont.yml` also says so from its own side. `target` is *the
class at the other end, whichever end authors the link*, and either end may author it.

This is stated because the derivation once read only a class's own list, looking for a
`direction: in` entry. That treated a class's silence about inbound edges as a positive
declaration that nothing points at it, while the ontology said elsewhere that something
does — the inverse of the over-read above. It was measured on the worked example, where all
three classes derived as source classes and `orphan-in` could not fire anywhere in the
corpus. **You do not need to declare an edge from both ends**, and declaring it twice is not
an error; it is simply not required.

Two cases the derivation deliberately leaves alone:

| Declaration | Read as |
|---|---|
| a class with no `edges:` at all | **not** a source class — silence is not a contract |
| a self-edge (`reach -downstream-of-> reach`) | says instances relate to each other, not that every instance is cited: any acyclic self-relation has an endpoint that is not, so it decides nothing either way |
| an edge with no `direction:` | exempts neither end — it says a relationship exists, not which way it runs |

**A non-empty `edges:` is not a contract either, and this is the part that was got wrong.**
Naming the relationships a class enters into says *these exist*; on its own it never said
*and no others may*. Reading it as the second put 210 errors on a derived corpus that coins
precise single-use verbs on purpose — 107 distinct relationships across 18 classes, none of
them a defect. `edge_policy:` is what makes the difference sayable:

| `edge_policy:` | An undeclared relationship is |
|---|---|
| `exhaustive` | an **error**. The class closed its vocabulary and asked for this gate. |
| `characteristic` | not reported. `edges:` names what the class is *defined by*, and a verb outside it is a deliberate coinage. |
| *absent* | a **warning**. The typo case is real and worth seeing; gating on it would enforce a contract nobody wrote. |

Declaring it is how a corpus that means `exhaustive` gets a gate it can rely on, and how one
that means `characteristic` stops being told its own vocabulary is wrong. `characteristic`
licenses an *undeclared* relationship and nothing more: where a **declared** one may land is
still `edge-target-class`'s question, and that check does not read the policy at all.

**Only edges between instances are licensed edges.** The `instance-of` link to
`../<class>.ont.yml` and a citation into `catalog/` are not relationships and no class
declares them, so the licensing checks read only links landing on another corpus instance.
A link that resolves to nothing is `dangling-edge`'s finding and is not reported twice.

### Properties every class may carry

`.yidam/corpus/universal.yml` is the corpus speaking about itself rather than about one of
its classes. It is absent from most corpora, and exists because two shapes were measured
that no per-class declaration handles:

```yaml
properties:
  - name: seeded_because
    type: text
    description: Why this node is in the corpus at all
  - pattern: '^fy\d{4}(_\d{2})?_[a-z0-9_]+$'
    type: text
    description: A fiscal year's figures, pasted onto the node they describe
```

A `name:` is apparatus that applies to every class — declaring it per class would be sixteen
copies of one decision, and a seventeenth class would silently not have it. A `pattern:` is a
self-describing family: a fiscal-year snapshot recurs, so it is not a typo, but declaring
each by name would mean editing an ontology every July to permit next year's. Between them
these were 29 of one derived corpus's 29 `undeclared-property` findings.

**Universal does not mean untyped.** `property-type` checks these exactly as it checks a
class's own, and a class declaring the same name wins — the more specific statement about
its own instances. `missing-property` never reports one, because *any class may carry it* is
not *every instance does*. Anchor a pattern: an unanchored one licenses every name that
merely contains a match, and `undeclared-property` is what catches the next real typo.

This is deliberately **not** a property-side `edge_policy`. The corpus that needed it
measured its property vocabulary at 94% declared against its relationships at 68%, and wants
the property gate; what it lacked was a way to say that two specific shapes are apparatus
rather than schema.

`missing-property` gates on what the class asked for. A property declares whether instances
must carry it:

```yaml
properties:
  - name: parameter
    type: string
    required: true          # absent means false
    description: The measured quantity, by its publisher's parameter code.
```

Omitting a `required: true` property is an **error**; omitting any other declared property is
reported and does not gate. That is the same rule its four siblings follow — they gate on
something the ontology actually *said* being contradicted — and it is why the check could not
gate before the field existed: without it, an omission contradicted nothing, and a node
carrying no `claim_tag` is a real state rather than a defect. `unlicensed-edge` sits in the
same place, gating only where the class said `exhaustive`.

**Absent means false**, and that default is load-bearing rather than timid. Every corpus
written before this field existed was written under a schema where the question could not be
asked, so defaulting to `true` would gate every class in every derived repository on a
declaration nobody made.

The declaration decides two things at once: the gate above, and the JSON Schema below, which
lists exactly the required properties as its own `required`. One statement, so the editor and
the build cannot come to disagree about which fields a node owes.

### Published, not only enforced

`yidam schema` compiles every `.ont.yml` into a JSON Schema at
`.yidam/schemas/class/<class>.json` and maps it to `.yidam/corpus/<class>/*.yml`, so an
editor validates an instance against *its own class* while you type — and so does any
validator or CI step that never links against yidam.

The compiled schema is **no stricter than the checks above**. Declared properties are typed
but never `required`, because `missing-property` does not gate. The property bag is closed,
because `undeclared-property` does — and every universal property is folded into it, by name
or as `patternProperties`, so the editor accepts exactly what the gate accepts. A schema that
stayed strict where the gate had relaxed would underline a field the build is happy with,
which is the same drift arriving from the direction nobody watches. Relationships are published for completion under
`x-yidam-edges` rather than constrained, because whether an edge is licensed depends on
where its target resolves and no schema can see that.

That symmetry is the point. A consumer that rejected what the gate accepts would fail a
build on a file that looked fine everywhere else, and the ontology would get the blame.

### Changing a class

An ontology is only as good as its ability to change, and once the contract gates, editing
a class definition in place breaks the corpus that adopted it: add a property and every
instance trips `missing-property`, retype one and they trip `property-type`, re-target an
edge and they trip `edge-target-class`. That is a strong incentive to leave a definition
wrong.

`yidam migrate` does both halves as one event:

| Command | What it touches |
|---|---|
| `migrate class <old> <new>` | the class file, its directory, every instance's `class:`, the `instance-of` edge into the class file, and every edge declaring the class at either end |
| `migrate property <class> <old> <new>` | the declaration, and the key on every instance carrying it |
| `migrate retype <class> <prop> <type>` | the declaration — and **refuses** if any instance's value would not satisfy the new type |
| `migrate edge <class> <rel> <target>` | the declaration at both ends, plus a report of the instances now in violation |

`--dry-run` prints the plan and writes nothing.

**A retype is refused rather than guessed.** `type: string` → `type: date` over a value
reading `last spring` has no mechanical conversion, and writing it back unchanged while
reporting success would leave the corpus in a state its own gate rejects. The predicate that
decides is the one `property-type` gates on, so a migration that succeeds leaves a corpus
`yidam lint` still accepts.

**An edge re-target reports what it cannot decide.** Which instances should now point
elsewhere is a question about the corpus, not about the ontology. The migration names every
one of them; it does not repoint them.

Each applied migration writes a record to `.yidam/migrations/` naming the operation, the
files it rewrote, and any violations it left behind. That is the *mechanical* half of the
event. The **argument** — why the class was wrong — belongs in `.yidam/decisions/`, and the
two are meant to be read together: a record that also had to carry the reasoning would make
`decisions-log` a list of two different kinds of thing.

## Residence time

A finding about corpus state has a level and, on its own, no clock — and the level cannot
say the thing worth knowing. A node uncited for five commits is a sweep in progress and
entirely healthy. A node uncited for two hundred is over-collection. The measurable quantity
is **how long the condition has held**, not how bad it looks.

So `orphan-in` reports both the commit its finding dates from and how long it has held:

```text
INFO [orphan-in] Node nothing points to — 2 finding(s)
  .yidam/corpus/recording/scum.yml: nothing links to this node — uncited since 2026-03-04, 3 commit(s)
  .yidam/corpus/concept/tailwater.yml: nothing links to this node — uncited since 2025-11-02, 214 commit(s)
```

**Commits, not days.** A day count is a function of when you ran the report, so the same
corpus answers differently tomorrow and nothing can pin or cache it. A commit count is a
function of `HEAD`. Only commits that touched `.yidam/corpus` are counted: a commit that
changed nothing here could not have cited anything, so counting it would inflate every age
in a repository that also holds code.

The clock starts when the condition *began*, which is not the same as when the node was
written. A node cited on the day it was authored and orphaned two hundred commits later
dates from the orphaning — which is why this is a replay of the graph rather than a look at
each file's age.

### Reachability is per class, not a corpus rate

`yidam replay` reports uncited nodes per class, against what the class declared:

```text
Uncited at HEAD, by class, against what the class declares
  recording                 13 of 20   declared cited — this is the asymmetry worth reading
  person                    12 of 12   uncited by design — the ontology holding
  note                       3 of 5    the class declares no edges, so no expectation to read against
```

Three readings, and a single corpus-wide percentage sums them into one that means none of
them. A source class at 12 of 12 is the model working. The same figure on a class declaring
an inbound edge is the only one of the three that is a finding. A class that declared no
edges is not being scored at all — and saying so beats printing nothing, which reads as a
pass.

The series keeps a percentage column because a trend needs one number per commit to be a
trend. It counts the population `orphan-in` reports, so source classes are excluded from it
— which is the difference between the 22% and the 7% the same corpus reads at, and why the
column says what it is a percentage *of*.

### Ageing into an error

Residence time is what makes a corpus-state check gate-eligible at all. The commit checks
must stay at Warn because history cannot be rewritten to fix a verb, so a gate on immutable
state could only ever be noise. Corpus state is not immutable: an orphaned node can be
linked or deleted today.

A corpus may declare how long is too long, in `.yidam/config.toml`:

```toml
[lint]
# Corpus-touching commits an aged finding may hold before it fails the build.
escalate_after = 100
```

A finding that reaches it escalates to an error and gates; its younger siblings under the
same check do not. **Absent, nothing ever escalates** — and that is the default. The right
number depends on how fast this corpus is meant to consume what it collects, so a value
compiled into the binary would be one repository's judgement arriving as a build failure in
another that never agreed to it. Declaring it here keeps the argument for the number in the
repository that has to live with it.

An escalated finding is ordinary debt: `yidam lint --bless` records it like any other, and
the gate is quiet until something new ages past the line.

## The baseline, and its own clock

`.yidam/lint-baseline.yml` records the error-severity findings that were already true when
the gate was installed, so that the next one is attributable to the commit that introduced
it. Two things about it are easy to get wrong, and both were measured going wrong.

**A repository with no baseline has no ratchet.** Not a lenient one — none. `yidam lint`
reports `no regression` on every commit, whatever the corpus does, because there is nothing
to compare against. `mise run yidam-vendor-update` therefore runs `yidam lint
--init-baseline`, which writes a baseline if and only if there is not one already. It is
safe to run at any time and leaves an existing file untouched.

**A baseline is a scheduled repayment, not a permanent exemption.** Each entry records
`since` — the corpus commit at which it was *first* accepted — and the file may declare how
long an entry stands:

```yaml
expire_after: 200        # corpus-touching commits an entry may survive
violations:
  dangling-edge:
    - node: .yidam/corpus/concept/tailwater.yml
      detail: 'target does not exist: ../concept/gone.yml'
      since: 4f2a1c9…
```

Past that, the entry stops forgiving and the violation gates again. **Blessing does not
reset the clock** — `since` is carried forward, so re-running `--bless` records new debt
without forgiving old debt a second time. The two ways out of an expired entry are to fix
the finding, or to raise `expire_after` and say in the commit message why this corpus needs
longer than it said it did. That argument then lives in the repository, in a diff somebody
reviews.

Absent, entries never expire, which is where every baseline starts.

### Two clocks, two questions

They are named apart because they are different questions:

| Declaration | Where | Counts | Asks |
|---|---|---|---|
| `escalate_after` | `.yidam/config.toml` | commits | how long a **finding** may hold before it becomes an error |
| `expire_after` | `.yidam/lint-baseline.yml` | commits | how long an **accepted entry** may stand before it gates again |
| `ttl_days` | a catalog entry, or `[catalog]` in `.yidam/config.toml` | **days** | how long a **source record** may stand before it is worth looking at again |

The first is about the corpus; the second is about the file that forgives it.

The third counts days and the other two count commits, which is a deliberate departure rather
than an oversight. The commits rule is about *corpus state*: how long a node has gone uncited
is a fact about this repository, so the repository's clock is the honest one. A source's
staleness is not a fact about this repository — a statute does not become stale because you
committed, and a gauge record does not stay fresh because you did not. So that one report does
answer differently tomorrow, and that is what a TTL is for. A finding can
escalate under the first, be blessed, and later expire under the second — and each step is
a different thing having happened.

## Commits as events

Not all commits carry the same kind of meaning. Two types coexist in every yidam-derived
repository:

**Epistemic commits** add or revise understanding. They are the primary knowledge events of
the graph: authored nodes, synthesis, assessment, open questions resolved or opened. Write
these in the active voice of inquiry:
> `establish: confounding variable framework — links to identification and intervention`
> `revise: identification conditions — updated after reviewing Pearl 2009`

**Operational commits** advance the corpus through pipeline work: data extraction, connector
refreshes, bundle generation, catalog reconciliation. They are legitimate provenance records
but are not epistemic events. Write these by naming the pipeline step and its output:
> `extract: NPDES permit fields for site X — 14 structured values from document Y`
> `refresh: ECHO inventory — 3 new dischargers added since last pull`

Both types appear in the git log and both are part of the graph's history. Keeping them
distinct is what preserves the log's readability as a knowledge record — and "distinct by
style" is not enough to do it. A reader can eyeball the difference; a tool cannot, and
neither can a reader six months and four hundred commits later. The distinction is carried
by a **closed vocabulary of leading verbs**.

## Commit vocabulary

Every commit's subject line begins `<verb>: `. The verb determines the commit's type. This
list is closed: `yidam lint --commits` reports any verb outside it.

**The verb stands alone — no conventional-commits `(scope)` suffix.** Everything before the
first `: ` is the verb, so `vendor(yidam): …` is read as the verb `vendor(yidam)`, which is
in no list. That costs twice: the lint reports it as outside the vocabulary, and
classification falls through to Epistemic, silently filing an operational commit as a change
in understanding. Put the scope in the subject instead — `vendor: yidam prelude into …`.

**Epistemic** — understanding was added, revised, or retracted:

| Verb | When |
|---|---|
| `establish` | New understanding committed — a node authored |
| `revise` | Committed understanding corrected |
| `assess` | Hypotheses weighed against evidence (an Assessment phase) |
| `scope` | A sweep widened or bounded — names what the wider net caught |
| `synthesize` | Nodes linked or merged across inquiry threads (a Synthesis phase) |
| `withdraw` | A claim retracted — say what replaces it, or that nothing does |
| `open` | A question opened |
| `close` | A question resolved |
| `transport` | An elector's position carried onto the baseline, verbatim — *collective mode only* |
| `resolve` | A resolution event settled — *collective mode only* |
| `adopt` | A settled baseline taken into an elector's position — *collective mode only* |
| `decide` | A choice recorded in `.yidam/decisions/` |
| `phase` | A phase settled — names the phase and what it produced |
| `genesis` | The root commit of an empty-repo bootstrap |
| `overlay` | The root graph commit of an existing-repo bootstrap |

`scope` is the verb for the act that precedes a finding: the search was widened from one
instrument to every instrument on the thread, from one member to the whole commission, and
the commit reports what the wider net caught. It is distinct from `establish`, which authors
a node, and from `assess`, which weighs hypotheses already held. A sweep that finds nothing
is still a `scope` commit and still worth writing — a negative result about coverage is the
only durable record that the coverage was checked.

`transport`, `resolve` and `adopt` are the three acts of collective mode, and the
vocabulary had none of them until derived repositories needed them: the constitution makes
resolution events first-class and this document gives them a ref namespace, but nothing
named the commits that perform one. They occur in that order.

`transport` carries an elector's position file from their `ma/<elector>` branch onto the
baseline, **unmodified**, so that the other electors and the eventual resolution can read
it. It is carriage and not synthesis, which is what makes it legal outside a resolution
event: Article V confines synthesis to resolutions, and copying a file verbatim introduces
no node, edge or claim that its author did not hold. That constraint is the whole of the
verb — a `transport` commit that edits the position it carries is a resolution performed in
the wrong place, by one elector, having read nobody.

Without it a position is committed to a branch nobody else is on. A derived repository ran
twenty resolutions that way and found what it costs: four corpus nodes citing position
files that resolved for their author and for nobody else, and two resolutions standing on
the baseline whose arguments were not. It coined this verb itself, used it twenty-six
times, and never wrote it down anywhere but its commit bodies.

`resolve` produces the `rigpa/<evolution>` tip. `adopt` is each elector taking that settled
baseline back into their own `ma/<elector>` position afterwards — the routine follow-on,
which happens once per elector per resolution and is the single most repeated act in a
collective repository. None of them is `consume`, which means one thing only: a transient
bootstrap layer consumed at genesis.

**Operational** — the pipeline advanced; no understanding changed:

| Verb | When |
|---|---|
| `extract` | Structured data pulled from a primary source (an Extraction phase) |
| `refresh` | A connector re-run against its source |
| `compute` | A calculator run and its output committed |
| `index` | The vector index rebuilt |
| `bundle` | The export bundle regenerated |
| `reconcile` | Catalog and corpus brought back into agreement |
| `regen` | REGEN blocks refreshed |
| `build` | The domain computer built or changed |
| `implement` | A connector or calculator implemented |
| `scaffold` | Structure created |
| `catalog` | A source anchor added to `.yidam/catalog/` |
| `migrate` | Data or schema moved |
| `fix` | A defect corrected |
| `vendor` | The prelude re-vendored |
| `consume` | A transient bootstrap layer consumed |

### A tool may write three of these verbs

`yidam propose` drafts `open`, `withdraw` and `close` commits onto a `propose/<head>` branch,
and nothing else in this list. It is the only tool that writes an epistemic commit, and what
licenses it is `transport`'s licence one paragraph up: **carriage is not synthesis.** A
proposal records a question a finding already phrased, withdraws a node this corpus declared
over-collected in its own `.yidam/config.toml`, or retires a question `propose` itself opened
once the finding is gone. Each asserts only what was already asserted.

What it may never do follows from the same rule, and the boundary is worth stating because it
is easy to cross by accident: **no `establish`** (authoring a node), **no `revise`**
(retagging a claim, or splitting one node into two), **no `synthesize`**, **no `resolve`**.
Drawing an edge asserts a relationship; retagging asserts a standing; splitting asserts two
nodes and a partition of claims. None of those is in any finding.

**Nothing merges itself.** The branch is reviewed as commits and rejected by deleting it. See
[RFC-0020](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0020-proposal-surface.md) for the argument, including why three of
the four acts originally proposed for it are not here.

**Why closed.** An open vocabulary decays into one verb per commit. A repository derived
from this template ran a hundred commits with roughly sixty distinct leading words — `lift`,
`grain`, `void`, `worst`, `viewport` — each individually evocative and collectively useless:
nothing could recover which commits changed what was known and which merely moved bytes.
A verb outside this list is not a richer description, it is an unclassifiable one.

Reach for the closest verb rather than inventing one. If a commit genuinely does not fit —
that is a gap in the vocabulary, which is an yidam-level change, not a local one.

**A step that produces a commit names its verb, in the step.** Any instruction — in a
protocol, a skill, a convention, a README — that tells a reader to commit something must
say which verb, beside the instruction rather than a document away. This is the only
mechanism in the system that acts *before* the commit exists. `yidam lint --commits` is
Warn severity and correctly so, since history cannot be rewritten to fix a verb; that also
means it reports drift only after the drift is permanent. A derived repository put four
consecutive resolution commits on the wrong verb and the finding sat in a warning nobody
read. Its own remedy was to record the verb at the step that writes the commit, which is
the only place the next one can be caught.

**Naming the *kind* is not naming the verb.** "Commit this as an epistemic commit" leaves
the reader to pick one, and the kind is derived from the verb rather than the other way
around — `classify_commit` takes the verb and returns the kind. This template prescribed
four commits that way, two of them in consecutive sentences, and every one had an obvious
right verb nobody had written down. `no_step_names_a_commit_kind_instead_of_its_verb`
gates that shape; it cannot see a step that says "commit this" with no qualification at
all, and no check can.

**Merge commits.** A merge whose subject git wrote — `Merge branch 'phase/outcome-axis'` —
is exempt: nobody chose that verb, so nothing can be said about the choice. But a merge *is*
a synthesis event, and git's default subject says only which ref was read, not what joining
it meant. Prefer to write one:

```
git merge --no-ff -m "phase: the local half — 11 nodes, two questions closed" phase/the-local-half
```

A merge subject that carries a verb is checked like any other commit. Exemption is detected
by parent count rather than by subject text, so every form git generates is covered —
including `Merge <ref>`, which the prefix test used to miss and which is what a derived
repository produced ten times.

Every commit of either type should answer:
- What changed?
- Why? (What prompted this — what question, what source, what finding?)
- What does it connect to?

Commit messages are part of the graph. They record the provenance of every node.

## Branches as inquiry

Open a branch to explore a speculative direction. The branch represents an inquiry thread
that may or may not be merged into the main graph. Merging is synthesis; abandoning a branch
is a deliberate choice to exclude that thread. Both are valid knowledge acts.

## Collective resolution

*Applies only to repositories bootstrapped as `governance: collective`. Skip this section
if you are the sole elector.*

When multiple participants maintain the graph, two ref namespaces encode their relationship:

`refs/heads/ma/<elector>` branches are individual positions — each elector (human or agent)
commits their working understanding here freely, without requiring consensus. Positions are
expected to diverge.

`refs/heads/rigpa/<evolution>` branches are settled evolutions — points where individual
positions have been synthesized into shared understanding. A resolution event reads all
`ma/*` tips, identifies agreement and tension, and produces a new rigpa branch as a named
collective baseline. Elector branches diverge again from there.

These two namespaces hold each elector's **corpus**. They do not hold their **argument** —
why they hold what they hold, what they conceded, which of their own earlier grounds they
withdrew — and a resolution turns on the argument. That goes in `sangha/positions/`, as a
file, before the resolution reads it; once the resolution merges, an unwritten argument is
gone into the merge base.

See [sangha/](https://github.com/goedelsoup/yidam/blob/main/sadhana/sangha/README.md) for
the full resolution protocol. The link is absolute because the directory is **conditional**:
bootstrap creates `.yidam/sangha/` only under `governance: collective`, and single-elector is
the default — the case the skill tells the agent to take when the user is unsure. A relative
`../../sangha/README.md` is correct in a collective repository and points at nothing in every
other one, where it becomes a permanent `unauthored-prose-link` the reader cannot act on.
