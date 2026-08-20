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

The last two exist only in repositories bootstrapped as `governance: collective`. In a
single-elector repository — the common case — the baseline is `main` and inquiry runs on
`phase/<name>` branches. See [PHASES.md](PHASES.md).

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

**Why closed.** An open vocabulary decays into one verb per commit. A repository derived
from this template ran a hundred commits with roughly sixty distinct leading words — `lift`,
`grain`, `void`, `worst`, `viewport` — each individually evocative and collectively useless:
nothing could recover which commits changed what was known and which merely moved bytes.
A verb outside this list is not a richer description, it is an unclassifiable one.

Reach for the closest verb rather than inventing one. If a commit genuinely does not fit —
that is a gap in the vocabulary, which is an yidam-level change, not a local one.

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

See [sangha/](../../sangha/README.md) for the full resolution protocol. The path is relative
to this document's home in a derived repository, `.yidam/.vendor/prelude/` — the sangha
lives at `.yidam/sangha/`, one level above the vendor directory rather than inside it.
