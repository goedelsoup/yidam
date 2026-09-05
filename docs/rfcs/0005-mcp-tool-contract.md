# RFC-0005 — One MCP tool contract across the Rust CLI, TS, and Python servers

- **Status:** Implemented
- **Track:** I5
- **Relates to:** RFC-0002 (node-model unification), RFC-0001 (report contract), RFC-0006 (correctness reconciliation), RFC-0003 (feature-gated builds)
- **Versioning layers touched:** SDK+parity (the contract) / template (the Rust CLI implements it)
- **Downstream reference case:** Project BOSC (watermark-directory)

> **Pointer, 2026-09-04.** The prose below describes the contract at freeze time — four core
> tools, a capability example at contract 0.4.0 — and is left as written. The canonical list is
> `yidam/prelude/sdks/parity/mcp/tools.json`, which says of itself that it is the only place the
> list lives; the contract has since grown to thirteen tools at 0.13.0, **on the record, through
> later RFCs rather than edits here**: RFC-0017 (0.4.0 → 0.5.0; `pack`, `absence`, `estimate` at
> 0.7.0–0.9.0), RFC-0018 (0.5.0 → 0.6.0), RFC-0019 (`check_citation` at 0.12.0), with the
> issue-level bumps between them documented in `parity/mcp/README.md`. Extending it next:
> [RFC-0027](0027-openai-profile.md) (the `profiles` projection) and
> [RFC-0029](0029-write-tier.md) (the `act` tier — the first write-capable surface, and the RFC
> that decides how this frozen contract changes rather than quietly grows).

## Summary

yidam exposes its corpus over MCP in three mutually incompatible forms — the TypeScript
`yidamMcpTools`, the Rust `serve --mcp` binary, and BOSC's in-process Python server — and **no two
agree on a single tool name.** An agent written against one cannot call another. The three also
disagree on the node shape a read returns, on whether listing and graph-walk are tools or
resources, and on the degraded-retrieval signal. This RFC freezes one canonical MCP tool + resource
contract on the shared parity/spec layer, with a capability model so a thin server can *declare*
what it omits instead of diverging silently, and a conformance fixture every server is tested
against — the same move RFC-0001 makes for the reports.

## Problem

The three surfaces, side by side. Every row is a capability an agent needs; every cell is a
different name (or a hole).

| Capability | TS `yidamMcpTools` | Rust `serve --mcp` | BOSC Python |
|---|---|---|---|
| retrieve / search | `semantic_query` (`sdks/README.md:422`) | `retrieve` (`cmd/serve/tools.rs:12`) | `yidam_query` keyword (`yidam_tools.py:270`) **+** `yidam_semantic_search` vector (`:310`) |
| read one node | `corpus_node` (`:422`) | `get_node` (`tools.rs:27`) | `yidam_read_node` (`:252`) |
| list nodes | — | — (a *resource*, `yidam://corpus/<class>`, `resources.rs:34-41`) | `yidam_list_nodes` (`:225`) |
| neighbors / walk | — | `neighbors` (`tools.rs:39`) | — |
| open questions | `open_questions` (`:422`) | `open_questions` (`tools.rs:52`) | `yidam_open_questions` (`:360`) |
| phase status | `phase_status` (`:422`) | — | — |
| sangha positions | `sangha_positions` (`:422`) | — | — |

Three tool names collide only on `open_questions`. `retrieve`/`semantic_query`/`yidam_semantic_search`
name one operation three ways; BOSC further *splits* it into a keyword tool and a vector tool where
Rust keeps one adaptive tool. `get_node`/`corpus_node`/`yidam_read_node` likewise.

The divergence runs below the names:

- **Node shape.** Rust `get_node` returns structured JSON — `{id, class, label, description,
  content, links:[{target, relationship}]}` (`tools.rs:205-215`) — serialized into the MCP text
  block (`tools.rs:71-73`). BOSC `yidam_read_node` returns a **human YAML render** — a
  `# yidam://corpus/...` header plus `node.to_dict()` YAML (`yidam_tools.py:204-207, 260-267`). TS
  `corpus_node` returns a `CorpusNode` from the *Markdown* model (RFC-0002's Model A). So even the
  envelope disagrees: JSON-in-text vs YAML-in-text vs a different model entirely.
- **The degraded signal.** Rust `retrieve` emits `"degraded": true` with keyword results when no
  vector index exists (`tools.rs:105, 190-199`) and `"degraded": false` on the vector path
  (`tools.rs:144-145`). BOSC's `yidam_semantic_search` **omits the flag** and lazily *builds* an
  index on first use (`yidam_tools.py:100-105`) — so it never signals degradation, it silently
  re-embeds in a different vector space (RFC-0006).
- **The open-question predicate.** Rust shares one predicate between tool and resource: label starts
  `?` **or** body contains `[open]` (`resources.rs:14-16`). BOSC adds a third arm — a structured
  `claim_tag == open` (`yidam_tools.py:194`) — because its nodes carry no `?`/`[open]` in label or
  text. The replica and the tool it claims parity with already disagree on the same corpus (RFC-0001).
- **Resources exist only in Rust.** `serve --mcp` serves `yidam://graph/summary`,
  `yidam://corpus/<class>`, `yidam://corpus/<class>/<name>`, `yidam://skills/<name>`,
  `yidam://decisions/<name>` (`resources.rs:26-66`, `read` `:68-118`). BOSC's in-process SDK server
  has **no resource channel**, so it folds resources into `list`/`read` *tools*, nodes still carrying
  the `yidam://corpus/...` URI (`yidam_tools.py:11-18, 109-130`). Nothing says the two are meant to
  be the same surface.
- **The list is frozen by hand, per server.** The Rust E2E test asserts the tool list is *exactly*
  `["retrieve","get_node","neighbors","open_questions"]` (`tests/mcp_serve.rs:159-162`) — an
  idiosyncratic list, pinned in one language's test, that neither other server can satisfy.
- **Transport is per-language and unrelated to any of this.** Rust hand-rolls newline-delimited
  JSON-RPC 2.0 over stdio (`cmd/serve/mod.rs:1-9, 155-192`), *deliberately not* an MCP SDK — the
  crates need a newer toolchain than the repo pins (`mod.rs:3-5`; RFC-0003). TS uses
  `@modelcontextprotocol/sdk` (`sdks/README.md:440`); BOSC uses `create_sdk_mcp_server`
  (`yidam_tools.py:389-391`). Three transports is fine. Three *contracts* is the bug.

## Proposal

Define one canonical MCP tool + resource contract on the shared parity/spec layer. Transport stays
per-language; the contract constrains only the `tools/list`, `tools/call`, and `resources/*` JSON —
which is identical regardless of framing.

### Normalized tool vocabulary

Bare names (no `yidam_` prefix — see Open questions). **Core** tools every server MUST back:

- **`retrieve`** — `{query, k?, class?}` → `{degraded: bool, results: [{path, class, label, text,
  score}]}`. One adaptive tool: vector search when an index is loaded, keyword search otherwise,
  distinguished only by `degraded`. Subsumes `semantic_query` and folds BOSC's `yidam_query` +
  `yidam_semantic_search` into one name (its keyword path becomes the `degraded: true` branch).
- **`get_node`** — `{id}` → the **unified node model of RFC-0002** (`id, class, label, description,
  content, links:[{target, relationship}]`). Fixes the envelope: the MCP text block carries the
  canonical JSON, not a language-specific YAML render. Subsumes `corpus_node`, `yidam_read_node`.
- **`list_nodes`** — `{class?}` → `[{id, class, label, description}]`. The tool form of the
  `yidam://corpus/<class>` resource; makes a tools-only server first-class. Subsumes
  `yidam_list_nodes`.
- **`open_questions`** — `{}` → `[{id, label, path}]`, using the **frozen predicate of RFC-0001**.
  No server may add arms (BOSC's `claim_tag` extension reconciles there, not here).

**Optional** capabilities, present iff the server declares them:

- **`neighbors`** — `{id, depth?}` → the bidirectional graph walk (`tools.rs:218-271`). A served
  in-memory mirror can back it; a server that cannot omits it.
- **`phase_status`** / **`sangha_positions`** — read live `ma/*` / `rigpa/*` git refs
  (`sdks/README.md:103-110, 323-328`). Backable only by a server with the working git repo; a
  projected or on-disk mirror (BOSC) cannot, and declares them absent.

### Capability model

MCP already carries a `capabilities` object in the `initialize` response — Rust sets
`{"resources":{}, "tools":{}}` (`mod.rs:215`). Extend it with an experimental `yidam` block the
server fills honestly:

```json
"capabilities": {
  "tools": {}, "resources": {},
  "yidam": {
    "contract": "0.4.0",
    "retrieve": { "vector": true },
    "graph": true, "phases": false, "sangha": false, "resources": true
  }
}
```

A thin server (BOSC: no phases, no sangha, no MCP resource channel) declares `graph:true`,
`phases:false`, `sangha:false`, `resources:false` — an explicit, testable statement, not a hole an
agent discovers by a tool-not-found error. `retrieve.vector:false` means retrieve is always
keyword-degraded. An agent reads capabilities once and knows the reachable surface.

### Resource contract, and the tools-only bridge

The `yidam://…` URI scheme is normative: `yidam://graph/summary`, `yidam://corpus/<class>`,
`yidam://corpus/<class>/<name>`, `yidam://skills/<name>`, `yidam://decisions/<name>`
(`resources.rs:26-118`). A server that backs resources (`resources:true`) serves them over the MCP
`resources/*` methods. A tools-only server (`resources:false`) maps them **without diverging
semantically**: `read_resource(uri)` ≡ the matching tool call —
`read_resource("yidam://corpus/<class>/<name>")` MUST return the same node as
`get_node("<class>/<name>")`, and `list_nodes({class})` MUST equal `read_resource(
"yidam://corpus/<class>")`. BOSC already accepts the URI *and* the bare id interchangeably
(`normalize_id`, `yidam_tools.py:109-130`), so the bridge is a naming rule, not new code. Skills,
decisions, and `graph/summary` — with no tool peer — are exposed by a tools-only server through a
generic `read_resource({uri})` tool, or declared unavailable.

### The degraded signal is part of the contract

`retrieve` MUST always set `degraded`. `false` only when the query was embedded with the index's
own model (RFC-0006's `embed.config.json` weights). A server that cannot load those exact weights
MUST return `degraded: true` with keyword results — **never** re-embed with different settings (the
doctrine at `domain-computer.md:69-70`). This closes BOSC's silent-drift path: today it auto-builds
and reports nothing (`yidam_tools.py:100-105`); under the contract that build is legal only when it
reconciles the embedding space, else it degrades and says so.

## Conformance

A fixture spec on the parity layer — `prelude/sdks/parity/mcp/` — analogous to RFC-0001's report
fixtures:

- **`tools.json`** — the canonical tool list with input/output JSON Schemas and each tool's
  capability tier (core / optional-`graph` / optional-`phases` / …).
- **`cases/<tool>/<name>.json`** — a call → expected-response pair over a shared fixture corpus (the
  two-concept graph in `tests/mcp_serve.rs:19-50` is the seed). Cases assert the response *shape*
  and the invariant fields (`degraded`, node model, `direction`), not embedding scores.
- **`resources/<uri>.json`** — expected `resources/read` payloads, plus the tools-only-bridge
  equivalences a `resources:false` server must satisfy (`read_resource(uri)` ≡ its tool peer).

Each server runs one harness against the spec: Rust reuses the E2E pattern in `mcp_serve.rs` with
the hardcoded `assert_eq!(names, vec![...])` at `:159-162` **replaced by a check against
`tools.json`** (the frozen list becomes derived, not hand-written); TS drives its
`@modelcontextprotocol/sdk` server; Python drives `create_sdk_mcp_server`. The `get_node`/`neighbors`
cases assert the RFC-0002 unified model; the `open_questions` case, the RFC-0001 frozen predicate. A
server that declares a capability MUST pass its cases; one that declares it absent MUST return
capability-not-supported and has those cases skipped — capability flag and tool list checked for
agreement.

## Transport

Keep transport per-language; freeze only the contract. The Rust hand-rolled loop stays for the
toolchain reason it cites (`mod.rs:3-5`); TS and Python keep their SDK transports. The contract is
transport-agnostic by construction — it governs JSON payloads the three already exchange. When the
Rust pin advances (RFC-0003), the hand-rolled loop MAY be swapped for the official Rust MCP SDK with
**no contract change**, because conformance is on the payloads, not the framing. The one transport
rule the contract imposes: `initialize` MUST return the `yidam` capability block, and `tools/list`
MUST match the declared capabilities.

## Migration & compatibility

- **Rust CLI** is already canonical for `retrieve`/`get_node`/`neighbors`/`open_questions`. Add
  `list_nodes` (today resource-only), emit the `yidam` capability block, and drive the E2E list from
  `tools.json`. No renames.
- **TS SDK** renames `semantic_query`→`retrieve`, `corpus_node`→`get_node`; keeps `open_questions`;
  declares `phases`/`sangha` so `phase_status`/`sangha_positions` survive as optional-capability
  tools. Ship deprecation aliases for one minor of `@yidam/core`.
- **BOSC** renames `yidam_list_nodes`→`list_nodes`, `yidam_read_node`→`get_node`,
  `yidam_open_questions`→`open_questions`, and folds `yidam_query`+`yidam_semantic_search`→`retrieve`
  (keyword path = `degraded:true`); switches `get_node` output from YAML render to the RFC-0002 JSON;
  adds the `degraded` flag; declares `phases:false, sangha:false, resources:false`. The
  `mcp__yidam__*` namespace is unaffected — it comes from the *server* name, so bare `get_node`
  already yields `mcp__yidam__get_node`, dropping the redundant prefix rather than adding one.
- **Versioning.** The contract is a joint SDK+parity artifact; bump `prelude/sdks/parity/VERSION`
  (today `0.3.0`) — a minor for additive capability declarations, a major when a tool renames without
  an alias. The template-layer Rust CLI states the contract version it implements in the `yidam`
  capability block, so drift between binary and spec is visible (RFC-0004).

## Alternatives considered

- **Standardize on the official MCP schema types** (`@modelcontextprotocol/sdk`, the `rmcp` Rust
  crate) as the shared type surface. Rejected for now: Rust cannot take the dependency at the pinned
  toolchain — the exact reason `serve` is hand-rolled (`mod.rs:3-5`; RFC-0003). Keeping the contract
  as plain JSON Schema lets all three conform without a common type dependency; revisit once RFC-0003
  lifts the pin.
- **Namespaced (`yidam_*`) tool names.** Rejected. MCP already namespaces by server —
  `mcp__<server>__<tool>` — so BOSC's `yidam_read_node` double-namespaces to
  `mcp__yidam__yidam_read_node`. Bare names are safe even for an agent connected to several servers,
  because the server name disambiguates.
- **Leave transport-and-contract coupled per language** (status quo). Rejected: it is exactly what
  produced three incompatible surfaces from one corpus.

## Open questions

- **One `retrieve` or two?** The contract folds keyword and vector into one adaptive tool with
  `degraded`. BOSC deliberately split them (`yidam_query` vs `yidam_semantic_search`,
  `yidam_tools.py:270, 310`) to offer exact-term matching as a distinct affordance. Is that split a
  blessed optional `search` capability, or a `mode` parameter on `retrieve`?
- **Do `phase_status`/`sangha_positions` belong in an MCP tool contract at all,** or are they a
  TS-only agent-context concern (`assembleContext`, `sdks/README.md:404-412`)? They read live git
  refs; no served mirror can back them, so they may never be more than a single-server capability.
- **`get_node` payload encoding.** The contract fixes the *model* (RFC-0002) but the MCP `content`
  block is text. Is the text canonical JSON (Rust, `tools.rs:71-73`) or a rendered document a model
  reads more fluently (BOSC's YAML, `yidam_tools.py:204-207`)? A `structuredContent` field
  alongside the text block may let both coexist.
- **Is `list_nodes` a tool everywhere,** given Rust exposes listing only as a resource? The answer
  depends on whether tools-only servers are first-class — this RFC says yes (BOSC is the reference),
  but a resource-first server may find the duplicate tool redundant.
