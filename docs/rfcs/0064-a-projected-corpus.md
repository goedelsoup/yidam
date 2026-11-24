# RFC-0064 — A projected corpus declares its producer and its id, and yidam stops assuming it was authored

- **Status:** Draft
- **Track:** I47
- **Relates to:**
  - #1510 (a projection mode for generated corpora), parent epic #1513
  - #126 (`authorship.yml generated:`, which this subsumes for the corpus directory)
  - #421 and #1000 (`--root`, and a named root that finds no corpus fails)
  - RFC-0033 (vector keys built from the first twelve characters of the genesis hash)
  - RFC-0047 (sealed articles, which read the corpus's history)
  - RFC-0028 §6 (`direction: projected`, an edge whose far side is computed; a different
    sense of the word, see "What this does not do")
- **Versioning layers touched:**
  - **Config.** `.yidam/config.toml` gains a `[corpus]` table with `mode` and `generated_by`.
    Absent means `mode = "authored"`, which is every corpus today. `YidamConfig` ignores keys it
    does not know, so a CLI from before this change reads a projection as authored. It does not
    refuse, it simply behaves as it does now (§1.3).
  - **Corpus files.** A projection carries `.yidam/corpus/corpus.toml`, written by its producer.
    The corpus walk reads only `.yml` and `.md`, so the file is invisible to every older check.
  - **Ontology.** A class may declare `outgoing: optional`. This is a new key on `.ont.yml`, in
    either mode. An older CLI ignores it and keeps raising `orphan-out`.
  - **CLI.** Every command that reads a corpus honours `--root`, and a new `YIDAM_ROOT`
    environment variable. Commands that write a corpus refuse in projection mode.
  - **Reports.** `status --format json` reports `genesis_date: null` and a new `mode` key for a
    projection. The report schema declares both.
  - **The bump.** It is minor for the CLI. No authored file moves and no migration is owed. An
    upgrade note is owed when this is implemented, since a projection's producer has a file to
    write. No existing note is rewritten (§6).
- **Downstream reference case:** watermark-directory, read through the GitHub API at `main`
  691b9db1 (2026-11-18). Nothing was written to it and its local checkout was not touched.

## Summary

Some corpora are not written by people. watermark-directory projects its application database
into a git-ignored `.yidam/corpus/`, once per site, for 27 sites. CI then runs `graph-check`,
`lint` and `vault verify` over each projection. Every file in that tree is overwritten on the
next run, so no one ever edits one.

yidam assumes otherwise in four places. It names the corpus after the repository's first
commit. It finds the corpus by walking from the working directory. It treats "generated" as a
prose-check concern only. And it requires every node to link somewhere, so the producer invents
an anchor node for each site to link to.

This RFC adds one declaration, `[corpus] mode = "projection"` with a named `generated_by`
producer, and keys four changes off it:

- the corpus id comes from a file the producer writes (§2);
- commands that author or commit a corpus refuse, naming the producer (§3);
- the corpus directory is a generated region without an `authorship.yml` entry (§4);
- `doctor` stops checking the template-vendoring surfaces a projection never has (§5).

Two changes apply in either mode. A class may say its nodes need no outgoing link (§6). Every
corpus-reading command honours `--root` and `YIDAM_ROOT` (§7).

## Problem

### 0.1 What was measured

watermark-directory's `src/watermark/site/corpus_mirror.py` is 65 KB. It writes the active
site's nodes into `.yidam/corpus/`, and `.yidam/authorship.yml` declares three generated
regions: `.yidam/corpus` (by `watermark corpus-mirror`), `.yidam/exports` and `.yidam/schemas`.
Its `.yidam/config.toml` holds nothing but a `[vault.default]` table.

| what yidam assumes | what the projection does about it |
|---|---|
| the corpus is named by `genesis_hash(root)` | all 27 sites get watermark-directory's own genesis, so their RDF subjects and vector keys collide. A peer site projected into a scratch directory gets `None`, and `export-rdf` bails with *"Give the corpus a repository of its own"*. |
| every node has an outgoing link (`orphan-out`, an error) | every node falls back to a link to a synthetic `artifact/site-<slug>`, which exists for no other reason. That fallback appears at nine places in the mirror. |
| an edge with properties is a node | relations are projected as `relation/a-rel-b` nodes. All were renamed when `name-not-a-slug` became an error. |
| `generated:` is about prose | the declaration downgrades the prose-link and line-citation walks, coupling and `authorship-region-stale`. Graph checks, `doctor`, the genesis checks and upgrade notes ignore it. The repo reports that four of its five upgrade notes applied to nobody there. |
| the corpus is the nearest one to the working directory | a work folder inside the repository reported on the main mirror, and the repository added a guard. |

### 0.2 What `--root` covers today

`--root` is a per-command `RootArg`, resolved by `resolve_root` (`paths.rs`), which walks up to
the nearest `.yidam/`. Fifty-two call sites still call `repo_root()` directly, which reads
`git rev-parse --show-toplevel` and ignores the flag.

The nine calls in `cmd/vault.rs` matter most here, because `vault verify` is one of the three
commands watermark-directory's CI runs. Outside vault, the direct calls sit in
`rename`, `bundle`, `embed`, `migrate`, `schema`, `init`, `backfill`, `propose`, `bench`,
`cluster status`, `cluster egress`, `cluster workflow`, `overlay`, `clone`, `phase`,
`index-build`, `gather`, `regen`, `export-llms`, `catalog fetch`, `catalog extract`,
`catalog reconcile`, `build`, `run`, `dispatch` and the tonpa subcommands. Some of those
calls are a fallback beside `resolve_root`, and the rest ignore the flag outright.

### 0.3 Why a nickname was rejected before

`Provenance.genesis_hash` in `model.rs` records why a declared corpus name was not adopted. Of
sixteen corpora measured, none declared one. And a subject must not change: an RDF subject or a
vector key that moves when someone edits a config file orphans every consumer that stored it.

Both reasons still hold for an authored corpus, and this RFC leaves that case alone. They do not
hold for a projection. A projection has no genesis of its own to fall back on: its host
repository's genesis is shared by every site it projects. And it already has a stable name, the
one its producer uses to choose what to project. The question is not whether a corpus may name
itself. It is who owns that name's stability, and for a projection the only candidate is the
producer (§2.3).

## Design

### 1. The declaration

```toml
# .yidam/config.toml
[corpus]
mode = "projection"
generated_by = "watermark corpus-mirror"
```

#### 1.1 Both keys or neither

`mode` takes `"authored"` (the default) or `"projection"`. `generated_by` is required when
`mode = "projection"` and refused otherwise. It is a free-text name of the command or service
that writes the corpus. Every refusal in §3 quotes it, so it should be what a reader would run.

The table is `deny_unknown_fields`. That is the lesson `[derive]` learned in RFC-0063 §3.5: a
mistyped key is otherwise read as absent.

#### 1.2 Why config.toml and not `.yidam.toml`

`.yidam.toml` is the template pin, and it is the file `yidam clone` writes. A projection has
no use for a pin: watermark-directory's is there only so CI can build the CLI at the same
commit. The mode describes the corpus, so it goes in the file that describes the corpus.

#### 1.3 An older CLI

A CLI from before this change ignores `[corpus]`. It reads the projection as authored, which is
exactly what it does today, anchor nodes and shared genesis included. Nothing that passes now
starts failing. A producer that adopts `outgoing: optional` and drops its anchors (§6) needs the
new CLI, and the mirror's pin already says which CLI CI builds.

### 2. The corpus id

#### 2.1 The producer writes it

```toml
# .yidam/corpus/corpus.toml, written by the producer on every run
id = "3f9c2a71b0de"
```

`id` is twelve lowercase hex characters, the length RFC-0033 already cuts the genesis hash to
for vector keys. In projection mode a missing file, a missing `id` or a malformed one is an
error that names `generated_by`. yidam does not fall back to the host repository's genesis,
because that fallback is the collision this RFC exists to remove.

The file sits in the projected tree, not in config, because the producer rewrites the tree and
only the tree. A per-site id in `.yidam/config.toml` would mean the producer edits a file it
otherwise leaves alone, or every site shares one config.

#### 2.2 One function, two kinds of consumer

A new `corpus_id(root)` returns the declared id in projection mode, and the first twelve
characters of `genesis_hash(root)` otherwise. Every consumer that uses genesis as a *name* moves
to it:

- `export-rdf`'s `corpus_component`;
- the vector store's corpus id, `index push` and `remote.corpora`;
- retrieval's corpus scoping;
- the warehouse corpus table;
- the bundle manifest;
- `embed`'s cache key.

The consumers that use genesis as *history* do not. They read the first commit for what it says
about the past. A projection's host repository has a past, but it is not the corpus's. So in
projection mode they report that the corpus has no genesis instead:

- `status` reports `genesis_date: null`, and its domain line says *projected by
  `<generated_by>`* rather than a date;
- RFC-0047's sealed-article checks do not run, since there is no authored history to seal
  against;
- `doctor`'s shadow detection, which compares a corpus's genesis with its neighbours', skips.

`remote.corpora` values stay twelve hex characters in both modes, so an index that serves an
authored corpus and a projection does not need to know which is which.

#### 2.3 Stability is the producer's

yidam checks the id's shape and nothing else. It cannot check stability: a producer that mints
a fresh id on every run produces a valid file every time. The upgrade note will recommend
deriving the id from the producer's own stable key. For watermark-directory that is the first
twelve hex characters of a SHA-256 of the site slug. The note will also say what changes when
the id does: RDF subjects, vector keys, and the entry in any `remote.corpora` list.

### 3. Writers refuse

In projection mode, every command that writes under `.yidam/corpus/` or commits on the corpus's
behalf refuses before it reads anything:

> This corpus is a projection written by `watermark corpus-mirror`. Change it there and
> re-run the projection; `yidam propose` would be overwritten on the next run.

The commands are `propose`, `phase`, `rename`, `migrate` and `backfill`, plus the scaffold and
vendor steps of `init`, `clone` and `overlay`. Each one already resolves a root, so the check is
one call, `refuse_if_projection(root, "<verb>")`, placed at that point.

Commands that write outside the corpus keep working: `regen`, `export-*`, `schema`, `index-build`
and `embed`. watermark-directory's `generated:` entries for `.yidam/exports` and `.yidam/schemas`
show those outputs are expected there.

### 4. The corpus directory is generated

In projection mode `.yidam/corpus` is a generated region attributed to `generated_by`, whether or
not `authorship.yml` says so. An explicit entry for the same path stays valid and wins, so the
existing file in watermark-directory keeps meaning what it means. Two entries with different
`by:` values are an error, since then the attribution in a finding would be a guess.

Graph findings stay errors. A dangling edge in a projection is a defect, and a downgrade to info
would hide the producer's bug. What changes is who a finding names: in projection mode, every
`graph-check` and `lint` finding under the corpus carries the same *"written by
`<generated_by>`"* attribution the prose walks already give generated regions. The reader is
sent to the producer, not to the YAML.

### 5. `doctor` skips the vendoring surfaces

A projection has no vendored prelude, no regenerated template blocks and no scaffolded CI. In
projection mode `doctor` skips `PRELUDE`, `REGEN`, `ROUTES`, `SCAFFOLD`, `STAMPS` and `CI`. It
prints them as *skipped: projection* rather than omitting them, so a reader can tell a skipped
check from a check that passed. Every other check runs unchanged. A new `MODE` line at the top
of the report says which mode was read and from where.

### 6. A class may need no outgoing link

```yaml
# an .ont.yml class
- name: site
  outgoing: optional
```

`outgoing` takes `required` (the default) or `optional`. `orphan-out` skips nodes of a class that
says `optional`. This works in either mode. An authored corpus has the same shape: a root class,
a glossary entry or a terminal source that nothing further is said about. Tying it to projection
mode would force those corpora to invent anchors too.

This is a class-level exemption, not a per-node one. A per-node flag would let a producer write
`outgoing: optional` on everything and turn the check off. A class has to be declared in the
ontology, which is where its shape is reviewed.

For watermark-directory, the `artifact/site-<slug>` anchors and the nine fallbacks to them can
go once the classes that legitimately terminate say so.

### 7. `--root` everywhere, and `YIDAM_ROOT`

Every command that reads a corpus takes `--root`. `YIDAM_ROOT` supplies it when the flag is
absent, and the flag wins when both are given. Both go through `resolve_root`, so a named root
that finds no `.yidam/` fails exactly as `--root` does today (#1000).

A gate fails on any direct `repo_root()` call outside `paths.rs`, except in the commands that
create a corpus where none exists yet: `init`, `clone` and `overlay`. The existing
`tests/suite/root_flag.rs` gate then covers the new commands automatically, since it reads the
roster from `--help`.

`YIDAM_ROOT` is what a producer's CI wants: it sets the variable once per site and runs the
same three commands 27 times, with no working-directory guard.

## What this does not do

- **It does not add edge records.** Relations stay nodes. An edge with properties is a larger
  change to the node model, the RDF export and every graph check. It needs its own RFC, and
  `outgoing: optional` removes the anchor nodes without waiting for it.
- **It does not publish the node and ontology schema.** A producer still reads the Rust parser
  as the spec. Publishing the schema and conformance cases as a release artifact is #1565.
- **It does not touch upgrade notes.** Notes stay written for authored corpora. When this is
  implemented, the projection note will list which existing notes do not apply to a projection.
  No note grows a mode switch.
- **It does not change authored corpora.** No key, no default and no output moves for a corpus
  without `[corpus]`.
- **It is not RFC-0028's `direction: projected`.** That is an edge computed from its far side,
  inside an authored corpus. This is a whole corpus computed from outside yidam.

## Alternatives considered

- **`--corpus-id` on each command.** Every invocation would have to repeat it, and the one that
  forgets it silently names the corpus after the host's genesis. A file in the tree travels with
  the tree.
- **The id in `.yidam/config.toml`.** It suits a repository with one projection. It does not suit
  27 sites in one repository, where the producer rewrites only the projected tree.
- **Derive the id from the root's path.** It changes when the checkout moves, which is the
  instability §0.3 rejects.
- **Downgrade graph findings to info in a generated region.** This is how prose findings are
  treated, but a prose finding in generated text is usually the template's wording. A graph
  finding in a projection is a bug in the producer, so it stays an error that names the producer.
- **Let `generated:` in `authorship.yml` imply the mode.** A region can be generated inside an
  authored corpus, and `.yidam/exports` is. The mode is a claim about the whole corpus, so it is
  stated once, about the whole corpus.
- **A per-node `outgoing: optional`.** It turns the check off for anyone who writes it everywhere
  (§6).

## Migration

None is owed. An authored corpus reads no new key. A projection adopts the mode by writing
`[corpus]` and `corpus.toml`, and adopts `outgoing: optional` class by class. Its existing
`authorship.yml` entry stays valid.

For watermark-directory the sequence would be:

1. Write `corpus.toml` from the mirror, with the id derived from the site slug.
2. Add `[corpus]` to `.yidam/config.toml`.
3. Mark the terminal classes `outgoing: optional` and drop the anchor fallbacks.
4. Replace the working-directory guard with `YIDAM_ROOT`.

Step 1 changes every RDF subject and vector key the 27 sites have published, from the shared
host genesis to a per-site id. Those keys collided before, so nothing correct is orphaned, but
the vector index has to be re-pushed.

## Falsifier

This design is wrong if any of these turns out true when it is implemented:

- **A history consumer needs the host's genesis in a projection.** If `status`, the sealed
  articles or `doctor`'s shadow check turns out to need the host repository's past to say
  something true about a projection, then §2.2's split between name and history is wrong.
- **watermark-directory keeps an anchor.** If, with `outgoing: optional` on the classes the
  mirror names, any site still needs a synthetic node to pass `graph-check`, then the exemption
  belongs per node, or edge records are the real fix.
- **A writer is missed.** If any command still writes under `.yidam/corpus/` in projection mode
  without refusing, then the gate in §3 should be by write path, not by command list.
- **The `repo_root()` gate needs a long allow-list.** If more than the three corpus-creating
  commands need an exemption, `--root` coverage is the wrong rule and some commands are not
  corpus readers at all.

## Unresolved

- **Should `bundle` carry the mode?** A bundle made from a projection would be read as authored
  by whoever unpacks it. Recording the mode and `generated_by` in the manifest seems right, but
  no consumer of a projected bundle exists yet to say.
- **Does `vault` belong under §3?** `vault verify` reads, and CI runs it. `vault push` writes to
  remote storage, not to the corpus. Today neither honours `--root`, and §7 fixes that. Whether
  a projection should push its own vault, or the producer's, is not settled here.
