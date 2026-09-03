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
the Homebrew tap, mise and `cargo binstall` all give you. No protoc, no ONNX runtime, no C
toolchain. Through mise, that is:

```sh
mise use -g "github:goedelsoup/yidam[version_prefix=cli/v]@latest"
```

`version_prefix` filters this repository's four tag prefixes down to the CLI's. Without it
`@latest` resolves the editor release, which ships only a `.vsix` — so it fails rather than
installing something stale. See [installation](installation.md#mise).

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
cargo install --git https://github.com/goedelsoup/yidam --tag cli/v0.8.0 --locked \
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

### Claude Code, as a plugin

This is the one that gives you both halves. Four of the thirteen tools below exist because
the practice is documented in the prelude and *an agent that has to hold that prose in
context complies by having remembered* — and the plugin is what puts the prose and the tools
in the same install.

```
/plugin marketplace add goedelsoup/yidam
/plugin install yidam@yidam
```

What arrives: the MCP server, registered, plus five skills that fire at the point each
decision is made — before a commit subject, before an evidence tag, before a link, before a
`cites:`, and before answering a question from the corpus rather than from the model. Each
one names the tool to call and points at the prelude for the reasoning; none of them restates
a rule, because a second copy of a closed vocabulary is a second thing to hold in step. About
680 tokens are always on; the rest is paid only when a skill fires.

**The plugin carries no binary.** Install `yidam` first — any channel in
[installation](installation.md) — and the plugin's launcher will find it. If it cannot, it
says so with the install line rather than failing as a dead server, and if `yidam` lives
somewhere off `PATH`, set `YIDAM_BIN` to it.

**It refuses to serve a directory that is not a corpus.** A plugin is installed once and
Claude Code starts its servers in every project you open; the launcher checks for `.yidam/`
first, because a server that starts outside a corpus answers every tool with nothing and says
so only in a banner nobody reads. That is the working-directory footgun below, closed rather
than documented.

`bootstrap.md` does not travel. It is the prelude skill for an *empty* repository and is
actively wrong to load into a corpus that already exists — which is what the plugin installs
into.

### Claude Code, by hand

Run this from inside the corpus repository:

```sh
claude mcp add yidam -- yidam serve --mcp
```

`--scope project` writes it to `.mcp.json` in the repository instead of your local settings,
which is what makes the server part of what a collaborator checks out rather than something
each of them rediscovers. You get the server and none of the skills.

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

### Over HTTP, for a client that cannot spawn a process

Every configuration above spawns `yidam` as a subprocess, which a client can only do on a
machine it is running on. A hosted assistant reached through a browser cannot, and neither can
the OpenAI Responses API: they take a URL.

```sh
yidam serve --mcp --http                       # http://127.0.0.1:8787/mcp
yidam serve --mcp --http --port 0              # any free port; the banner says which
yidam serve --mcp --http --bind 0.0.0.0 --port 8080
```

One endpoint, `POST /mcp`, carrying one JSON-RPC message per request. It answers with a single
JSON object; a notification gets `202` and no body. `GET` returns `405` — this server sends no
messages a client did not ask for, so it opens no event stream, which the MCP spec allows in as
many words.

**It is the same server.** The tools, the capability block, the `absence` and `degraded` fields
are produced by the same code the stdio transport calls; only the framing differs.

**Read-only, one corpus, and it authenticates nobody.** The defaults reflect that:

| | |
|---|---|
| `--bind` | `127.0.0.1`. A server on `0.0.0.0` is reachable by anything on the network, and this one asks no one who they are. |
| `--allow-origin` | Empty. A request carrying an `Origin` header is refused unless you name it; one carrying none — `curl`, and every server-to-server client — passes. |

The `Origin` rule is the MCP spec's defence against DNS rebinding, where a page on some other
site drives a server bound to your loopback. A browser always sends the header, so naming the
origins that may reach it is what makes the check mean something:

```sh
yidam serve --mcp --http --allow-origin https://chat.example
```

To expose it beyond your machine, put it behind something that terminates TLS and supplies
authentication. Neither is in this transport, and neither is planned for it.

---

## 3. The tools, and when an agent should reach for each

| Tool | Answers | Reach for it when |
|---|---|---|
| `retrieve` | Top-k nodes for a natural-language query, with `class` and `k`; an empty answer says which kind of empty | You do not know which node holds the answer |
| `get_node` | One node's full YAML content and its outgoing links | You know the id and need what it actually says |
| `neighbors` | Nodes linked to one node, both directions, to `depth` hops | You want the argument around a node, not the node |
| `list_nodes` | Every node, optionally in one class | You want the shape of the corpus |
| `open_questions` | Nodes flagged as unsettled | You are looking for what is *not* known |
| `claims` | Assertions with the standing each is made at, filterable by standing | You want what the corpus *holds*, not the documents holding it |
| `check_subject` | Whether a commit subject is in the closed vocabulary | Before you write the commit |
| `check_citation` | Whether a `cites:` into a dependency would hold, and which of the four checks would fire | Before you write the citation |
| `claim_tags` | The three tags, their meanings, and how each may be written | Before you tag a claim |
| `licensed_edges` | What a class declares it may link to | Before you write the link |
| `query` | A typed path over the graph — `reach -measured-by-> gage`, optionally `across` the dependency set | You know the *shape* of the answer, not the node |
| `pack` | That path's answer as prose, filled to a token budget, with what did not fit | You are about to write from the corpus and have a budget |
| `estimate` | What that would cost, in nodes and approximate tokens, before you pay for it | You have a budget and want to know what fits |

**`claims` returns assertions; every other tool returns documents.** A node is 2–10
sentences by the model's own rule, so asking `get_node` what a corpus takes as verified means
paying node-sized tokens for a claim-sized answer and reading the tags out of prose yourself.
`claims` gives you the statement and its standing, and `sources` names the catalog entries
its node cites. It serves the tag or serves nothing — an untagged sentence is prose, and
prose is what `get_node` is for.

**`check_citation` is the one that cannot be answered by reading.** `retrieve` reaches a
dependency, `get_node` reads a node out of one, and `query --across` walks them — and none of
them says whether leaning on what it returned would stand. The package may be installed at a
different pin than the one you are about to write, and a `span:` is a claim about text that no
read-tool checks. It answers with the check ids the gate would report, the installed set, and
the pin each dependency actually carries — which is the value a correct `commit:` must hold and
is reachable no other way on this surface.

**`check_subject`, `claim_tags` and `licensed_edges` are the practice, callable.** The commit
vocabulary, the evidence tags and the edges a class licenses are all documented in the prelude, and an agent that has to hold
that prose in context complies by having remembered. These make it cheap to ask instead, at
the point in the loop where the decision is actually made. The prose stays: it carries the
reasoning, which is what makes the rules arguable.

**`query` walks by the types; `neighbors` floods.** `neighbors` chains outbound and inbound
edges unconditionally and filters on neither relationship nor direction — it carries both out
as labels on the result and reads neither as an input. `query` takes the relationship and the
direction as the question. The difference matters most where it is least visible: a
misspelled relationship comes back from a flood as a plausible neighbourhood and from `query`
as a rejection naming the near miss.

**Both say why an empty answer is empty.** Zero rows is otherwise indistinguishable from a
bad embedding, a class nobody has written into, and a corpus that genuinely has no view — and
an agent that cannot tell those apart fills the gap from its own weights. `absence` carries a
code read off what the corpus *states*: the class is declared and empty; it has instances and
none has that value (and here are the values it does have); the relationship is declared and
no instance authors it; the edges exist and go somewhere else. **An empty result is where an
agent invents**, and this is the field that stops it. `absence.elsewhere` names installed
packages holding what this corpus does not — a pointer, and whatever it names is that corpus's
claim rather than this one's.

**Every tool answers from the corpus the server loaded at startup.** `retrieve`, `get_node`,
`neighbors` and `query` all read the corpus and index built on disk when the process started —
there are no live git operations and no per-request file reads. `query --select body` is
included: it returns the node text as it was read then, not as the file reads now. A server
left running while the corpus is edited keeps answering with the corpus it was started
against, which is what the staleness banner at connect time is reporting. This is one
snapshot on purpose: a `body` that reached for the working tree at request time would be the
one field, on the one tool, answering about a different corpus than every other field beside
it. Restart to move the snapshot.

**A `query` response says what it is about.** `kind` is `query`, and the CLI's `--between`
emits a series under the same envelope — a client must not tell the two apart by testing for
an absent key. `at` is the commit the answer is about, null for the corpus the server holds,
and present on rejected responses too: a refusal about a tag and a refusal about now are
different claims.

**`pack` is `query` with a budget and a receipt.** `query` reports `matched` beside
`returned`, which says how many nodes it dropped; `pack` says *what kind* — `omitted_by_class`
— and that is the difference between knowing you are missing 28 nodes and knowing they were
all `recording` instances. Only the second lets you decide between spending more budget and
reporting that the corpus does not cover this. It is unbudgeted unless you ask for a budget,
and the token figure is `chars / 4` and says so: an honest approximation beats a
precise-looking number computed with the wrong tokenizer.

**`estimate` quotes; `pack` accounts.** An agent budgets in tokens and could otherwise only
discover what a retrieval cost by paying for it — so the only strategy available was to ask
for less than might be needed and hope. `estimate` runs the traversal, returns none of the
prose, and prices the same match set at each projection with a `fits` verdict against your
budget. `chars` is exact — it is the payload that would come back — and `~tokens` is
`chars / 4` and says so; use `chars` if you have a real tokenizer. The decision it serves is
not *whether* to ask but *how much of each node to ask for*, which is why it comes back as a
table rather than a number.

It is also the most speculative thing here, and the epic that asked for it says so: if agents
do not act differently given a quote, this is the surface to drop. Nothing depends on it.

`licensed_edges`, `query`, `pack` and `estimate` are the four a server may not back — all of them need
the class definitions, and a projected mirror can hold nodes and edges and no `.ont.yml`. Such a
server declares `"ontology": false` in the handshake, which is a statement you can read rather
than a hole you discover.

**A dependency is queryable, and only if you ask.** `query` takes `across: true`, which runs
the query over every installed dependency's corpus as well — one execution per corpus, every
result attributed by `origin`, foreign ids qualified `pkg::class/name.yml`. Without it you
cannot see a foreign node at all, which is the boundary rather than an oversight. A hop never
crosses between corpora: two corpora sharing a class name is not agreement, so the walk stays
inside whichever corpus it started in. `scope` on the response says what actually happened —
asking to span a repository with no dependencies installed answers `local`, and that is not an
error.

`pack` and `estimate` have no `across`, deliberately. A pack is what you write *from*, and one
mixing two corpora would put a dependency's prose under this repository's class names, where
the `omitted_by_class` receipt would be arithmetic over a category nobody declared. Retrieve
across, query across, then pack what you decided to keep.

**`retrieve` finds; `get_node` reads.** The distinction is worth stating plainly because
the gap is easy to miss: a `retrieve` result carries `label`, `class`, `path`, `score` and a
`text` — the node's `description` on the keyword path, the indexed embedding text on the
vector one. On a corpus that writes real prose into `description` that `text` is substantial,
which is exactly what makes the omission quiet. It is still not the node. The raw YAML is in
`get_node`'s `content`, and **the links are only there** — an agent that only ever retrieves
sees the claims and never the edges between them, which on a knowledge graph is most of what
it came for. Retrieval chooses what to read; `get_node` is the read.

**An empty `retrieve` says which kind of empty it is.** `results: []` used to mean four
different things at once, and an agent that cannot tell them apart fills the gap from its own
weights under a claim that will be attributed to having worked in the corpus. Every response
carries `rejected` and `absence`, both null on an answer that found something:

- A `class` that names no declared class is **rejected** (`unknown-class`, with the near miss)
  rather than searched. A filter that cannot match cannot produce a true negative.
- An empty answer carries `absence: {code, message, instances}`. `class-unpopulated` means the
  class is declared and nothing has been written into it; `no-term-match` means keyword search
  read every node and none of them uses these words, which is a statement about the words and
  not about coverage; `class-unindexed` means the corpus has the nodes and the index predates
  them, so re-embed. On a corpus with no `.ont.yml` the class rows cannot be derived at all,
  and `class-undeclared` says exactly that instead of guessing.

`instances` is how many nodes the filter admitted to the search. *None of four* and *none of
nine hundred* are different facts, and it is the difference between "nobody has written this"
and "your words missed it".

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

**Over HTTP there is no banner to read.** The server is on another machine, so the same facts
are in the handshake — see `capabilities.yidam.corpus` below. Nothing about the check changes;
only where you look for it.

**`capabilities`, on `initialize`.** The server declares what it backs under
`capabilities.yidam` rather than letting a client discover the holes through
tool-not-found errors:

```json
{"contract": "0.13.0",
 "corpus": {"domain": "streamflow", "commit": "a1b2c3d",
            "nodes": 8, "skills": 1, "decisions": 2,
            "indexed_commit": null, "stale": false},
 "retrieve": {"vector": false, "reason": "no_index"},
 "graph": true, "ontology": true, "dependencies": true,
 "phases": false, "sangha": false, "resources": true}
```

`corpus` is the banner, in the protocol (contract 0.13.0). Every other key here says what this
server *can do*; this one says *which corpus it is*, and until 0.13.0 that was only ever
printed to stderr — which a client that spawned the server can read and one that reached it by
URL cannot.

`stale` has three states. `true` when the index was built at another commit; `false` when it
was not, which covers both *current* and *no index at all* — `indexed_commit` distinguishes
those; and **`null` when the server cannot tell**, which is the honest answer from a projected
mirror with no working git repository behind it. Null rather than absent, so a client never has
to tell "not stale" apart from "a server too old to say".

`phases` and `sangha` are false and will stay false for this server: both read live `ma/*`
and `rigpa/*` refs, and this one reads a built model on disk. `ontology` follows the corpus:
a repository with no `.ont.yml` has no class contract to back, and the four tools at that
tier — `query`, `pack`, `estimate`, `licensed_edges` — are then neither listed nor callable.

`dependencies` follows the corpus the same way: it is true iff this server resolved at least
one installed dependency, and `check_citation` is neither listed nor callable when it did not.
A server with nothing installed *could* serve the tool and answer
`external-citation-unresolved` to every citation put to it — correct every time, and a
statement about a dependency set it does not have. That is the same thing the contract
forbids one tier over, where a server with no `.ont.yml` must not call a class unpopulated.

**A tool this server does not back refuses by name.** Calling one returns an MCP tool error
whose text begins `capability-not-supported`, naming the capability that is false, rather
than `unknown tool`. The two are different repairs: one says you mistyped a name, the other
says this server declines a name the contract froze. Before contract 0.11.1 an unbacked tool
was merely absent from `tools/list` and answered anyway, which put the hole back where the
capability block exists to close it.

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

**The vector path does not span dependencies.** `yidam embed` gathers text from
`.yidam/corpus/` and `.yidam/catalog/` and nowhere else, so the index `index-build` writes
holds this repository's own nodes; an indexed `retrieve` returns local results carrying no
`origin`. Note that absent is not `null` here — a key that is missing means the search never
looked outside this corpus, which is a different fact from a node being local. On a composed
corpus, `degraded: true` is currently the *more* complete search.
[sharing-derivations.md](sharing-derivations.md) has the history and why this is a gap rather
than a boundary.

**`id` is on both arms, and used not to be.** This paragraph read "carrying neither `id` nor
`origin`" and stated the two absences as one fact. They are not one fact. A local node has an
id under either arm, and the vector path simply did not emit it — so `retrieve` found nodes
and `get_node` could not be handed them, on the arm a corpus gets *for having built an index*.
The degraded keyword path was the followable one. Fixed in #425: the vector arm resolves each
row through the same `find_node` that `get_node` resolves with, so an id it hands back is an
id that fetches by construction, and it is `null` rather than absent for a row that resolves
to no node — a catalog source, or an index built before a file moved.

**An agent reading a foreign node is reading someone else's claim.** It is evidence to
consult, not a commitment this repository has made — and it may never be an edge target.
`neighbors` refuses a qualified id for exactly that reason rather than pretending the node is
missing. [sharing-derivations.md](sharing-derivations.md) is the document on what a
cross-corpus citation is and is not; an agent working a composed corpus should have read it.

---

## 5. When it answers nothing

In rough order of likelihood.

**The working directory.** Check the domain and node count first — in the banner over stdio, in
`capabilities.yidam.corpus` over HTTP. Started outside a
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

**`query` or `pack` finds nothing.** Read `absence` rather than guessing: it says whether the
class is declared and empty, whether it has instances that all fail the predicate (and what
values they do carry), whether the relationship is one the ontology promises and no instance
has written, or whether the edges are there and land somewhere else. Each has a different
repair, and only the first and third mean the corpus is genuinely quiet on the subject.

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
