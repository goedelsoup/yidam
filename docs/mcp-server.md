# Connecting an agent

*How to put a corpus behind an MCP server and point an agent at it. Five minutes.*

`yidam serve --mcp` is the surface that makes a corpus reachable by an agent, which is close
to the point of the whole system. [RFC-0005](rfcs/0005-mcp-tool-contract.md) specifies the
tool contract — the names, the arguments, the response shapes — and that is what someone
implementing a second server needs. This is the other document: what to put in a client
configuration, which tool to reach for, and how to tell what you are actually connected to.

---

## 1. Which binary carries `serve`

**Any of them.** `serve --mcp` is in the light default build — the one the install script,
the Homebrew tap and `cargo binstall` all give you. No protoc, no ONNX runtime, no C
toolchain.

What the `index` feature adds is not the server but the *quality of one tool*. With it,
`retrieve` is semantic search over a vector index. Without it, `retrieve` falls back to
keyword search and says so on every call. `get_node`, `neighbors`, `list_nodes`,
`open_questions` and every resource are identical in both builds.

```sh
yidam --version
# 0.2.1 (a1b2c3d) [reports tonpa]          ← serves; retrieve is keyword
# 0.2.1 (a1b2c3d) [reports index tonpa]    ← serves; retrieve is semantic
```

If you want the semantic build, it needs protoc 31, a C toolchain and an ONNX runtime:

```sh
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.2.1 --locked \
  --features index yidam
```

Inside a yidam checkout, `mise install && mise run yidam-build` provisions the toolchain and
installs a `--features full` binary into `.local/bin`.

> Worth knowing what the fallback is, because it is not a stub. Keyword retrieval spans
> installed dependencies, labels each result with its `origin`, and qualifies foreign ids —
> which the vector path does not do. See §4. It is lexical rather than semantic, and that is
> the whole of the difference.

---

## 2. Configure the client

### Claude Code

Run this from inside the corpus repository:

```sh
claude mcp add yidam -- yidam serve --mcp
```

`--scope project` writes it to `.mcp.json` in the repository instead of your local settings,
which is what makes the server part of what a collaborator checks out rather than something
each of them rediscovers.

### Any client that takes an `mcpServers` block

```json
{
  "mcpServers": {
    "yidam": {
      "command": "yidam",
      "args": ["serve", "--mcp"]
    }
  }
}
```

Transport is stdio. The server reads newline-delimited JSON-RPC on stdin and writes frames to
stdout; **the startup banner and every warning go to stderr**, so a client that merges the two
streams will corrupt the protocol.

### The working directory is load-bearing

`serve` locates the corpus with `git rev-parse --show-toplevel` from wherever the process was
started, and falls back to the current directory when that fails. Neither step asks whether
it found the repository you meant. A server launched from your home directory does not
error — it serves an empty corpus, answers every tool with nothing, and says so only in the
banner.

The two blocks above are correct for a client that starts its servers in the project
directory. For one that does not, or when you are unsure, pin it:

```json
{
  "mcpServers": {
    "yidam": {
      "command": "sh",
      "args": ["-c", "cd /abs/path/to/my-corpus && exec yidam serve --mcp"]
    }
  }
}
```

An absolute path and `exec` — the `exec` so signals reach the server rather than the shell.

---

## 3. The tools, and when an agent should reach for each

| Tool | Answers | Reach for it when |
|---|---|---|
| `retrieve` | Top-k nodes for a natural-language query, with `class` and `k` | You do not know which node holds the answer |
| `get_node` | One node's full YAML content and its outgoing links | You know the id and need what it actually says |
| `neighbors` | Nodes linked to one node, both directions, to `depth` hops | You want the argument around a node, not the node |
| `list_nodes` | Every node, optionally in one class | You want the shape of the corpus |
| `open_questions` | Nodes flagged as unsettled | You are looking for what is *not* known |

**`retrieve` finds; `get_node` reads.** The distinction is worth stating plainly because
the gap is easy to miss: a `retrieve` result carries `label`, `class`, `path`, `score` and a
`text` — the node's `description` on the keyword path, the indexed embedding text on the
vector one. On a corpus that writes real prose into `description` that `text` is substantial,
which is exactly what makes the omission quiet. It is still not the node. The raw YAML is in
`get_node`'s `content`, and **the links are only there** — an agent that only ever retrieves
sees the claims and never the edges between them, which on a knowledge graph is most of what
it came for. Retrieval chooses what to read; `get_node` is the read.

`neighbors` is the other half of that. Half the interesting connections into a node are
inbound, and reading a node's own YAML shows you only the edges it asserts — the ones
asserted *at* it are invisible from the file.

Alongside the tools, the server publishes MCP resources under a `yidam://` scheme:
`yidam://graph/summary`, `yidam://corpus/<class>`, `yidam://corpus/<class>/<name>`,
`yidam://skills/<name>`, `yidam://decisions/<name>`. `yidam://corpus/<class>` and
`list_nodes` are required to answer identically; the resource channel exists for clients that
prefer to browse rather than call.

---

## 4. Knowing what you are connected to

Three signals, at three different moments.

**The banner, at startup**, on stderr:

```text
yidam MCP server — domain "streamflow", 8 node(s), 1 skill(s), 2 decision(s)
vector index: absent (no_index) — `retrieve` degrades to keyword search; run `yidam embed && yidam index-build` to build one
serving MCP over stdio
```

That first line is the check that matters. A domain you do not recognise, or `0 node(s)`,
means the working directory was wrong — see above. It also warns when HEAD has advanced past
the commit the index was built at, and keeps serving the stale index rather than refusing.

**`capabilities`, on `initialize`.** The server declares what it backs under
`capabilities.yidam` rather than letting a client discover the holes through
tool-not-found errors:

```json
{"contract": "0.4.0", "retrieve": {"vector": false, "reason": "no_index"},
 "graph": true, "phases": false, "sangha": false, "resources": true}
```

`phases` and `sangha` are false and will stay false for this server: both read live `ma/*`
and `rigpa/*` refs, and this one reads a built model on disk.

**`degraded` and `degraded_reason`, on every `retrieve`.** Both are present on every
response — there is no third state, and `degraded_reason` is `null` exactly when `degraded`
is `false`. `true` means the answer came from case-insensitive term matching over label,
description and body, scored by the fraction of query terms hit. That is a real answer and
often a good one, but it is lexical: it will not find a node that says the same thing in
different words.

The reason says which repair you need, and they are not the same repair:

| `degraded_reason` | What it means | What to do |
|---|---|---|
| `no_index` | This corpus has no vector index | `yidam embed && yidam index-build` |
| `no_vector_support` | An index exists; this binary cannot read it | Reinstall with `--features index` |

The distinction is why the field exists. Both answer keyword-degraded, and being told to
build an index you already built is the kind of advice that costs an afternoon. Note the
precedence: a light binary on a corpus with *no* index reports `no_index`, not
`no_vector_support` — indexing is the repair under either build, so the reason names the
nearer cause.

`yidam index-status` says whether an index exists and how stale it is.

**`origin`, on a degraded result and on every `get_node`.** Once a dependency is installed
with `tonpa`, the keyword path spans it: results carry `id` and `origin` — the package name
for a foreign node, `null` for a local one, and `null` rather than absent so a client testing
the key never has to distinguish "local" from "an older server that never said." A foreign id
is qualified as `pkg::class/name`, and `get_node` accepts that form and answers with `origin`
either way.

**The vector path does not span dependencies.** `index-build` indexes this repository's
corpus only, so an indexed server's `retrieve` returns local nodes and its results carry
neither `id` nor `origin`. The asymmetry is worth knowing before you read too much into an
absent `origin`: on a composed corpus, `degraded: true` is currently the *more* complete
search.

**An agent reading a foreign node is reading someone else's claim.** It is evidence to
consult, not a commitment this repository has made — and it may never be an edge target.
`neighbors` refuses a qualified id for exactly that reason rather than pretending the node is
missing. [sharing-derivations.md](sharing-derivations.md) is the document on what a
cross-corpus citation is and is not; an agent working a composed corpus should have read it.

---

## 5. When it answers nothing

In rough order of likelihood.

**The working directory.** Check the banner's domain and node count first. Started outside a
corpus, the server does not complain — it names the directory it landed in and serves
nothing:

```text
yidam MCP server — domain "nowhere", 0 node(s), 0 skill(s), 0 decision(s)
```

`list_nodes` then answers `{"nodes": []}`, which from the client side is indistinguishable
from an empty corpus. `0 node(s)`, or a domain that belongs to another repository, and
nothing else in this list matters. Pin the directory with the `sh -c 'cd … && exec …'` form.

**The corpus was never built out.** A repository that has been cloned but not bootstrapped
has `.yidam/` and no nodes in it. That is a legitimate empty corpus, and it looks exactly
like a successful connection to nothing. `yidam status` in the repository will tell you which
one you have.

**`retrieve` finds nothing and `list_nodes` finds plenty.** Almost always `degraded: true`
plus a query phrased in words the corpus does not use — keyword search matches terms, not
meanings. Try the corpus's own vocabulary, or read `degraded_reason` and do what it says.

**The client sees garbled frames.** Something is folding stderr into stdout. The banner is
not protocol.

---

## Reference

| For | See |
|---|---|
| The frozen tool contract — names, schemas, response shapes | [rfcs/0005-mcp-tool-contract.md](rfcs/0005-mcp-tool-contract.md) |
| The machine-readable freeze every server is tested against | [`prelude/sdks/parity/mcp/tools.json`](../yidam/prelude/sdks/parity/mcp/tools.json) |
| Dependencies, `origin`, and what a cross-corpus citation is | [sharing-derivations.md](sharing-derivations.md) |
| The index the non-degraded `retrieve` runs on | [domain-computer.md](domain-computer.md) |
| Getting a corpus to serve in the first place | [quickstart.md](quickstart.md) |
