# Agent Conduct

Guidelines for agents operating in yidam-derived repositories.

**How to read this file.** Every rule below is a sentence you can act on. The incident that
produced a rule, the measurement that set its threshold, and the failure it was built against
are in [agent-conduct.evidence.md](agent-conduct.evidence.md) — one section per rule, reached
by the `[why]` link beside it. Read a rule's evidence when you are deciding a hard case,
arguing that a rule is wrong, or changing one. You do not need it in order to comply, and it
is not part of the recurring read.

## Commit deliberately

Every commit is a permanent node in the knowledge graph. Before committing:
- Is this a complete, coherent knowledge event?
- Is the commit message legible as a graph event description?
- Are new nodes linked to existing ones?

Do not commit partial work or exploratory scratch to the settled baseline. Use branches for
open-ended exploration; commit to the baseline when knowledge is settled.

## Link generously

New nodes should reference existing related nodes. Orphan nodes — files with no incoming
or outgoing edges — weaken the graph. When adding a file, ask: what does this connect to?

## Stay within scope

The corpus grows through sustained, directed inquiry. Do not add nodes speculatively or
outside the domain of the current repository. Breadth should be driven by need, not by
completeness anxiety.

## Make synthesis explicit

When two ideas relate, say so in the files — and in the commit. A synthesis commit that
adds edges between existing nodes is a first-class knowledge contribution, not housekeeping.

## Preserve provenance

Do not delete or rewrite committed nodes without a record of why. If a node is superseded,
mark it as such and link to its successor. The graph's history is part of its value.

## Mark claim confidence

Corpus nodes often contain claims at different levels of certainty. Tag them inline so
readers and agents can assess the node's reliability without reading sources:

- `[verified]` — supported by a committed primary source linked from this node or its catalog entry
- `[inference]` — a reasonable conclusion drawn from verified facts; not directly witnessed
- `[open]` — a live question; the answer is unknown, contested, or under investigation

**Rules:**

- Untagged claims are only implicitly verified if the node is a direct transcription of a
  primary source. In all other cases, tag every non-obvious claim.
- `[inference]` is not a weakness — it is honest. Untagged inference is the problem.
- `[open]` claims do not need to be resolved before committing. An open question is a valid
  and permanent knowledge contribution.
- A synthesis node will typically contain all three: verified facts it draws on, inferences
  it makes, and open questions it generates. This is expected and good.
- **Write the tag exactly, and put anything else beside it.** `[verified — Pearl 2009]` is not
  a tag. Write `[verified]` and then the citation. `yidam lint` reports the near miss as
  `claim-tag-malformed`. [why](agent-conduct.evidence.md#claim-tag-malformed)
- **A `[verified]` claim in a node that links no catalog entry is reported**, as
  `verified-unsourced`. The fix is a citation or a demotion, and which it is belongs to you:
  nothing proposes a promotion. A `cites:` into a dependency does not discharge it, because a
  foreign tag is the producer's and does not transfer.
  [why](agent-conduct.evidence.md#verified-unsourced)
- **To name a tag rather than make one, say that you are naming it.** The signal is **grammar,
  not typography** — a tag is read as named on four shapes and no others:

  | Shape | Example |
  |---|---|
  | Pluralised | *Two of those three `[open]`s are closed.* |
  | Named by the noun after it | *The `[open]` tag marks an unanswered claim.* |
  | Object of a past-tense reporting verb | *An earlier version said they were `[open]`.* |
  | Negated | *This claim is not `[verified]`.* |

  Anything inside a fenced block is shown rather than said, and is never counted. Backticks
  decide nothing. The present tense is not narration — *"this node now carries `[open]`"*
  applies a tag — and neither is a copula: *"why the appointment was made is `[open]`"* is a
  live claim. The rule is frozen alongside the `open_questions` arms in
  [`sdks/parity/mcp/tools.json`](../sdks/parity/mcp/tools.json).
  [why](agent-conduct.evidence.md#naming-a-tag)

### An edge is a claim

An edge is a claim written as structure, so the rule about untagged inference applies to it and
the honest options are the same three. State the relationship if you can defend it. Say what it
rests on in the node body, and tag it there, if it is an inference. **Leave it out if you cannot
do either.** Prefer a weaker relationship you can defend to a stronger one you cannot.
[why](agent-conduct.evidence.md#edge-is-a-claim)

**The tag belongs on the edge, and the link carries two keys for it.** A link may declare
`claim_tag:`, read by the same rule a `type: claim` property is read by, and `source:` where
the standing is `verified`:

```yaml
# person/aldermanic-clerk.yml
links:
  - target: ../place/ward-nine.yml
    relationship: resided-in
    claim_tag: inference       # the roster states an address, not a residence
  - target: ../place/city-hall.yml
    relationship: worked-at
    claim_tag: verified
    source: 1889-municipal-register
```

[why](agent-conduct.evidence.md#edge-claim-keys)

Three checks read them, and all three are named in `GRAPH.md`:

- `edge-verified-unsourced` reports an edge asserting `verified` with no `source:` — the edge
  half of `verified-unsourced`. It needs no declaration, because writing the tag is what opts
  an edge in.
- `edge-untagged` reports an empirical edge that declares no standing, or one whose `claim_tag`
  spells none, and it runs only where the corpus asked for it:

  ```yaml
  # .yidam/corpus/universal.yml
  edge_claims:
    required: true
    structural:              # bookkeeping — these assert nothing, so nothing is asked of them
      - instance-of
      - concerns
      - subject-of
  ```

  The two keys are separate on purpose: naming the verbs that are bookkeeping is a fact about a
  vocabulary, and recording it must not switch a gate on as a side effect.
  [why](agent-conduct.evidence.md#edge-claims-opt-in)
- `edge-standing-unheld` reports an edge asserting a standing stronger than one its own
  endpoints declare. A node's standing there is the declared claim-typed field below, not the
  weakest marker in its prose; a node that declares no claim-typed field has no standing and is
  compared to nothing. The check is one-directional, so an `open` edge between two `verified`
  nodes is not a defect. Prefer the demotion to the promotion.
  [why](agent-conduct.evidence.md#edge-standing-unheld)

**And the tag is read, not only graded.** `open-questions`, `status`, `corpus-index` and the
MCP `claims` and `open_questions` tools all see a tagged edge. An edge tagged `open` is an open
question in its own right, listed beside the node ones and addressed by its triple. The counts
are reported **beside** the node ones rather than added to them.
[why](agent-conduct.evidence.md#edge-tags-reported-beside)

### A tag may be a field rather than a sentence

Inline tags are the default and stay the default: a claim is usually a sentence, and the tag
belongs where the sentence is.

A corpus whose evidence markers are **structured** — a typed vocabulary stored as a value
rather than written into prose — declares the field in its class, with `type: claim`:

```yaml
# concept.ont.yml
properties:
  - name: claim_tag
    type: claim
    description: The evidence standing of this node, as a tag rather than as prose.
```

```yaml
# concept/low-flow.yml
properties:
  claim_tag: open        # `[open]` is read the same way
```

Then `open-questions`, `status`, `corpus-index` and the MCP server all see it. The class has to
declare the field, and only the declared field is read.
[why](agent-conduct.evidence.md#claim-typed-field)

### `[verified]` is a claim about provenance, not about confidence

A figure can be almost certainly correct and still not be `[verified]`, because the tag says *a
committed source supports this*, not *I am sure*.

Where the source of record is unreachable and an aggregator or republisher carries the same
figures, those figures may be retrieved, computed on, and published. They support `[inference]`,
and they never support `[verified]`, however good they are. Where a connector reaches such a
source, make the distinction **unrepresentable rather than advisory**, and name the substitution
wherever the figures appear rather than only at the connector.
[why](agent-conduct.evidence.md#provenance-not-confidence)

### A claim tag cannot reach a class definition

Every rule above runs on tags, and a tag attaches to a claim somebody makes on a node. A class
definition in `<class>.ont.yml` is not that, so a class whose definition *states a proposition*
puts that proposition beyond the reach of the entire apparatus.

**Read `.ont.yml` files against your evidentiary rules directly, and on a schedule** — no
instance-level check will do it for you. A class definition describes a *kind*; the moment it
starts describing a *reason*, it is making a claim no reader will see it make. `yidam lint`
reports the shape of this as `class-asserts-purpose`, but a lint over wording is a prompt to look,
not a proof of absence. Two checks read a class file's prose and neither replaces that reading:
every field is scanned for the near miss above, and `class-claim-uncounted` reports, at `Info`,
how many well-formed tags a class asserts. A class with an argument worth counting should be
putting it in a node. [why](agent-conduct.evidence.md#class-asserts-purpose)

## Prefer a base rate to a refusal

Where a documentary sequence invites a causal reading the record cannot support, saying *do not
infer that* is weaker than saying *here is how ordinary that outcome is*: a refusal asks a reader
not to draw the inference, and a base rate removes the reason to. Where the corpus can compute
one it should — in the same passage as the sequence, not below it.

### But a reference class defined by the outcome it is meant to place is not a base rate

A denominator earns the name only if it was drawn without reference to how this case turned out,
and if the population could have contained cases that came out otherwise. Check the shape too: a
count of who *is* something does not place a claim about who *became* it. Two shapes to
recognize:

- **The sequence restated as a fraction.** "Six of eight plans were four-year plans," where all
  eight are already on the thread under discussion.
- **One act counted twice.** "Two of two officers removed themselves," where being an officer
  *is* the precondition for the act.

**Where no base rate is computable, say so and say why.** That sentence is worth as much as a
rate and is not a failure to have looked. A fabricated denominator is worse than none, because
it launders the sequence into arithmetic. [why](agent-conduct.evidence.md#reference-class)

## When claims leave the repository

A repository that publishes — a site, a report, a brief, anything read by someone who will not
read the corpus — needs the derivation checked too, and three rules govern it.

**A derived assertion travels only as far as the weakest claim beneath it.** Its tier is the
**minimum tag across the whole supporting chain**, computed rather than declared. `[verified]`
may reach public material; `[inference]` reaches attributed memos and backgrounders; `[open]`
does not leave the repository.

**Cite a span, not a node.** An external assertion names a **verbatim span** of the corpus node
it rests on, and the gate asserts that span appears there character-for-character. This does not
verify the inference; nothing can. It forces the actual sentence to sit beside the assertion,
where the gap between them is visible to a reader.

**A refusal in the cited block fails the build.** Where a corpus node carries a refusal beside
the claim — a sentence of the form *this corpus does not infer X from this* — an assertion
citing across it is refused, and the author must answer it rather than route around it.
[why](agent-conduct.evidence.md#outbound-claims)

## When claims arrive from another repository

The mirror of the section above. A repository can declare a dependency on another corpus — a
`.yiz` bundle, fetched and pinned — and `retrieve` then returns nodes this sangha never settled,
beside nodes it did.

**Check `origin` on every result.** It is the package name for a foreign node and `null` for a
local one, and it is always present. A foreign node's id is qualified — `pkg::class/name` — and
its path points into `.yidam/tonpa/<pkg>/`, not into this corpus.

**A foreign node may be read. It may not be an edge target.** The tooling enforces this — no
local edge resolves across the boundary, and traversal does not cross it — so the part that
needs you is what you do instead: **put what you took into a local node, in this corpus's terms,
tagged at this corpus's standard, saying in prose where it came from.** That local node is the
thing this sangha becomes accountable for. [why](agent-conduct.evidence.md#foreign-nodes)

**A foreign tag is the producer's tag.** `[verified]` in a dependency means *that* corpus's
electors accepted *that* provenance. It does not transfer, and you cannot check it. The rule that
a derived assertion travels only as far as the weakest claim beneath it still holds, and across
this boundary "weakest" is genuinely unknown. [why](agent-conduct.evidence.md#foreign-tags)

**The prose form has a structured form, and the gate reads it.** A local node may carry a
`cites:` block beside its `links:` — never inside it, because a citation is not a relationship
and must never enter a traversal:

```yaml
cites:
  - package: upstream          # the dependency, as .yidam/tonpa.toml names it
    node: concept/base-flow    # <class>/<name> over there; `package` already says whose
    commit: 8d35441            # the manifest commit it was read at; absent for a path dependency
    tag: verified              # the producer's standing AS OBSERVED — recorded, never transferred
    span: >-                   # verbatim text from that node
      the slowly varying component sustained by groundwater discharge
```

**`span` is the load-bearing field, for the reason the outbound rule gives**, and the gate
asserts it still appears there. When it fails, **the repair is never to re-quote** — the far side
changed its mind, and the question is whether your claim survives it. `commit` is recorded rather
than enforced: a moved pin reports, never fails.
[why](agent-conduct.evidence.md#span-is-load-bearing)

**A shared class name is not agreement.** Classes are named per-corpus. A `concept/risk` there
and a `concept/risk` here are two nodes sharing a string. That is a question worth investigating,
not an identity.

**A stale dependency is a normal state, not a finding.** It is pinned deliberately. Where its
currency bears on a conclusion, say which pin you read.

## When a claim rests on a node beside it

The same grammar, with `package` left off. **A `cites:` entry with no `package` names a node in
this corpus**, and it is the form most claims actually need.

```yaml
cites:
  - node: reach/tailwater      # <class>/<name> in this corpus; `.yml` may be written or not
    tag: inference             # the standing this corpus holds that span at, and it must agree
    span: >-                   # verbatim text from that node
      Discharge below the dam tracks the release schedule within a day
```

`commit` has no meaning here and is not read: the node is in this tree, so git already records
which state it was in. What is different is `tag`. **Inside one corpus the producer is you.** A
citation declaring `[verified]` over a paragraph this corpus tags `[inference]` says something
its own corpus denies, and `local-citation-tag-drift` refuses it. Citing an `[open]` span is
legal and declaring it as anything else is not — the corpus said it does not know, and a
citation may rest on that as long as it says so. [why](agent-conduct.evidence.md#local-citation)

**Writing one is opt-in and writing none is a normal state.** Prose already links nodes to each
other constantly; a link is not a citation, and nothing turns one into a finding.

**A citation is still not an edge.** It sits beside `links:` and never inside it, for the same
reason the external form does.

## When your corpus disagrees with the one it cites

Every example above is a node leaning on a foreign span. The other case is a node that has read
one and thinks it is wrong.

**It goes in the same place, in the same form.** A disagreement is a taking like any other — you
cannot contradict a sentence you have not read — so it is a local node, in this corpus's terms,
tagged at this corpus's standard, carrying a `cites:` block whose `span` is the verbatim sentence
it contradicts, at the pin it was read at, with `tag` recording the standing the producer held it
at. No new field and no new object: quoting what you disagree with is the whole of the form.

**Nothing resolves it, and that is the answer rather than a gap.** Two corpora may contradict
each other permanently, and both records stand; there is no forum between them and one would be
wrong to build. A disagreement with the **prelude** that constitutes you is the other case and
does have a forum — an issue upstream, decided there, arriving by re-vendor; see
[upstream.md](upstream.md). [why](agent-conduct.evidence.md#no-forum)

**What the record buys is that the contradiction cannot go silent.**
`external-citation-span-drift` fires when the sentence is no longer there,
`external-citation-pin-moved` when the bundle moved, and `tag` records what they held so a
demotion is visible. The repair is the one the agreement case states: never re-quote — which for
a disagreement may be that they conceded. [why](agent-conduct.evidence.md#cannot-go-silent)

**Nothing marks a citation as a contradiction**, and a reader cannot tell one from the other by
looking at the block. That is deliberate for now rather than overlooked.
[why](agent-conduct.evidence.md#contradiction-unmarked)

## The safeguards were built against carelessness, not against interest

Every mechanism above catches an agent that was sloppy. None of them catches an agent that wants
a particular answer, because selection is invisible to a check that reads what is there.

The partial defense is to write down the search rather than only the finding: **what was examined
and not used**, kept beside the claim it supports. It is a weak instrument and it is the only
auditable trace of selection that exists. If you are working somewhere with an interest in the
answer, behave accordingly, and write the ledger. [why](agent-conduct.evidence.md#selection)
