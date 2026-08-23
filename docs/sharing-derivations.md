# Sharing a derivation

*How one derived corpus is published, how another consumes it, and what changes — and
pointedly does not change — once it has.*

A yidam corpus is built by one repository's sangha, under one ontology, with one revision
history. Composition is what makes it useful to anyone else: a `.yiz` bundle leaves the
repository that built it and is read somewhere that did not.

That crossing is the whole subject of this document. Everything below is either a mechanism
for making it happen or a limit on what it is allowed to mean.

## The shape of it

| | Producer | Consumer |
|---|---|---|
| Declares intent | `.yidam/publishable` | `.yidam/tonpa.toml` |
| Runs | tag `v*` → `release.yml` | `yidam tonpa add`, `install`, `update` |
| Produces | `bundle.yiz` on a GitHub release | `.yidam/tonpa/<name>/`, `.yidam/tonpa/tonpa.lock` |
| Gate | the publish guard, then `graph-check` | `yidam tonpa verify` |

## What a bundle is

A `.yiz` file is a gzipped tar archive. It is a **corpus**, laid out the way the repository
that produced it lays out its own — which is why the code that reads an installed dependency
is the same code that reads local instances, and why a change to instance parsing cannot
apply to one and not the other.

```text
manifest.yml               provenance and counts
corpus/<name>.ont.yml      class schemas
corpus/<class>/<name>.yml  instances
skills/<name>.md           skills
decisions/<name>.yml       decision records
index/corpus.md            rendered instance table
index/graph.md             graph integrity report
index/decisions.md         rendered decisions table
index/skills.md            rendered skills table
index/corpus.arrow         Arrow IPC, if an index was present
index/meta.json            vector index metadata, if an index was present
index/embed.config.json    embedding reproducibility contract, if an index was present
```

`manifest.yml` carries `bundle_version`, `commit`, `genesis`, `generated_at`, `domain`, and
counts of `classes`, `instances`, `skills` and `decisions`, plus `vector_index_model` —
which is `null` when the bundle carries no index.

Note what is **not** in the list: no `sangha/`, no `catalog/`, no git history. A bundle
carries the corpus's claims, not the record of who settled them or what they were checked
against. That absence is load-bearing and the section on citation returns to it.

### The version contract

`bundle_version` is `"1"`. It is incremented **only on a breaking change**: removing a
field, renaming a field, changing a field's type, or removing a file from the archive
layout. Adding a field or a new archive entry is not breaking.

The obligation this places on a consumer is the usual one and it is not optional: **ignore
unknown fields and unknown archive paths.** A consumer that fails on an unrecognized entry
converts every additive change into a breaking one, and the version number stops meaning
anything.

The authority for this is [`cmd/bundle.rs`](../yidam/cli/src/cmd/bundle.rs)'s
`render_bundle` doc comment, which is where the layout is actually defined. If this document
and that comment disagree, the comment is right and this document is stale.

## Publishing

### 1. Opt in, in the repository

Create `.yidam/publishable`. Say in it **who may read this corpus and why that is
acceptable.**

The file is not a security control — anyone who can push a tag can add a file. It is a
statement of intent that lives in the repository and outlasts the person who made it.
Without it, `git tag v1 && git push --tags` would publish an entire corpus, and *"I did not
know the tag did that"* is not something anyone can take back after the first download.

### 2. Tag

`sadhana/github/workflows/release.yml` runs on `v*`. It also runs on `workflow_dispatch`,
which builds and guards without publishing — use it. **A publishing workflow nobody has ever
run is a workflow that gets debugged for the first time on the day it matters.**

The workflow fails closed at every step. A check that cannot establish that publishing is
safe refuses to publish.

1. **`.yidam/publishable` must exist.**
2. **No declared-private material may enter the bundle.** See below.
3. **`yidam graph-check` must pass.** Publishing a corpus that fails its own gate ships
   broken edges to every consumer, where they read as the producer's claims.
4. **`yidam export --format bundle`**, attached to the release as `bundle.yiz` — that exact
   name, because that is what the consumer resolves.

### The privacy guard, and why it is not ci.yml's

`ci.yml` already has a privacy job. The release guard is stricter, and the difference is the
point:

> `ci.yml` protects material that sits **in** the repository, and permits declared-private
> material as long as the repository is private. That is the right rule for a checkout and
> the wrong one for an artifact. **The artifact outlives the access.**

So `.yidam/private-paths` is read again at release time against the three directories that
actually enter a bundle — `.yidam/corpus`, `.yidam/skills`, `.yidam/decisions` — and a match
fails the release *whether or not the repository is private*. Private material inside a
published bundle is one download away from anywhere.

That list of bundled directories is duplicated in the workflow, and the workflow says so. A
directory added to the bundle and not added there is a directory the guard does not know it
is publishing.

### An index is opt-in, by omission

The workflow installs the light build. `export --format bundle` includes a vector index
**only when one is already built and committed**, so publishing never needs protoc or an
ONNX runtime. A corpus is useful to a consumer without an index; requiring the ML stack in
order to publish would put sharing behind a toolchain nobody needs to read Markdown.

## Consuming

`tonpa` is in the CLI's default feature set, so every published binary carries it.

### Fetched dependencies

```sh
yidam tonpa add org/repo          # → releases/latest/download/bundle.yiz
yidam tonpa add org/repo@v1.2.0   # → releases/download/v1.2.0/bundle.yiz
yidam tonpa add https://…/bundle.yiz
```

`add` writes the declaration to `.yidam/tonpa.toml`, fetches the bundle, records its
`sha256` in `.yidam/tonpa/tonpa.lock`, and unpacks it to `.yidam/tonpa/<name>/`. **Commit
both files.** The unpacked directory is a build product; the declaration and the lock are
the record.

| Command | Answers |
|---|---|
| `tonpa status` | Is each declared dependency installed, locked, and intact? |
| `tonpa list` | What is installed — node counts, embedding model, genesis date |
| `tonpa verify` | Do the installed bundles still hash to what the lock says? Nonzero if not |
| `tonpa install` | Bring the tree into agreement with the lock, fetching at the pinned hash |
| `tonpa update [name]` | Re-fetch at the *latest* bundle and move the pin |
| `tonpa remove <name>` | Drop the declaration, the lock entry, and the files |

The distinction that matters is `install` versus `update`. **`install` fetches at the hash
in the lock; `update` fetches whatever is published now and rewrites the lock.** A CI job
runs `install` and `verify`. A person decides to `update`.

A fetched dependency is **stale by construction**: it is whatever was published, until
someone updates it. That is not a defect. It is the only property that makes a consumer's
build reproducible when the producer's corpus is under active revision.

### Path dependencies

A sibling repository, read where it sits:

```sh
yidam tonpa add ../sibling-corpus
```

Nothing is fetched, hashed, or locked — **hashing a working tree that changes under you
records nothing.** `tonpa status` reports it as `[linked]` and unpinned; `install` skips it;
`update` declines it and says why.

This is the only form that supports a development loop. An edit in the producer is visible
in the consumer without cutting a release, which is how you find out whether the ontologies
actually meet before either side commits to a version.

A name declared as a path dependency **wins over an unpacked bundle of the same name**. The
path form is the one someone is actively editing, and silently preferring a stale unpacked
copy would make an edit appear to have no effect — the one failure a development loop must
not have.

## What changes once a dependency is installed

Deliberately narrow: **readable and searchable, never an edge target.**

**Keyword retrieval spans dependencies.** `serve --mcp`'s `retrieve` returns foreign nodes
alongside local ones whenever it is answering from keyword search — which is every call in
the default build, and every call in any build over a corpus with no index. Each result
carries an `origin` field: the package name for a foreign node, and `null` for a local one.
`null` rather than absent, so a client testing the key never has to distinguish "this node
is local" from "this server is old and never said."

**Vector retrieval does not.** `yidam embed` reads `.yidam/corpus/` and `.yidam/catalog/`
and nothing else, so an index holds this repository's own nodes; and a vector result carries
neither `id` nor `origin`, only `path`, `class`, `label`, `text` and `score`. On a composed
corpus, `degraded: true` is therefore the **more complete** search — the one arm that sees
the whole of what is installed.

That is a gap, not a rule. Everything else in this section is an argued boundary — a
foreign node may be read and never cited — and that boundary holds under either retrieval
path. Nothing in it implies search should get *narrower* when an index is present.

The history is plain enough: both arms shipped together, and dependency-spanning was added
to only one of them, by the change that made an installed bundle readable at all. That
change's own opening line was that `tonpa` fetched a corpus and *nothing* read it — "not
`serve --mcp`, not `graph`, not `neighbors`, not the index". It repaired the first of those
and left the last. So until the embedding step learns about `.yidam/tonpa/`, a
dependency-aware agent should know which arm it is talking to. The handshake's
`retrieve.vector` says, before the first query.

**Foreign ids are qualified.** A dependency's node is `pkg::class/name`, and `get_node`
accepts that form. An id a client is shown and cannot then fetch is a worse affordance than
not surfacing the node at all.

**Its `path` points into the dependency** — `.yidam/tonpa/<pkg>/corpus/<class>/<name>.yml`,
never into the local corpus. An agent that opens the file gets someone else's file.

**Nothing else reads a dependency.** Not `graph`, not `neighbors`, not `graph-check`, not
`lint`, not `status`. Traversal does not cross the boundary, and no local edge resolves to a
foreign node. Reports stay local, and that is a correctness property rather than a
limitation: a `status` or `lint` that silently counted another repository's nodes would make
every corpus metric in every derived repository meaningless.

> **Both halves are in the same build.** `serve --mcp` used to need `--features index`
> while `tonpa` was in the light default set, so the binary that could fetch a dependency
> was not the binary that could read one. It is now — and note which way round that landed:
> the default build's keyword retrieval is the arm that spans dependencies, so composition
> works out of the box and `--features index` trades that reach for semantic ranking.
> [mcp-server.md](mcp-server.md) says what an agent should do with `origin`.

## The epistemic status of a cross-corpus citation

The graph model says an **edge is a claim**, and the constitution has articles about who may
assert one. A citation into a corpus with a different ontology, its own electors, and its
own revision history is not the same object as a local edge — and it does not become one by
arriving as a side effect of a package manager.

So the boundary above is not an implementation stage. It is the answer:

- A foreign node may be **read** and **retrieved**. It is evidence an agent can consult.
- A foreign node may **not** be an edge target. A local claim cannot rest on it structurally.
- Foreign nodes carry their own links, for display. They resolve within their own corpus.

`graph-check` needs a rule for what happens when the far side moves or a pinned bundle goes
stale — and under this boundary it needs none, because nothing local depends on the far side
holding still. That is most of why the boundary is where it is.

Recall what a bundle omits: no sangha, no elector register, no resolution history. A
consumer receives a producer's conclusions with none of the apparatus that made them
accountable. A claim tag that means *"this elector body checked this"* in the producing
repository means *"someone's elector body checked something"* here.

**`cites-external` — a genuine cross-corpus edge — is deferred to its own argued change.**
It wants an RFC and a rule for the far side moving. It does not want to be shipped because a
fetch already worked.

## Working in a repository that has dependencies

Guidance for an agent; see also
[`prelude/guidelines/agent-conduct.md`](../yidam/prelude/guidelines/agent-conduct.md).

**Check `origin` on every retrieval result — and know when there is none to check.** A
result with a non-null `origin` is not this corpus's claim: it was settled by another
sangha, under another ontology, and this repository's constitution never governed it. A
result with no `origin` key at all is a vector result, which means the search never looked
outside this corpus; absent is not the same as `null`, and treating it as such is how a
foreign node would get read as a local one. `get_node` answers with `origin` either way.

**You may reason from a foreign node. You may not cite it as though it were local.** Put
what you took from it in a local node, in this corpus's own terms, tagged at this corpus's
own standard — and say in prose where it came from. That local node is the thing this
sangha becomes accountable for.

**A foreign node's claim tag is the producer's tag, not yours.** `[verified]` there means
*that* corpus's electors accepted *that* provenance. It does not transfer. The rule that a
derived assertion travels only as far as the weakest claim beneath it applies across the
boundary too, and the boundary is a place where "weakest" is genuinely unknown — nothing in
the bundle tells you how that corpus adjudicates.

**Two ontologies agreeing on a word is not agreement.** Classes are named per-corpus. A
`concept/risk` in a dependency and a `concept/risk` here are two nodes that share a string.
Treat a shared name as a question worth investigating, not as an identity.

**A stale dependency is a normal state, not a finding.** It is pinned. If its currency
matters to a conclusion, read `tonpa list` for the genesis date and the pinned commit, and
say in the node which pin you read.

## Related

- [what-yidam-is.md](what-yidam-is.md) — the graph model, and why an edge is a claim
- [information-architecture.md](information-architecture.md) — the corpus layout a bundle mirrors
- [web-interface.md](web-interface.md) — the other bundle contract, for the web shell
- [constitutional-governance.md](constitutional-governance.md) — who may assert an edge
- [post-genesis-measurement.md](post-genesis-measurement.md) — why a report counting foreign nodes would be meaningless
