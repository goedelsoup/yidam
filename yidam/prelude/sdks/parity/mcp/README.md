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
    "contract": "0.1.0",
    "retrieve": { "vector": false },
    "graph": true, "phases": false, "sangha": false, "resources": true
  }
}
```

`retrieve.vector` is not a capability tier — `retrieve` is core either way. It says whether
the vector index is loaded, which is the same fact `degraded` reports per call. A server that
declares `vector: false` is promising every `retrieve` will come back `degraded: true`.

## The degraded signal

`retrieve` MUST always carry `degraded`. `false` only when the query was embedded with the
index's own contract (`embed.config.json`); `true` for keyword fallback, for an absent index,
and — this is the arm that gets missed — for an index built with different embedding
settings. A server that lazily builds its own index on first use and reports `degraded: false`
is answering from a different vector space and saying it is not.

Omitting the flag is not an option the contract offers. There is no third state.

## Cases

`cases/<tool>/<name>.json` is a call and the shape its response must have, over the fixture
corpus in `corpus/`. They assert invariant fields — `degraded`, the node model, `direction` —
and never embedding scores, which are a property of a model rather than of a contract.

A server declaring a capability MUST pass its cases. One declaring it absent MUST return a
capability-not-supported error, and its cases are skipped rather than passed.
