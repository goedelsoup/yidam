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

## What is verified, and what is not

There is **no emulator for S3 Vectors** — no MinIO, no localstack path this crate can rely on —
so nothing in CI reaches the service. That is a fact about the design's test strategy, not an
excuse: it is why the loops take the transport as an argument and are exercised against a
recorded one, and why `transport.rs` is kept thin enough to check by eye.

What that left standing, and what settled each. **Four of these rows were settled against a live
index on 2026-09-20** — §8.2 records that run and its numbers — and a fifth was settled against
the documentation (§8.1). One row is still open, and it is the one that needs no account:

| Claim | Settled by |
|---|---|
| ~~A request signed for `s3vectors` is accepted~~ | **Settled live on 2026-09-20, and true.** Every operation answered. Signing for `s3` instead is refused by the service in its own words — *"Credential should be scoped to correct service: 's3vectors'"* — so the server is recomputing the signature, which is what a unit test could not establish |
| ~~`score = 1 - distance` for cosine~~ | **Settled live on 2026-09-20, and true to float32 precision.** Against three vectors whose cosine is known by hand, the service returned distances `0.0`, `0.2928932309150696`, `1.0` for cosines `1`, `0.70710678`, `0` — residual `1.2e-8`, which is f32 rounding. Measured twice: through this crate, and through `aws s3vectors query-vectors` with no yidam code in the path |
| ~~The mirror's delete reaches the service~~ | **Settled live on 2026-09-20, and true.** A key dropped from the local set stops coming back from `QueryVectors`. Deleting the delete loop from `apply_mirror` makes the live test fail and every recorded-transport test still pass, which is the gap this row existed for |
| ~~`GetIndex`'s response shape~~ | **Settled live on 2026-09-20, and true.** `/index/dimension` and `/index/distanceMetric` are where `response::disagreement` reads them. The served body carries four fields the unit fixture does not (`vectorBucketName`, `indexArn`, `creationTime`, `encryptionConfiguration`); neither pointer moved |
| ~~A hand-populated index is consumable by Bedrock Knowledge Bases~~ | **Settled against the documentation on 2026-09-20, and false as stated** — see §8.1. A knowledge base embeds the *query* with a model it owns, and yidam's vectors are not in that model's space. §4.2's key set survives, but for the route §8.1 names rather than this one |
| The 40 KB metadata ceiling is comfortable for real corpora | Measuring the `text` length distribution across the derived corpora. The push truncates and flags rather than failing, so the cost of being wrong is visible rather than silent |

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

**What is still unsettled** is the last row of the table above: whether the 40 KB metadata ceiling
is comfortable for real corpora. That one needs no account at all — it is a measurement over the
derived corpora's `text` lengths — and it was left alone here rather than answered badly.

## Phases

1. **This.** One corpus, one remote index: push, query, verify, report.
2. ~~**AWS-native consumers.** Populate the Bedrock keys §4.2 already declares — after testing
   the premise in the table above.~~ **Withdrawn.** The premise was tested and is false: §8.1.
   Populating the two keys buys nothing while a corpus's vectors are `fastembed`'s, because a
   knowledge base embeds the query with Titan or Cohere regardless. The route that would make
   them useful — building the index in Bedrock's vector space — is a second embedding backend,
   not a metadata change, and is tracked separately.
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
3. ~~**What should an anchored step do about class push-down?**~~ **Answered by #837: it
   pushes.** The premise was tighter than it looked — a class is a function of a path and a row
   carries its path, so only a derivation that differed *between binaries* can produce a
   disagreement, and an index can be asked about that once rather than trusted about it per
   query. §4.5 records the two halves: one derivation, held in the output by
   `tests/class_derivation.rs`, and a `class_source` claim `index-build` earns by checking
   every record. An index that claims nothing is searched exactly as wide as it was before.
