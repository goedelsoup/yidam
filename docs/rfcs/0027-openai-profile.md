# RFC-0027 — A profile is a projection, not a second contract (the `openai` profile)

- **Status:** Draft
- **Track:** I22
- **Relates to:**
  - RFC-0005 (the contract this puts a second vocabulary beside — whose one-operation-one-name rule turns out to be the argument *for* a profile rather than against one, and whose 0.11.1 refusal clause this extends)
  - RFC-0002 (the node model `fetch` renders)
  - RFC-0019 (the rule that decides what a dependency node's `url` may say)
  - RFC-0003 (the light binary this must run in)
  - RFC-0018 (the precedent that a new surface is a surface and **not** a fourth parity function)
  - RFC-0029 (the `act` tier, whose two write tools this profile must refuse, and whose §2.5 decides the contract version)
  - RFC-0032 (whose §4.2 locator is what `url` renders, and which generalises `public_base` off this profile)
- **Versioning layers touched:** SDK+parity (`mcp/tools.json` gains a `profiles` section
  and one frozen refusal token; new conformance cases) / tooling (the Rust CLI implements
  `serve --mcp --profile openai`; `.yidam.toml` gains `public_base`) — **no template change,
  no node-model change**
- **Parent epic:** #420 — this RFC specifies **#426**. ~~Blocked on **#423** for a transport~~
  (#423 shipped 2026-09-03). Leaves Draft on one live observation (Open questions). It does
  **not** wait on #428 to be built: §5 accepts a loopback base.
- **Downstream reference case:** none yet — `examples/streamflow` under D1 (#428), by
  construction.

> **Version coordination, 2026-09-04.** Migration as first written reserved contract `0.12.0 → 0.13.0` for
> the `profiles` addition. That version is no longer available: commit 8b49753, landing the corpus
> handshake block three hours after this RFC did, consumed 0.13.0, and cli/v0.9.0 released it —
> live `tools.json` reads 0.13.0 with no `profiles` key. Per
> [RFC-0029](0029-write-tier.md) §2.5, a contract version is now **claimed at landing, never
> reserved in prose**: the `profiles` change takes the next free minor on the day it merges
> (expected 0.14.0, ahead of RFC-0029's `act` tier at 0.15.0), and the two changes are two
> separate minors, never one.

> **Re-measured against the tree, 2026-10-01.** Four weeks of other work moved the surface this
> RFC projects, and the text below is amended in place where it had gone false. What changed, so a
> reader of the September draft knows what to re-read:
>
> - **The transport exists.** #423 shipped `serve --mcp --http` behind the `serve-http` feature,
>   with `--bind`, `--allow-origin`, a bearer token and a vault-backed bundle source. The
>   "set aside, unbuilt" framing in Problem is gone.
> - **Sixteen tools, not thirteen, at contract 0.27.0.** `paths` (ontology) and the `act` tier's
>   `propose` and `cycle` were added. Fourteen go unlisted under the profile, two of them write
>   tools — §2 now says what happens when the profile meets `[serve] act = true`.
> - **`retrieve` grew.** It takes `corpora` and `where`, and answers `scope` and a per-row
>   `corpus` (#835, contract 0.24.0); `degraded_reason` gained `remote_unavailable` (RFC-0033).
>   §3 and §4 say what the profile does with each.
> - **`url` has a grammar now.** RFC-0032 §4.2 defines the locator as `https://<base>/<kind>/<path>`,
>   so a node renders `<base>/node/<class>/<name>` and not this draft's `<base>/<class>/<name>`;
>   and it reads the base as a property of the *corpus*, which `export --format rdf` is waiting on
>   too (`export_rdf.rs`, `instance_iri`). §5 moves the key out of `[serve]` for that reason.
> - **The vendor now asks for an `outputSchema`.** Re-read 2026-10-01: *"Declare an output schema
>   for each tool so clients can validate the result shape."* No yidam tool declares one today.
>   That makes the open question about the `yidam` key mostly ours to answer — §4(a) and Open
>   questions.
> - **No dependency can declare a base.** `LockedPackage` (`deps.rs:88-103`) records `url`,
>   `commit` and `sha256`, and no base. §5's per-package branch is unreachable today, so every
>   dependency node is omitted from `search` and counted.
>
> The design is unchanged: a projection, one new token, a configured base. The conformance list
> grows from seven cases to nine, plus one CLI test (§7).

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

Re-read again 2026-10-01: both quotations above still stand, and the page has gained a sentence —
*"Declare an output schema for each tool so clients can validate the result shape"* — with an
example generating `output_schema` from typed return models. It is advice, not a requirement, and
it constrains nothing outside the two tools. §4(a) takes it.

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
alternative — add two tools to the sixteen — and the rest of this RFC is that argument.

### Why the conclusion survives

Two reasons, both internal to this repository, plus one the vendor states about itself.

1. **One operation, one name (RFC-0005).** `search` and `retrieve` are one operation. Serving
   both from one list is the failure RFC-0005 was written to end — its alternatives section
   rejects the neighbouring form of this for `yidam_*` prefixes, on the grounds that one operation
   with three names is what produced three incompatible servers. The constraint that a platform
   spells the name `search` does not make it a second operation; it makes it a second *rendering*.
2. **The shapes are not compatible, and the incompatibility is load-bearing.** `retrieve` returns
   `degraded`, `degraded_reason`, `rejected`, `absence` and `scope` on every call —
   `yidam/sdks/parity/mcp/tools.json` freezes all five. The `search` shape has three fields per
   result and no envelope. A single tool cannot satisfy both without one of them lying about what
   it carries.
3. **The vendor argues against a long list on its own account.** The connectors guide:
   *"Some MCP servers can have dozens of tools, and exposing many tools to the model can result in
   high cost and latency,"* with `allowed_tools` offered as the remedy. yidam serves sixteen. A
   profile is `allowed_tools` decided by the server, which is the side that knows which subset is
   coherent.

## Problem

The state of the surface. The transport is no longer a question: #423 shipped
`serve --mcp --http` (`main.rs`, the `Serve` arm's `http` flag, behind the `serve-http` feature),
so a platform can reach a server today and nothing here waits on a transport.

`yidam/sdks/parity/mcp/tools.json` freezes sixteen tools at contract `0.27.0`: `retrieve`,
`get_node`, `list_nodes`, `open_questions`, `claims`, `check_subject`, `claim_tags` (`core`);
`check_citation` (`dependencies`); `neighbors` (`graph`); `query`, `paths`, `pack`, `estimate`,
`licensed_edges` (`ontology`); and `propose`, `cycle` (`act`, RFC-0029). None is named `search`
or `fetch`, and the two nearest disagree in shape as well as name:

| | canonical | required by the platform |
|---|---|---|
| find | `retrieve` → `{degraded, degraded_reason, rejected, absence, scope, results:[{id, path, corpus, class, label, text, score, origin}]}` | `search` → `{results:[{id, title, url}]}` |
| read | `get_node` → `{id, origin, class, label, description, content, links}` | `fetch` → `{id, title, text, url, metadata}` |

Three things follow, and each is a decision this RFC has to take rather than discover:

- **The canonical envelope has five fields with nowhere to go.** `degraded` and `degraded_reason`
  say whether an answer is semantic or keyword and what to repair; `absence` says which kind of
  nothing an empty answer is, with the denominator it is about; `rejected` says a call was refused
  rather than answered empty; `scope` says whether a span across corpora took effect. The `search`
  shape has room for none of them. Dropping them silently is the move this repository keeps
  declining to make.
- **`url` is mandatory and this corpus has no address.** It is the field a research citation is
  rendered from. A node's identifier, `yidam://<corpus>/node/<class>/<name>` (RFC-0032 §4.1), is
  well-defined and resolves nowhere a reader can follow.
- **Fourteen canonical tools would go unlisted,** and `tools.json` says in terms that this is not
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
  (`capabilities` in `cmd/serve/tools.rs`). Which vocabulary it speaks is a fact about how it was *started*.
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
(the `initialize` handler in `cmd/serve/mod.rs`) gains `"profile": "openai"`, null under the
canonical vocabulary. Without it, a client seeing two tools where the contract froze sixteen
cannot tell a conforming yidam server from a broken one.

**The profile projects tools, and only tools.** `resources/list` and `resources/read` are
unchanged under it: the platform reads neither, nothing in the `search`/`fetch` shape refers to a
resource, and a profile that also rewrote the resource namespace would be a second vocabulary
for something no platform asked to have renamed.

### 2 — Fourteen tools go unlisted, and 0.11.1 says that is not enough

Under `--profile openai`, `tools/list` is exactly `search` and `fetch`. The other fourteen are not
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

**When two refusals apply, `not-in-profile` wins.** Under the profile, `query` on an
unschematised corpus is both unbacked and unspoken. The token a caller receives should name the
repair that would make the call succeed, and no repair to the corpus will: building an ontology
leaves `query` exactly as unreachable under this profile as it was. `capability-not-supported`
sends the caller to fix something that is not the obstacle. So the profile check runs first, in
front of `refuse_unbacked` (`cmd/serve/tools.rs`), and the capability block still reports
`ontology: false` truthfully for anyone who reads it.

**The profile is read-only, and a corpus that asks to be written refuses it.** `propose` and
`cycle` are the `act` tier (RFC-0029), declared only when `[serve] act = true`. Both platform
tools are read-only by requirement and D2 holds writes closed off-platform, so under the profile
the `act` tier is unreachable whatever the corpus says. Two readings were possible: declare
`act: false` and serve, or refuse to start. **Refuse to start, naming both settings.** RFC-0029
§2.2 already decided this shape for its own clauses — *a server that was told to write and serves
reads instead is a deployment that believes something false about itself* (`ServeConfig::act`'s
doc comment) — and a silently-downgraded `act` is that sentence's exact case. The repair is one
line, and the operator who wrote `act = true` learns about it at startup rather than from a
connector that never writes.

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

The two arguments `retrieve` gained after this draft are unexposed for the same reason. **`where`**
(the date/number predicate) has no slot in a one-string call. **`corpora`** is never passed, so
`scope` is always `local` under the profile and no row can come from another corpus in a shared
vector index. That matters beyond tidiness: such a row's id is a `yidam://<corpus>/node/…`
identifier that `get_node` cannot fetch (#835 — *"an identifier rather than a handle"*), and
`search` hands out ids for the sole purpose of being fetched. Not passing `corpora` is what keeps
every `search` id fetchable; a later profile revision that spans corpora would have to omit and
count foreign rows exactly as §5 does for base-less dependencies.

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
is present. `search` is lossy per result — `path`, `corpus`, `class`, `text`, `score` and
`origin` are not in the shape — and the loss is recoverable by the `fetch` that the pair exists to make possible.

### 4 — `degraded` and `absence` have somewhere to go

The envelope fields are carried three ways, deliberately overlapping, because the one consumer
that most needs them is the one least able to read structured extras.

**(a) The canonical response rides along.** The profile's returned object is
`{results: [...], yidam: <the canonical `retrieve` response, verbatim>}`, in `structuredContent`
and in the JSON-encoded `content` string alike — the platform asks that the two carry *the same
value*, and they do. Any client that reads the extra key gets `degraded`, `degraded_reason`,
`rejected`, `absence` and `scope` unabridged. This is also what makes the projection
**invertible**, and §7 turns that into a case.

**Both profile tools declare an `outputSchema`, and the `yidam` key is in it.** The vendor page
now asks for one (re-read 2026-10-01). Declaring it turns the question this draft left open —
*would a strict validator reject the extra key?* — into one the server answers about itself: a
client that validates against the declared schema finds `yidam` declared, as an optional object,
and `results` / `id` / `title` / `url` required. What the schema cannot settle is whether the
platform's own parser holds the shape to a schema of *its* writing instead of ours; that residue
stays in Open questions. These are the first `outputSchema`s any yidam tool declares, and they
are generated from `tools.json`'s `profiles` entry, not written beside it — a schema stated in two
places is the second freeze §7.1 forbids.

**(b) A degraded retrieval appends a notice result.** When `degraded` is true, the profile
appends one synthetic result:

- `id`: `yidam:notice/degraded` — a reserved prefix no node id in a conforming corpus can collide
  with: a node id is `<class>/<name>`, a dependency's is `pkg::class/name`, a reference is
  `yidam://…`, and `:` is not a slug character (`uri::is_slug`). Conformance is reported rather
  than assumed (RFC-0032's #777 amendment), so `fetch` tests the prefix **before** handing an id
  to `get_node` rather than relying on no node being named that.
- `title`: the reason in a sentence, one per frozen `degraded_reason` — e.g. for `no_index`,
  *"Results are keyword matches, not semantic: this corpus has no vector index."*
- `url`: the locator of the base itself (§5)

`fetch` on that id returns the frozen `degraded_reason` and its repair as `text`, with
`metadata.kind = "notice"`. It is appended rather than prepended so it never displaces a real hit.

**`remote_unavailable` is a property of the call, not the deployment** (RFC-0033; `tools.json`
says so of it in terms). Its notice therefore cannot be fetched back on a later call as though it
described the server: `fetch("yidam:notice/degraded")` re-reads the reason the *server* holds now,
which for the other three values is the same answer and for this one may be none. `fetch` on a
notice whose condition no longer holds returns the notice with `text` saying so — never `node not
found`, which would tell a research agent the citation it just made was fabricated.

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

> `serve --mcp --profile openai` **requires** a public base, from `--public-base <url>` or a
> top-level `public_base` in `.yidam.toml`. Without one it exits, naming both.

**A node renders RFC-0032's locator, `<base>/node/<class>/<name>`,** built by the one renderer in
`yidam_core::uri` rather than formatted here. This draft first said `<base>/<class>/<name>`;
RFC-0032 §4.2 has since fixed the locator as `https://<base>/<kind>/<path>`, and a profile that
assembled its own spelling would be the "fourth hand-built spelling of an id" `export_rdf.rs`
already records as a defect. The notice results' `url` (§4b) is the base itself.

**The key is top-level, not under `[serve]`.** This draft put it in `[serve]` and called the base
"a deployment fact, not a corpus one". RFC-0032 §4.2 reads it the other way — *"derived,
per-corpus, from a declared base"* — and it has a second reader that is not a server: the RDF
export's subject IRIs use the locator where a base is declared and a `urn:yidam:` form where none
is, and `instance_iri` (`cmd/export_rdf.rs`) says in a comment that its locator branch "lands with
`public_base`". A key under `[serve]` read by `export` would misname what it is. `--public-base`
stays as the flag, because a base for one run — a tunnel, a localhost test — is a deployment fact,
and the flag overrides the file for that run only.

The base may be any absolute URL, `http://localhost:…` included, so testing against the Responses
API needs no deployment — the refusal is about *absence*, not about publicness. This is also why
the build does not wait on #428: #428 supplies a base worth citing, not the code that renders one.

**A dependency node cannot use the local base.** `retrieve` searches every installed dependency and
hands back qualified ids; rendering `<local-base>/upstream::concept/foo` would assert that this
corpus publishes a node it does not own. RFC-0019's rule is about edge targets and a rendered
citation is not an edge, so this is not a constitutional violation — it is simply false. The lock
file records where each dependency came from and at what commit (`deps.rs:88-103`:
`LockedPackage { name, url, sha256, commit, genesis, model, dims, nodes }`), so the honest render
is that package's own declared base. Where a dependency declares none:

> Its nodes are **omitted from `search` under this profile, and the omission is counted** — in the
> `yidam` key and, when the omission empties the answer, in the absence notice. Never silently.

**Today that is every dependency.** `LockedPackage` has no base field and nothing copies a
dependency's `.yidam.toml` into the lock, so the per-package branch has no input and the omission
branch is the only reachable one. That is acceptable for this RFC — the omission is counted, and
`fetch` still reads a dependency node by its qualified id (§6) — but carrying a base through
`deps install` is its own change to the lock format and is **not** specified here. Writing the
branch before a lock can feed it is the surface-with-no-consumer shape RFC-0032 declined for the
same reason; the profile implements omit-and-count only, and the per-package branch lands with
whatever puts a base in the lock.

`url` is what still makes D1 concrete: until #428 exists, the only true thing `--public-base` can
name is a loopback address or a tunnel, which is enough to build and measure against and not
enough to publish citations from.

### 6 — `fetch` takes qualified ids, and that is already settled

#426 lists this as open. It is not — the code answered it, and the answer is forced.

`fetch` takes the id `search` returned. `search` projects `retrieve`, and since #425 both arms of
`retrieve` return the **qualified** id: the result builder in `retrieve` (`cmd/serve/tools.rs`)
resolves a local row's path through `find_node` and takes `qualified_id()`. `get_node` already
accepts that form — `find_any_node` asks `yidam_core::uri::parse_reference` which corpus an id
names and reads the dependency when it names one, while `find_node` returns nothing for any id
that names a corpus, so a bare id can never fall through to a dependency and silently change what
this repository says about itself. Both behaviours are under test
(`get_node_reads_a_dependency_by_its_qualified_id` and its neighbours in `tools.rs`). Since
RFC-0032, `yidam://<pkg>/node/<class>/<name>` reaches the same dependency node by the same path.

So `fetch` accepts `pkg::class/name` because refusing it would break the pair, not because this
RFC decides so. What the profile adds is §5's consequence: a qualified id is fetchable and its
node is only *listed* by `search` when its package declares a base.

### 7 — Conformance

Profile cases live beside the canonical ones, under `yidam/sdks/parity/mcp/profiles/openai/`,
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
8. **`not-in-profile` outranks `capability-not-supported`** (§2). `query` over
   `corpus-unschematised/` is the case: the one call where both refusals apply, so a server that
   checks them in the other order fails it and no other case.
9. **Each profile tool's `outputSchema` is the one `tools.json` generates,** and every response in
   cases 4–7 validates against it. A schema the server served but its own answers violate is the
   failure §4(a) exists to rule out.

One property is a CLI test, not a parity case, because it is a startup refusal and not a response:
**`--profile openai` over a corpus with `[serve] act = true` exits non-zero naming both** (§2).
It belongs beside `tests/mcp_act_tier.rs`, which already runs servers over `corpus-acting/`.

## What this does not do

- **It does not add a transport.** #423 did, and the profile rides on it.
- **It does not deploy anything.** #428 owns the base `url` points at.
- **It does not touch the canonical sixteen.** No rename, no alias, no new tier. A server started
  without `--profile` is byte-identical to today's.
- **It does not open a write path.** D2 holds writes closed off-platform, both platform tools are
  read-only by requirement, and §2 refuses to start rather than serve a corpus that asked for
  `act`.
- **It does not carry a base through the lock.** §5's per-package branch waits for a change that
  puts one there.
- **It does not make a second parity function.** Following RFC-0018: a new surface is a surface.
  The projection is specified on the parity layer because it is a contract; it adds no function to
  the parity function set.

## Migration & compatibility

- **Parity layer.** `mcp/tools.json` gains `profiles` and the `not-in-profile` token — one
  additive minor, its number claimed at landing under RFC-0029 §2.5 (from 0.27.0 as of
  2026-10-01, so 0.28.0 if nothing lands first). The bump moves every place the version lives:
  `tools.json`, `mcp/VERSION`, the README and `docs/mcp-server.md` handshake examples, and a new
  record appended to `mcp/CONTRACT_SHA`. New cases under `profiles/openai/`.
  `yidam/sdks/parity/VERSION` takes a minor.
- **Rust CLI.** `Serve` gains `--profile <name>` and `--public-base <url>`, both requiring `--mcp`;
  `public_base` joins `YidamConfig` as a top-level key (§5). The profile is a rendering layer over
  the existing dispatch; it adds no retrieval or read path. It is meaningful on stdio as well as
  `--http` — nothing in the projection depends on the transport — though only `--http` reaches a
  platform.
- **RDF export.** Unchanged by this RFC. Once `public_base` exists, `instance_iri`'s locator
  branch has its input, and turning it on is RFC-0032's change to make, not this one's.
- **TS and Python servers.** Unaffected until they choose to implement the profile. A server that
  does not declares no profile and serves the canonical sixteen, which is what it does today —
  so this is additive for every existing consumer, BOSC included.
- **Derived repositories.** Nothing changes for a repository that does not pass `--profile`. One
  that does needs a base — committed as `public_base` if the corpus has a public home, or passed
  per run as `--public-base` if it does not.

## Alternatives considered

- **Add `search` and `fetch` to the canonical sixteen.** The one-line version, and legal now that
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

- **Does the platform hold `search` to a shape of its own writing?** Narrowed 2026-10-01. The
  September draft asked whether a strict `outputSchema` rejects the `yidam` key; §4(a) now has the
  server declare the schema, so a client validating against it finds the key declared. What is
  left is whether ChatGPT parses the result against *its* expected shape and refuses an extra key
  there — in which case the fallback is that the canonical fields survive only in the notice
  result and in `fetch` text, a real loss for structured clients. **This is not knowable from
  documentation and must be observed before this RFC leaves Draft.** It does not need #428: one
  `search` call through the Responses API's MCP tool, against a server behind a tunnel with
  `--public-base` set to the tunnel's URL, answers it. It is the one thing here a fixture cannot
  settle, and it does **not** block building the profile — only accepting the RFC.
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
