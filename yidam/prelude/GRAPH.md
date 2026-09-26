# Knowledge Graph Model

This repository's knowledge graph lives in git. No external store is required.

**How to read this file.** Every rule below is a sentence you can act on. The incident that
produced a rule, the measurement that set its threshold, and the failure it was built against
are in [GRAPH.evidence.md](GRAPH.evidence.md) — one section per rule, reached by the `[why]`
link beside it. Read it when a rule surprises you, when you are about to argue with one, or
when you need to know how far a number can be pushed. You do not need it in order to comply,
and it is not part of the recurring read.

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

A `propose/` branch is named after the commit it was computed against, not dated.
[why](GRAPH.evidence.md#propose-namespace)

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

### An open question is marked in its label

**A node whose `label` begins with `?` is a question the corpus has not closed:**

```yaml
class: reach
label: "? Whether the 1987 gage relocation broke the rating curve"
```

**Quote the label.** A bare `?` followed by a space is YAML's explicit-key indicator and does
not parse. [why](GRAPH.evidence.md#quote-the-question-label)

The marker is on the label and not the path — the filename stays a slug, and the `?` is what
a reader sees in every index, link and report the label renders into.

**Nothing gates on it.** No lint reports its absence, no class may require it, and a corpus
that never writes a `?` is well-formed. A corpus that keeps its open questions another way is
not thereby wrong. [why](GRAPH.evidence.md#open-question-marker-gates-nothing)

**It is not the `[open]` evidence tag.** `[open]` is the standing of a claim *inside* a node —
this sentence is unsettled — and it says nothing about what the node is. The tag says a claim
is unsettled; the `?` says the node **is** the question.
[why](GRAPH.evidence.md#question-node-versus-open-claim)

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

`edge-target-class` is the one no other check could produce: `dangling-edge` catches an edge to
nothing, and an edge to the *wrong* thing resolves, traverses, and exports, and is simply false.

**A `date` is accepted at the precision it is known to** — `YYYY`, `YYYY-MM`, or
`YYYY-MM-DD`. `last spring` and `[open] No date.` still fail.
[why](GRAPH.evidence.md#date-precision)

**A `number` is a YAML number, unquoted**, and its unit is declared on the class as
`unit: km`, never in the value. `"24"` is text and fails.
[why](GRAPH.evidence.md#number-unquoted)

**Silence is not a contract**, read one field at a time. A class with no `properties:` has
said nothing about properties and none are checked; a class with no `edges:` has said nothing
about edges and none are licensed. The same rule decides which classes are source classes for
`orphan-in`. [why](GRAPH.evidence.md#silence-is-not-a-contract)

### Which classes are source classes, and which end declares it

A **source class** is one the ontology says nothing points at. Its instances have no inbound
edges by design, so reporting them as orphans reports the ontology working.

**Both ends of the edge are read.** `gage` declaring

```yaml
edges:
  - relationship: sources-from
    target: concept
    direction: out
```

is a statement that gages point at concepts, so `concept` is a class something is meant to
point at — whether or not `concept.ont.yml` also says so from its own side. `target` is *the
class at the other end, whichever end authors the link*, and either end may author it. **You do
not need to declare an edge from both ends**, and declaring it twice is not an error.
[why](GRAPH.evidence.md#both-ends-are-read)

Two cases the derivation deliberately leaves alone:

| Declaration | Read as |
|---|---|
| a class with no `edges:` at all | **not** a source class — silence is not a contract |
| a self-edge (`reach -downstream-of-> reach`) | says instances relate to each other, not that every instance is cited: any acyclic self-relation has an endpoint that is not, so it decides nothing either way |
| an edge with no `direction:` | exempts neither end — it says a relationship exists, not which way it runs |

**A non-empty `edges:` is not a contract either.** Naming the relationships a class enters
into says *these exist*; on its own it never said *and no others may*. `edge_policy:` is what
makes the difference sayable:

| `edge_policy:` | An undeclared relationship is |
|---|---|
| `exhaustive` | an **error**. The class closed its vocabulary and asked for this gate. |
| `characteristic` | not reported. `edges:` names what the class is *defined by*, and a verb outside it is a deliberate coinage. |
| *absent* | a **warning**. The typo case is real and worth seeing; gating on it would enforce a contract nobody wrote. |

`characteristic` licenses an *undeclared* relationship and nothing more: where a **declared**
one may land is still `edge-target-class`'s question, and that check does not read the policy
at all. [why](GRAPH.evidence.md#edge-policy)

**Only edges between instances are licensed edges.** The `instance-of` link to
`../<class>.ont.yml` and a citation into `catalog/` are not relationships and no class
declares them, so the licensing checks read only links landing on another corpus instance. A
link that resolves to nothing is `dangling-edge`'s finding and is not reported twice.

### What an edge rests on

Everything above asks whether an edge is *well-formed*. None of it asks whether it is **true**,
and `guidelines/agent-conduct.md` is explicit that an edge is a claim written as structure.
A link may say so:

```yaml
links:
  - target: ../place/ward-nine.yml
    relationship: resided-in
    claim_tag: inference
  - target: ../place/city-hall.yml
    relationship: worked-at
    claim_tag: verified
    source: 1889-municipal-register
```

`claim_tag` is graded by the same reader that grades a `type: claim` property, so `verified`,
`[verified]` and `[verified — as proposed]` all read, and so does a list of standings.

**An edge's tag is reported beside the node's, never added to it.** `yidam status` and
`yidam corpus-index` count tagged edges in their own figure, and both render it only where a
corpus tags edges. An edge tagged `open` is listed by `yidam open-questions` in its own right,
addressed by its triple, rather than promoting the node that authors it. The MCP `claims` and
`open_questions` tools answer the same way. A node's own counters skip the `links:` block.
[why](GRAPH.evidence.md#edge-tags-reported-beside)

| Check | Finding | Gates |
|---|---|---|
| `edge-verified-unsourced` | an edge asserting `verified` and naming no `source:` | no — Warn |
| `edge-untagged` | an empirical edge declaring no standing, or one spelling none | no — Warn, and only where the corpus asked |
| `edge-standing-unheld` | an edge asserting a standing **stronger** than one its endpoints declare | no — Warn |

The first needs no declaration: writing `claim_tag: verified` is itself the opt-in. The second
cannot be, so it runs only where the corpus says so, once, for the whole graph:

```yaml
# .yidam/corpus/universal.yml
edge_claims:
  required: true
  structural:
    - instance-of
    - concerns
    - subject-of
```

`structural:` names the relationships that are **bookkeeping**: they file a node rather than
assert something about the world, and a standing on one of them would grade nothing.

**The two keys are separate deliberately.** Recording which verbs are bookkeeping is a fact
about a vocabulary; `required: true` is a request for a gate over every other edge in the
corpus, and one must not arrive as a side effect of the other.
[why](GRAPH.evidence.md#edge-claims-two-keys)

`edge-standing-unheld` reads an edge's standing against the ones its **own endpoints** declare.
A node's standing here is a property its class declared `type: claim` — the node's own grade,
not the weakest marker in its prose. A node that declares no such property has no standing and
is compared to nothing. The end an edge is measured against is the **weaker** of the two that
declare one, and the edge's own standing is the **strongest** it spells. It never gates.
[why](GRAPH.evidence.md#edge-standing-unheld)

### Which keys hold prose

A class says which of its top-level keys carry prose, and the corpus may say it once for
every class:

```yaml
# <class>.ont.yml — a key this class's instances carry
prose: [findings]
```

```yaml
# universal.yml — apparatus every class may carry
prose: [summary, findings]
```

The effective set is `{description} ∪ universal ∪ class`.

**Union rather than override.** Universal *properties* let a class win, because two
declarations of one property's `type` contradict each other. Two declarations that a key holds
prose agree, so there is nothing for the more specific one to win.
[why](GRAPH.evidence.md#prose-keys)

### Prose in a property

A property declaration says whether its value is prose:

```yaml
# <class>.ont.yml
properties:
  - name: method
    type: string
    prose: true
    description: How the figure was computed.
```

**Absent means false**, for the reason `required:` absent means false: a corpus written before
the field existed never had the chance to say. It is per class — there is no universal
property, because a property belongs to the class that declared it.

**It is a flag beside `type:`, not a type.** Prose-ness is orthogonal to what a value *is*:
`method` and `identifier` are both strings. A `type: prose` would also change the compiled
class schema, which three SDKs are held to.

**Flagging one changes what reports measure, so re-run `yidam index-build`.** `node-too-long`
counts those lines, `missing-description` stops reporting a node whose whole substance is a
transcription in a property, and `yidam embed` composes it.
[why](GRAPH.evidence.md#prose-in-a-property)

### Retrievable without being prose

Retrievability is declared on its own:

```yaml
# <class>.ont.yml
properties:
  - name: parameter
    type: string
    retrievable: true
    description: The measured quantity, by its publisher's parameter code.
```

**Identifiers, codes and units are the part of a corpus most likely to be typed verbatim into
a query**, and `prose: true` is the wrong way to reach them.
[why](GRAPH.evidence.md#retrievable-not-prose)

**Prose is already retrievable.** `embed` composes every declared prose field, so flagging a
`prose: true` property `retrievable` as well is redundant rather than wrong. The implication
runs one way only: a retrievable identifier is not thereby prose.

**`embed` is the only reader.** Flagging a property changes what is retrievable and changes no
report — so **re-run `yidam index-build`**. **Absent means false**, so a class that flags
nothing emits byte-identical embeddings to a corpus written before the field existed.

**`description` is always in the set**, including for a class that declares `prose:` and omits
it. Silence is not a contract, here as everywhere: naming `findings` says findings is prose, and
never said *and description is not* — reading it as the second would let one added line silently
stop measuring the field every corpus writes.

Absent both declarations the set is `[description]`, which is every corpus written before this
existed — nothing changes for them.

| Reads the declared set | What changes |
|---|---|
| `node-too-long` | counts every declared prose field; the finding says `lines of prose`, not of `description` |
| `missing-description` | reports a node with prose in **no** declared field. A node carrying a `summary` and no `description` has said something |
| `yidam embed` | embeds all the prose. A node whose substance is in `summary` was retrievable by its title and by nothing it says |

The claim counter is unchanged: `count_in_node` and `has_open_claim` always read the whole
file. [why](GRAPH.evidence.md#claim-counter-unchanged)

`description` is **not** in the node schema's `required` list, because the check no longer asks
for that key by name. A schema demanding it would reject, in the editor, a node the build
accepts.

### A question this tool carried

`yidam propose` opens a question by recording the finding on the node it is about:

```yaml
yidam:
  findings:
    - id: 7f3a1c94b2e1
      check: orphan-in
      opened_at: 4f2a1c9
      detail: 'nothing links to this node — uncited since 2026-03-04, 3 commit(s)'
      standing: open
```

The `id` is a digest of the check and the finding's own words, so the same finding computes
the same id at any commit.

**Under `yidam:` and not at the top level**, because the obvious key is taken. The tool claims
one key, named after itself, and the corpus keeps the rest of the top level.

**A carried question is not a claim the corpus makes.** `open-questions` and the claim counts
report what the corpus asserts, and a question this tool carried is not that. It is still
reported — by `yidam lint`, which is where it came from.
[why](GRAPH.evidence.md#carried-findings)

Paragraphs written by an earlier release are still read and still closed. Nothing strands.

### Properties every class may carry

`.yidam/corpus/universal.yml` is the corpus speaking about itself rather than about one of its
classes. It is absent from most corpora:

```yaml
properties:
  - name: seeded_because
    type: text
    description: Why this node is in the corpus at all
  - pattern: '^fy\d{4}(_\d{2})?_[a-z0-9_]+$'
    type: text
    description: A fiscal year's figures, pasted onto the node they describe
```

A `name:` is apparatus that applies to every class. A `pattern:` is a self-describing family.
[why](GRAPH.evidence.md#universal-properties)

**Universal does not mean untyped.** `property-type` checks these exactly as it checks a
class's own, and a class declaring the same name wins — the more specific statement about its
own instances. `missing-property` never reports one, because *any class may carry it* is not
*every instance does*. **Anchor a pattern**: an unanchored one licenses every name that merely
contains a match.

This is deliberately **not** a property-side `edge_policy`.
[why](GRAPH.evidence.md#not-a-property-side-edge-policy)

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
something the ontology actually *said* being contradicted. `unlicensed-edge` sits in the same
place, gating only where the class said `exhaustive`.

**Absent means false**, and that default is load-bearing rather than timid.
[why](GRAPH.evidence.md#required-absent-means-false)

The declaration decides two things at once: the gate above, and the JSON Schema below, which
lists exactly the required properties as its own `required`. One statement, so the editor and
the build cannot come to disagree about which fields a node owes.

### Published, not only enforced

`yidam schema` compiles every `.ont.yml` into a JSON Schema at
`.yidam/schemas/class/<class>.json` and maps it to `.yidam/corpus/<class>/*.yml`, so an editor
validates an instance against *its own class* while you type — and so does any validator or CI
step that never links against yidam.

The compiled schema is **no stricter than the checks above**. Declared properties are typed but
never `required`, because `missing-property` does not gate. The property bag is closed, because
`undeclared-property` does — and every universal property is folded into it, by name or as
`patternProperties`, so the editor accepts exactly what the gate accepts. Relationships are
published for completion under `x-yidam-edges` rather than constrained, because whether an
edge is licensed depends on where its target resolves and no schema can see that.
[why](GRAPH.evidence.md#schema-no-stricter)

### Changing a class

`yidam migrate` does both halves of a class change as one event:

| Command | What it touches |
|---|---|
| `migrate class <old> <new>` | the class file, its directory, every instance's `class:`, the `instance-of` edge into the class file, and every edge declaring the class at either end |
| `migrate property <class> <old> <new>` | the declaration, and the key on every instance carrying it |
| `migrate retype <class> <prop> <type>` | the declaration — and **refuses** if any instance's value would not satisfy the new type |
| `migrate edge <class> <rel> <target>` | the declaration at both ends, plus a report of the instances now in violation |

`--dry-run` prints the plan and writes nothing.

**A retype is refused rather than guessed.** The predicate that decides is the one
`property-type` gates on, so a migration that succeeds leaves a corpus `yidam lint` still
accepts.

**An edge re-target reports what it cannot decide.** Which instances should now point elsewhere
is a question about the corpus, not about the ontology. The migration names every one of them;
it does not repoint them. [why](GRAPH.evidence.md#changing-a-class)

Each applied migration writes a record to `.yidam/migrations/` naming the operation, the files
it rewrote, and any violations it left behind. That is the *mechanical* half of the event. The
**argument** — why the class was wrong — belongs in `.yidam/decisions/`, and the two are meant
to be read together.

## Residence time

A finding about corpus state has a level and, on its own, no clock. The measurable quantity is
**how long the condition has held**, not how bad it looks.

So `orphan-in` reports both the commit its finding dates from and how long it has held:

```text
INFO [orphan-in] Node nothing points to — 2 finding(s)
  .yidam/corpus/recording/scum.yml: nothing links to this node — uncited since 2026-03-04, 3 commit(s)
  .yidam/corpus/concept/tailwater.yml: nothing links to this node — uncited since 2025-11-02, 214 commit(s)
```

**Commits, not days.** Only commits that touched `.yidam/corpus` are counted.
[why](GRAPH.evidence.md#commits-not-days)

**The clock starts when the condition *began***, which is not the same as when the node was
written — which is why this is a replay of the graph rather than a look at each file's age.

### Reachability is per class, not a corpus rate

`yidam replay` reports uncited nodes per class, against what the class declared:

```text
Uncited at HEAD, by class, against what the class declares
  recording                 13 of 20   declared cited — this is the asymmetry worth reading
  person                    12 of 12   uncited by design — the ontology holding
  note                       3 of 5    the class declares no edges, so no expectation to read against
```

Three readings, and a single corpus-wide percentage sums them into one that means none of them.
[why](GRAPH.evidence.md#reachability-per-class)

The series keeps a percentage column because a trend needs one number per commit to be a trend.
It counts the population `orphan-in` reports, so source classes are excluded from it — and the
column says what it is a percentage *of*.

### Ageing into an error

Residence time is what makes a corpus-state check gate-eligible at all. The commit checks must
stay at Warn because history cannot be rewritten to fix a verb. Corpus state is not immutable:
an orphaned node can be linked or deleted today.

A corpus may declare how long is too long, in `.yidam/config.toml`:

```toml
[lint]
# Corpus-touching commits an aged finding may hold before it fails the build.
escalate_after = 100
```

A finding that reaches it escalates to an error and gates; its younger siblings under the same
check do not. **Absent, nothing ever escalates** — and that is the default.
[why](GRAPH.evidence.md#escalate-after)

An escalated finding is ordinary debt: `yidam lint --bless` records it like any other, and the
gate is quiet until something new ages past the line.

**Ask the tool which checks it reaches**, rather than this document, which would go stale:

```
$ yidam lint --format json | jq -r '.checks[] | select(.escalation_eligible) | .id'
orphan-in
```

Every check that ran is in that array, including the ones that found nothing.
[why](GRAPH.evidence.md#which-checks-escalate)

## The baseline, and its own clock

`.yidam/lint-baseline.yml` records the error-severity findings that were already true when the
gate was installed, so that the next one is attributable to the commit that introduced it.

**A repository with no baseline has no ratchet.** Not a lenient one — none: `yidam lint`
reports `no regression` on every commit, whatever the corpus does, because there is nothing to
compare against. `mise run yidam-vendor-update` therefore runs `yidam lint --init-baseline`,
which writes a baseline if and only if there is not one already. It is safe to run at any time
and leaves an existing file untouched.

**A baseline is a scheduled repayment, not a permanent exemption.** Each entry records `since`
— the corpus commit at which it was *first* accepted — and the file may declare how long an
entry stands:

```yaml
expire_after: 200        # corpus-touching commits an entry may survive
violations:
  dangling-edge:
    - node: .yidam/corpus/concept/tailwater.yml
      detail: 'target does not exist: ../concept/gone.yml'
      since: 4f2a1c9…
```

Past that, the entry stops forgiving and the violation gates again. **Blessing does not reset
the clock** — `since` is carried forward. The two ways out of an expired entry are to fix the
finding, or to raise `expire_after` and say in the commit message why this corpus needs longer
than it said it did. [why](GRAPH.evidence.md#baseline-expiry)

Absent, entries never expire, which is where every baseline starts.

### Two clocks, two questions

| Declaration | Where | Counts | Asks |
|---|---|---|---|
| `escalate_after` | `.yidam/config.toml` | commits | how long a **dated finding** may hold before it becomes an error |
| `expire_after` | `.yidam/lint-baseline.yml` | commits | how long an **accepted entry** may stand before it gates again |
| `ttl_days` | a catalog entry, or `[catalog]` in `.yidam/config.toml` | **days** | how long a **source record** may stand before it is worth looking at again |

The first is about the corpus; the second is about the file that forgives it. The third counts
days and the other two count commits, which is a deliberate departure.
[why](GRAPH.evidence.md#two-clocks)

A finding can escalate under the first, be blessed, and later expire under the second — and
each step is a different thing having happened.

## Commits as events

Not all commits carry the same kind of meaning. Two types coexist in every yidam-derived
repository:

**Epistemic commits** add or revise understanding. They are the primary knowledge events of the
graph: authored nodes, synthesis, assessment, open questions resolved or opened. Write these in
the active voice of inquiry:
> `establish: confounding variable framework — links to identification and intervention`
> `revise: identification conditions — updated after reviewing Pearl 2009`

**Operational commits** advance the corpus through pipeline work: data extraction, connector
refreshes, bundle generation, catalog reconciliation. They are legitimate provenance records
but are not epistemic events. Write these by naming the pipeline step and its output:
> `extract: NPDES permit fields for site X — 14 structured values from document Y`
> `refresh: ECHO inventory — 3 new dischargers added since last pull`

The distinction is carried by a **closed vocabulary of leading verbs**, not by style.
[why](GRAPH.evidence.md#distinct-by-style-is-not-enough)

## Commit vocabulary

Every commit's subject line begins `<verb>: `. The verb determines the commit's type. This list
is closed: `yidam lint --commits` reports any verb outside it.

**The verb stands alone — no conventional-commits `(scope)` suffix.** Everything before the
first `: ` is the verb, so `vendor(yidam): …` is read as the verb `vendor(yidam)`, which is in
no list. Put the scope in the subject instead — `vendor: yidam prelude into …`.
[why](GRAPH.evidence.md#the-verb-stands-alone)

**Epistemic** — understanding was added, revised, or retracted:

| Verb | When |
|---|---|
| `establish` | New understanding committed — a node authored |
| `revise` | Committed understanding corrected |
| `assess` | Hypotheses weighed against evidence (an Assessment phase) |
| `scope` | A sweep widened or bounded — names what the wider net caught |
| `synthesize` | Nodes linked or merged across inquiry threads (a Synthesis phase) |
| `withdraw` | A claim retracted — say what replaces it, or that nothing does |
| `open` | An elector's position opened, or a question put in play — the two are not the same act |
| `close` | A question resolved |
| `transport` | An elector's position carried onto the baseline, verbatim — *collective mode only* |
| `resolve` | A resolution event settled — *collective mode only* |
| `adopt` | A settled baseline taken into an elector's position — *collective mode only* |
| `decide` | A choice recorded in `.yidam/decisions/` |
| `phase` | A phase settled — names the phase and what it produced |
| `genesis` | The root commit of an empty-repo bootstrap |
| `overlay` | The root graph commit of an existing-repo bootstrap |

`open` covers two acts and the second is not a slip: opening a *position* — the first move of a
deliberation — and putting a question in play. `close` answers for either.
[why](GRAPH.evidence.md#the-open-verb)

`scope` is the verb for the act that precedes a finding: the search was widened from one
instrument to every instrument on the thread, and the commit reports what the wider net caught.
It is distinct from `establish`, which authors a node, and from `assess`, which weighs
hypotheses already held. **A sweep that finds nothing is still a `scope` commit and still worth
writing** — a negative result about coverage is the only durable record that the coverage was
checked.

`transport`, `resolve` and `adopt` are the three acts of collective mode, and they occur in
that order.

`transport` carries an elector's position file from their `ma/<elector>` branch onto the
baseline, **unmodified**, so that the other electors and the eventual resolution can read it.
It is carriage and not synthesis, which is what makes it legal outside a resolution event. **A
`transport` commit that edits the position it carries is a resolution performed in the wrong
place**, by one elector, having read nobody.
[why](GRAPH.evidence.md#the-collective-verbs)

`resolve` produces the `rigpa/<evolution>` tip. `adopt` is each elector taking that settled
baseline back into their own `ma/<elector>` position afterwards. None of them is `consume`,
which means one thing only: a transient bootstrap layer consumed at genesis.

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
and nothing else in this list. What licenses it is `transport`'s licence: **carriage is not
synthesis.** Each asserts only what was already asserted.

What it may never do follows from the same rule: **no `establish`** (authoring a node), **no
`revise`** (retagging a claim, or splitting one node into two), **no `synthesize`**, **no
`resolve`**. Drawing an edge asserts a relationship; retagging asserts a standing; splitting
asserts two nodes and a partition of claims. None of those is in any finding.

**Nothing merges itself.** The branch is reviewed as commits and rejected by deleting it. See
[RFC-0020](https://github.com/goedelsoup/yidam/blob/main/docs/rfcs/0020-proposal-surface.md)
for the argument. [why](GRAPH.evidence.md#a-tool-writes-three-verbs)

**Why closed.** A verb outside this list is not a richer description, it is an unclassifiable
one. [why](GRAPH.evidence.md#why-the-vocabulary-is-closed)

Reach for the closest verb rather than inventing one. If a commit genuinely does not fit — that
is a gap in the vocabulary, which is an [yidam-level change](guidelines/upstream.md), not a
local one.

**A step that produces a commit names its verb, in the step.** Any instruction — in a protocol,
a skill, a convention, a README — that tells a reader to commit something must say which verb,
beside the instruction rather than a document away. This is the only mechanism in the system
that acts *before* the commit exists. [why](GRAPH.evidence.md#a-step-names-its-verb)

**Naming the *kind* is not naming the verb.** "Commit this as an epistemic commit" leaves the
reader to pick one, and the kind is derived from the verb rather than the other way around.
`no_step_names_a_commit_kind_instead_of_its_verb` gates that shape.
[why](GRAPH.evidence.md#naming-the-kind-is-not-the-verb)

**Merge commits.** A merge whose subject git wrote — `Merge branch 'phase/outcome-axis'` — is
exempt: nobody chose that verb, so nothing can be said about the choice. But a merge *is* a
synthesis event, and git's default subject says only which ref was read, not what joining it
meant. Prefer to write one:

```
git merge --no-ff -m "phase: the local half — 11 nodes, two questions closed" phase/the-local-half
```

A merge subject that carries a verb is checked like any other commit. Exemption is detected by
parent count rather than by subject text. [why](GRAPH.evidence.md#merge-commits)

Every commit of either type should answer:
- What changed?
- Why? (What prompted this — what question, what source, what finding?)
- What does it connect to?

Commit messages are part of the graph. They record the provenance of every node.

## Branches as inquiry

Open a branch to explore a speculative direction. The branch represents an inquiry thread that
may or may not be merged into the main graph. Merging is synthesis; abandoning a branch is a
deliberate choice to exclude that thread. Both are valid knowledge acts.

## Collective resolution

*Applies only to repositories bootstrapped as `governance: collective`. Skip this section if
you are the sole elector.*

When multiple participants maintain the graph, two ref namespaces encode their relationship:

`refs/heads/ma/<elector>` branches are individual positions — each elector (human or agent)
commits their working understanding here freely, without requiring consensus. Positions are
expected to diverge.

`refs/heads/rigpa/<evolution>` branches are settled evolutions — points where individual
positions have been synthesized into shared understanding. A resolution event reads all `ma/*`
tips, identifies agreement and tension, and produces a new rigpa branch as a named collective
baseline. Elector branches diverge again from there.

These two namespaces hold each elector's **corpus**. They do not hold their **argument** — why
they hold what they hold, what they conceded, which of their own earlier grounds they withdrew
— and a resolution turns on the argument. **That goes in `sangha/positions/`, as a file, before
the resolution reads it**; once the resolution merges, an unwritten argument is gone into the
merge base.

See [sangha/](https://github.com/goedelsoup/yidam/blob/main/sadhana/sangha/README.md) for the
full resolution protocol. The link is absolute because the directory is **conditional**.
[why](GRAPH.evidence.md#the-sangha-link-is-absolute)
