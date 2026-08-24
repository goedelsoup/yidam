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
    "contract": "0.5.0",
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

## Assertions, not documents (contract 0.5.0)

Four tools were added at 0.5.0. Three are `core`; `licensed_edges` introduces the `ontology`
capability, because a projected mirror can hold nodes and edges and hold no `.ont.yml`.

| Tool | Tier | Answers |
|---|---|---|
| `claims` | core | the assertions a corpus makes, with the standing each is made at |
| `check_subject` | core | is this commit subject in vocabulary, before the commit is written |
| `claim_tags` | core | the three tags, their meanings, and how each may be written |
| `licensed_edges` | ontology | what a class declares it may link to |

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
