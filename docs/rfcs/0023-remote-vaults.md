# RFC-0023 — Bytes the catalog fetched and never kept (remote vaults)

- **Status:** Draft
- **Track:** I18
- **Relates to:** RFC-0003 (the light binary this must run in, and the feature gate it argues
  about), RFC-0001 and RFC-0016 (the report contract the new checks and the new report emit
  on), RFC-0019 (which established that a `.yiz` bundle is a tarball with no object store
  behind it — this is the object store), RFC-0005 (the MCP contract this deliberately does
  not touch)
- **Versioning layers touched:** template (catalog frontmatter gains an optional field;
  `prelude/guidelines` gains a rule) / tooling (`yidam` CLI implements it) — **no
  parity-surface change and no MCP contract change**; see [What this does not
  touch](#what-this-does-not-touch)
- **Parent epic:** #411 — this RFC specifies **#412** through **#418**, one per phase

## Summary

The catalog records that a source was fetched and keeps nothing that was fetched. The derived
artifacts have the inverse problem: they exist, they are large, and they have nowhere to live
that a second machine can reach.

Both are the missing half of a store, and this specifies one:

> **A vault stores bytes. Git stores the record of them** — which bytes, and which vault they
> are allowed in.

Every pointer into a vault is a committed file, so a vault holds no mutable state at all.
That single constraint is what makes the feature safe to add to a repository whose entire
thesis is that the graph *is* the git history: losing a vault costs no knowledge claim, only
the time to re-fetch.

Vaults are **named**, and a name carries an audience. The first pass allows exactly one, named
`default`, and refuses a second — but it refuses it in the plural config shape, for reasons
argued below that are about migration rather than about isolation.

## Problem

### The catalog records retrieval and holds nothing retrieved

`prelude/guidelines/directories.md` is explicit about what the flag means, and the sentence is
correct as far as it goes:

> **`obtained: true` means fetched, not read.** The flag is about retrieval and nothing else,
> and an entry can be honestly marked retrieved while every claim inside the document has gone
> unexamined.

What it does not say, because there has never been a reason to, is that **nothing anywhere
holds what was fetched.** There is no hash, no bytes, and no way to check. An entry marked
obtained and an entry marked obtained *falsely* are the same observation, and every check in
this repository passes on both.

That is a gap of a particular kind. The guidelines go on to describe a derived repository that
audited all 23 of its entries and found three documents "located, fetched, cited by nothing,
summarized nowhere", and conclude: *"Nothing detects this: the flag is true, the entry is
well-formed, and every check passes."* The remedy offered is to write the body honestly, which
is right and is a remedy a person performs. There is a second half nobody can perform by
hand — whether the document the entry names is the document anyone still has — and it needs a
hash.

### The template already promised the column

The REGEN header in [`sadhana/catalog/README.md`](../../sadhana/catalog/README.md) has always
specified what `catalog-audit` reports:

```
Fields per entry: slug, source type (paper/dataset/API/database/other), description,
                  integrity status (hash present/absent, stale flag),
                  corpus citation count (used-by).
```

[`cmd/catalog.rs:207`](../../yidam/cli/src/cmd/catalog.rs) emits six columns and none of them
is that one:

```
| Entry | Type | Description | Obtained | Nodes | Elsewhere |
```

This is worth stating plainly rather than filing as a defect in the report. The column was
specified and never built **because there has never been anything to hash.** Building the
store is what makes the promise implementable; nothing else does.

That the report already carries a fixed comment saying two of its columns "were specified and
neither was emitted" — and then emits them — is the precedent for how this lands: the
specification was right, and the missing substrate is the thing to build.

### The derived artifacts have the opposite problem

`.yidam/index/` is a LanceDB store. It is gitignored as `*.lance/`, it is built only by a
binary compiled `--features index`, and that feature needs protoc 31 and an ONNX runtime.
The release workflow builds the light default — [`Cargo.toml:100`](../../yidam/cli/Cargo.toml),
`default = ["reports", "tonpa"]` — for the reasons RFC-0003 argues at length.

The consequence has been accepted as a fact of life and is worth naming as a defect:

> **The binary almost everyone installs cannot build the index that upgrades `serve --mcp`'s
> `retrieve` from keyword to vector search, and there is no channel by which an index built
> anywhere else reaches it.**

`retrieve` degrades, reports `degraded_reason`, and is entirely correct to — the index is
genuinely not there. The MCP contract's `retrieve` capability block exists so a client learns
this at connect time rather than one failed search later. All of that machinery reports the
absence accurately and none of it can end the absence.

The same shape covers `.yidam/embeddings/`, and `bundle.yiz` has a partial answer already: a
GitHub release, which is public and therefore unavailable to a private corpus.

### The shape exists, for exactly one artifact

`tonpa` fetches bytes over HTTPS, hashes them with `sha256_hex`
([`deps.rs:115`](../../yidam/cli/src/deps.rs)), records the hash in a lock file, and re-hashes
on `verify`. That is a content-addressed store with one artifact kind and one location.

It also just demonstrated the module structure this needs. Commit `99713ec` moved `load_lock`
and `sha256_hex` out of `cmd/tonpa/` and into `deps.rs`, because `doctor` must read a lock in
a build where `cmd::tonpa` does not exist — the feature buys the *network*, and reading a file
and hashing it are not network operations. This RFC adopts that split from its first commit
rather than rediscovering it.

## Design

### The vault stores bytes; git stores the record

Every artifact is identified by the SHA-256 of its bytes. Every *pointer* to an artifact —
which hash a catalog entry obtained, which hash this commit's index is — lives in a committed
file.

What that buys, in descending order of how much it matters:

- **A stale vault cannot lie.** The hash is in the commit; the bytes either match it or they
  are not the bytes.
- **Losing the vault loses no knowledge claim.** The corpus survives its own storage, which is
  the property this repository is built on and the one a remote store most threatens.
- **Push is idempotent and deduplication is free.** Two repositories citing the same paper
  store it once; so do two vaults.
- **Garbage collection is exactly computable:** anything the working tree does not name.

### Rejected: mutable refs

The obvious alternative is a small mutable namespace in the store — `refs/index/latest`
pointing at a hash. It is rejected rather than deferred, and naming it is the point: it places
a pointer outside git, and *"which index corresponds to this commit"* then has two answers that
can disagree. A committed lock file costs one line and removes the question.

### The key namespace

```text
# in a vault — immutable, content-addressed, nothing else
<vault prefix>/sha256/<aa>/<64-hex>

# local cache — machine-wide AND vault-blind
${XDG_CACHE_HOME:-~/.cache}/yidam/vault/sha256/<aa>/<64-hex>   (YIDAM_VAULT_CACHE overrides)

# working copy — hardlinked from the cache, gitignored, for when a person
# or a connector needs a real file with a real name
.yidam/vault/<slug>/<filename>
```

No manifests in the store. No index objects. No `latest`. A cross-device hardlink falls back
to a copy.

The cache is deliberately **not** partitioned by vault. Bytes are bytes; the same hash in two
stores is the same file, and duplicating it locally would buy nothing. Isolation is a property
of *where bytes may be sent*, which the record answers, not the cache. Keeping that straight
is what stops a cache hit from ever being read as permission.

### Vaults are named, and a name is an audience

Not every artifact may sit in the same bucket. A repository's own index and a licensed PDF it
obtained have different readerships, and one store cannot express both.

So a vault has a name and its declaration says who can read it. The precedent is
`.yidam/publishable`, which the release path already requires
([sharing-derivations.md](../sharing-derivations.md)):

> The file is not a security control — anyone who can push a tag can add a file. It is a
> statement of intent that lives in the repository and outlasts the person who made it.

A vault's `audience` is that file's shape applied to a store. It is prose, nothing can check
it, and that is the design. What *is* checked is that it exists, so no vault is configured
without someone having written down who gets to read it.

| Vault | Holds | Audience it declares |
|---|---|---|
| `default` | the repository's own derived output — index, embeddings, bundle | whoever can read the corpus |
| `sources` | third-party documents obtained under a licence to read | the sangha only |
| `none` | embargoed material, anything unlicensed for storage | the local cache; it never leaves the machine |

Three tiers as illustration, not a fixed set — a corpus declares what it needs. What is fixed
is that every vault declares an audience, and that `none` is a routable answer rather than an
absence.

#### Routing, and a shape this RFC first got wrong

The draft above published a separate `[vault.routes]` table. **It does not parse**, and #413
found out by running it rather than by reading it. `vault` is a table of stores keyed by name,
so `[vault.routes]` is a store *called* `routes`, and the shipped binary says so:

```text
TOML parse error at line 9, column 1
  |
9 | catalog    = "sources"
  | ^^^^^^^
unknown field `catalog`, expected one of `url`, `audience`, `region`, `endpoint`, `path_style`
```

The failure is at least loud — `deny_unknown_fields` on `VaultConfig`, added so a misspelled
`endpiont` could not silently point a vault somewhere nobody intended, catches this too. But
the error describes a vault the author never meant to declare, which is a bad way to learn that
an example in the specification was never run.

**A vault declares what it holds.** `holds` is a list of artifact kinds, on the store that
takes them:

| | |
|---|---|
| one vault, no `holds` | it holds everything — which is #413's behaviour, unchanged |
| two or more | every kind is claimed by exactly one vault |
| a kind claimed by none | refused, naming the kind |
| a kind claimed by two | refused, naming both vaults |
| a record's own `vault:` | overrides the route |

Three reasons this rather than a central table, in ascending order of how much they settle it:

1. **No collision**, without nesting the stores a level deeper.
2. **The claim sits beside the audience it has to be consistent with.** *"This store is for the
   sangha, and it holds the catalog"* is one block a reader checks at once; a central table puts
   the two halves of that judgement in different places and lets them drift.
3. **`[vault.default]` has shipped.** The obvious alternative — `[vault.stores.default]`
   alongside `[vault.routes]` — works, and it moves a section derived repositories may already
   have written. That is precisely the migration cost the plural table was chosen to avoid, so
   paying it to fix a typo in an example would be self-defeating. `holds` is purely additive.

#416 implements this unless it finds a reason not to, and the reason would have to be stronger
than tidiness: the shipped section name is now a constraint, not a preference.

#### What #416 found: *refused* was not the whole rule

`holds` shipped as specified. The table above, though, says a kind claimed by no vault is
**refused** and does not say *when* — and the two readings are not the same design:

- **At resolve.** Every kind in the vocabulary must be claimed, or the config is bad.
- **At the artifact.** A kind is refused when something of that kind needs a route.

The implementation takes the second, because the first would make the list of artifact kinds a
**compatibility surface**. `catalog` is the only kind anything produces today; `index`,
`embeddings` and `bundle` are named ahead of #417 and #418 so a corpus can declare their routes
before they arrive. Under the eager reading, adding a fifth kind in a later release would turn
every multi-vault config in the wild red — for a kind those corpora have none of, and with no
way to write a config that survives both releases.

That is the failure this repository has already recorded once, in the `edge_policy` episode:
*a list of permitted values never says "and no others" unless somebody checks what the others
are.* Here nobody could check, because two of the four kinds do not exist yet.

The lazy reading loses nothing that matters. The refusal still happens before any byte moves,
it names the kind and the remedy, and `yidam doctor` reports a stranded artifact offline. What
the closed vocabulary is still *for* is the typo: `holds = ["catalouge"]` claims nothing and is
otherwise indistinguishable from a vault that meant to claim nothing. That is refused at
resolve, and it costs nothing, because it only ever looks at kinds somebody actually wrote.

Two rules were added that the RFC did not specify, both of them the same argument the plural
table was chosen for:

- **With two or more vaults, every vault declares `holds`.** The alternative is letting an
  unclaimed vault be the catch-all for whatever the others did not take — and routing by
  default is exactly how a licensed document reaches the store meant for public output.
- **A record's own `vault:` overrides the route, and `--vault` never does.** The flag is
  applied *after* routing, so it can only remove artifacts from a plan. There is no code path
  by which typing a store's name puts something into it; moving an artifact between stores is
  an edit to its record, in a commit.

### Name the table before you need the second entry

**This is why naming lands in #413 rather than in #416.** `[vault]` and `[vault.default]` are
different config shapes, and derived repositories adopt configuration quickly — a corpus that
wrote the singular form is a corpus whose `.yidam/config.toml` a later release breaks. The
plural table costs nothing today and cannot be retrofitted quietly.

What the first pass does *not* do is honour a second entry. A config declaring two vaults is
refused, by name, naming the version that will accept it. The alternative is accepting the
declaration and silently routing everything to the first — which is the exact failure an
isolation boundary exists to prevent, arriving as a success.

### The record

Additive, optional frontmatter on a catalog entry:

```yaml
artifacts:
  - sha256: 9f2c8e…          # the identity — and the SigV4 payload hash, already computed
    bytes: 4194304
    media_type: application/pdf
    retrieved: 2026-08-22
    from: 0                   # which `location` it came from, or a literal URL
    vault: sources            # where these bytes may be stored; `none` = local only
    redistributable: false    # whether they may leave this machine at all
```

**Not a new `location` kind.** `location` answers *where can a reader reach this*; a content
hash is not a place anyone goes. Widening `CATALOG_LOCATION_KINDS` would also change a value
[`yidam schema`](../../yidam/cli/src/cmd/schema.rs) publishes and re-run
`catalog-location-malformed` over every existing corpus, in order to describe something that
is not a location.

#### Two fields, because they answer two questions

`vault:` is **routing** — where these bytes may go, in the vocabulary the config declares.
`redistributable:` is a **licensing fact about the source**, and it overrides every route: an
artifact can name a vault and still be refused, and the refusal names both.

Collapsing them into one field is tempting and wrong. A route is edited casually — somebody
reorganising storage moves a dozen entries from `sources` to `default` in an afternoon. A
licence is not something that edit is allowed to undo. Keeping the assertion in its own field
means the reorganisation meets a refusal instead of publishing a paper.

### Configuration, and where the credential is not

`.yidam/config.toml` is committed. It holds the stores and never a secret.

```toml
[vault.default]
url        = "s3://corpus-artifacts/yidam"   # or file:///mnt/archive/yidam
region     = "us-east-1"                     # SigV4 scope; MinIO wants one too
endpoint   = "https://s3.example.net"        # omit for AWS
path_style = true                            # default true when endpoint is set
audience   = "Anyone who can read this corpus. Derived output only."
holds      = ["index", "embeddings", "bundle"]

[vault.sources]
url        = "s3://licensed-sources/yidam"
region     = "us-east-1"
audience   = "The sangha. Documents obtained under a licence to read, not to host."
holds      = ["catalog"]
```

Credentials come from the environment only, per vault:
`YIDAM_VAULT_<NAME>_ACCESS_KEY_ID`, `_SECRET_ACCESS_KEY`, `_SESSION_TOKEN`.

Plain `AWS_ACCESS_KEY_ID` and friends are honoured as a fallback **for the vault named
`default`, and for no other.** The asymmetry is deliberate. An ordinary AWS environment is
plausibly already configured for the store a repository publishes its own output to; a second
vault exists precisely because its readership differs, and letting it silently inherit ambient
credentials is the failure the boundary was drawn to prevent. A vault that wants isolation has
to say which keys it uses.

This repository has already found an untracked `.env` that any of its own prescribed
`git add -A` steps would have staged. The vault must not add a second route by which a key
gets committed.

### Feature gating

| Feature | Covers | Gate | Why |
|---|---|---|---|
| `vault` | CAS, cache, `file://`, verify, gc | **ungated** | Pure `sha2`/`hex`/std — all base dependencies. The light build can hash, cache, verify and read a `file://` vault on a mounted archive with nothing new. |
| `vault-s3` | the S3 transport | **in `default`** | PR CI never compiles `--features index`, so gated code ships that CI has not built. Anything in `default` is compiled by every pull request. |

Placing `vault-s3` in the default set is affordable because `reqwest` and `tokio` are already
there via `tonpa`. If #412 selects hand-rolled SigV4, the marginal cost to the released binary
is one small pure-Rust crate — not an ML stack, not a C library, not a protoc.

### The transport, and why it is decided by measurement

The store needs three verbs — GET, PUT, HEAD — and **never chunked signing**, because a
content-addressed store has already computed `x-amz-content-sha256` before it sends anything.
Hashing streams from disk, so a PUT streams the file with a precomputed header rather than
buffering a 500 MB index into memory. Dropping `STREAMING-AWS4-HMAC-SHA256-PAYLOAD` removes
most of what makes SigV4 implementations wrong.

That argues for hand-rolling over `reqwest`. It did not settle it, and #412 answered the
question the way `Cargo.toml`'s own TLS argument was answered — by resolving each candidate
against the real crate and reading the tree. The measurement is
[below](#what-the-transport-spike-measured), and it is decisive.

**A note on where that record lives.** This RFC first said the result would go to
`.yidam/decisions/`. That is a *derived-repository* path and this repository is the template;
it has no `.yidam/`. The template's mechanism for a decision of this kind is the RFC that
raised it, and RFC-0021's `What implementation found` is the precedent. Corrected here rather
than in a directory invented to hold one file.

### The commands

| Command | Does | Network |
|---|---|---|
| `yidam vault list` | configured vaults, each with its audience and what routes to it | no |
| `yidam vault put <path>` | hash, cache, print the hash | no |
| `yidam vault get <sha>` | cache first, then the artifact's own vault; `--out` to materialize | on miss |
| `yidam vault path <sha>` | where it is locally, or exit 1 | no |
| `yidam vault status` | every hash the tree names, grouped by vault: cached / stored / missing / **mismatched** / **unroutable** | only with `--remote` |
| `yidam vault push` | upload what the tree names and its vault lacks; `--dry-run` prints the canonical request | yes |
| `yidam vault pull` | fetch what the tree names and the cache lacks; `--index` for the derived set | yes |
| `yidam vault verify` | re-hash everything cached | no |
| `yidam vault gc` | drop cached blobs no commit names | no |
| `yidam catalog fetch <slug>` | resolve the entry's `location`, fetch, hash, cache, write the record | yes |

`--remote` costs one HEAD per declared artifact, against that artifact's own vault — bounded
by the catalog, not by the bucket. **Nothing here ever lists a bucket.**

`push`, `pull` and `status` take `--vault <name>` to **narrow** the operation to one store,
which matters when one vault is reachable from a CI runner and another is not. It never
*re-routes*: an artifact routed to `sources` is not pushed to `default` because somebody typed
a flag. Moving an artifact between vaults is an edit to its record, in a commit, like every
other assertion this repository makes.

### Checks

| Check | Fires when | Severity |
|---|---|---|
| `catalog-artifact-mismatch` | cached bytes hash to something other than the record | **Error** |
| `catalog-artifact-unroutable` | a record names a vault `.yidam/config.toml` does not declare | **Error** |
| `catalog-artifact-missing` | a record names a hash present neither locally nor in its vault | Warn |
| `catalog-obtained-without-artifact` | `obtained: true` and no `artifacts:` | Info |

Both Errors are safe to gate because **both sides are committed** — each check answers
identically in every clone, so each reports a defect in the corpus and never a fact about your
credentials. Whether a *declared* vault is reachable belongs to `vault status --remote`, not
to lint.

The Info is deliberate and load-bearing: it fires on **every entry in every existing corpus**
the day it ships. It is reported as a count rather than a violation list until a corpus opts
in. This is [`LintConfig::escalate_after`](../../yidam/cli/src/config.rs)'s argument applied —
a threshold compiled into the binary is one corpus's judgement arriving as a build failure in
another that never agreed to it.

### The push guard

`prelude/guidelines/directories.md` already states the gap a vault opens, in a section written
before anything could open it:

> **This is access control over material at rest. It says nothing about data leaving at
> runtime.** […] an egress check would have to know every network call the domain computer
> makes, and CI is hermetic precisely so that it makes none.

It lists the channels a yidam repository opens — connectors, a deployed web shell, a hosted
encoder, anything with telemetry — and says each is the reader's responsibility. **A vault push
is that channel, and it is the first one `yidam` itself opens.**

So `vault push` reads `.yidam/private-paths` and refuses to upload any artifact whose record
sits under a declared path. That is the rule `release.yml` already applies to a bundle, for the
reason it gives in its own words: *the artifact outlives the access.*

`redistributable` is the second half, with a split default:

- **A third party's bytes default to no.** A default of "upload unless told otherwise" would
  make the first `vault push` anybody runs a redistribution nobody chose.
- **The repository's own derived output defaults to yes.** An index, an embedding set and a
  bundle are things this repository made.

When a push is refused, the hash, the size and the retrieval date still commit. Provenance
survives intact — `vault status` can still say *this is the document that was read* — and only
the bytes stay local. The refusal quotes the destination's own audience line back at whoever
typed the command, because *"routed to `sources`, whose audience is 'the sangha'; the entry is
marked not redistributable"* is a sentence someone can act on and `Permission denied` is not.

### Reports and clocks

- `catalog-audit` gains the **Integrity** column the template has promised since it was
  written.
- `yidam vault-status` — a REGEN block, sibling to `index-status` and `bundle-status`.
- `doctor` gains an **offline** check per vault: is it declared with an audience, are its
  credentials in the environment, is the cache readable, does anything routed to it sit
  uncached. It also warns when two vaults resolve to the same credentials — a legal
  configuration, and also exactly what a half-finished isolation setup looks like, and the two
  are indistinguishable unless something says so. Reachability is `vault status --remote`'s
  job; `doctor` is documented read-only and offline and stays that way.
- `yidam due` gains **no clock**. An artifact does not go stale; its *record* does, and
  `[catalog] ttl_days` already counts exactly that.

## What this does not touch

- **The parity surface.** Catalog frontmatter is parsed by the CLI alone. The parity surface is
  `parse_node`, `extract_claims`, `extract_links`, `classify_commit`, `parse_markers`,
  `update_regen`, `find_reachable`, `find_citations`, `is_recognized_verb` and
  `compile_class_schema` — none of which reads the catalog. No SDK changes.
- **The MCP contract.** No tool is added and no capability changes. An artifact-serving tool is
  named under [Open questions](#open-questions) and wants its own RFC, because a new tool is a
  capability rather than a field and the contract is frozen at 0.12.0.
- **`format_version`.** New report fields are additive and consumers must ignore what they do
  not know. The asymmetry is worth stating: `format_version` protects a consumer reading an
  *older* producer and does nothing in the other direction, so an extension built against
  `artifacts` would crash against a released CLI that does not emit it. The extension therefore
  reads the new field and the new column as optional from the first commit that mentions them.
- **The corpus.** No node model change, no new class, no new edge kind. A catalog entry is not
  a corpus node and this changes only the catalog.
- **`.yidam/publishable` and the release guard.** A bundle's publication path is unchanged.
  The vault is a second channel beside it, not a replacement.

## What the transport spike measured

**#412, resolved. The transport is hand-rolled SigV4 over the `reqwest` already in the default
feature set, adding one crate: `hmac`.**

Each candidate was added to `yidam/cli` with `cargo add` and the resolved graph compared
against the default build's 153 packages. Counts are unique `name vX.Y.Z` pairs, so a second
*version* of a crate already present counts as an addition — which is the number that matters
here.

| Option | Added | Total | Second HTTP client | C needing a tool the release build lacks |
|---|---|---|---|---|
| **A · hand-rolled SigV4 + `hmac`** | **+1** | 154 | no | no |
| D · `object_store` 0.14 (`aws`) | +36 | 189 | **yes** — `reqwest` 0.13 beside 0.12 | **yes** — `aws-lc-sys` |
| B · `rust-s3` 0.35 (rustls, no defaults) | +48 | 201 | **yes** — `hyper` 0.14, `rustls` 0.21, `http` 0.2 | no |
| C · `aws-sdk-s3` + `aws-config` | +93 | 246 | **yes** — same shape as B | — |

### The count is not the argument; the duplication is

Every alternative brings a **second copy of the HTTP and TLS stack** into a binary that
already has one. `rust-s3` 0.35 resolves `hyper 0.14` beside the existing `hyper 1.10`,
`rustls 0.21` beside `rustls 0.23`, `http 0.2` beside `http 1.4`. `object_store` looks smaller
until you check the right name — it does not duplicate `hyper` or `rustls`, it duplicates
**`reqwest` itself**, resolving 0.13.4 alongside the 0.12.28 `tonpa` already uses.

### `object_store` was rejected for a sharper reason than this RFC first gave

The earlier draft dismissed it as "a large tree, some of it native", lumped in with
`aws-sdk-s3`. That was imprecise and nearly wrong — at +36 it is the *smallest* of the three
libraries. The real disqualifier is a single transitive dependency:

```
aws-lc-sys v0.42.0
```

AWS-LC is a C cryptography library whose build requires **CMake**. The release workflow
installs exactly one package for the aarch64 target — `gcc-aarch64-linux-gnu`
([`release.yml:78`](../../.github/workflows/release.yml)) — so adopting `object_store` means
adding a build tool to the cross-compile in order to compile a *second* C crypto library
beside the `ring 0.17.14` already in the tree.

That is precisely the class of change [`Cargo.toml`](../../yidam/cli/Cargo.toml)'s TLS comment
exists to refuse. It distinguishes vendored C that cc-rs builds with the cross-compiler already
present, which is fine, from a system dependency the build has to go and find, which is not.
`aws-lc-sys` is the second kind wearing the first kind's clothes.

### Streaming the upload is free

The RFC's claim that a PUT streams from disk rather than buffering a 500 MB index needed
checking, because `reqwest`'s `stream` feature is not in the current feature list. Adding it
resolves **zero** new packages — `tokio` and `tokio-util` are already present via `tonpa`. So
the +1 above is the complete cost of a streaming, correctly-signed S3 client.

### What would reverse this

- **A need for chunked signing.** The whole argument that hand-rolled SigV4 is small rests on
  never emitting `STREAMING-AWS4-HMAC-SHA256-PAYLOAD`, which holds only while the payload hash
  is known before the request — true for a content-addressed store and false the moment
  something streams an unknown body.
- **Multipart upload** (deferred; see [Open questions](#open-questions)). Multipart is a second
  signing surface and a state machine, and it is where a library starts earning its tree.
- **A second cloud.** One hand-rolled signer for S3 is a few hundred lines; a second for Azure
  is a second few hundred, and at that point `object_store`'s tree buys something real. GCS
  does not count — it speaks S3 through an interoperability endpoint.
- **`aws-lc-sys` gaining a pure-Rust default**, or the release build acquiring CMake for an
  unrelated reason. Either removes `object_store`'s only disqualifier.

### What was not measured, and why

**The aarch64 cross-compile was not run for option A.** `hmac 0.12.1` has no `build.rs` and no
non-Rust source file, so there is nothing in it that a cross-build can fail on, and a CI run
proving that a pure-Rust crate compiles would be theatre. The existing
`ci (cli · aarch64 cross-compile)` job covers it for real when #413 lands the dependency.

**No bytes moved.** No candidate was run against a live MinIO. The question #412 asks is what
each option *costs*, not whether S3 clients work, and a round-trip would have distinguished
none of them. Correctness of the signing implementation is #415's problem and is answered
there by AWS's published test vectors rather than by a server.

## Phasing

| | Issue | Ships |
|---|---|---|
| P0 | #412 | the transport decision, as a record and no library code |
| P1 | #413 | the store, offline: CAS, cache, `file://`, the plural config with one entry allowed |
| P2 | #414 | `artifacts:`, the four checks, the integrity column, `yidam catalog fetch` |
| P3 | #415 | the S3 transport, `push`/`pull`/`status --remote`, the private-paths guard, `redistributable` |
| P4 | #416 | named vaults plural: `holds` routing, per-vault credentials, `--vault`, the isolation warnings |
| P5 | #417 | index, embeddings and bundle into their vault; `.yidam/index.lock`; the derived-artifact privacy guard |
| P6 | #418 | `gc`, materialization, the `vault-status` report, the docs |

Two orderings were considered and one deliberate choice made. **#416 lands before #417**
because routing is cheap to add while there is one artifact kind and expensive to retrofit
across three; the index phase then declares a route rather than migrating to one. The cost is
that the payoff in #417 — a light build answering `retrieve` non-degraded over an index it
never built — arrives one phase later than it could. If that payoff is wanted sooner the two
swap cleanly, and nothing else in this RFC depends on the order.

**That payoff turned out not to be this RFC's to deliver.** #417 found that a light build
cannot embed a query whatever arrives on disk; see [What #417
found](#what-417-found). The ordering argument still held — routing was cheap to add before
there were three artifact kinds, and #417 declared a route rather than migrating to one.

The guard in #415 does **not** move. The first release that can upload is the first release
that can leak.

## What #417 found

### The payoff this phase was named for cannot be reached by transport

#417's "done when" is: *a light build, given a vault and a committed lock file, answers
`retrieve` **non-degraded** over an index it never built.* The vault half is built and works.
The sentence is still false, and no amount of transport makes it true.

`retrieval::load` in a build without `index` returns `NoVectorSupport` whenever an index is on
disk, and it is right to. Two things are missing, and only one of them is about reading:

1. **Decoding `index/corpus.arrow`** needs `arrow-ipc`.
2. **Embedding the query** needs `fastembed`. A pulled index carries the *document* vectors;
   turning `what does this corpus say about X` into a vector to compare them against needs the
   ONNX model, and there is nowhere to get it from but the model.

The second is the one that settles it. Delivering the index is necessary and not sufficient,
and the issue's premise — that the missing piece was a channel — was half right.

### But the expensive half is the build, not the read

Measured rather than assumed: **`lancedb` is named in exactly one file**, `cmd/index_build.rs`,
and `lancedb` is what requires protoc 31. Reading an index and embedding a query need
`fastembed` and `arrow-*` and neither needs protoc.

So the reachable version of the payoff is a third build, between the two that exist:

| | builds an index | reads one | needs protoc |
|---|---|---|---|
| default (`reports`) | no | no | no |
| a `vector-read` feature | no | **yes** | **no** |
| `index` | yes | yes | yes |

That is a packaging change — a new feature, `resolve_model` moved out of the gated
`cmd/index_build.rs` the way `sha256_hex` moved out of `cmd/tonpa/`, and a decision about
whether it becomes a released artifact. It is not a vault change, and it does not belong in
this RFC's track. **Filed separately; #417 ships the channel and says plainly that the light
build still degrades.**

What #417 *does* deliver end to end is the same property one build short of the goal: a machine
with a full build and no index pulls one it never built and uses it. That is most of the value
— an index is built once and read on every other machine that can read one — and it is tested.

### An index inherits the privacy of everything it encodes

This is new design, and it is the reason the phase needed a guard at all.

`model::VectorRow` carries the node's **`text`**, verbatim, and `cmd/embed.rs` composes that
text from `.yidam/corpus/` *and* `.yidam/catalog/`. An index is therefore not a file that
happens to sit beside the corpus; it is a re-encoding of it. Pushing one to a vault publishes
every node it walked.

The catalog guard cannot see this. A catalog artifact is refused for what its own record says,
and an index has no record — nobody wrote one, because nobody fetched it. So `vault push
--index` answers a different question: **may everything this was derived from leave?** A
declared-private path intersecting `.yidam/corpus` or `.yidam/catalog`, in either direction,
refuses the push and names the path.

The rule is deliberately the one `sadhana/github/workflows/release.yml` already applies to a
bundle, rather than a cleverer one — the two guards answer the same question about the same
class of artifact, and a repository that enforced two different rules for that would be worse
than one that enforces a blunt one twice.

> **An adjacent hole, found while mirroring it and not fixed here.** A bundle carries
> `index/corpus.arrow` (`cmd/bundle.rs:147`), and that index encodes catalog text — but the
> release workflow's `bundled=` list names only `.yidam/corpus`, `.yidam/skills` and
> `.yidam/decisions`. `tests/publish_guard.rs` reasons `index/` away as "generated rather than
> authored", which is true of the file and false of its contents. A private catalog directory
> can therefore reach a published bundle. Filed separately: it is a defect in the bundle
> channel, it predates this RFC, and making a derived repository's release guard stricter is a
> change to ship deliberately rather than inside a vault PR.

### Derived artifacts default the other way

`policy.rs` promised this in #415 and this phase pays it: a catalog artifact is refused unless
its record licenses redistribution, and a repository's own output is pushed by default. There
is no `redistributable` to set on an index, because there is no third party whose licence it
could be. The only thing that stops it is the privacy rule above.

## Testing

CI is hermetic and no test may reach a bucket.

- **`file://` is not a test double.** It is a shipped backend that happens to be trivially
  testable, so every behavioural test runs against one in a `TempDir` and exercises production
  code.
- **SigV4 is tested against AWS's published test-suite vectors** — canonical request,
  string-to-sign, signature — with no server and no network. It is the only part that can be
  silently wrong, and a live MinIO would not have caught it faster.
- A golden pins `vault push --dry-run`'s canonical request.
- **Mutate the guards before trusting them.** Delete the private-paths guard's body, keep its
  comment, confirm it goes red. A file-scanning test that looks at nothing passes, and prose in
  a file answers a grep the same way code does. #416 added four more, each confirmed red:
  routing that ignores `holds` and sends everything to the first vault; `--vault` re-routing
  instead of narrowing; a silent second vault becoming a catch-all; and `doctor` no longer
  noticing two vaults on one account.
- **Every route gets a test that a licensed artifact does not reach the public store.** The
  three ways it could — a flag, a route edit, an ambient `AWS_*` — are separate mechanisms and
  a test of one says nothing about the other two.
- Goldens carrying `retrieved` dates run under `TZ=UTC`, which is what the runner does.
- Run the cargo gates sequentially; `ci` and `ci-cli-full` share one target directory.

## Open questions

- **Multipart upload.** Deferred, with a stated limit rather than a silent one: a single PUT
  caps at 5 GiB, over ~100 MB warns, and over the cap refuses with a message that says why. A
  corpus whose sources are routinely larger than that will want this and none is known.
- **Encryption at rest, bucket ACLs, per-vault KMS keys.** The operator's, and `doctor` will
  not pretend to have checked them — unknown is not proof of protected. A vault's `audience`
  line is where an operator records what they actually did, and it is prose for that reason.
- **An MCP tool that serves an artifact.** The natural shape is a `get_artifact` returning
  bytes or a URI, and it is a capability rather than a field. It also raises a question this
  RFC has not answered: whether an agent reading a corpus should be able to pull a licensed
  PDF through the same surface that serves nodes, and what `redistributable` means when the
  consumer is a model context rather than a bucket.
- **Cross-vault migration** (`yidam vault move`). Moving an artifact between stores is an edit
  to its record, plus a push and a delete somebody performs deliberately. A command doing all
  three would make a route change look cheap, and route changes are exactly what
  `redistributable` exists to survive. Left out until somebody has done it by hand enough
  times to say what the command should refuse.
- **GCS and Azure.** The `file://` and `s3://` split already forces a backend trait, so a
  third backend is then a file rather than a redesign. Not built because nothing has asked.
- **Whether `obtained:` should become derivable.** An entry with a verified artifact is
  obtained, provably, and the flag then restates what the hash proves. Deriving it would
  silently change every existing corpus's `catalog-audit` output, so this RFC keeps the flag
  and adds `catalog-obtained-without-artifact` at Info instead. The question is whether that is
  a permanent answer or a migration step.
- **What a vault should do about a hash it holds that no corpus names any more.** `gc` handles
  the local cache, where the answer is obvious. In the store it is not: the bytes may be the
  only surviving copy of a source a *past* commit rested on, and history is the point of this
  repository. #418 implements no remote `gc` for that reason.
