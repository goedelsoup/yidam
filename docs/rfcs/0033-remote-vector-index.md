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
  asked *"which of these already says something about X"*.
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
*impossible*. Three of the ten keys an index is allowed, spent on a phase that may never
happen, to avoid the one failure mode that cannot be repaired.

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

`retrieve` pushes its class filter, because there the filter is a claim about the row.
**An anchored step does not**, and the reason is worth stating: a remote row's `class` was
written by whatever `yidam embed` wrote when the index was built, and a node's class is computed
now from its parent directory. The two derivations agree today and nothing holds them to it — a
pushed predicate assuming they agree would turn a disagreement into an empty result rather than
an error. The ownership test is authoritative either way, so not pushing costs a wider fetch and
never a wrong answer.

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

**That identity is an assumption, and it is recorded as one.** What the unit tests pin is that
the conversion is monotone decreasing — a nearer vector always scores higher, which is the
property ranking depends on and which holds under any affine reading of the metric. The
absolute values are §8's business.

### 4.7 — A declared remote index wins

A corpus that writes `[index.remote]` is queried out of it, even when a local `.yidam/index/`
is also present. Falling back to the local index when the service is unreachable was considered
and rejected: two indexes that can disagree, switched between silently, is a ranking whose
provenance nobody can state.

What happens instead is what already happens when there is no index at all — keyword search,
under a reason that says why.

## Configuration

```toml
[index.remote]
kind   = "s3-vectors"
bucket = "yidam-corpora"
index  = "yidam-main"
region = "us-east-1"
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

## What is not verified

There is **no emulator for S3 Vectors** — no MinIO, no localstack path this crate can rely on —
so nothing in CI reaches the service. That is a fact about the design's test strategy, not an
excuse: it is why the loops take the transport as an argument and are exercised against a
recorded one, and why `transport.rs` is kept thin enough to check by eye.

What that leaves standing, and what would settle each:

| Claim | Settled by |
|---|---|
| A request signed for `s3vectors` is accepted | The live smoke test. Unit tests pin that the scope differs from `s3` and that the digest covers the body sent; only a server can say the whole is right |
| `score = 1 - distance` for cosine | Embedding one query against one corpus locally and remotely and comparing scores element-wise. Until then the tests pin monotonicity only |
| A hand-populated index is consumable by Bedrock Knowledge Bases | Nothing yet. The documented Bedrock path is ingestion from an S3 data source into an index it manages, so this is a premise of a later phase, not a claim of this one. §4.2's key set is what keeps the question open rather than answering it |
| The 40 KB metadata ceiling is comfortable for real corpora | Measuring the `text` length distribution across the derived corpora. The push truncates and flags rather than failing, so the cost of being wrong is visible rather than silent |

## Phases

1. **This.** One corpus, one remote index: push, query, verify, report.
2. **AWS-native consumers.** Populate the Bedrock keys §4.2 already declares — after testing the
   premise in the table above.
3. **Many corpora, one index.** Keys are already prefixed and `corpus` is already filterable, so
   this is a filter and a way to render a foreign corpus's node reference (RFC-0032), not a
   re-push.
4. **Scale.** Drop the assumption that `k` is small and a corpus fits in memory. Measure first.

## Open questions

1. **Is the ambient credential fallback right?** The argument in §5 is that one index crosses no
   boundary. A corpus whose vectors are meant for a narrower audience than its shell is the case
   that would falsify it, and nothing has met one yet.
2. **Should `indexed_commit` be answerable for a remote index?** It is `None` today, because
   reading it would cost a round trip at startup on a path that may never search. The commit is
   on every record's metadata; a phase that wanted the answer could put it on the witness.
3. **What should an anchored step do about class push-down?** §4.5 declines it on a stated
   premise about two class derivations. Holding them to each other with a guard would make the
   push-down safe and is a smaller change than it sounds.
