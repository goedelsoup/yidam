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

#### Name the table before you need the second entry

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

[vault.routes]                               # default vault per artifact kind
catalog    = "sources"
index      = "default"
bundle     = "default"
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

That argues for hand-rolling over `reqwest`. It does not settle it, and #412 exists so the
question is answered the way `Cargo.toml`'s own TLS argument was answered — by building it and
measuring the dependency tree and the aarch64 cross-compile, then writing the result to
`.yidam/decisions/`.

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

## Phasing

| | Issue | Ships |
|---|---|---|
| P0 | #412 | the transport decision, as a record and no library code |
| P1 | #413 | the store, offline: CAS, cache, `file://`, the plural config with one entry allowed |
| P2 | #414 | `artifacts:`, the four checks, the integrity column, `yidam catalog fetch` |
| P3 | #415 | the S3 transport, `push`/`pull`/`status --remote`, the private-paths guard, `redistributable` |
| P4 | #416 | named vaults plural: routes, per-vault credentials, `--vault`, the isolation warnings |
| P5 | #417 | index, embeddings and bundle into their vault; `.yidam/index.lock` |
| P6 | #418 | `gc`, materialization, the `vault-status` report, the docs |

Two orderings were considered and one deliberate choice made. **#416 lands before #417**
because routing is cheap to add while there is one artifact kind and expensive to retrofit
across three; the index phase then declares a route rather than migrating to one. The cost is
that the payoff in #417 — a light build answering `retrieve` non-degraded over an index it
never built — arrives one phase later than it could. If that payoff is wanted sooner the two
swap cleanly, and nothing else in this RFC depends on the order.

The guard in #415 does **not** move. The first release that can upload is the first release
that can leak.

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
  a file answers a grep the same way code does.
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
