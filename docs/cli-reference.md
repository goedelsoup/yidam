# CLI reference

Every command `yidam` carries, grouped as `yidam --help` groups them. This page is the map;
`yidam <command> --help` is the detail, and is generated from the same source as the behaviour,
so it cannot disagree with the binary you have.

Two conventions run through the whole surface.

**A `*` means the command rewrites files in the repository it is run against.** Twenty-three
do. That was previously visible only in each command's long help, where you had to already
suspect it to go looking — which is the wrong way round for a tool people point at a checkout
they only meant to inspect.

**`--format json` is available on most commands** and emits the machine-readable report
contract of [RFC-0016](rfcs/0016-editor-surface.md). `text` is the default and is byte-stable:
the editor surface and CI both consume the JSON, so the prose is free to stay prose. Commands
below that take no options at all are marked *(no flags)*.

Some commands need a build carrying the matching cargo feature. Those are marked, and
[Installation](installation.md#which-build-you-have) has the table. `yidam --version` prints
the feature list of the binary in your hand.

---

## Checks and gates

Read-only, and they exit nonzero on a problem — which is what makes them usable as CI steps.

| Command | What it answers |
|---|---|
| `doctor` | Is this setup sound? Nine checks, each with a verdict and a remedy. `--strict` makes warnings fail. See [Troubleshooting](troubleshooting.md) |
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

The baseline is what makes `lint` answer *did this change make the corpus less clean?* rather
than *is the corpus clean?* — see [Configuration](configuration.md#yidamlint-baselineyml).

## The practice

One command, and it is not a gate. Read-only, offline, and it exits zero however much is owed.

| Command | What it answers |
|---|---|
| `due` | What is due? Four clocks read together — index staleness, catalog TTL, unanswered questions, phases in flight. `--strict` exits nonzero on a due clock |

### `due` is not `doctor`, and the difference is the point

`doctor` answers *is this setup sound now*. It is read under suspicion, and it exits nonzero on
what is wrong. `due` answers *is it time*, and it is read on a cadence — from a cron job, a
weekly ritual, or the start of a session.

**A corpus with three expired sources is not unhealthy. It is owed.** Nothing about it is
broken, no traversal will lie, and the gate is green. Folding that into `doctor`'s warnings
would tell a reader that a repository doing exactly what it is meant to do has a problem, and
the reader would learn to skip the line. So the two reports are separate, `due` has its own
verdicts — `due`, `ok`, `undeclared`, `unmeasurable` — and it exits zero unless you pass
`--strict` to ask for a signal.

### Every interval is declared, and a clock nobody set never comes due

A clock is an age and an interval. The age is measured; the interval is always something the
corpus said about itself, never a number in the binary — the reasoning is
[`escalate_after`](configuration.md#lint-escalate_after)'s.

| Clock | Measures | Interval | Unit |
|---|---|---|---|
| `index` | Corpus files changed since the index was built | `[due] index_after` | files |
| `catalog` | How long since a source record was retrieved | `[catalog] ttl_days`, or an entry's own | days |
| `questions` | How long a question has gone unanswered | `[due] questions_after` | corpus commits |
| `phases` | How long a bounded inquiry has been in flight | `[due] phases_after` | days |

The catalog clock reads the interval [where it already lived](configuration.md#catalog-ttl_days)
rather than restating it under `[due]`: a source's TTL is a statement about the source, and two
places to set one number means one of them is wrong.

A clock with no interval reports what it measured and is never due — with the key that would
set it as its remedy. That is the state of every repository that has not opted in, and it is
the design rather than a degraded mode.

Two of the four count days and two do not, which is deliberate. How long a question has gone
unanswered is a fact about the repository, so its clock is `HEAD`: a corpus that has not
committed has not ignored anything. A source's TTL and a phase's time in flight are facts about
the world, which does not stop moving because nobody committed.

### What discharges a clock

`due` reports; it does not act. Only one of the four clocks is discharged by something
[`propose`](#propose-is-deliberately-small) can draft:

| Clock | What discharges it |
|---|---|
| `index` | `yidam index-build`. A build, not a commit |
| `catalog` | `yidam propose`, which already drafts an `open:` against each expired source |
| `questions` | A person. Deciding a question is answered is a resolution event, and Article V confines those to a sangha |
| `phases` | A person. Settling a phase or abandoning it is not a mechanical consequence of a finding |

Each clock names its own remedy in the report, so the distinction is visible where it matters
rather than only here.

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

In a derived repository a stale REGEN block is a failing build, so `mise run regen` before
committing is the ordinary loop and `yidam regen --check` is what CI runs.

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
| `check-diff <range>` | What a code diff names that the ontology does not ([RFC-0021](rfcs/0021-diff-alignment.md)) |
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

**Whitespace around a hop is required.** `-rel->` and `<-rel-` are single tokens, and that is
what lets a hyphenated relationship name be unambiguous. `~"…"` is a similarity anchor, which
opens on `--anchor-k` entry nodes (default 1) — an anchor is a starting point, not an answer.

| Flag | Applies to | Effect |
|---|---|---|
| `--select` | `query`, `estimate` | Fields to project: `node`, `class`, `label`, `description`, `body`, `properties.<name>` |
| `--limit` | `query`, `estimate` | Bounds the *projection*, not the traversal — the reported count is always the full one (default 50) |
| `--budget` | `pack`, `estimate` | Approximate token budget, 1 token ≈ 4 chars. `pack` is **unbudgeted by default**, because a default budget would silently truncate the first pack anybody builds |
| `--at <ref>` | `query` | Answer as of a commit, reconstructed from git objects; the working tree is never touched |
| `--between <a..b>` | `query` | Answer at every corpus-touching commit in the range, as a series |
| `--across` | `query` | Query installed dependencies too. Every result says whose corpus it came from, and no hop crosses a corpus boundary |

### `migrate` subcommands

A class definition cannot be corrected in place once the class contract gates: editing it puts
every instance in violation until each is fixed by hand. These do both halves together and
write a record of what they touched.

| Subcommand | Changes |
|---|---|
| `class` | Rename a class: its definition, its directory, and every edge that named it |
| `property` | Rename a declared property on a class and on every instance carrying it |
| `retype` | Change a declared property's type; refuses when an instance would not satisfy it |
| `edge` | Point a declared relationship at a different class, at both ends |

### `propose` is deliberately small

Three acts only. `open` records a finding's question against the node it is about; `withdraw`
deletes a node this corpus declared over-collected via `[propose] withdraw_uncited_after`;
`close` retires a question this command opened whose finding is gone.

Nothing merges itself and nothing synthesizes — no edge is drawn, no claim is re-tagged, no
node is authored. It writes git objects and one ref: the working tree, the index and `HEAD` are
untouched, so it is safe to run mid-edit. [RFC-0020](rfcs/0020-proposal-surface.md) has the
argument for why the surface is this small.

## Index and embeddings

| Command | What it does |
|---|---|
| `embed` * | Extract embedding text from corpus instances to `.yidam/embeddings/`. `--no-catalog` |
| `index-build` * | Build the LanceDB vector index and export Arrow IPC for the web shell. `--model`. **Needs `--features index`** |

`embed` walks `.yidam/catalog/` by default. In a real derived corpus the catalog was 51.3% of
the indexable text against the corpus's 41.9%, and leaving it out had been a scope decision
nobody made on purpose.

## Export

| Command | What it does |
|---|---|
| `export` * | Export the domain model. `--format`, `--out`, `--list` |
| `bundle` * | Alias for `export --format bundle`, kept for compatibility *(no flags)* |
| `schema` * | Emit JSON Schema for the corpus shapes into `.yidam/schemas/`. `--settings` prints the editor `yaml.schemas` mapping instead |

`yidam export --list` reports each format and its implementation status in *your* build, which
is the reliable answer — two of the six are feature-gated:

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
| `serve --lsp` | LSP over stdio — the editor surface. See [Editor setup](editor-setup.md) |

**Both transports are in the light default build.** `--features index` upgrades MCP's
`retrieve` from keyword to semantic search and adds nothing else; a default binary still serves
every other tool, and says `degraded` on the calls where the difference shows.

## Measuring the corpus

| Command | What it does |
|---|---|
| `bench` | The committed goal set: anchored traversal against flat retrieval. `--budget`, `--scaling` |

`--scaling` measures the arms that are functions of N over generated corpora rather than this
repository's own, and needs no index — the flat arm is constant in N and is excluded by
argument rather than by omission.

## Deriving and maintaining a repository

| Command | What it does |
|---|---|
| `clone <target>` * | Copy the template into a new directory and `git init` it. Target must not exist |
| `overlay <target>` * | Add yidam infrastructure to an existing git repo. `--backfill`, `--backfill-ref` |
| `backfill` * | Write a decision record for each epistemic commit in history. `--since` |
| `tonpa <sub>` * | Manage bundle dependencies in `.yidam/tonpa/`. **Needs `tonpa`** (a default) |

`clone` copies everything except `docs/` and `examples/` — yidam's own documentation describes
yidam, and an example is a whole foreign corpus that a new repository should not be born
holding. `overlay` refuses a target that already has a `.yidam/`, and adds `yidam/`,
`sadhana/`, `BOOTSTRAP.md`, `mise.yidam.toml` and the `.yidam.toml` pin without touching the
repository's own content.

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
