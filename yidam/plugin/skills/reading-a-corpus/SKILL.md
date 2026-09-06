---
name: reading-a-corpus
description: Use when answering a question from a yidam corpus — a repository with a .yidam/ directory — rather than from your own knowledge. Picks the right yidam MCP tool for the question, and explains why an empty answer must be read rather than filled in. Triggers on "what does the corpus say", "search the corpus", "find the node", "open questions", "what is known about", "look this up in the graph".
---

# Reading a corpus

The corpus is a knowledge graph with provenance, and the point of asking it is to get **its**
answer rather than yours. Two traps do more damage than anything else here, and both look
like success.

## Trap 1 — an empty result is where an agent invents

Zero rows is otherwise indistinguishable from a bad embedding, a class nobody has written
into, and a corpus that genuinely has no view. Every retrieval carries `rejected` and
`absence`, both null when something was found:

- `rejected` — the filter itself was refused. A `class` that names no declared class comes
  back as `unknown-class` **with the near miss**, rather than being searched. Fix the name.
- `absence.code` — `class-unpopulated` (declared, nothing written into it),
  `no-term-match` (keyword search read every node and none uses these words — a statement
  about your words, not about coverage), `class-unindexed` (the nodes exist and the index
  predates them), `class-undeclared` (no `.ont.yml` to derive class rows from).
- `absence.instances` — how many nodes the filter admitted. *None of four* and *none of nine
  hundred* are different facts.
- `absence.elsewhere` — installed dependencies holding what this corpus does not. Whatever it
  names is that corpus's claim, not this one's.

**Report what the absence says. Do not close the gap from your own weights** — an answer you
supply here will be attributed to having worked in the corpus.

Also read `degraded` on every retrieval: it says whether `retrieve` ran as semantic search or
fell back to keyword search, and why.

## Trap 2 — the server is answering about a snapshot

Every tool answers from the corpus and index loaded **when the process started**. There are
no live git operations and no per-request file reads. If you have just edited nodes in this
repository, the server has not seen them — including `query --select body`, which returns the
text as it was read then. Restart the server to move the snapshot, and say so rather than
reporting a stale answer as current.

## Which tool

| Question | Call |
|---|---|
| I do not know which node holds the answer | `retrieve` |
| I know the id and need what it actually says — **and its links** | `get_node` |
| I want the argument around a node, not the node | `neighbors` |
| I want the shape of the corpus | `list_nodes` |
| I want what is *not* known | `open_questions` |
| I want what the corpus **holds**, with the standing of each | `claims` |
| I know the *shape* of the answer — `reach -measured-by-> gage` | `query` |
| I am about to write from the corpus and have a token budget | `pack` |
| I want to know what that would cost before paying for it | `estimate` |

Three distinctions worth holding:

- **`retrieve` finds; `get_node` reads.** A retrieval result carries a `text` that can be
  substantial, which is what makes the omission quiet — it is still not the node, and **the
  links are only in `get_node`**. An agent that only ever retrieves sees the claims and never
  the edges between them, which on a knowledge graph is most of what it came for.
- **`claims` returns assertions; every other tool returns documents.** Asking `get_node` what
  the corpus takes as verified means paying node-sized tokens for a claim-sized answer and
  reading the tags out of prose yourself.
- **`query` walks by the types; `neighbors` floods.** A misspelled relationship comes back
  from a flood as a plausible neighbourhood and from `query` as a rejection naming the near
  miss.

## Dependencies are queryable, and only if you ask

`query` takes `across: true` to run over every installed dependency as well, attributing each
result by `origin` and qualifying foreign ids. Without it you cannot see a foreign node at
all. A hop never crosses between corpora — two corpora sharing a class name is not agreement.
`pack` and `estimate` have no `across`, deliberately: a pack is what you write *from*, and one
mixing two corpora would put a dependency's prose under this repository's class names.

## What this server can and cannot do

The capability block at connect time declares the holes. `query`, `pack`, `estimate` and
`licensed_edges` all need class declarations, and a projected mirror can hold nodes and edges
and no `.ont.yml`; such a server declares `"ontology": false` and refuses those four with
`capability-not-supported`. That is a fact you can read rather than a hole you discover.

## Where the reasoning is

`docs/mcp-server.md` §3 in this repository, and RFC-0005 for the frozen contract.
