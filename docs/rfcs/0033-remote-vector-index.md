# RFC-0033 — A vector index a corpus is queried out of, not one it carries

- **Status:** Draft
- **Track:** I28
- **Relates to:**
  - RFC-0023 (which gave a built index a way to travel, as a tarball a reader fetches whole; this is the other answer to the same problem and does not replace it)
  - RFC-0003 (whose feature partition this extends by one feature that costs zero packages, and whose `vector-read`/`index` split is why a push needs no protoc)
  - RFC-0005 (whose MCP tool contract freezes the `degraded_reason` vocabulary; §6 adds a fourth value the way that freeze prescribes, and bumps the contract in all three copies)
- **Versioning layers touched:** tooling (`yidam index-push`, the read path, `doctor`) / **MCP contract** (§6 — a minor bump, in all three copies). No on-disk format change: `.yidam/index/` keeps its three files and its meaning.
- **Verified against AWS documentation on 2026-09-19.** Every limit and shape in §3 is quoted from the S3 Vectors API reference rather than recalled; the figures are load-bearing and two of them decide the design.
- **Not yet verified against a live service.** §8 says exactly which claims that leaves standing and which test discharges each.
- **Amended 2026-09-20 (#834).** §8's Bedrock row was settled against the documentation and came back false; §8.1 records what that kills, what survives, and what is still unasked. §4.2 and the phase list are corrected to match.
- **Amended 2026-09-20 (#837).** §4.5's class push-down is no longer deferred. The premise it was deferred on is stated more precisely — it is about two *derivations*, never about data drift — and discharged: one derivation, and a `class_source` claim an index earns by checking its own records. Open question 3 is answered. `embed.config.json` gains one optional key and `format_version` does not move, which is what that document already says an additive field does; an index without the key is read exactly as it was before, so nothing on disk has to be rebuilt.
- **Amended 2026-09-21 (#835).** Phase 3 is built: §4.8 is the spanning filter, the rendering of a foreign node's reference, and the invariant that makes a shared index safe to read across. Three things it decided that this document did not: *every corpus in the index* is **not** offered, because it can only be written as a negative predicate whose semantics nobody here has measured; a push into an index whose witness disagrees is **refused**, because one index carries one contract and the second push would overwrite the first; and a foreign row's `id` is RFC-0032's absolute identifier rather than a handle, because there is no node here to resolve. The MCP contract goes to **0.24.0** in all three copies, and `retrieve` gains a terminal route.
- **Amended 2026-09-21 (#848).** §8's last row is settled, and it needed no account: §8.3 records the composed-`text` distribution over 3,246 rows in sixteen derived corpora. The answer is *not* "comfortable" — one row in 3,246 is cut, and the largest row that is not cut clears the ceiling by 597 bytes. The same pass found `MAX_FILTERABLE_METADATA_BYTES` declared and never read; it is enforced now, and §8.3 records the headroom that had made the hole latent.

## Summary

`.yidam/index/` is built only by a binary compiled `--features index` — protoc 31 plus an ONNX
runtime — so a corpus's vectors exist on whichever machine could build them and nowhere else.
RFC-0023 gave them a way to travel: pack the directory, hash it as one object, put it in a
vault, record the digest. A reader fetches the whole thing.

Amazon S3 Vectors is a store that holds vectors and answers similarity queries over them, with
no infrastructure to run and no server to keep up. This proposes that a corpus may declare one,
mirror its index into it with `yidam index-push`, and be queried out of it — by `retrieve`, by
an anchored `query`, by anything that goes through `retrieval::load`.

**It does not make semantic search available to the light build**, and that is the first thing
to say because it is the natural assumption. Answering a query means embedding the query text,
which is `fastembed`, which is `vector-read`. What stops being local is the corpus's vectors,
not the model.

## Problem

### 1 — An index is where it was built

RFC-0023 states the gap it was closing:

> `.yidam/index/` is built only by a binary compiled `--features index` — protoc 31 plus an
> ONNX runtime. The release workflow builds the light default, and nothing keeps the index in
> git.

Its answer works and has a shape: the index is an artifact, addressed by content, fetched
whole. For a reader who wants the index, that is right. For a reader who wants *an answer*, it
is a hundreds-of-megabytes download to ask one question — and the thing they were going to do
with it is a dot product against five rows.

### 2 — The retrieval layer has no adopters

Measured across roughly 170 derived-repository sessions, `query`, `pack` and `estimate` ran
**zero** times; corpora are built with `grep`. Whatever else is true, a retrieval layer that
requires a heavyweight build, a manual index step and a transfer is not being adopted, and a
proposal that adds surface to it owes an account of why this one is different.

The honest account is narrow. This removes one of those three obstacles — the transfer — and
leaves the other two standing. It is not a bet that remote vectors will produce adoption; it
is the phase that makes the *next* two questions askable, because a corpus whose vectors are in
one place can be asked a question by something that is not in that place.

### 3 — Two things a local index cannot become

Both are named now because they decide two irreversible choices in §4, not because they are
being built here:

- **Many corpora, one index.** Eighteen derived corpora, each with its own file, cannot be
  asked *"which of these already says something about X"*. **Built (#835), and §4.8 is what it
  cost: a filter, a rendering, and one refusal on the push side.** The key prefix is what made
  it that rather than a re-push of eighteen corpora, which is the whole of why it was decided
  here for a phase that did not exist.
- **Consumers that are not yidam.** Bedrock Knowledge Bases and OpenSearch read an S3 vector
  index. A file in a vault is legible to nothing but this crate.

## What S3 Vectors is

Quoted, not recalled:

- Service namespace `s3vectors` — **not** `s3`. API version `2025-07-15`, endpoint
  `https://s3vectors.<region>.api.aws`, SigV4 only, 34 regions.
- JSON over `POST`, one path per operation: `/CreateIndex`, `/PutVectors`, `/QueryVectors`,
  `/GetVectors`, `/ListVectors`, `/DeleteVectors`. No query strings, no path parameters.
- `dataType` is `float32` and nothing else. `dimension` 1–4096. `distanceMetric` is
  `euclidean` or `cosine`.
- `PutVectors` and `DeleteVectors` take ≤500 vectors; any request payload is ≤20 MiB.
- `QueryVectors` takes `topK` ≤10,000 and returns **≤100 results per page**.
- Per vector: ≤40 KB of metadata total, ≤50 keys, of which ≤2 KB may be filterable.
- **Non-filterable metadata keys are fixed at `CreateIndex` and cannot be changed afterwards.**
- Writes are strongly consistent.

Two of these decide more than they look like they do.

**`topK` ≤10,000 but ≤100 per page.** A reader who takes `topK: 400` to mean four hundred rows
in one response has written a loop that silently returns a quarter of what it asked for. The
read path pages.

**Non-filterable keys are irreversible.** An index created with the wrong set can only be
replaced. That is why §4.2 decides a question belonging to a phase that has not been built.

## Design

### 4.1 — Nothing new in the dependency graph

`reqwest`, `tokio`, `hmac`, `sha2`, `hex` and `serde_json` are already in the `default` feature
set through `tonpa`, `vault-s3` and `catalog-fetch`. S3 Vectors is JSON over POST with SigV4,
so there is nothing else to buy: the `s3-vectors` feature resolves **zero** new packages, and
the aarch64 cross-compile is untouched.

That is what lets it sit in `default`, where every pull request compiles it. `Cargo.toml`
records four separate occasions on which gated code shipped without a single PR having built
it, and this is the fifth feature to be put in the default set for that reason rather than for
its size.

The feature buys **the network and nothing else**. Everything that decides what would be sent
and what came back is ungated in `src/s3vectors/` — request bodies, filter translation, batch
limits, response decoding, ordering — and is unit-tested by the light build. Only the embedding
of a query is behind `vector-read`.

### 4.2 — The two irreversible choices

**Keys are `<corpus>/<repo-relative path>`**, where `<corpus>` is the first twelve characters
of the genesis hash — the one identity every corpus has and no two share. Prefixing costs
nothing now and is what makes *many corpora, one index* a filter change later instead of a
re-push of every corpus. A repository that cannot see its own root commit — a shallow clone —
is refused rather than keyed under something else.

**Non-filterable keys are `text`, `embed_config`, `AMAZON_BEDROCK_TEXT` and
`AMAZON_BEDROCK_METADATA`.** The first two are this crate's: `text` is the large field and
filtering on it is meaningless, and the witness (§4.4) carries a whole JSON document, which
filterable metadata's 2 KB ceiling and string/number/boolean/list typing have no room for.

The last two are Bedrock Knowledge Bases' internal keys, they are unused by anything here, and
they are declared anyway — because declaring them is free, and declaring them later is
*impossible*. Four of the ten keys an index is allowed, two of them spent on a route that may
never be taken, to avoid the one failure mode that cannot be repaired.

That route is **not** the one this section originally had in mind, and §8.1 is where it is
corrected: a knowledge base cannot read vectors this crate embedded, whatever their metadata
says. The two keys are held open for the day an index is built in Bedrock's own vector space,
which is the only arrangement on which they do any work.

Filterable: `corpus`, `class`, `label`, `commit`.

### 4.3 — A push is a mirror

`yidam index-push` writes every local row, then deletes the keys **under this corpus's prefix**
that the local index no longer has. The second half is the whole difference between this and an
uploader. #807/#808 recorded the same failure in another part of this repository: an additive
writer leaves entries nothing can see, and a node deleted from the corpus goes on being
findable indefinitely.

What it never deletes is anything that is not this corpus's — the witness, and another corpus's
vectors. That is a safety property rather than a nicety, because the cost of getting it wrong
is someone else's corpus, and it is asserted directly rather than argued.

Puts go before deletes, so a push interrupted between the halves leaves an index that is a
superset of the truth rather than one missing rows that exist.

`index-push` needs `vector-read` and **not** `index`: decoding `corpus.arrow` wants no protoc.
The machine that can push is not necessarily the machine that built.

### 4.4 — The witness, over the wire

`embed.config.json` is what stops a binary querying an index built in another vector space
(#536). A remote index has no file to put beside its rows, so the contract is stored as the
metadata of a reserved record, `__yidam__/embed-config`, fetched on the same first-search path
that loads the model.

The record needs a vector because a vector index holds nothing else. It is the first basis
vector — `[1, 0, 0, …]` — and it is not zeros because a zero vector has no direction, and what
a cosine index does when asked to compare against one is unspecified rather than documented.
Nothing reads it.

Its `corpus` is `__yidam_meta`, which is how every real query excludes it: each query pushes
`corpus = <genesis>` as a *positive* predicate, which the witness fails by construction. A
negative predicate would work too, and `$ne` against a vector that lacks the key has semantics
this code would be guessing at — and a wrong guess leaks an internal record into a user's
results.

A witness that cannot be read gives `Unverifiable`, not an error. A contract that could not be
fetched is not a contract that disagrees, and the alternative is a corpus whose retrieval stops
working because one `GetVectors` was throttled.

### 4.5 — One filter, read two ways

`search` used to take `keep: impl Fn(&VectorRow) -> bool`. Half of that predicate can be pushed
to a service and half cannot, and a closure cannot be asked which half it is. So a `Filter`
carries the pushable half — the class set — and the residual stays a closure at the call site.

Both backends read the one type: the local scan tests it in Rust, the remote one renders it as
a metadata filter, and an equivalence test holds the two to admitting the same rows.

`retrieve` pushes its class filter, because there the filter is a claim about the row: the
caller asked for rows the index labels `concept`, and the index's own labels are what answer
that. **An anchored step's class set is a claim about the *node* a row resolves to**, which is a
different thing, and it was originally not pushed at all. The reason as first written: a row's
`class` was written by whatever `yidam embed` wrote when the index was built, a node's class is
computed now from its parent directory, the two agree today and nothing holds them to it — and a
pushed predicate assuming they agree would turn a disagreement into an empty result rather than
an error.

**It is pushed now (#837), because the premise turned out to be checkable rather than hopeful.**
A class is a function of a path, and a row carries the path its class was derived from. So the
two derivations can disagree only if the *derivation* differed between the binary that built the
index and this one — never because a node moved, since a node's class **is** its parent
directory: a move changes the path too, and the ownership residual already rejects a row whose
path names no node.

That turns a premise held per row into a question an index can be asked once. Two things answer
it:

- **`paths::class_of_path` is the one derivation.** `yidam embed` and the query path both call
  it, so no index this binary builds can disagree with itself. `tests/class_derivation.rs` holds
  it in the *output* as well as in the source — it runs `yidam embed` over all four example
  corpora and checks every record written (45 nodes, 14 classes, measured 2026-09-20) — because
  a shared function says nothing about records already on disk.
- **An index declares whether its own rows satisfy the rule.** `index-build` checks every record
  it indexes and writes `class_source: "parent-directory"` into `embed.config.json` only when
  all of them do, so the claim is earned over the actual rows rather than asserted by whichever
  binary happened to run. That document already travels everywhere an index goes, including into
  a vector bucket as the witness record, so a remote reader gets the answer on a fetch it
  already makes.

`Filter::as_applied` is where the two meet: a filter naming nodes is applied by an index that
vouches for its class metadata and widened to `any()` by one that does not. Every index built
before the field existed is in the second case — which is exactly what this surface did before,
a wider fetch and never a wrong or empty answer. Each backend asks the question with what it
has, and a remote one asks it *after* the witness fetch rather than before, because that is when
it first knows.

Results are owned rather than borrowed, because a row that arrives over a network is not
borrowed from anything this process holds. Both backends order through one function: score
descending, ties broken on the path — the rule `vector::search` already had, for the reason it
already gave.

### 4.6 — Scores, or no answer

The local path scores a row as the dot product of two normalized vectors, which is the cosine
similarity. S3 Vectors returns a *distance*. A number in the wrong units would not look wrong;
it would rank plausibly and differently, which is exactly the failure `embed.config.json`
exists for on the other axis.

So `distanceMetric` is checked on every response and a euclidean index is refused rather than
converted. For a cosine index the score is `1 - distance`.

**That identity was an assumption, and it was recorded as one until it could be measured.** What
the unit tests pin is that the conversion is monotone decreasing — a nearer vector always scores
higher, which is the property ranking depends on and which holds under any affine reading of the
metric. The absolute values were §8's business, and §8.2 settles them: the identity is exact to
float32 precision against a live cosine index.

### 4.7 — A declared remote index wins

A corpus that writes `[index.remote]` is queried out of it, even when a local `.yidam/index/`
is also present. Falling back to the local index when the service is unreachable was considered
and rejected: two indexes that can disagree, switched between silently, is a ranking whose
provenance nobody can state.

What happens instead is what already happens when there is no index at all — keyword search,
under a reason that says why.

### 4.8 — Many corpora, one index

The storage worked from the first push. What did not exist was the *question*, and asking it
turned out to be three separate decisions.

**The filter is positive, and the set is named.** `corpus = <genesis>` becomes
`corpus $in [<genesis>, …]` — still a positive predicate, so the witness (§4.4) fails it by
construction, which is the same mechanism and not a second one. `retrieval::Corpora` is what
carries the set, ungated, and both the pushed JSON and the local predicate read it: the
equivalence test in `s3vectors::filter` now runs over a matrix that includes rows of a corpus
that was *not* asked for, which is a row a `Filter` alone cannot express.

**There is no "every corpus in this index", and that is a decision.** It would have to be
written `$nin: ["__yidam_meta"]`, and §4.4 already refused a negative predicate for the reason
that applies here unchanged: what S3 Vectors does with `$nin` against a vector lacking the key
is not documented, and a wrong guess leaks an internal record into somebody's results. So a
caller names the corpora it is asking about. Where the names come from is the listing a push
already reads: `index-push` reports the other corpora in the index with a row count each, so
`--dry-run` is a read-only way to ask an index who is in it. A corpus that wants to write those
down declares `[index.remote.corpora]`, which is a nickname table — RFC-0032 §4.5's argument,
one level down, and unique within the file that declares it.

**The serving corpus is always in the set.** A span *widens*; it cannot substitute. Every other
answer `retrieve` gives is about the corpus it was started in — `absence.instances` counts that
corpus's nodes, `get_node` reads its files — so a call that could replace it would make those
answers about something else. A caller that wants only the neighbours filters on the `corpus`
field, which is what that field is for.

**A foreign row is named by the grammar, and it is an identifier rather than a handle.**
`retrieve` resolved every row's `id` through `find_node`, which knows one repository, and has
no answer at all for a row out of another corpus's half of the index: there is no file here.
RFC-0032 is what such a thing is written in, so the id is `yidam://<corpus>/node/<class>/<name>`
— rendered by `paths::reference_of_path`, which is the one derivation from a path to a
reference and is now also what `yidam migrate references` uses. The authority is the genesis
hash and never the nickname: a result whose identity changed with the reader's configuration
would not be an identity, which is the correction the RDF export already made for its subjects
(RFC-0032 §2, P3).

`get_node` cannot fetch such an id, and the contract says so rather than implying otherwise.
That is a true statement about a corpus this server does not hold. The case where it *could*
be resolved — the foreign corpus is also installed under `.yidam/tonpa/` — is left alone: a
manifest has carried `genesis_hash` since #781, so the join is available, and nothing has met a
repository in that position. It is a smaller question than #476, which owns it.

**`corpus` and not `origin`, because one word for both would be a lie about followability.**
`retrieve`'s keyword arm already reports `origin` for an installed dependency, whose nodes this
process read and whose ids resolve. A corpus sharing a vector index is not installed and not
readable. Spelling both as `origin` would tell a client it could fetch something it cannot.

**And `text` is all a reader of a foreign row gets**, which settles the third question the
issue raised. Nothing new is sent; what changes is what it means. A local row's `text` is a
convenience beside a node the caller can open, and a foreign row's is the whole of what anyone
will ever see of that node. So the `truncated` flag #853 added matters more on a spanning call
than on any local one — a cut that went unsaid would be the entire record, halved, with nothing
saying which half.

#### One index, one vector space

A shared index has **one** witness record, and every push overwrites it. That was unremarkable
while an index held one corpus. It is the mechanism by which a shared index goes wrong: two
corpora of the same dimension and different models can both be pushed, the second overwrites
the first's contract, and a spanning query then ranks one corpus's rows in the other's space —
plausibly, with scores in the range a correct ranking has. §8.1's failure, arriving from inside
an index rather than through a consumer. *The dimension would catch it* is false: eleven of the
thirty `fastembed` models offers produce a dimension another one also produces.

So `index-push` fetches the existing witness and **refuses** a push whose contract disagrees.
`embed_config::space_disagreement` is the comparison — document against document, with no
embedder in the room, because a push loads no model — over the fields that decide what a vector
is, plus the witness probe where both documents carry one.

**Evidence, not fail-closed, and the asymmetry is argued.** A witness that *disagrees* refuses.
A witness that could not be *read* is a printed note and the push proceeds. The reason is IAM
rather than optimism: a push needs `ListVectors`, `PutVectors` and `DeleteVectors`, and a
write-only credential is a legitimate shape for one — so treating a 403 on `GetVectors` as a
refusal would break pushes that have nothing wrong with them, on a check that exists to protect
a *reader*.

**What that leaves standing.** This makes a mixed-space index detectable at the moment it would
be created, by any pusher that can read the index it is writing to. It is not a proof about an
index nobody could read. The thing that would be one is a witness per corpus rather than per
index — `__yidam__/<corpus>/embed-config`, fetched for each corpus a query names and excluded
with a reason where it disagrees — and that is a change to both the push and the read path,
with a cost on every spanning call, for a hazard nothing has met. It is named here so that
whoever meets one knows what the answer looks like.

`ops::witness_agreement` is where the rule lives, and not in `cmd/index_push.rs`: that command
is behind two features, so a decision written inside it would be one no pull request compiles.
It takes the fetch's outcome as an argument, which is the split this module's own design note
prescribes.

#### An anchored step still cannot span

`query`'s similarity anchor passes `Corpora::own` even where the index holds several corpora.
Its residual is *"and this path resolves to a node this repository owns"*, so a foreign row
could only ever be fetched and discarded — and the MCP contract already refuses the same thing
for dependencies, under `anchor-across`, for the reason that a ranking entering someone else's
corpus is a ranking nothing here can walk from.

## Configuration

```toml
[index.remote]
kind   = "s3-vectors"
bucket = "yidam-corpora"
index  = "yidam-main"
region = "us-east-1"

# The other corpora sharing that index, by a name this repository chose for each (#835).
# Optional: a query may name a corpus by its genesis hash directly.
[index.remote.corpora]
ohio-budget = "3f2a9c4d1b70"
```

`deny_unknown_fields`, so `regoin` is refused rather than silently leaving the region unset.
`region` has **no default**, unlike a vault's: there is no S3 Vectors emulator and no
compatible store, the region selects the endpoint as well as the signing scope, and defaulting
it would point a misconfigured repository at a bucket in Virginia it does not own.

Never a credential — `.yidam/config.toml` is committed. Credentials come from
`YIDAM_INDEX_ACCESS_KEY_ID` and `YIDAM_INDEX_SECRET_ACCESS_KEY`, with `AWS_*` as a fallback.

That fallback reads as inconsistent with the vault rule, which withholds `AWS_*` from any vault
but `default`, so the asymmetry is argued rather than assumed: that rule exists because a
*second* vault exists precisely when its readership differs, so inheriting the shell's identity
is the boundary failing at the moment it was drawn to hold. A corpus declares at most one remote
index. There is no second one to be confused with, and `YIDAM_INDEX_*` still wins wherever it is
set — which is how a corpus whose vectors belong to a narrower audience than its shell says so.

## The contract change

`prelude/sdks/parity/mcp/tools.json` freezes the `degraded_reason` vocabulary and says what to
do about a new one: *"a value outside this set is a divergence; a server needing one should add
it here first."*

`remote_unavailable` is added there first, and the contract goes to **0.20.0** in all three
copies — `tools.json`, `VERSION`, and the README example.

It is needed because the three existing values cannot say this. `no_index` is false: the corpus
has one and says where. `no_vector_support` is false: this binary can read an index.
`stale_contract` is a claim about the vector space, and nothing has established one when the
service never answered. A 503 reported as any of them sends a reader to a repair that cannot
work.

**One value for every remote failure**, which is a decision and not an omission. A 403 and a 503
have different repairs, and a client branching on this value is deciding one thing: whether to
trust the ranking. The answer is the same for both. Which it was belongs in the human-readable
message beside it, where it is a diagnosis rather than a contract.

**A remote index that answers is not degraded.** `remote_unavailable` describes a *call*, not a
configuration, so a server may report it on one request and nothing on the next — unlike the
three before it, which are properties of a deployment and hold for its lifetime. A client
caching the first `degraded_reason` it sees will be wrong about the second question it asks.

## What is verified, and what is not

There is **no emulator for S3 Vectors** — no MinIO, no localstack path this crate can rely on —
so nothing in CI reaches the service. That is a fact about the design's test strategy, not an
excuse: it is why the loops take the transport as an argument and are exercised against a
recorded one, and why `transport.rs` is kept thin enough to check by eye.

What that left standing, and what settled each. **Four of these rows were settled against a live
index on 2026-09-20** — §8.2 records that run and its numbers — a fifth was settled against the
documentation (§8.1), and the last was settled on 2026-09-21 by measurement over the derived
corpora, with no account involved at all (§8.3). **Six rows were closed and two of them came
back false.** #835 added two more (2026-09-21) and **both are open**: they need the account the
six were settled with. Neither is load-bearing for a *wrong* answer — a `$in` the service
rejected is a failed query rather than a bad ranking, and a witness that cannot be read is
already the "could not check" arm of §4.8 — but until this file is run against an index, the
spanning query has never been made.

| Claim | Settled by |
|---|---|
| ~~A request signed for `s3vectors` is accepted~~ | **Settled live on 2026-09-20, and true.** Every operation answered. Signing for `s3` instead is refused by the service in its own words — *"Credential should be scoped to correct service: 's3vectors'"* — so the server is recomputing the signature, which is what a unit test could not establish |
| ~~`score = 1 - distance` for cosine~~ | **Settled live on 2026-09-20, and true to float32 precision.** Against three vectors whose cosine is known by hand, the service returned distances `0.0`, `0.2928932309150696`, `1.0` for cosines `1`, `0.70710678`, `0` — residual `1.2e-8`, which is f32 rounding. Measured twice: through this crate, and through `aws s3vectors query-vectors` with no yidam code in the path |
| ~~The mirror's delete reaches the service~~ | **Settled live on 2026-09-20, and true.** A key dropped from the local set stops coming back from `QueryVectors`. Deleting the delete loop from `apply_mirror` makes the live test fail and every recorded-transport test still pass, which is the gap this row existed for |
| ~~`GetIndex`'s response shape~~ | **Settled live on 2026-09-20, and true.** `/index/dimension` and `/index/distanceMetric` are where `response::disagreement` reads them. The served body carries four fields the unit fixture does not (`vectorBucketName`, `indexArn`, `creationTime`, `encryptionConfiguration`); neither pointer moved |
| ~~A hand-populated index is consumable by Bedrock Knowledge Bases~~ | **Settled against the documentation on 2026-09-20, and false as stated** — see §8.1. A knowledge base embeds the *query* with a model it owns, and yidam's vectors are not in that model's space. §4.2's key set survives, but for the route §8.1 names rather than this one |
| A spanning query's `$in` on `corpus` is accepted and applied (#835) | **Not settled.** Every query this crate has ever sent filtered `corpus` with `$eq`, and the class filter's `$in` arm was never exercised live either — so what is open is the *operator*, not the idea. `tests/s3vectors_live.rs::a_query_can_ask_across_two_corpora_in_one_index` is written and has not been run: it puts a row in each of two corpora, asks each way, and fails if the foreign row — deliberately the **nearer** vector — comes back from a single-corpus query |
| A stored witness refuses a push from another space (#835) | **Not settled.** `witness_agreement` is unit-tested against a `Fetched` this crate built; `a_push_into_another_vector_space_is_refused_against_a_stored_witness` writes the witness through `PutVectors` and reads it back through `GetVectors`, which is where a contract that serialises but does not survive the service's own metadata would hide |
| ~~The 40 KB metadata ceiling is comfortable for real corpora~~ | **Measured on 2026-09-21 over sixteen derived corpora, and false as stated** — see §8.3. One row in 3,246 is cut, so truncation is not routine; but the largest row that is *not* cut sits **597 bytes** under the ceiling, which is not what "comfortable" claims. Measured by running the functions that compose the text — `yidam embed --dry-run` — over corpora nothing was written into |

### 8.1 — The Bedrock premise, tested and false

The row above was open from the day this RFC was written. It is now closed, against the
documentation rather than against a live service, and the answer is that **the phase as
described cannot be built** — for a reason that has nothing to do with metadata keys.

**A knowledge base always embeds the query with a model it owns.**
[`VectorKnowledgeBaseConfiguration.embeddingModelArn`](https://docs.aws.amazon.com/bedrock/latest/APIReference/API_agent_VectorKnowledgeBaseConfiguration.html)
is `Required: Yes`. There is no field for precomputed embeddings and no way to omit the model.
The [supported set](https://docs.aws.amazon.com/bedrock/latest/userguide/knowledge-base-supported.html#knowledge-base-supported-embeddings)
is Titan Embeddings G1 (1536), Titan Text Embeddings V2 (256, 512, 1024) and Cohere Embed
English/Multilingual (1024). None of them is a model `fastembed` can load.

So a `Retrieve` against a `yidam index-push` index compares a Titan or Cohere query vector
against vectors this crate produced with `AllMiniLML6V2Q`. Different weights, different space.

**And the mismatch does not announce itself.** The `fastembed` linked here offers dimensions
384 (nine models), 512 (two), 768 (ten) and 1024 (nine). Eleven of the thirty therefore produce
a dimension a knowledge base accepts. A corpus built with `BGELargeENV15` would attach, ingest,
answer, and rank by noise — with scores in the same range a correct ranking has. This is the
exact failure `embed.config.json` and the witness record (§4.4, #536) exist to make impossible,
arriving through a consumer that never reads either.

Two further findings from the same pass, neither decisive on its own:

- **The knowledge-base path caps custom metadata at 1 KB**, filterable and non-filterable
  together, and 35 keys per vector — against the 40 KB §3 quotes and `request::metadata`
  truncates to. A row that fits S3 Vectors need not fit a knowledge base reading it.
- **§4.2 said "three of the ten keys".** Four are declared and two of them are Bedrock's.
  Corrected below; the argument it was making is unaffected.

**What survives.** `AMAZON_BEDROCK_TEXT` and `AMAZON_BEDROCK_METADATA` stay in
`NON_FILTERABLE_KEYS`. #832's argument for them was that declaring them later is impossible,
and that is still true; what has changed is the route they are held open *for*. They are not
two keys away from a working knowledge base. They are two keys that would still be needed on
the day a yidam index is built in Bedrock's vector space — which is a different piece of work
and is now its own issue rather than a phase of this one.

**What is still unverified**, and deliberately so: whether a knowledge base pointed at a
correctly-spaced hand-populated index retrieves from it at all — whether `Retrieve` requires an
ingestion job to have run, what shape `AMAZON_BEDROCK_METADATA` must carry for source
attribution, and whether the vector key format is load-bearing. Those questions only become
askable once there is an index in Bedrock's space to ask them of, so they belong to that issue.
Nothing here should be read as having answered them.

### 8.2 — The live run, and why its green means anything

`tests/s3vectors_live.rs` shipped with #832 and had never been run. It was run on **2026-09-20**
against a vector bucket created for the purpose in an AWS sandbox account (`us-east-2`, a
4-dimensional cosine index), and the bucket was deleted afterwards. Six tests, all passing.

**Six passing tests is not the finding, and on its own it is not even evidence.** Without
`YIDAM_S3VECTORS_TEST` set, all six report `ok` in 0.00s having called nothing: the skip notice
goes to stdout, which cargo captures unless `--nocapture` is passed. A run against no account and
a run against a healthy index are distinguishable only by their duration. The module doc's claim
that they "skip loudly" is true only of the `--nocapture` case.

So each of the four claims was settled by **breaking the code and watching the live test fail**,
not by watching it pass:

| Mutation | What the live suite did |
|---|---|
| `score = 1 - distance` → `score = distance` | `the_score_is_the_cosine_similarity_to_five_decimals` fails, and reports the ranking exactly reversed — the failure mode §4.6 describes, produced on demand |
| `/index/dimension` → `/index/dimensions` | `the_served_get_index_body_has_the_shape_the_push_check_reads` fails on the clash it should have found |
| the delete loop removed from `ops::apply_mirror` | `a_deleted_key_stops_being_findable` fails with the deleted node still in the results; **every unit test still passes** |
| signing scope `s3vectors` → `s3` | every test fails 403, with the service naming the correct scope |

Each mutation was reverted and the full suite re-run green.

**Two things the run turned up that are not claims about the design.**

The transport is flaky on a run's first call: two of roughly ten runs failed with
`error sending request for url` before any request was answered, and passed on an immediate
retry. A live failure that is a transport error is not a finding about the code, and a person
reading a red run here should retry once before believing it.

And the `GetIndex` body corroborates a correction §8.1 made in passing: the index reports four
non-filterable metadata keys — `text`, `embed_config`, `AMAZON_BEDROCK_TEXT`,
`AMAZON_BEDROCK_METADATA` — which is the "four are declared and two of them are Bedrock's" that
replaced §4.2's original "three of the ten".

**What this run did not settle** is the last row of the table above: whether the 40 KB metadata
ceiling is comfortable for real corpora. That one needs no account at all — it is a measurement
over the derived corpora's `text` lengths — and it was left alone here rather than answered
badly. §8.3 is that measurement.

### 8.3 — The 40 KB ceiling, measured, and not comfortable

Run on **2026-09-21** over **sixteen derived corpora**: 3,246 rows, 2,759 corpus nodes and 487
catalog sources, 9,238,363 bytes of composed text.

**The composing functions were run, not a proxy for them.** `text` is not a field. For a node it
is `compose_text(label, description, links)` over `prose::text`, which since #746 reaches the
properties a class has flagged as well as the declared prose fields — so a grep for
`description:` under-counts. For a catalog source it is `compose_source_text(…)`, whose bulk is
the whole markdown body. `yidam embed --dry-run` (#848) composes every record, hands each to
`request::metadata`, and writes nothing — not the record, not `.yidam/embeddings/`, not the
directory. The corpora it was run over are read, not ours to leave artifacts in, and each was
checked unchanged afterwards: `git status` in all sixteen, the whole tree hashed before and
after in one, and — in the two that already carried an `.yidam/embeddings/` from their own
earlier runs — no file under it newer than the measurement.

**Two repositories are out of the population, and the reason is the measurement rather than
hygiene.** `watermark-directory` is a declared non-vendoring consumer: 496 corpus files on disk
and **0 tracked**, a git-ignored projection from its own exporter. Counting it measures a
generator. `enamel-pin-design` has no commits. For each of the sixteen that remain, the walked
set was compared against `git ls-files` and matched **exactly** — corpus and catalog both — so
the walk read what git tracks and nothing else.

| corpus | rows | node `text` p50 / max | source `text` p50 / max | largest row | filterable max | cut |
|---|---:|---:|---:|---:|---:|---:|
| allen-county-ohio | 864 | 3,459 / 21,076 | 3,782 / 19,860 | 21,464 | 184 | 0 |
| ohio-budget | 797 | 531 / 7,704 | 2,565 / 7,355 | 7,930 | 156 | 0 |
| hegeomai | 219 | 1,869 / 8,404 | 1,863 / 3,902 | 8,609 | 162 | 0 |
| demi-moore | 207 | 2,144 / 8,094 | 2,745 / 4,415 | 8,365 | 224 | 0 |
| matt-huffman | 185 | 3,096 / 33,610 | 7,228 / **72,910** | 40,350 | **237** | **1** |
| ohio-education-funding | 181 | 6,387 / **39,630** | 4,726 / 13,124 | **40,363** | 166 | 0 |
| grindcore | 146 | 2,000 / 23,701 | 3,191 / 27,439 | 27,941 | 167 | 0 |
| allen-recorder | 141 | 597 / 13,295 | 4,878 / 20,348 | 20,891 | 158 | 0 |
| bitrecover-bitwipe | 121 | 1,572 / 8,097 | 2,552 / 5,133 | 8,342 | 158 | 0 |
| hermetic-ch | 94 | 1,168 / 3,904 | 1,293 / 2,119 | 4,139 | 185 | 0 |
| audio-effect-design | 79 | 1,251 / 4,692 | 1,769 / 8,080 | 8,356 | 144 | 0 |
| bitlocker | 67 | 2,358 / 5,716 | 2,934 / 6,027 | 6,280 | 169 | 0 |
| shawnee-township-event-planning | 66 | 1,668 / 15,484 | 6,246 / 17,262 | 17,745 | 150 | 0 |
| yidam-proto-001e | 42 | 1,983 / 3,391 | 2,903 / 3,132 | 3,582 | 137 | 0 |
| yidam-proto-001d | 27 | 638 / 1,174 | 1,769 / 3,866 | 4,003 | 162 | 0 |
| yidam-proto-002c | 10 | 293 / 399 | — | 552 | 141 | 0 |

`allen-county-ohio`, `ohio-budget` and `ohio-education-funding` are public and the rest are
private or have no remote, so three of the sixteen rows can be re-derived by anyone with the
binary. Every figure is bytes. The last three columns are against 40,960 and 2,048.

**The distribution, merged.** Counts rather than percentiles, because counts add across
repositories and medians do not — the per-corpus p50s above are the medians, and this is the
shape they sum to. The bucket is the *whole row* as `request::metadata` renders it, which is
what the ceiling is on: every field, plus the JSON escaping that makes a byte of prose not
always a byte of body.

| row bytes | rows | share | cumulative |
|---|---:|---:|---:|
| ≤ 256 | 3 | 0.09% | 0.09% |
| 256 – 1,024 | 965 | 29.73% | 29.82% |
| 1,024 – 4,096 | 1,528 | 47.07% | 76.89% |
| 4,096 – 16,384 | 712 | 21.93% | 98.83% |
| 16,384 – 40,960 | 38 | 1.17% | 100.00% |
| > 40,960 | 0 | 0.00% | 100.00% |

**Four things this says.**

**1 — Truncation is not routine.** One row in 3,246, or 0.031%. The row said the cost of being
wrong would be visible rather than silent; the honest reading is that it has been paid once. A
finding that truncation were routine would have belonged here and would have been the answer;
it is not what the corpora say.

**2 — "Comfortable" is still the wrong word.** The largest row that is *not* cut is
`ohio-education-funding`'s `.yidam/corpus/formula-component/fsfp-local-capacity-measure.yml` at
**40,363 of 40,960 bytes** — 597 bytes of headroom, 98.54% of the ceiling, on an ordinary
corpus node nobody wrote with a ceiling in mind. A ceiling one row clears by less than a
paragraph is not a comfortable one; it is one the next edit to that node crosses.

**3 — The two tails are different, and only one of them is bounded.** Node text tops out at
39,630 bytes and averages 2,447; source text tops out at 72,910 and averages 5,108. A node's
text is what a person wrote into declared fields. A catalog source's text is a whole markdown
document, and nothing in the format caps it. The one row that is cut is a catalog source —
matt-huffman's `.yidam/catalog/ohio-lobbying-register.md`, 72,910 bytes composed and 37,399
kept, **48.7% of it dropped**. So the ceiling binds on the half of the corpus RFC-0033 was not
thinking about when it wrote the row.

**4 — The flag is written and never read.** `request::metadata` sets `text_truncated` so that
"a consumer reading `text` and finding no such key is reading all of it" — and this crate's own
consumer does not look. `response::decode_query` builds a `Hit` from `class`, `label`, `text`
and the distance, and drops every other key. So the one row that is cut comes back from
`retrieve` rendered as if whole, with nothing in the result saying half of it is missing. That
is a read-path gap rather than a push-path one, it is not fixed here, and it is
[#853](https://github.com/goedelsoup/yidam/issues/853): the fix is a field on `retrieval::Hit`
that both backends answer for and that the surfaces render, which is a change to the retrieval
layer rather than to this one.

#### The 2 KB filterable ceiling, which nothing was enforcing

`MAX_FILTERABLE_METADATA_BYTES` was declared in `s3vectors/mod.rs` and read by nothing.

It was a hole rather than a dead constant. `NON_FILTERABLE_KEYS` is `text`, `embed_config` and
the two Bedrock keys, so `corpus`, `class`, `label`, `commit` and `text_truncated` are all
filterable and share that 2 KB — and the shrink loop only ever cuts `text`, which is not in it.
A row whose filterable half was too large would pass the 40 KB check, be truncated or not, go
on the wire, and come back a `ValidationException` naming a constraint rather than a node.
**Cutting `text` cannot fix it**, which is why one check could never have stood in for the
other.

`request::metadata` now refuses such a row locally, on both its return paths — the one that
fits and the one that had to cut — with a message that names the class and the label's size.
Both guards are load-bearing: deleting either reddens a test written for it.

**It stays latent, and the measurement is why that is worth saying.** The largest filterable
half anywhere in the sixteen corpora is **237 bytes of 2,048** — 8.6× headroom, and the
per-corpus maxima run 137 to 237, a range of a hundred bytes across corpora spanning ten rows
to eight hundred. The figure barely moves because four of the five filterable keys are
structural — `corpus` is twelve characters, `commit` is seven, `class` is a directory name, and
`text_truncated` is a boolean. `label` is the only one a corpus controls the size of, and no
corpus has yet written a long one. That is an argument for an
assertion and a test — which is what this is — rather than for a redesign, and re-running
`yidam embed --dry-run` is how anyone would find out that the headroom had started closing.

## Phases

1. **This.** One corpus, one remote index: push, query, verify, report.
2. ~~**AWS-native consumers.** Populate the Bedrock keys §4.2 already declares — after testing
   the premise in the table above.~~ **Withdrawn.** The premise was tested and is false: §8.1.
   Populating the two keys buys nothing while a corpus's vectors are `fastembed`'s, because a
   knowledge base embeds the query with Titan or Cohere regardless. The route that would make
   them useful — building the index in Bedrock's vector space — is a second embedding backend,
   not a metadata change, and is tracked separately.
3. ~~**Many corpora, one index.**~~ **Built (#835): §4.8.** It was a filter and a rendering, as
   this line predicted, and it was also one thing this line did not see — an index carries a
   single embedding contract, so *one index, one vector space* had to become an invariant the
   push enforces before spanning one was safe to read. It also gave `retrieve` a terminal
   route, because a spanning question needs somebody able to ask it.
4. **Scale.** Drop the assumption that `k` is small and a corpus fits in memory. Measure first.

## Open questions

1. **Should a shared index be able to say who is in it?** #835 answers *"which corpora does
   this index hold"* out of a full `ListVectors` sweep, which is the listing a push already
   makes and is proportional to the index. A reserved roster record would answer it in one
   `GetVectors`, and would also be where a per-corpus embedding contract went (§4.8). Neither
   is worth writing for eighteen corpora and one index; both become worth it at a scale
   nothing here has met, and phase 4 is where that gets measured rather than assumed.
2. **Is the ambient credential fallback right?** The argument in §5 is that one index crosses no
   boundary. A corpus whose vectors are meant for a narrower audience than its shell is the case
   that would falsify it, and nothing has met one yet.
3. **Should `indexed_commit` be answerable for a remote index?** It is `None` today, because
   reading it would cost a round trip at startup on a path that may never search. The commit is
   on every record's metadata; a phase that wanted the answer could put it on the witness.
4. ~~**What should an anchored step do about class push-down?**~~ **Answered by #837: it
   pushes.** The premise was tighter than it looked — a class is a function of a path and a row
   carries its path, so only a derivation that differed *between binaries* can produce a
   disagreement, and an index can be asked about that once rather than trusted about it per
   query. §4.5 records the two halves: one derivation, held in the output by
   `tests/class_derivation.rs`, and a `class_source` claim `index-build` earns by checking
   every record. An index that claims nothing is searched exactly as wide as it was before.
