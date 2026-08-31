# Artifact vaults

*Where a corpus keeps bytes too large, too derived, or too licensed for git — and why losing
that store costs no knowledge at all.*

A catalog entry records that a paper was fetched. For a long time nothing anywhere held what
was fetched, and an entry marked obtained honestly was indistinguishable from one marked
obtained falsely. The derived artifacts have the opposite problem: the vector index exists, it
is large, and there is nowhere it can live that a second machine can reach.

Both are the missing half of a store, and a **vault** is that store. One sentence carries the
whole design:

> **A vault stores bytes. Git stores the record of them** — which bytes, and which vault they
> are allowed in.

Every pointer into a vault is a committed file, so a vault holds no mutable state whatsoever.
That single constraint is what makes the feature safe in a repository whose thesis is that the
graph *is* the git history:

- **A stale vault cannot lie**, because the digest is in the commit.
- **Losing a vault costs no knowledge claim**, only the time to re-fetch.
- **Garbage collection is exactly computable**, because the live set is the set the working
  tree names.

## Nothing is configured, and that is a working state

A corpus with no vault keeps its artifacts in a machine-wide cache and nowhere else. That is
every corpus until somebody configures a store, and it is not degraded — `yidam vault put`,
`path` and `verify` all work against the cache with no repository and no configuration at all.

```sh
yidam vault put ~/Downloads/pearl-2009.pdf   # prints the content address
yidam vault verify                           # re-hashes everything cached
```

The cache lives at `$XDG_CACHE_HOME/yidam/vault` (or `YIDAM_VAULT_CACHE`) and is deliberately
**not** partitioned by vault or by repository — two corpora citing the same paper store it
once. A cache hit answers *do I have these bytes*, never *may I send them*.

## Declaring a store

```toml
[vault.default]
url      = "file:///mnt/archive/yidam"
audience = "Who can read this store, and why that is acceptable."
```

`audience` is required and nothing can check it. That is `.yidam/publishable`'s argument
applied to a store: it is not a security control, it is a statement of intent that lives in the
repository and outlasts the person who made it. What is enforced is that somebody wrote one.

`s3://bucket/prefix` works too, with `region`, `endpoint` and `path_style` for anything
S3-compatible. **Credentials come from the environment only** — `.yidam/config.toml` is
committed and must never carry one. See [CLI reference](cli-reference.md#s3-compatible-stores)
for the variables.

## Two audiences need two stores

A repository's own index and a licensed PDF it obtained have different readerships, and one
store cannot express both. Each vault declares what it `holds`:

```toml
[vault.default]
url      = "s3://corpus-artifacts/yidam"
audience = "Anyone who can read this corpus. Derived output only."
holds    = ["index", "embeddings", "bundle"]

[vault.sources]
url      = "s3://licensed-sources/yidam"
audience = "The sangha. Documents obtained under a licence to read, not to host."
holds    = ["catalog"]
```

With more than one vault, **every** vault declares `holds`. A vault claiming nothing would have
to be the catch-all for whatever the others did not take, and routing by default is how a
licensed document ends up in the store meant for public output.

A record's own `vault:` overrides the route its kind would take, and `vault: none` is a route —
the local cache and nowhere else — spelled rather than omitted, so that *nobody has decided* and
*decided to keep it here* are different states.

## What refuses to leave

`vault push` is the first egress channel yidam itself opens, and two independent rules gate it.
Neither implies the other; an artifact clears both or neither.

**For something you fetched**, the default is refusal. A catalog artifact is not pushed unless
its record says `redistributable: true`. Upload-unless-told-otherwise would make the first push
anybody runs a redistribution nobody chose, and a catalog is full of papers.

**For something you computed**, the default is the other way — there is no third party whose
licence it could be — but the question changes shape. An index is not a file that happens to
sit in `.yidam/index/`; it is a re-encoding of the corpus, and each row carries the node's text
verbatim. So `push --index` asks whether *everything it was derived from* may leave:

| Artifact | Derived from |
|---|---|
| `index`, `embeddings` | `.yidam/corpus`, `.yidam/catalog` |
| `bundle` | those, plus `.yidam/skills`, `.yidam/decisions` |

A path `.yidam/private-paths` declares private that intersects one of those refuses the push
and names it — the same rule the release workflow applies to a bundle, for the reason it gives:
*the artifact outlives the access.*

**`.yidam/private-paths` applies over the top of both**, and is checked first: it is a statement
about this repository that the person running the command can act on, while a licence is a fact
about a third party they may not be able to change at all.

## The index, which the binary you installed cannot build

`.yidam/index/` is built only by a binary compiled `--features index` — protoc plus an ONNX
runtime — and nothing keeps it in git. So the index exists on whichever machine could build it
and nowhere else. The vault is the channel between them:

```sh
yidam vault push --index                     # where one was built
git add .yidam/index.lock && git commit       # the only part that travels through git
yidam vault pull --index                     # anywhere else
```

`.yidam/index.lock` names **the store as well as the hash**. A pull reads the store from there
rather than re-deriving it from `holds`, because a routing edit made after the push would
otherwise send it somewhere the bytes are not — a mutable ref wearing a lock file's clothes.

An index is a directory and a vault stores one object, so it is packed into a single
deterministic archive and hashed as a whole. A `corpus.arrow` from one build beside a
`meta.json` from another is a corrupt index nothing would notice; making partial arrival
inexpressible is better than checking for it.

> **The default build still degrades, and there is a build that does not.** Pulling an index
> does not make `retrieve` non-degraded in the light binary: reading the index needs
> `arrow-ipc`, and embedding the *query* needs the ONNX model. The channel is necessary and not
> sufficient.
>
> `--features vector-read` is the build that completes it. It reads an index and answers over
> it, and it needs **no protoc** — that is `lancedb`'s requirement, and `lancedb` is only ever
> used to *write* an index. See [Installation](installation.md#which-build-you-have).

## Opening a file, and reclaiming the space

Content addressing is right for storage and useless for opening. `yidam vault materialize`
hardlinks cached artifacts into `.yidam/vault/<entry slug>/<slug>.<ext>` so a person, a
connector, or a PDF reader has a real file with a real name. It refuses to write until
`.yidam/vault/` is ignored by git — a licensed document in a tracked path is the leak `push`
refuses, arriving through `git add -A` instead.

`yidam vault gc` reports cached artifacts no committed file names, and deletes nothing until
`--yes`. Read the list first: the cache is shared by every yidam repository on this machine, so
an artifact another one names looks exactly like an orphan from here. Usually that costs a
re-fetch — but an artifact recorded `vault: none` is in a cache and nowhere else by decision,
and there the cache is the only copy.

## What to run when something looks wrong

| | |
|---|---|
| `yidam vault list` | what is declared, what each holds, whether each store opens |
| `yidam vault status` | where each artifact goes and where it is, grouped by store |
| `yidam vault status --remote` | the same, having asked each vault — one HEAD per record |
| `yidam vault verify` | re-hash the cache; exits nonzero on anything that is not what it claims |
| `yidam doctor` | offline: are the vaults coherent, are credentials present, is anything stranded |

`doctor` warns when two vaults resolve to the same credentials. That configuration is legal —
one account can own two buckets — and it is also exactly what a half-finished isolation setup
looks like. The two are indistinguishable from outside, so it reports the shape and lets you
decide.

The design and its rejected alternatives are in
[RFC-0023](rfcs/0023-remote-vaults.md).
