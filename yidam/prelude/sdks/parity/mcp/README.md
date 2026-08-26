# The MCP contract

One tool and resource surface, three servers. This directory is what they conform to.

`tools.json` is the frozen list — names, tiers, and input schemas. It is the **only** place
the list lives: the Rust E2E test reads it rather than restating it, and any other server's
harness does the same. Before this, the Rust list was frozen in a test, the TypeScript list
in a README, and a third implementation had drifted so far that **one tool name was shared
across two servers** out of five capabilities.

## Tiers

| Tier | Meaning |
|---|---|
| `core` | Every server MUST back it. |
| a capability name | Backed **iff** the server declares that capability true. |

Optional is not the same as absent. A server that cannot walk the graph declares
`"graph": false` — an explicit, testable statement rather than a hole an agent discovers
through a tool-not-found error. The capability flag and the tool list must agree, and the
harness checks that they do.

## The capability block

`initialize` returns MCP's own `capabilities` object with a `yidam` block inside it:

```json
"capabilities": {
  "tools": {}, "resources": {},
  "yidam": {
    "contract": "0.10.0",
    "retrieve": { "vector": false, "reason": "no_index" },
    "graph": true, "ontology": true,
    "phases": false, "sangha": false, "resources": true
  }
}
```

`retrieve.vector` is not a capability tier — `retrieve` is core either way. It says whether
the vector index is loaded, which is the same fact `degraded` reports per call. A server that
declares `vector: false` is promising every `retrieve` will come back `degraded: true`, with
the `reason` it names here. `reason` is null exactly when `vector` is true.

## Which kind of nothing (contract 0.10.0)

`retrieve` gains `rejected` and `absence`. Both are always present and at most one is non-null.

0.8.0 answered *why is this empty* for `query`, `pack` and `estimate` — the surfaces where the
ontology derives the reason. It did not touch `retrieve`, which is the tool an agent reaches
for **before** it knows enough to write a query. There, `results: []` still meant all of: the
corpus has nothing on this subject, the query's words are not the corpus's words, the class
filter names a class holding no instances, and the class filter names a class that does not
exist.

**The last one was not an absence at all.** `retrieve` took a `class` argument and never read
it, so `retrieve("hydropeaking", class: "gauge")` against a corpus declaring `gage` returned
zero results and reported nothing wrong — the exact failure `query`'s `unknown-class` rejection
exists to prevent, one tool over, on the more-used tool. It is now rejected *before* the
search: a filter that cannot match cannot produce a true negative, and searching with it would
report a typo as a fact about the corpus.

| Code | Path | Means |
|---|---|---|
| `class-unpopulated` | either | the ontology declares the class and no node here or in a dependency belongs to it |
| `class-undeclared` | either | no node carries the class **and** this corpus declares no ontology, so a misspelling and an empty class cannot be told apart |
| `class-unindexed` | vector | the index holds no rows for the class though the corpus holds instances — it was built before they were written |
| `index-empty` | vector | the index holds no rows and there was nothing to search |
| `query-no-terms` | keyword | the query contains no searchable terms |
| `no-term-match` | keyword | none of the nodes searched contains any word of the query |

**A smaller shape than `query`'s, deliberately.** `{code, message, instances}` — no `step`,
because `retrieve` has no steps, and no `elsewhere`, because `retrieve` already searches every
installed dependency. That field exists on `query` to point at corpora a local walk did not
read; here it would be empty on every response by construction, which is a field that teaches
a client nothing and costs it a branch.

**`instances` is the denominator the message is about**: how many nodes the class filter
admitted to the search. *None of four* and *none of nine hundred* are different facts about a
corpus, and it is the whole difference between `no-term-match` and `class-unpopulated`.

**The `core` tier is why `class-undeclared` exists.** Unlike `query`'s family this tool cannot
hide behind the `ontology` tier: a server with no `.ont.yml` backs `retrieve` and can derive
*neither* class row — it cannot reject an unknown class, because none is declared, and it
cannot call one unpopulated, because nothing declares it a class at all. A server MUST NOT
report `class-unpopulated` on an unschematised corpus; that asserts an ontology it does not
have. It says which case it is in instead.

**The semantic path says more than a threshold would.** There is no score threshold and a
conforming server must not invent one — that would be a claim about a model rather than about a
corpus. But nothing is dropped for scoring badly, so an empty vector answer is not a *weak*
answer: it is proof the filter admitted no rows at all. That is derivable without asserting
anything about similarity, and it is what makes `class-unindexed` reachable — the one diagnosis
the keyword path is blind to. A weak-but-non-empty answer stays undiagnosed.

## Which corpus, and when (contract 0.9.1)

Two keys on `query`'s response, both always present, both about the same question: *what is
this answer about?*

`kind` is `query`. The CLI's `--between` emits a **series** — a row per commit in a range —
under the same RFC-0016 envelope at the same `format_version`, so without a discriminator a
client tells the two shapes apart by testing for a key that is absent. That is precisely the
inference `at` exists to forbid one field over.

`at` is the commit the answer is about, and null for the working tree. A server answering from
its loaded corpus reports null; the CLI's `--at` reports `{rev, commit, date}`. It is on
**every** response including a rejected one: a refusal about a tag and a refusal about now are
different claims, and a client must never infer which it holds from a missing key.

### The corpus is the one loaded at startup — `select=body` included

Every read a `serve --mcp` process makes comes from the corpus and index built on disk when it
started: `retrieve`, `get_node`, `neighbors` and `query` alike. `select=body` returns the node
text as it was read then, not as the file reads now, so a long-running server answers with the
corpus it was started against until it is restarted.

This is deliberate rather than incidental. A `body` that reached for the working tree at
request time would be one field, on one tool, answering about a different corpus than every
other field beside it — including `at`, which would still say null, meaning *the corpus this
server holds*. The staleness banner at connect time reports the same snapshot. A server that
wants freshness restarts; it must not make this one field live.

## Quote before, account after (contract 0.9.0)

One tool, `estimate`, at the `ontology` tier. An agent budgets in tokens and, until this,
could only discover what a retrieval cost by paying for it — so the only strategy available was
to ask for less than might be needed and hope.

**It is cheap for the caller and not for the server, and that asymmetry is the whole point.**
Knowing exactly what a query costs means running it, so a quote is the answer with the prose
withheld. The server holds every node either way — the same reason `nodes_read` refuses to
count the corpus load — and the caller pays for what comes back. A conforming server returns no
rows here, and runs the traversal exactly **once**: a quote that resolved a similarity anchor
twice charges double for the thing it exists to call affordable.

```text
8 node(s) match — priced against a budget of 200 token(s)

  select                          chars    ~tokens
  node,class,label                  639        159  fits
  node,class,label,description     5991       1497  over budget
  node,class,label,body            8512       2128  over budget
  a context pack                   6138       1534  over budget

  chars are exact; ~tokens is chars/4 — use chars with a real tokenizer
```

**A table, because the decision is not whether.** A caller has its question either way; what it
chooses is how much of each node to ask for. Every row prices the same match set at a different
`select`, cheapest first, and `fits` is the verdict against the quoted budget — null exactly
when no budget was given.

**`chars` is exact and `tokens` is not.** `chars` is the serialized length of the payload that
would come back; a server that rounds it has broken the only promise here. There is
deliberately **no range**: a range would be a second invented number laid over the first, and a
caller with a real tokenizer needs the exact figure rather than a wider guess.

**`limit` is part of the call, so it is part of the quote.** Projections are priced at it;
`pack` has no limit, so its row prices the whole match set. The two sit side by side because a
caller comparing them is comparing a page of names against the corpus's prose.

**A quote of zero reads as cheap**, which is why `absence` is carried here too. A caller told a
query costs nothing, and not told the class is unpopulated, has been handed the most affordable
possible way to learn nothing. `pack.chars` on an empty answer is not zero — the pack carries
the diagnosis, and that is the part worth paying for when there is no answer.

**Speculative, and said so.** This is the one tool added on a guess about how callers behave
rather than on a measured gap. Nothing else depends on it, it reads nothing new, and it
computes entirely from what `query` already produces.

## Where a corpus is ignorant (contract 0.8.0)

`query` and `pack` gain `absence`: **why** an answer is empty, when it is.

Zero rows is otherwise indistinguishable from a bad embedding, a class nobody has written
into, and a corpus that genuinely has no view. A caller that cannot tell those apart fills the
gap from its own weights — confidently, and under a claim that will be attributed to having
worked in the corpus. `retrieve` already has the right instinct one door over: it degrades and
says so rather than quietly answering worse. This is the same principle applied to **coverage**
rather than to method.

Every code is read off something the corpus *states* — a class the ontology declares, a
relationship a class licenses, the edge set the gate resolves — so the diagnosis can be trusted
at the moment it matters most.

| Code | Step | Means |
|---|---|---|
| `class-unpopulated` | entry | the ontology declares the class and it holds no instances |
| `predicate-unsatisfied` | entry | it holds instances and none satisfies the predicate |
| `anchor-empty` | entry | the similarity anchor resolved to no entry node |
| `relationship-unauthored` | hop | a class in the source set declares it and no instance authors it |
| `relationship-unknown` | hop | no class declares it and no instance authors it |
| `no-edge-from-here` | hop | authored elsewhere, and by nothing that reached the previous step |
| `edge-lands-elsewhere` | hop | edges were followed and nothing they landed on satisfied the step |
| `no-match` | either | none of the above; reported rather than asserted away |

**`relationship-unauthored` is worth the most, and is why the corpus declares `supersedes`.**
A relationship a class licenses and no instance uses is either an ontology that reached past
its corpus or a gap worth an open question — and it is invisible from every other angle: the
class file says it exists, the gate has nothing to complain about, and the traversal comes back
exactly as a mistyped name would. It is split from `relationship-unknown` because the two
return identically and their repairs are opposite; one code for both answers a misspelling with
a sentence about the ontology.

**Absence is not a rejection.** `rejected` says the query is wrong; `absence` says the query is
right and the corpus is quiet. A server reporting a typo as `class-unpopulated` tells a caller
its mistake was a true negative. Both keys are on every response and at most one is non-null.

**`elsewhere` is a pointer, not an answer.** It names installed packages holding what this
corpus does not, and whatever it names is *that* corpus's claim. Empty by construction on a run
that already spanned the dependency set — it looked — and on a run about a past commit, because
a dependency set has no history and naming today's packages to explain an older corpus's
silence would be an anachronism dressed as a lead.

**On `pack` it matters more.** A pack is what a caller reads *as* the corpus, so an empty one
that says nothing is a context window asserting the corpus has no view. A conforming server
renders the diagnosis into `text` as well as into the field: the artefact travels without the
envelope around it.

## Packing for a goal (contract 0.7.0)

One tool, `pack`, at the `ontology` tier — `query`'s answer rendered as prose, filled to a
token budget, with **an account of what did not fit**.

The static whole-corpus export already stated the principle and delivered it: *an honest
account of what it contains, so a caller can report what it wrote rather than what it was
given.* It is whole-corpus and static. The moment an agent has an actual question there was no
equivalent, and it fell back to top-k with nothing said about the rest.

| | Says | A caller can |
|---|---|---|
| `retrieve` | here are 5 nodes | nothing about the rest |
| `query` | 12 of 40 matched | know how much it is missing |
| `pack` | 12 of 40, and the 28 dropped were `recording` | spend more budget, or report the gap |

**The receipt is the contract; the prose is not.** No case pins a byte of `text` — a server
whose sections carry different headings conforms, and a case pinning `written: 2` on a corpus
of four would freeze one implementation's layout on every other. What is frozen is the
accounting: `written + omitted == reachable`, `omitted_by_class` sums to `omitted`, and
`reachable` is the count **before** the budget. A server reporting the post-budget count
answers `12 of 12`, which is what silent truncation looks like from the inside.

**The receipt is also the floor.** A budget too small to hold the account itself cannot be met,
and `budget.over_budget` says so rather than the pack dropping its own receipt to fit — which
would produce a pack that silently holds nothing, the failure mode arriving through the door
marked compliance. Whenever one node is written, the pack fits.

`budget.basis` names how the estimate was computed (`chars/4` for a server that does not
tokenize). An honest approximation, stated, beats a precise-looking number computed with the
wrong tokenizer.

## Walking by the types (contract 0.6.0)

One tool, `query`, at the `ontology` tier. It is the answer to a gap that had been open since
the ontology acquired relationship names: **the graph was typed and the only way to walk it
was an undirected flood.** `neighbors` chains outbound and inbound edges unconditionally,
filters on neither relationship nor direction, and carries both out as *labels* on the result
while reading neither as an input. A server offering only that has typed its graph and left
no way to walk by the types.

`query` takes a path — `concept -enables-> concept`, `concept <-enables- concept`,
`*[claim_tag=open]`, `concept~"embedding space" -relates-to-> concept` — and answers with the
nodes, the diagnostics, and what the walk cost.

**Its tier is `ontology`, not `graph`, and the reason is worth stating.** The tier names what
the *diagnosis* needs. Without `.ont.yml` every class name and every relationship is accepted,
so a misspelling comes back as zero results — indistinguishable from a true negative, which is
the one failure this tool exists to prevent. A server that backs `ontology` and holds no edges
is fine: its hops are licensed and match nothing, which is a true statement about its corpus.

Three distinctions a conforming server has to get right, each with a case:

| | Means | `rejected` |
|---|---|---|
| **rejection** | the query is wrong — unknown class, unlicensed edge, unprojectable field | `{step, code, message}` |
| **diagnostic** | the query ran and something is worth saying — an undeclared relationship under a non-exhaustive `edge_policy`, a `*` narrowed to the classes declaring a property | `null` |
| **empty** | the query was well-formed and matched nothing | `null` |

A server that promotes diagnostics to rejections refuses legal queries: a non-empty `edges:`
says *these relationships exist*, not *and no others may*, and reading it as the second put 210
errors on a compliant corpus. A server that demotes rejections to empty results tells an agent
its typo was a true negative. Neither is signalled with MCP's `isError` — a rejection is an
answer, and a client branches on `rejected.code`.

**The anchor.** `class~"…"` enters by similarity and leaves by typed edge. It is
class-qualified because a hop's verdict depends on the source class's `edge_policy`, so a bare
anchor could not be typechecked before it ran. It is an *entry* — only the first step may carry
one, and a later one is refused rather than reinterpreted as a similarity filter. It is
**local**: unlike `retrieve`, it must not enter through an installed dependency's node, because
the response says `"scope": "local"`. And it degrades on the keys `retrieve` already
established, with the same present-and-null discipline and the same frozen reason strings —
carried on `anchor`, and deliberately *not* duplicated at the top level, because a query with
no anchor performed no retrieval and a `degraded: false` there would read as retrieval having
succeeded.

## Assertions, not documents (contract 0.5.0)

Four tools were added at 0.5.0. Three are `core`; `licensed_edges` introduces the `ontology`
capability, because a projected mirror can hold nodes and edges and hold no `.ont.yml`.

| Tool | Tier | Answers |
|---|---|---|
| `claims` | core | the assertions a corpus makes, with the standing each is made at |
| `check_subject` | core | is this commit subject in vocabulary, before the commit is written |
| `claim_tags` | core | the three tags, their meanings, and how each may be written |
| `licensed_edges` | ontology | what a class declares it may link to |
| `query` | ontology | a typed path over the graph (added at 0.6.0, above) |
| `pack` | ontology | that path's answer, budgeted, with what did not fit (added at 0.7.0, above) |
| `estimate` | ontology | what either would cost, before paying for it (added at 0.9.0, above) |

The first exists because the other five tools all return **nodes**, and the unit of assertion
here is not the node — it is the claim. A node is 2–10 sentences by the model's own rule, so
an agent asking what is known about something pays node-sized tokens for a claim-sized answer
and learns the standing only if the tag survived into the prose.

The last three exist because the practice was prose an agent reloaded every session. A norm
holds when something echoes it back inside the act; for a human writing a commit that echo is
`lint --commits`, and for an agent it should be a call made *before* the act. Compliance by
asking, rather than by having remembered.

**`claims` serves the tag or serves nothing.** There is no untagged arm, and the rule for what
counts is the one the reports use — not the SDK's `extract_claims`, which is a line-oriented
parser for the markdown node model and reads `class: gage` as a claim over a YAML instance.
The full predicate is in `tools.json`'s notes for the tool; the part most easily got wrong is
that the invariant is *never make the corpus look better-evidenced than it is*, which is not
the same as "when in doubt, drop it": dropping an `[open]` promotes too.

## The degraded signal

`retrieve` MUST always carry `degraded`. `false` only when the query was embedded with the
index's own contract (`embed.config.json`); `true` for keyword fallback, for an absent index,
and — this is the arm that gets missed — for an index built with different embedding
settings. A server that lazily builds its own index on first use and reports `degraded: false`
is answering from a different vector space and saying it is not.

Omitting the flag is not an option the contract offers. There is no third state.

### And why (contract 0.4.0)

`degraded_reason` is required alongside it, and is null exactly when `degraded` is false —
null rather than absent, the convention `origin` already follows, so a client testing the key
never has to distinguish "not degraded" from "a server too old to say why".

| Value | Means | Repair |
|---|---|---|
| `no_index` | The corpus has no vector index | Build one |
| `no_vector_support` | An index exists; this build cannot read it | Install a build carrying the vector dependencies |
| `stale_contract` | An index exists, built with different embedding settings than this server would use | Rebuild the index, or embed with its contract |

The bare boolean made two different repositories look identical: one that never built an
index, and one whose index the running binary cannot read. Both answer from keyword search,
only one is fixed by indexing, and a client — or a person reading a startup banner — told
just `degraded: true` cannot tell which it has. yidam's own CLI acquired exactly that pair
the moment `serve` moved into the light default set, which is what forced the field.

**Precedence is by what must be fixed first, not by what the server notices first.** A build
with no vector support looking at a corpus with no index reports `no_index`: indexing is the
repair either way, and the missing artefact is the nearer cause. `no_vector_support` is
reserved for when the artefact is present and only the binary is in the way. That rule is
what lets `cases/retrieve/keyword-degraded.json` pin a single value every build of every
server must answer with — a case whose expected value changed with the harness's build would
be no freeze at all.

`stale_contract` is named here and emitted by no server in this repository: the Rust CLI
never re-embeds, so it cannot reach that state. It is frozen anyway, because a server that
does reach it will otherwise invent a string, which is the drift this directory exists to
stop.

## The corpus

`corpus/` is the tree every case runs against — a four-node `concept` graph, small enough to
read in one sitting and shaped so each case has exactly one thing it can fail on. Stage it as
a repository: copy it to a scratch directory, `git init`, commit once. The Rust harness
(`yidam/cli/tests/mcp_serve.rs`) does that in ten lines, and so should every other.

It ships here because the counts in `cases/` describe it and nothing else. For a while it did
not: the corpus was written as heredocs inside that Rust test, so a case asserting
`count: {open_questions: 3}` named nodes a consumer had no way to see. One did the reasonable
thing and re-expressed every `count` and `equals` against a corpus of its own — which turns a
conformance suite into a check that a server agrees with itself. Asserting a case's `count`
directly is the obvious way to consume these files, and it is now also the correct one.

| Node | Why it is there |
|---|---|
| `concept/knowledge-graph` | The only node that is **not** open, and the only one with an outgoing edge. `retrieve` ranks it first for `knowledge graph`. |
| `concept/traversal` | Open by its **label**. Nothing points out of it, so `neighbors` can answer from it only by walking the edge backwards. |
| `concept/retrieval` | Open by an **[open] claim in its body**. |
| `concept/embedding-space` | Open by a **declared `type: claim` property**, and no other way — no `?`, no bracketed token anywhere in the file. |

`concept.ont.yml` also declares `supersedes`, a relationship **no instance authors**. It is
the only way to reach `relationship-unauthored`: a relationship a class licenses and nothing
uses returns zero rows exactly as a mistyped name does, and without a declaration nobody has
used, no case here could tell that arm from `relationship-unknown`.

`concept.ont.yml` declares `claim_tag` as `type: claim`. Without that declaration the third
arm of the open-question predicate reads nothing — which is exactly why the node is here: it
is the arm a server can omit and never notice, because on a corpus that declares no such
field a two-arm server returns the identical set.

## Cases

`cases/<tool>/<name>.json` is a call and the shape its response must have, over `corpus/`.
They assert invariant fields — `degraded`, the node model, `direction` — and never embedding
scores, which are a property of a model rather than of a contract.

A server declaring a capability MUST pass its cases. One declaring it absent MUST return a
capability-not-supported error, and its cases are skipped rather than passed.

**Every name in an `expect` block is a dotted path.** `count`, `each`, `nonEmpty`,
`everyItemHas` and `fields` resolve `cost.nodes_read`, `anchor.entries` and `steps.0.classes`
the same way `equalsAt` always did; a single segment means what it has always meant. This
arrived with `query`, whose response is the first with structure below the top level — a
harness that could only assert on top-level keys would have had its cases written against a
flattened response, which is a second shape for one answer.
