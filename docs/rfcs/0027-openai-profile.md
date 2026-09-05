# RFC-0027 — A profile is a projection, not a second contract (the `openai` profile)

- **Status:** Draft
- **Track:** I22
- **Relates to:** RFC-0005 (the contract this puts a second vocabulary beside — whose
  one-operation-one-name rule turns out to be the argument *for* a profile rather than
  against one, and whose 0.11.1 refusal clause this extends), RFC-0002 (the node model
  `fetch` renders), RFC-0019 (the rule that decides what a dependency node's `url` may
  say), RFC-0003 (the light binary this must run in), RFC-0018 (the precedent that a new
  surface is a surface and **not** a fourth parity function)
- **Versioning layers touched:** SDK+parity (`mcp/tools.json` gains a `profiles` section
  and one frozen refusal token; new conformance cases) / tooling (the Rust CLI implements
  `serve --mcp --profile openai`) — **no template change, no node-model change**
- **Parent epic:** #420 — this RFC specifies **#426**. Blocked on **#423** for a transport
  and on **#428** for the thing `url` points at.
- **Downstream reference case:** none yet — `examples/streamflow` under D1 (#428), by
  construction.

> **Version coordination, 2026-09-04.** Migration below reserves contract `0.12.0 → 0.13.0` for
> the `profiles` addition. That version is no longer available: commit 8b49753, landing the corpus
> handshake block three hours after this RFC did, consumed 0.13.0, and cli/v0.9.0 released it —
> live `tools.json` reads 0.13.0 with no `profiles` key. Per
> [RFC-0029](0029-write-tier.md) §2.5, a contract version is now **claimed at landing, never
> reserved in prose**: the `profiles` change takes the next free minor on the day it merges
> (expected 0.14.0, ahead of RFC-0029's `act` tier at 0.15.0), and the two changes are two
> separate minors, never one.

## Summary

#426 was filed on a requirement that does not exist. The ChatGPT surfaces do **not** demand a
`tools/list` of exactly two entries; they demand that two entries be **present**, under exactly
the names `search` and `fetch`, in exactly two shapes. The exclusivity was mine, not the
vendor's.

The conclusion survives the correction, and the reason changes. A profile is no longer the way
to satisfy a platform rule — it is the way to keep **RFC-0005's rule**, which is that one
operation has one name. Adding `search` beside `retrieve` in the canonical list would put two
names on one operation, which is the exact failure RFC-0005 exists to close and which its
alternatives section already rejects once.

> A profile is a **projection** of the frozen list, not an addition to it and not a second
> freeze. `search` is *defined as* `retrieve`, rendered into a shape somebody else owns.

Because it is a projection, it is checkable as one: the profile's own response carries the
canonical response it was projected from, and a conformance case asserts they agree. Nothing on
this surface is dropped without a field that says so.

## The premise this was filed on is wrong

#426 and the #420 table both say the ChatGPT connector demands "a `tools/list` of **exactly**
`search` and `fetch`". Both cite vendor documentation read 2026-08-30, and both instruct a reader
to re-read before building. Re-read 2026-09-03:

- **`developers.openai.com/api/docs/mcp`** (the page `platform.openai.com/docs/mcp` now 301s to):
  *"To work with ChatGPT deep research and company knowledge, your MCP server should implement two
  read-only tools: `search` and `fetch`."* It says **implement**, not *implement only*. It states
  no constraint on any other tool a server serves.
- The requirement is also **scoped to two surfaces**, not to connectors generally. A server with
  neither tool can still be added as a connector and is usable in chat; it is inert in deep
  research and company knowledge, which retrieve only through that pair.

So the accurate statement of the constraint is:

| | filed as | actually |
|---|---|---|
| the names | the list must be exactly these two | these two must be present, spelled exactly |
| the scope | a connector requires them | deep research and company knowledge require them |
| other tools | prohibited | not addressed; deep research ignores them |

The shapes were reported correctly and are unchanged: `search` takes a single query string and
returns `{results: [{id, title, url}]}`; `fetch` takes one identifier string and returns
`{id, title, text, url, metadata}`; and both *"return this object as `structuredContent` and
include the same value as a JSON-encoded string in the content array for compatibility."*

**Why this correction is worth its own section.** The wrong premise pointed at the right design
for the wrong reason, and a design justified by a rule that does not exist is a design nobody can
argue with. Held to the true constraint, the profile has to earn its place against a one-line
alternative — add two tools to the thirteen — and the rest of this RFC is that argument.

### Why the conclusion survives

Two reasons, both internal to this repository, plus one the vendor states about itself.

1. **One operation, one name (RFC-0005).** `search` and `retrieve` are one operation. Serving
   both from one list is the failure RFC-0005 was written to end — its alternatives section
   rejects the neighbouring form of this for `yidam_*` prefixes, on the grounds that one operation
   with three names is what produced three incompatible servers. The constraint that a platform
   spells the name `search` does not make it a second operation; it makes it a second *rendering*.
2. **The shapes are not compatible, and the incompatibility is load-bearing.** `retrieve` returns
   `degraded`, `degraded_reason`, `rejected` and `absence` on every call —
   `prelude/sdks/parity/mcp/tools.json` freezes all four. The `search` shape has three fields per
   result and no envelope. A single tool cannot satisfy both without one of them lying about what
   it carries.
3. **The vendor argues against a long list on its own account.** The connectors guide:
   *"Some MCP servers can have dozens of tools, and exposing many tools to the model can result in
   high cost and latency,"* with `allowed_tools` offered as the remedy. yidam serves thirteen. A
   profile is `allowed_tools` decided by the server, which is the side that knows which subset is
   coherent.

## Problem

The state of the surface, with the transport question set aside (that is #423, and it is
unbuilt — `main.rs:490-499` still offers `Serve { mcp, lsp }` and nothing else).

`prelude/sdks/parity/mcp/tools.json` freezes thirteen tools at contract `0.12.0`: `retrieve`,
`get_node`, `list_nodes`, `open_questions`, `claims`, `check_subject`, `check_citation`,
`claim_tags`, `neighbors`, `query`, `pack`, `estimate`, `licensed_edges`. Ten are `core` or
`ontology`; none is named `search` or `fetch`; and the two nearest disagree in shape as well as
name:

| | canonical | required by the platform |
|---|---|---|
| find | `retrieve` → `{degraded, degraded_reason, rejected, absence, results:[{id, path, class, label, text, score, origin}]}` | `search` → `{results:[{id, title, url}]}` |
| read | `get_node` → `{id, class, label, description, content, links, origin}` | `fetch` → `{id, title, text, url, metadata}` |

Three things follow, and each is a decision this RFC has to take rather than discover:

- **The canonical envelope has four fields with nowhere to go.** `degraded` and `degraded_reason`
  say whether an answer is semantic or keyword and what to repair; `absence` says which kind of
  nothing an empty answer is, with the denominator it is about; `rejected` says a call was refused
  rather than answered empty. The `search` shape has room for none of them. Dropping them silently
  is the move this repository keeps declining to make.
- **`url` is mandatory and this corpus has no address.** It is the field a research citation is
  rendered from. `yidam://corpus/<class>/<name>` is well-defined (`cmd/serve/resources.rs:40-71`)
  and resolves nowhere a reader can follow.
- **Eleven canonical tools would go unlisted,** and `tools.json` says in terms that this is not
  allowed on its own: *"A TOOL A SERVER DOES NOT BACK MUST REFUSE, NOT MERELY GO UNLISTED"*
  (contract 0.11.1). The clause was written about capability holes. A profile is a different
  reason for the same silence, and the clause does not currently cover it.

## Proposal

### 1 — A profile is a projection of the frozen list

Not a tier, not a capability, not a second contract.

- **Not a tier.** `tools.json`'s `tier` is `core` or a capability name, and it *subsets*: a server
  backs a tier or declares it false. A profile replaces the vocabulary rather than narrowing it.
- **Not a capability.** The capability block says what this **corpus and build** can back —
  `retrieve.vector`, `ontology`, `dependencies` are all facts a server discovers about itself
  (`cmd/serve/tools.rs:31-61`). Which vocabulary it speaks is a fact about how it was *started*.
  The two are orthogonal: a degraded server under the `openai` profile still degrades, and must
  still say so.

`tools.json` gains a `profiles` section. A profile entry defines each of its tools **in terms of a
canonical call**, never as an independent implementation:

```json
"profiles": {
  "openai": {
    "why": "ChatGPT deep research and company knowledge retrieve only through `search` and `fetch`...",
    "tools": {
      "search": { "projects": "retrieve", "call": { "k": 5 } },
      "fetch":  { "projects": "get_node" }
    }
  }
}
```

`projects` is what keeps the freeze single. There is one implementation of retrieval and one of
node reading; a profile tool is a rendering of a canonical response, so a server cannot drift the
two apart without failing a case that compares them (§7).

**The handshake says which vocabulary is live.** The `yidam` capability block
(`cmd/serve/mod.rs:439-446`) gains `"profile": "openai"`, null under the canonical vocabulary.
Without it, a client seeing two tools where the contract froze thirteen cannot tell a conforming
yidam server from a broken one.

### 2 — Eleven tools go unlisted, and 0.11.1 says that is not enough

Under `--profile openai`, `tools/list` is exactly `search` and `fetch`. The other eleven are not
served. Contract 0.11.1 requires that an unserved tool **refuse** rather than merely go missing,
because a server that answers a tool it did not list has told a client two things at once.

That rule applies here, and its existing token does not. `capability-not-supported` asserts the
server **cannot** back the tool. Under a profile it can; it is declining to speak that name in
this vocabulary, and a caller told `capability-not-supported` would go looking for a missing
index or a missing `.ont.yml` that is not missing.

So the profile brings one new frozen token:

> A call to a canonical tool while a profile is active MUST refuse with `isError: true` and text
> beginning **`not-in-profile`**, naming the active profile and the tool's canonical name.

Three tokens, three different repairs, and none of them interchangeable: `unknown tool` (a
spelling mistake), `capability-not-supported` (this server cannot), `not-in-profile` (this server
can, and not under this name). Adding the token is a minor bump of the contract; it does not
change any behaviour outside a profile.

### 3 — The mapping

**`search(query)` → `retrieve({query, k: 5})`.**

| `search` result field | from | note |
|---|---|---|
| `id` | `results[].id` | The qualified id, as of #425. See §6. |
| `title` | `results[].label` | |
| `url` | rendered | See §5. |

`class` and `k` are not exposed: the platform's `search` takes one query string, so the profile
cannot pass a class filter and `retrieve`'s `rejected` / `unknown-class` arm is **unreachable
under this profile by construction**. That is worth stating rather than leaving to be noticed —
it reads as a hole otherwise.

`k` is fixed at the canonical default of 5. A profile that quietly retrieved a different number
would be a second retrieval policy hiding inside a rendering. Changing it is a claim about how a
particular model searches, which no case in this repository can check; it is revisable on evidence
from the reference deployment (#428) and not before.

**`fetch(id)` → `get_node({id})`.**

| `fetch` field | from |
|---|---|
| `id` | `id` (qualified) |
| `title` | `label` |
| `text` | `description` and `content`, rendered as one document |
| `url` | rendered — §5 |
| `metadata` | `{class, origin, links}` |

`metadata` is optional in the platform shape and is the honest home for the three canonical
fields that have no named slot. **`fetch` is therefore lossless**: every field `get_node` returns
is present. `search` is lossy per result — `path`, `class`, `text`, `score` and `origin` are not
in the shape — and the loss is recoverable by the `fetch` that the pair exists to make possible.

### 4 — `degraded` and `absence` have somewhere to go

The envelope fields are carried three ways, deliberately overlapping, because the one consumer
that most needs them is the one least able to read structured extras.

**(a) The canonical response rides along.** The profile's returned object is
`{results: [...], yidam: <the canonical `retrieve` response, verbatim>}`, in `structuredContent`
and in the JSON-encoded `content` string alike — the platform asks that the two carry *the same
value*, and they do. Any client that reads the extra key gets `degraded`, `degraded_reason`,
`rejected` and `absence` unabridged. This is also what makes the projection **invertible**, and
§7 turns that into a case.

**(b) A degraded retrieval appends a notice result.** When `degraded` is true, the profile
appends one synthetic result:

- `id`: `yidam:notice/degraded` — a reserved prefix no node id can collide with, since a node id
  is `<class>/<name>` and a dependency's is `pkg::class/name`
- `title`: the reason in a sentence — *"Results are keyword matches, not semantic: this corpus has
  no vector index."*
- `url`: the profile's base

`fetch` on that id returns the frozen `degraded_reason` and its repair as `text`, with
`metadata.kind = "notice"`. It is appended rather than prepended so it never displaces a real hit.

**(c) An empty answer is one notice result, not zero results.** When `results` is empty, the
profile returns exactly one result carrying the `absence` — code, message, and the `instances`
denominator the message is about. *None of four* and *none of nine hundred* are different facts
about a corpus, and the slot cost nothing: it was empty.

This is a genuine trade and the cost should be stated rather than buried. **A notice can be cited.**
Deep research renders a citation from `title` and `url`, and nothing in the shape obliges it to read
`metadata.kind` and filter. A report may end up footnoting "this corpus has no vector index" as a
source. Against that: the alternative is a report that concludes a corpus is silent on a subject
when the truth is that its index predates the nodes — which is precisely the state
`class-unindexed` was frozen to name, and a materially wrong conclusion rather than an untidy
footnote.

### 5 — `url` is configured, not derived, and the profile refuses to start without one

`url` is not an identity field — `id` is. Its whole job is that a reader can follow it, and a
`yidam://` URI fails that job by construction. Putting one there chooses an unresolvable link over
an unattractive one.

> `serve --mcp --profile openai` **requires** a public base, from `--public-base <url>` or
> `[serve] public_base` in `.yidam.toml`. Without one it exits, naming the flag.

A local node renders `<base>/<class>/<name>`. The base may be any absolute URL, `http://localhost:…`
included, so testing against the Responses API needs no deployment — the refusal is about
*absence*, not about publicness.

**A dependency node cannot use the local base.** `retrieve` searches every installed dependency and
hands back qualified ids; rendering `<local-base>/upstream::concept/foo` would assert that this
corpus publishes a node it does not own. RFC-0019's rule is about edge targets and a rendered
citation is not an edge, so this is not a constitutional violation — it is simply false. The lock
file records where each dependency came from and at what commit (`deps.rs:89-102`:
`LockedPackage { name, url, sha256, commit, … }`), so the honest render is that package's own
declared base. Where a dependency declares none:

> Its nodes are **omitted from `search` under this profile, and the omission is counted** — in the
> `yidam` key and, when the omission empties the answer, in the absence notice. Never silently.

This is the clause that makes D1 concrete rather than deferred: until #428 exists there is nothing
true for `--public-base` to point at, which is why this RFC specifies a profile and does not ship
one.

### 6 — `fetch` takes qualified ids, and that is already settled

#426 lists this as open. It is not — the code answered it, and the answer is forced.

`fetch` takes the id `search` returned. `search` projects `retrieve`, and since #425 both arms of
`retrieve` return the **qualified** id: `cmd/serve/tools.rs:263` resolves the vector row's path
through `find_node` and takes `qualified_id()`. `get_node` already accepts that form —
`find_any_node` (`cmd/serve/tools.rs:193`) splits on `::` and reads the dependency, while
`find_node` (`:170`) refuses `::` outright so a bare id can never fall through to a dependency and
silently change what this repository says about itself. Both behaviours are under test
(`tools.rs:1155` and the two tests following it).

So `fetch` accepts `pkg::class/name` because refusing it would break the pair, not because this
RFC decides so. What the profile adds is §5's consequence: a qualified id is fetchable and its
node is only *listed* by `search` when its package declares a base.

### 7 — Conformance

Profile cases live beside the canonical ones, under `prelude/sdks/parity/mcp/profiles/openai/`,
running against the same fixture corpora (`corpora.json`). The properties worth freezing:

1. **`tools/list` is exactly `["search", "fetch"]`,** derived from `tools.json`'s `profiles`
   section and not restated in a harness — the mistake RFC-0005 caught in `mcp_serve.rs`, which is
   how a hand-written list became a per-language freeze.
2. **The handshake declares the profile,** and `capabilities.profile` agrees with the list served.
3. **A canonical name refuses with `not-in-profile`** — `retrieve` and `get_node` are the cases
   that matter, since they are the two that *are* backed and merely unspoken.
4. **The projection is invertible.** `search(q).yidam` equals `retrieve({query: q, k: 5})`
   field-for-field. This is the case that makes "a projection, not a second contract" a checked
   property rather than a claim in a document.
5. **Degraded appends a notice, and the notice is fetchable.** The default fixture corpus has no
   index, so every `search` over it is degraded — the arm every server hits first, exactly as
   `cases/retrieve/keyword-degraded.json` notes for the canonical tool.
6. **An empty answer is one notice, never zero results,** over `corpus-unschematised/` where the
   absence codes are reachable.
7. **A dependency node without a declared base is omitted and counted.** `corpus/` installs
   `upstream`, so this is answerable there and nowhere else.

## What this does not do

- **It does not add a transport.** #423 owns that, and nothing here can be reached until it lands.
- **It does not deploy anything.** #428 owns the base `url` points at.
- **It does not touch the canonical thirteen.** No rename, no alias, no new tier. A server started
  without `--profile` is byte-identical to today's.
- **It does not open a write path.** D2 holds `propose` closed until OAuth supplies an author, and
  both platform tools are read-only by requirement anyway.
- **It does not make a second parity function.** Following RFC-0018: a new surface is a surface.
  The projection is specified on the parity layer because it is a contract; it adds no function to
  the parity function set.

## Migration & compatibility

- **Parity layer.** `mcp/tools.json` gains `profiles` and the `not-in-profile` token; contract
  `0.12.0` → `0.13.0` (additive: no canonical tool changes). New cases under
  `profiles/openai/`. `prelude/sdks/parity/VERSION` takes a minor.
- **Rust CLI.** `Serve` gains `--profile <name>` (`main.rs:490-499`) and `[serve] public_base`
  joins `YidamConfig` (`config.rs:7-21`). The profile is a rendering layer over the existing
  dispatch; it adds no retrieval or read path.
- **TS and Python servers.** Unaffected until they choose to implement the profile. A server that
  does not declares no profile and serves the canonical thirteen, which is what it does today —
  so this is additive for every existing consumer, BOSC included.
- **Derived repositories.** Nothing changes for a repository that does not pass `--profile`. One
  that does needs a `public_base`, which is a deployment fact, not a corpus one.

## Alternatives considered

- **Add `search` and `fetch` to the canonical thirteen.** The one-line version, and legal now that
  the exclusivity premise is gone. Rejected: it puts two names on one operation, which is the
  failure RFC-0005 exists to close, and it does it twice. It also leaves the shape problem
  untouched — the two new tools would still need `url`, still have nowhere for `absence`, and
  would now carry those defects inside the canonical contract rather than in a projection of it.
- **Alias `search` → `retrieve` with the canonical shape.** Rejected for a harder reason: the
  platform will not read it. A `search` returning `{degraded, results:[{path, score, …}]}` is not
  the shape deep research parses, so the alias is a name that satisfies a checker and answers
  nothing.
- **Let the caller filter with `allowed_tools`.** Works for the Responses API and not for a
  ChatGPT connector, which has no such knob, and it moves a question about coherence to the side
  that cannot answer it.
- **Derive `url` from the git remote** — `https://<forge>/<owner>/<repo>/blob/<rev>/.yidam/corpus/…`.
  Attractive because it needs no configuration and is already known. Rejected: it guesses at a
  hosting arrangement, and on a private corpus it renders an internal repository path into a
  citation somebody publishes.
- **Put `yidam://` URIs in `url` and accept dead citations.** Rejected: the field's only purpose is
  that it can be followed.
- **Return zero results on an empty answer and lose the diagnosis.** The simplest thing, and the
  one that makes the corpus lie by omission on the surface where an agent invents. Rejected in §4,
  with its cost stated there rather than here.

## Open questions

- **Does a strict `outputSchema` reject the `yidam` key?** §4(a) assumes an extra top-level key is
  additive. If the platform validates the output schema with `additionalProperties: false`, the key
  is refused and the fallback is that the canonical fields survive only in the notice result and in
  `fetch` text — a real loss for structured clients. **This is not knowable from documentation and
  must be measured against a live connector before this RFC leaves Draft.** It is the one thing here
  that a fixture cannot settle.
- **Is the notice-as-result right, or should the absence stay in the extra key alone?** §4 takes a
  side and states its cost. A single observation of deep research citing a notice would be enough to
  reverse it.
- **Should `phase_status`-style write-adjacent tools ever get a profile?** This RFC specifies one
  profile for one platform. Whether `profiles` is a general mechanism or a place with exactly one
  entry is answerable only when a second platform asks, and the design deliberately does not
  generalise ahead of that.
- **Where does the profile's `k` come from once there is evidence?** Fixed at 5 here for the reason
  in §3. If the reference deployment shows deep research issuing many narrow searches, the number is
  a tuning decision — but it should move in `tools.json`, where a case can see it, and not in a
  server.
