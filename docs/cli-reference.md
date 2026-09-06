# CLI reference

Every command `yidam` carries, grouped as `yidam --help` groups them. This page is the map;
`yidam <command> --help` is the detail. That help is generated from the same source as the
behaviour, so it cannot disagree with the binary you have.

Two conventions run through the whole surface.

**A `*` means the command rewrites files in the repository it is run against.** Twenty-three
do. That was previously visible only in each command's long help, where you had to already
suspect it to go looking. That is the wrong way round for a tool people point at a checkout
they only meant to inspect.

**`--format json` is available on most commands** and emits the machine-readable report contract
of [RFC-0016](rfcs/0016-editor-surface.md). `text` is the default and is byte-stable. The editor
surface and CI both consume the JSON, so the prose is free to stay prose. Commands below that take
no options at all are marked *(no flags)*.

Some commands need a build carrying the matching cargo feature. Those are marked, and
[Installation](installation.md#check-which-build-you-have) has the table. `yidam --version` prints
the feature list of the binary in your hand.

---

## Checks and gates

Read-only, and they exit nonzero on a problem — which is what makes them usable as CI steps.

| Command | What it answers |
|---|---|
| `doctor` | Is this setup sound? Ten checks, each with a verdict and a remedy. `--strict` makes warnings fail. See [Troubleshooting](troubleshooting.md) |
| `graph-check` | The graph gate: orphans, broken links, missing labels |
| `lint` | Corpus quality checks against the baseline ratchet |
| `index-verify` | Does an embedding provider reproduce this index's contract? |
| `samudaya-audit` | Inspect and validate `samudaya/` seed files *(no flags)* |

`lint` carries the most flags of any command, because it is the one with a ratchet:

| Flag | Effect |
|---|---|
| `--commits` | Also check the git log against the [commit vocabulary](what-yidam-is.md#two-kinds-of-commits-and-no-others) |
| `--range <RANGE>` | Restrict `--commits` to a revision range, e.g. `main..HEAD` |
| `--explain` | Print each check's rationale beside its findings |
| `--warn` | Report findings but always exit 0 |
| `--bless` | Rewrite `.yidam/lint-baseline.yml` from this run instead of gating on it |
| `--init-baseline` | Write the baseline only if absent, then exit — safe to run unconditionally |

`lint` answers *did this change make the corpus less clean?* — not *is the corpus clean?* The
baseline is what makes that the question. — see
[Configuration](configuration.md#yidamlint-baselineyml).

## The practice

Two commands, and neither is a gate. Both exit zero however much is owed. Both exit zero
however far a corpus has drifted.

| Command | What it answers |
|---|---|
| `due` | What is due? Four clocks read together — index staleness, catalog TTL, unanswered questions, phases in flight. `--strict` exits nonzero on a due clock |
| `kuten` * | What is this corpus's practice aimed at? Writes the declaration into `AGENTS.md`. `kuten check` reads the history against it |

### A kuten declares what the work is for

A repository declares what it is *about* — in its ontology, its classes, its central question.
It had no way to say what its work is **for**. A kuten is that declaration.

It is vendored with the prelude, and it carries a revision. A decision record in
`.yidam/decisions/kuten.yml` adopts it. `yidam kuten` writes it into `AGENTS.md`, so an agent
meets it at session start.

**A kuten narrows the loop and may not widen the model.** Five things it may not do:

- add a commit verb — it may declare a subset of the closed list and gloss it;
- add or alter a claim standing;
- contradict Articles I–VI;
- change the graph encoding;
- loosen a gate, except as a visible policy override.

**A repository holding no kuten is a supported state.** It reports as one.

### `kuten check` asks a question and never fails

It reads the declaration and the repository's own history. Then it says where they disagree —
*you declared `inquiry` and have settled no phase in two hundred commits.*

That is a question for a person, not a defect. `due`'s argument applies verbatim: a corpus that
has drifted is owed a look. So the command writes nothing and exits zero. Anything that refuses
arrives through [the policy layer](#the-rules-this-repository-writes-about-itself), visible as
an override.

**Vintage is never reported as divergence.** Every consumer reads the kuten and the prelude
this repository *vendored*, not the current ones. A repository whose vendored `GRAPH.md` has no
`phase` verb has not stopped running phases. It never could, and the check says so.

**A comparison across revisions is annotated, never silently made.** The vendored profile may
have moved past the revision the decision record names. The report says so, and the numbers
stay readable.

### `due` is not `doctor`, and the difference is the point

`doctor` answers *is this setup sound now*. It is read under suspicion, and it exits nonzero on
what is wrong. `due` answers *is it time*, and it is read on a cadence. That might be a cron job,
a weekly ritual, or the start of a session.

**A corpus with three expired sources is not unhealthy. It is owed.** Nothing about it is broken,
no traversal will lie, and the gate is green. Fold that into `doctor`'s warnings and it reads as a
problem. The repository is doing exactly what it is meant to do. The reader would learn to skip
the line. So the two reports are separate. `due` has its own verdicts: `due`, `ok`, `undeclared`,
`unmeasurable`. It exits zero unless you pass `--strict` to ask for a signal.

### Every interval is declared, and a clock nobody set never comes due

A clock is an age and an interval. The age is measured. The interval is always something the
corpus said about itself, never a number in the binary. The reasoning is
[`escalate_after`](configuration.md#lint-escalate_after)'s.

| Clock | Measures | Interval | Unit |
|---|---|---|---|
| `index` | Corpus files changed since the index was built | `[due] index_after` | files |
| `catalog` | How long since a source record was retrieved | `[catalog] ttl_days`, or an entry's own | days |
| `questions` | How long a question has gone unanswered | `[due] questions_after` | corpus commits |
| `phases` | How long a bounded inquiry has been in flight | `[due] phases_after` | days |

The catalog clock reads the interval [where it already lived](configuration.md#catalog-ttl_days)
rather than restating it under `[due]`. A source's TTL is a statement about the source. Two places
to set one number means one of them is wrong.

A clock with no interval reports what it measured and is never due. Its remedy is the key that
would set it. That is the state of every repository that has not opted in. It is the design, not a
degraded mode.

Two of the four count days and two do not, which is deliberate. How long a question has gone
unanswered is a fact about the repository, so its clock is `HEAD`. A corpus that has not committed
has not ignored anything. A source's TTL and a phase's time in flight are facts about the world.
The world does not stop moving because nobody committed.

### What discharges a clock

`due` reports; it does not act. Only one of the four clocks is discharged by something
[`propose`](#propose-is-deliberately-small) can draft:

| Clock | What discharges it |
|---|---|
| `index` | `yidam index-build`. A build, not a commit |
| `catalog` | `yidam propose`, which already drafts an `open:` against each expired source |
| `questions` | A person. Deciding a question is answered is a resolution event, and Article V confines those to a sangha |
| `phases` | A person. Settling a phase or abandoning it is not a mechanical consequence of a finding |

Each clock names its own remedy in the report. The distinction is visible where it matters, not
only here.

## README blocks

Each of these rewrites its own `<!-- REGEN: yidam <name> -->` block in the repository's README.
That is their purpose, and it is why every one carries a `*`.

| Command | Block content |
|---|---|
| `regen` * | Refresh every REGEN block in one pass. `--check` reports staleness and writes nothing |
| `status` * | Repository overview: nodes, open questions, catalog, index freshness, phases |
| `open-questions` * | Unresolved questions, newest first |
| `corpus-index` * | Every corpus node by class, with label and link count |
| `catalog-audit` * | Which catalog sources the corpus cites, and which it does not |
| `index-status` * | Whether the vector index is present, and how stale against the corpus |
| `agents-index` * | The domain agents in `.yidam/agents/` *(no flags)* |
| `skills-index` * | The domain skills in `.yidam/skills/` *(no flags)* |
| `crates-index` * | The domain-computer crates in `crates/` *(no flags)* |
| `packages-index` * | The domain-computer packages in `packages/` *(no flags)* |
| `bundle-status` * | Freshness of `.yidam/bundle.yiz` against the corpus it was built from *(no flags)* |

In a derived repository a stale REGEN block is a failing build. Run `mise run regen` before
committing; `yidam regen --check` is what CI runs.

**These are the commands to be careful with against a checkout you only mean to read.**
`yidam status` sounds read-only and is not — it rewrites the README block. `yidam doctor` is
the read-only overview.

## The corpus and its history

| Command | What it does |
|---|---|
| `graph` | The corpus graph: nodes, resolved edges, and the classes that license them |
| `neighbors <node>` | One node's neighbourhood — the traversal `serve --mcp` performs. `--depth` |
| `query <query>` | A typed path over the resolved graph |
| `pack <query>` | A query's full answer filled to a token budget, with an account of what did not fit |
| `estimate <query>` | What a query would cost before you run it |
| `diff <range>` | Node and edge changes between two git refs |
| `check-diff [range]` | What a code diff names that the ontology does not ([RFC-0021](rfcs/0021-diff-alignment.md)). Defaults to the merge-base with `main` — this branch's work |
| `log [range]` | Commit history classified as testimony or pipeline work. `--epistemic`, `--operational` |
| `phases` | Active inquiry phases — `ma/*` and `rigpa/*` branches |
| `replay` | Corpus health reconstructed across the repository's whole history. `--every` |
| `decisions-log` | Decision records in `.yidam/decisions/`, newest first *(no flags)* |
| `sangha` | Electors, positions, and settled resolutions |
| `vocabulary` | The closed commit vocabulary. `--check <subject>` tests a subject line before the commit exists |
| `rename <old> <new>` * | Rename a node, rewriting every edge into it. `--dry-run` |
| `migrate <sub>` * | Change an ontology and every instance that adopted it, as one event. `--dry-run` |
| `propose` * | Draft findings as proposed epistemic commits on a `propose/<head>` branch |

### The query language

`query`, `pack` and `estimate` share one language. A query is a class anchor and a sequence of
typed hops:

```sh
yidam query 'reach -measured-by-> gage'
yidam query 'concept~"hydropeaking" <-exhibits- reach'
```

**Whitespace around a hop is required.** `-rel->` and `<-rel-` are single tokens, and that is what
lets a hyphenated relationship name be unambiguous. `~"…"` is a similarity anchor. It opens on
`--anchor-k` entry nodes (default 1). An anchor is a starting point, not an answer.

| Flag | Applies to | Effect |
|---|---|---|
| `--select` | `query`, `estimate` | Fields to project: `node`, `class`, `label`, `description`, `body`, `properties.<name>` |
| `--limit` | `query`, `estimate` | Bounds the *projection*, not the traversal — the reported count is always the full one (default 50) |
| `--budget` | `pack`, `estimate` | Approximate token budget, 1 token ≈ 4 chars. `pack` is **unbudgeted by default**, because a default budget would silently truncate the first pack anybody builds |
| `--at <ref>` | `query` | Answer as of a commit, reconstructed from git objects; the working tree is never touched |
| `--between <a..b>` | `query` | Answer at every corpus-touching commit in the range, as a series |
| `--across` | `query` | Query installed dependencies too. Every result says whose corpus it came from, and no hop crosses a corpus boundary |

### `migrate` subcommands

A class definition cannot be corrected in place once the class contract gates. Editing it puts
every instance in violation until each is fixed by hand. These do both halves together and write a
record of what they touched.

| Subcommand | Changes |
|---|---|
| `class` | Rename a class: its definition, its directory, and every edge that named it |
| `property` | Rename a declared property on a class and on every instance carrying it |
| `retype` | Change a declared property's type; refuses when an instance would not satisfy it |
| `edge` | Point a declared relationship at a different class, at both ends |

### `propose` is deliberately small

Three acts only. `open` records a finding's question against the node it is about. `withdraw`
deletes a node this corpus declared over-collected via `[propose] withdraw_uncited_after`.
`close` retires a question this command opened whose finding is gone.

Nothing merges itself and nothing synthesizes — no edge is drawn, no claim is re-tagged, no node
is authored. It writes git objects and one ref. The working tree, the index and `HEAD` are
untouched, so it is safe to run mid-edit. [RFC-0020](rfcs/0020-proposal-surface.md) has the
argument for why the surface is this small.

## Index and embeddings

| Command | What it does |
|---|---|
| `embed` * | Extract embedding text from corpus instances to `.yidam/embeddings/`. `--no-catalog` |
| `index-build` * | Build the LanceDB vector index and export Arrow IPC for the web shell. `--model`. **Needs `--features index`** |

`embed` walks `.yidam/catalog/` by default. In a real derived corpus the catalog was 51.3% of
the indexable text, against the corpus's 41.9%. Leaving it out had been a scope decision nobody
made on purpose.

## Artifacts

Bytes a corpus rests on or produces are large, derived, or licensed — a fetched source, a built
index. Git is the wrong place for all three. A **vault** holds them; the repository holds the
record of which bytes. RFC-0023 states the constraint: *a vault stores bytes, git stores the
record of them*, . So every pointer into a vault is a committed file. Losing a vault costs no
knowledge claim, only the time to re-fetch.

| Command | What it does |
|---|---|
| `vault list` | Every store this repository declares — its `audience`, what it `holds`, how many artifacts route to it |
| `vault put <path>` * | Hash a file into the local cache; prints the content address on stdout |
| `vault get <sha256>` * | From the cache, else from the vault. `--out` also writes a named copy |
| `vault path <sha256>` | Where the artifact sits locally, or exit nonzero — so `… \|\| fetch` works |
| `vault verify` | Re-hash every cached artifact; exits nonzero if any is not what it claims |
| `vault push` * | Upload what the corpus names and the vaults lack. `--dry-run` prints the exact string that would be signed; `--artifact` and `--vault` narrow; `--index`/`--embeddings`/`--bundle` send what this repository *computed* instead |
| `vault pull` * | Fetch what the corpus names and the cache lacks; `--vault` narrows; `--index`/`--embeddings`/`--bundle` fetch and unpack what `.yidam/index.lock` records |
| `vault status` | Where each named artifact goes and where it is, grouped by store. `--remote` asks each vault — one HEAD per record, never a bucket listing |
| `vault gc` | Report cached artifacts no committed file names; `--yes` deletes them |
| `vault materialize` | Hardlink cached artifacts into `.yidam/vault/<slug>/` under names a person can open; `--entry` narrows |
| `vault-status` | Writes the `<!-- REGEN: yidam vault-status -->` block. Committed files only — never the cache, never the network |

Every artifact is named by the SHA-256 of its bytes, in lowercase hex. The cache is
**machine-wide** — `$XDG_CACHE_HOME/yidam/vault`, or `YIDAM_VAULT_CACHE` — so two repositories
citing the same source store it once. It is deliberately **not** partitioned by vault. A cache hit
answers *do I have these bytes*, never *may I send them*.

`list`, `get`, `push`, `pull` and `status` read `.yidam/config.toml` and need a repository.
`put`, `path` and `verify` touch only the cache and work anywhere.

```toml
[vault.default]
url      = "file:///mnt/archive/yidam"
audience = "Who can read this store, and why that is acceptable."
```

`audience` is required and nothing can check it — it is `.yidam/publishable`'s argument applied
to a store. What is enforced is that somebody wrote one.

A lone vault may be called anything and, if it declares no `holds`, takes everything. What a
name other than `default` gives up is the ambient `AWS_*` fallback below.

### Naming vaults, and routing between them

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

The kinds are `catalog`, `index`, `embeddings` and `bundle`. Only `catalog` artifacts exist
today. The others are named so a corpus can declare their routes before #417 and #418 start
writing them. That avoids reorganising storage on the release that does.

| | |
|---|---|
| one vault, no `holds` | it holds everything |
| two or more | **every** one declares `holds` — a vault claiming nothing would be a catch-all, and routing by default is how a licensed document reaches the public store |
| a kind claimed by two | refused, naming both |
| a kind claimed by none | refused when something of that kind needs a route, naming the kind |
| a `holds` entry that is not a kind | refused — a typo claims nothing |
| a record's own `vault:` | overrides the route its kind would take |

A kind nobody claims is refused **at the artifact** rather than when the config is read. The
alternative would make the list of kinds a compatibility surface. Adding one in a later release
would turn every multi-vault config red for a kind those corpora have none of.

`--vault <name>` on `push`, `pull` and `status` is a **narrowing** flag. It is useful where one
store is reachable from a runner and another is not. It never re-routes. An artifact routed to
`sources` is not pushed to `default` because somebody typed a flag. Moving an artifact between
stores is an edit to its record, in a commit. That is how every other assertion the repository
makes is done. Stores are opened lazily, one per vault that has work. A vault whose credentials
are absent does not block a push to one whose are present.

### S3-compatible stores

```toml
[vault.default]
url        = "s3://corpus-artifacts/yidam"
region     = "us-east-1"                    # signing scope; MinIO wants one too
endpoint   = "https://s3.example.net"       # omit for AWS
path_style = true                           # defaults true when an endpoint is set
audience   = "Anyone who can read this corpus."
```

Credentials come from the **environment only** — `.yidam/config.toml` is committed and must
never carry one:

| Variable | For |
|---|---|
| `YIDAM_VAULT_<NAME>_ACCESS_KEY_ID` | that vault, always |
| `YIDAM_VAULT_<NAME>_SECRET_ACCESS_KEY` | that vault, always |
| `YIDAM_VAULT_<NAME>_SESSION_TOKEN` | temporary credentials |
| `AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` | **the vault named `default`, and no other** |

That last asymmetry is deliberate. An ordinary AWS environment is plausibly already configured for
the store a repository publishes its own output to. A *second* vault exists because its readership
differs. That is the only reason to declare one. Letting it inherit whatever happens to be
exported is the failure the boundary was drawn to prevent.

A single `PUT` caps at 5 GiB, and multipart upload is not built. Over the cap the upload is
refused with a message that says so. It does not fail at the server as `EntityTooLarge`.

### The artifacts this repository computes

`.yidam/index/` is built only by a binary compiled `--features index`, which needs protoc 31 and
an ONNX runtime. Nothing keeps it in git. So the index exists on whichever machine could build it
and nowhere else. The same vault carries it:

```sh
yidam vault push --index      # on a machine that has one; writes .yidam/index.lock
git add .yidam/index.lock && git commit
yidam vault pull --index      # anywhere else; unpacks into .yidam/index/
```

`--embeddings` and `--bundle` do the same for `.yidam/embeddings/` and `.yidam/bundle.yiz`.

**These flags are an either/or, not an addition.** `vault push` alone sends what the catalog
names; `vault push --index` sends the index and nothing else. An index is hundreds of megabytes.
`--index` quietly also uploading a corpus of papers would be a surprise in the direction nobody
wants.

An index is a directory and a vault stores one object. So a directory is packed into a single
archive and hashed as a whole. A `corpus.arrow` from one build beside a `meta.json` from another
is a corrupt index that nothing would notice. The archive is deterministic (sorted entries, zeroed
mtimes), so pushing an unchanged index does nothing. A `.yiz` is already one object and is stored
verbatim.

#### `.yidam/index.lock`

Committed, and the only thing that travels through git:

```toml
format_version = 1

[index]
sha256 = "9f2c8e…"
bytes  = 41943040
vault  = "default"
```

It names **the store as well as the hash**. A pull reads the store from *there*, not by
re-deriving it from `holds`. A `holds` edit made after the push would otherwise send the pull to
somewhere the bytes are not. That is a mutable ref wearing a lock file's clothes — the one thing
this design exists to avoid.

#### What `push --index` refuses

**An index is not a file that happens to sit in `.yidam/index/`.** It is a re-encoding of
everything walked to build it, and each row carries the node's text verbatim. So the question
is not whether the index may leave but whether *everything it was derived from* may:

| Artifact | Derived from |
|---|---|
| `index`, `embeddings` | `.yidam/corpus`, `.yidam/catalog` |
| `bundle` | those, plus `.yidam/skills`, `.yidam/decisions` |

A path `.yidam/private-paths` declares private that **intersects** one of those — in either
direction — refuses the push, naming the path. This is the rule
`sadhana/github/workflows/release.yml` already applies to a bundle, for the reason it gives:
*the artifact outlives the access.* A declared directory holding only a `README.md` or a
`.gitkeep` is intent rather than material and does not refuse.

Unlike a catalog artifact, a repository's own output is pushed **by default**. There is no
`redistributable` to set, because there is no third party whose licence it could be.

### Reclaiming space, and opening a file

`yidam vault gc` reports cached artifacts that no committed file names. The live set is exactly
computable, because every pointer into a vault is a committed file. It deletes nothing until
`--yes`.

**Read the list before you pass it.** The cache is machine-wide and shared by every yidam
repository on this machine. An artifact another one names looks exactly like an orphan from here.
Usually deleting one costs a re-fetch. The exception is an artifact recorded `vault: none`, which
is in a cache and nowhere else *by decision*. For that one the cache is the only copy.

`yidam vault materialize` hardlinks cached artifacts to `.yidam/vault/<entry slug>/<slug>.<ext>`.
Content addressing is right for storage and useless for opening. Nobody wants to hand a colleague
`9f2c8e…` with no extension. The extension comes from the record's `media_type`, and an unlisted
type becomes `.bin` rather than a guess. A hardlink shares the bytes; a copy is the fallback when
the cache is on another filesystem.

It **refuses to write until `.yidam/vault/` is ignored**. It asks `git check-ignore` rather than
grepping `.gitignore`. A repository ignoring it some other way is not reported as broken. A
licensed document in a tracked path is the leak `push` refuses, arriving through `git add -A`
instead.

### What `push` refuses

**`vault push` is the first egress channel `yidam` itself opens.** An artifact must clear two
independent checks, and neither implies the other:

- **`.yidam/private-paths`** — about *this repository*. An artifact whose record sits under a
  declared path is never uploaded, whatever its licence says. Same rule the release workflow
  applies to a bundle, for the reason it gives: *the artifact outlives the access.*
- **`redistributable`** — about *the source*. A catalog artifact is **not pushed unless its record
  says `redistributable: true`.** A default of "upload unless told otherwise" would make the first
  push anybody runs a redistribution nobody chose. A catalog is full of papers.

**Both checks are [policy](#the-rules-this-repository-writes-about-itself)**, not code in this
binary. `disclose/record` covers an artifact the catalog names, `disclose/derived` one this
repository computed. What they say is unchanged. A repository may state its own rule in
`.yidam/policy/`, and `yidam policy check` names it if it has.

Refusals are grouped by the store they were headed for, each under that store's own `audience`.
The reader learns what they were about to publish to, and with several vaults, which boundary
held. `--artifact` and `--vault` narrow what is sent and never bypass either check. A digest the
corpus does not record is refused, because it carries no `redistributable` and no path to check.

`yidam doctor` warns when two vaults resolve to the same credentials. That is legal, since one
account can own two buckets. It is also what a half-finished isolation setup looks like. The two
are indistinguishable from outside, so it reports the shape and lets the reader decide.

## The rules this repository writes about itself

A gate's refusals are rules. A rule compiled into this binary is one the corpus it governs cannot
argue with. RFC-0024 makes the rule a committed file instead: **git stores the rule, and `yidam`
evaluates it.** The rules are [Rego](https://www.openpolicyagent.org/docs/policy-language),
evaluated in-process — there is no daemon, no sidecar, and no network.

| Command | What it does |
|---|---|
| `policy check` | Compile every rule; report each decision and whether it is inherited or this repository's own. Exits nonzero if a rule names a builtin this build does not carry |
| `policy eval --decision <name>` | Ask one decision about one situation. Reads the input as JSON from `--input <file>` or stdin; `--explain` names the rule that fired |
| `policy test` | Run every `test_*` rule in every `*_test.rego` |

None of these needs a repository. The default policy is compiled in. Somebody working out why a
push was refused can ask without a checkout.

### The decisions

The first family is **disclosure** — what this repository may let leave.

| Decision | Asks |
|---|---|
| `disclose/at_rest` | May this material sit in this repository, given whether it is public? |
| `disclose/record` | May these bytes be uploaded, given what their catalog record says? |
| `disclose/derived` | May this computed artifact — an index, an embedding set, a bundle — be uploaded, given what it was built from? |

`disclose/record` and `disclose/derived` are separate because their evidence is. A record-bearing
artifact is judged by what its record says. A computed one has no record, so it is judged by what
it encodes. Neither consults whether the repository is private, and `at_rest` does — *the artifact
outlives the access*.

**Routing is not a policy decision.** Which vault an artifact goes to is [`vault
list`](#artifacts)'s question. Whether it may go at all is this one. The two are kept apart
because they fail differently. A route is edited casually by somebody reorganising storage. A
licence is not something that edit is allowed to undo.

### Overriding a rule

Write `.yidam/policy/<name>.rego` declaring the same package. **That rule then decides** —
including by being more permissive than the default. RFC-0024 records the argument for and
against that.

An override is never silent. `policy check` names it and the file it came from. `policy test` runs
the *inherited* cases against your rule, and reports which expectations it no longer meets:

```
$ yidam policy test
  ok       test_no_overlap_permits
  changed  test_a_private_path_beats_a_licence
  changed  test_silence_is_not_a_licence

10 passed, 0 failed, 5 changed by an override
```

Those are not failures; a repository is entitled to decide. They are the list to read before
concluding the override says what you meant. `policy check` compares *text*. Whether a rule is
more permissive than the one it replaced is a question about every possible input. Nothing here
claims to have answered it. The `changed` list is the closest thing to an answer.

A repository's *own* `*_test.rego` failing is a failure, and it exits nonzero.

An override is also reported by `yidam lint` as `policy-override` at `Info`. It therefore reaches
the JSON report and the editor. `yidam doctor` reports it too, and is where a policy that does not
compile is caught, as a `fail`. Neither gates: the repository decided. What neither permits is
deciding *quietly*.

## Export

| Command | What it does |
|---|---|
| `export` * | Export the domain model. `--format`, `--out`, `--list` |
| `bundle` * | Alias for `export --format bundle`, kept for compatibility *(no flags)* |
| `schema` * | Emit JSON Schema for the corpus shapes into `.yidam/schemas/`. `--settings` prints the editor `yaml.schemas` mapping instead |

`yidam export --list` reports each format and its implementation status in *your* build. That is
the reliable answer, because two of the six are feature-gated:

| Format | Produces | Needs |
|---|---|---|
| `bundle` | `.yidam/bundle.yiz` — corpus, ontology, skills, decisions, index | default |
| `web` | Feeds for the browser shell. `--webllm-model` names the chat panel's model | default |
| `graphml` | GraphML for graph tools | default |
| `llms` | A flattened corpus for a context window. `--token-budget` | default |
| `rdf` | Turtle and JSON-LD. `--rdf-format` picks one | `export-graph` |
| `sqlite` | SQLite + sqlite-vec | `export-sqlite` |

## Serving the domain computer

| Command | What it does |
|---|---|
| `serve --mcp` | MCP over stdio — the agent surface. See [Connecting an agent](mcp-server.md) |
| `serve --mcp --http` | The same server over HTTP, for a client that takes a URL rather than spawning a process. `--bind` (loopback by default), `--port`, `--allow-origin` |
| `serve --lsp` | LSP over stdio — the editor surface. See [Editor setup](editor-setup.md) |

**`--root <DIR>` names the corpus**, on every transport. Without it `serve` finds one from
wherever the client started the process. That makes the working directory load-bearing, which
[Connecting an agent](mcp-server.md#the-working-directory-is-load-bearing) used to document a `sh
-c 'cd … && exec …'` workaround for. It takes the corpus directory or any directory inside one. A
directory that is not in a corpus is refused, not served empty.

**Both transports are in the light default build.** `--features index` upgrades MCP's `retrieve`
from keyword to semantic search, and adds nothing else. A default binary still serves every other
tool. It says `degraded` on the calls where the difference shows.

## Measuring the corpus

| Command | What it does |
|---|---|
| `bench` | The committed goal set: anchored traversal against flat retrieval. `--budget`, `--scaling` |

`--scaling` measures the arms that are functions of N, over generated corpora rather than this
repository's own. It needs no index. The flat arm is constant in N, and is excluded by argument
rather than by omission.

## Deriving and maintaining a repository

| Command | What it does |
|---|---|
| `clone <target>` * | Copy the template into a new directory and `git init` it. Target must not exist |
| `overlay <target>` * | Add yidam infrastructure to an existing git repo. `--backfill`, `--backfill-ref` |
| `backfill` * | Write a decision record for each epistemic commit in history. `--since` |
| `tonpa <sub>` * | Manage bundle dependencies in `.yidam/tonpa/`. **Needs `tonpa`** (a default) |

`clone` copies everything except `docs/` and `examples/`. The documentation here describes yidam
itself. An example is a whole foreign corpus that a new repository should not be born holding.
`overlay` refuses a target that already has a `.yidam/`. It adds `yidam/`, `sadhana/`,
`BOOTSTRAP.md`, `mise.yidam.toml` and the `.yidam.toml` pin, without touching the repository's own
content.

`backfill`'s classification is **heuristic** — it reads leading verbs. It does not extract
corpus nodes, and the records it writes are a starting point for a person, not testimony.

### `tonpa` subcommands

| Subcommand | Effect |
|---|---|
| `add` | Add a dependency, fetch it, and update the lock file |
| `install` | Install everything declared in `.yidam/tonpa.toml` |
| `remove` | Remove a dependency and delete its installed files |
| `list` | Installed packages with node counts and model info |
| `status` | Per-dependency: installed / missing / stale |
| `verify` | Check installed bundle hashes against the lock file |
| `update` | Re-fetch one or all dependencies to the latest bundle |

[Sharing a derivation](sharing-derivations.md) covers publishing a `.yiz` and consuming one,
and what a cross-corpus citation is and is not.
