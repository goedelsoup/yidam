# RFC-0051 — A read of code names the commit it was read at, and the repository that commit is in (the code-artifact pin)

- **Status:** Draft
- **Commands:** `migrate pins`
- **Track:** I37
- **Relates to:**
  - RFC-0019 (`cites:` and its `commit:` pin, the precedent for a pin that `lint` resolves and that moves)
  - RFC-0023 (the `artifacts:` record and its SHA-256 digest, which this leaves alone for bytes a corpus fetched)
  - RFC-0034 (a citation into a file the corpus owns, which this extends to a file the corpus's own repository, or a sibling's, owns)
  - RFC-0035 and #1491/#1495 (the coverage clock, which §6 extends to a registry kept as code)
  - RFC-0046 (the quotation's optional `sha256` pin, the other place a read is dated by its bytes)
  - RFC-0048 (the closed location-kind set, which this keeps closed)
- **Versioning layers touched:** template (`guidelines/directories.md` documents the three new fields on a `file` location) / tooling (a pinned read in the catalog clock and the superseded clock, five lint checks, `[[due.coverage]]` reads TOML and gains `orphans`, one migration). No MCP change. No SDK parity change: the SDKs read catalog frontmatter as a mapping and carry the new keys through untouched.
- **Downstream reference case:** city-engine, a game engine whose corpus lives in its own `.yidam/` and reads six sibling corpora as tonpa dependencies. Measured read-only on 2026-11-18 at its v0.15.0 pin. Parent epic #1513.

## Summary

A corpus that studies code cites the code by a sentence: "read at commit c3b13c3". Nothing
parses that sentence. So nothing can say whether the commit exists, which repository it is in,
whether the file is still what was read, or which of a node's claims rest on a read that has
since gone stale. city-engine has 2,144 such sentences, naming 716 distinct commits. 56 of those
commits are in no repository the sentence names. People find stale reads by hand: 543 of the
repository's 5,048 commits are `refresh:` commits, re-reading a probe after a code change.

This RFC gives a `file` location three optional fields: `commit`, `repo` and `symbol`. Together
they make a **pin**. `lint` resolves a pin the way it resolves a line citation. The pinned
file's identity is its git object id at that commit, which is derived and never stored. `due`
reports a pinned entry once that object has changed, and reports the nodes that cite it once the
entry is re-read. A registry kept as code (a TOML table of keys) can then be a coverage universe,
checked in both directions.

## Problem

### 1.1 What city-engine's catalog says about the code it reads

| Measure (city-engine `.yidam/`, v0.15.0 pin) | Count |
|---|---:|
| `catalog/ce-*.md` entries | 253 |
| … whose location is a path under `crates/` | 244 |
| … that date themselves in prose ("read at commit …", "read again at …") | 108 |
| … with any structured digest | 0 |
| "at &lt;sha&gt;" references in corpus nodes | 2,144 |
| … distinct commits named | 716 |
| … commits that do not resolve in city-engine | 56 |
| … of those, commits that resolve in exactly one sibling corpus | 56 |
| `refresh:` commits in history | 543 of 5,048 |

The issue was filed from a review two days earlier, which counted 219 entries, 1,482 references
and 42 unresolvable commits. Every figure has grown since, and the shape has not changed.

A typical entry is a `kind: file` location and a body that dates itself:

```yaml
location:
  - kind: file
    value: crates/game/src/lib.rs
```

> Read at commit c3b13c3, and again at 86ba7d0, with crates/game/src/clock.rs and
> crates/game/src/city.rs.

A typical citation in a node takes the catalog link and dates it again:

> Source: [the binding crate](../../catalog/ce-binding.md), read at 5d50eabf.

### 1.2 What the prose cannot say

**Which repository.** All 56 unresolvable commits resolve in one of city-engine's six tonpa
dependencies: 13 in traffic-design, 10 each in urban-land-markets and civic-infrastructure, 9 in
land-use-activity, and 7 each in travel-demand and urban-environment-health. The seven
repositories share one unlabelled namespace of hex prefixes. A reader can only tell which
repository a commit belongs to from the surrounding sentence, and a tool can tell nothing.

**Whether the read is still good.** The catalog clock measures age (`retrieved:` against
`ttl_days`). A code read does not age. It goes stale when the code changes, and it stays good
for a year if the code does not. The superseded clock (`cmd/catalog/superseded.rs`) knows when
an entry's *digest* changes, but a code entry has no digest. So city-engine writes `refresh:`
commits: someone re-runs a probe after a change, re-reads it, and re-dates the prose. Many of
those re-reads find nothing changed, and nothing could have told them so in advance.

**Whether the symbol still exists.** A `used_by` field names Rust paths
(`binding::game_micro_idm`, `game::MicroDefaults::load`) and nothing checks them. The line-citation
checks (`cmd/lint/line_citations.rs`, `label_symbols`) do check a symbol named in a link's
label, but only for a `#L` link into the working tree, and only at HEAD.

### 1.3 A registry kept as code

`gameplay-defaults.toml` holds 169 `[[entry]]` keys. Each is meant to have one `engine-choice`
node, carrying it as `properties.key`. 10 keys have no node, and 3 of those (for example
`pro-forma-build-period`) appear nowhere in the corpus. The coverage clock (#1491) is built for
exactly this question, but it reads only a catalog artifact, and only through a JSON path or a
CSV column. A TOML file in the corpus's own repository is neither. The coverage clock also asks
only one direction: it does not report a node whose key the registry no longer holds.

### 1.4 What exists to build on

- `CatalogLocation` (`parse.rs`) has no `deny_unknown_fields`, so a binary that predates a new
  field reads the entry and ignores the field.
- `CATALOG_LOCATION_KINDS` is a closed set (RFC-0048 decision 2). A new kind is refused by every
  older binary's `catalog-location-malformed`.
- Tonpa dependencies (`cmd/tonpa/deps.rs`) are stable names. A `path` dependency carries a
  `rev`, the commit the corpus accepted, and `cmd/tonpa/pinned.rs` already reads a dependency's
  git objects at that commit (today only under `.yidam/corpus`).
- `relocate` and `label_symbols` in `cmd/lint/line_citations.rs` already resolve a symbol by
  name against a file's lines.

## Proposal

### 2. The pin: three fields on a `file` location

```yaml
location:
  - kind: file
    value: crates/game/src/lib.rs
    commit: 86ba7d0f3c...        # the read; full or abbreviated (≥ 7), resolved by lint
    repo: traffic-design          # optional; a tonpa dependency name; absent = this repository
    symbol: game::MicroDefaults::load   # optional; checked to exist in the file at `commit`
```

- **`commit`** makes the location a pinned read. Without it, a `file` location means what it
  means today.
- **`repo`** names a key of `[dependencies]` in `.yidam/tonpa.toml`. When it is absent, the
  repository is the one the corpus lives in, which is git's own top level and not the corpus
  root. That matters because city-engine's corpus is `.yidam/` inside the engine repository, so
  `crates/` is outside the corpus root but inside the repository.
- **`symbol`** is a `::`- or `.`-separated path. The check looks for its last segment as a whole
  word in the file at `commit`, which is what `label_symbols` checks at HEAD. It is a presence
  check and claims nothing about the symbol's meaning.
- **`value` may name a directory.** Its object is then a git tree, and a change to any file
  under it is a change to the read.

The pin's **identity** is the git object id of `value` at `commit` in `repo`: a blob for a file,
a tree for a directory. It is never written down. Any clone that has the commit can compute it,
and a stored copy could only disagree with it.

### 3. `lint` resolves the pin

| Check | Severity | Fires when |
|---|---|---|
| `catalog-pin-unresolved` | error | `commit` is not a commit in `repo`; or `repo` is not a dependency name; or `value` does not exist at `commit` |
| `catalog-pin-ambiguous` | error | an abbreviated `commit` matches more than one object in `repo` |
| `catalog-pin-symbol-missing` | warning | `symbol`'s last segment does not occur as a word in `value` at `commit` |
| `catalog-pin-unchecked` | info | the pin cannot be checked here: a shallow clone without the commit, or a `url`/`github` dependency with no local checkout |
| `prose-commit-unpinned` | warning | node or catalog prose says "at &lt;hex&gt;" (7–40 digits) for a commit that is not in this repository; the finding names the dependency it does resolve in, if any |

`catalog-pin-unchecked` is separate from `-unresolved` for the same reason the superseded clock
counts `unread` apart (`clock_superseded`): a clone that cannot see a commit has not shown that
the commit is missing. CI checkouts are often shallow.

`prose-commit-unpinned` is the migration's lever and not a permanent rule about prose. A commit
in this repository named in prose is left alone, because it is unambiguous. The check fires only
on the case the reference data shows to be unreadable, a commit that belongs to someone else, and
it lands with a baseline (`.yidam/lint-baseline.yml`), so existing prose costs nothing until it is
next touched.

All resolution goes through one `git cat-file --batch-check` process per repository, fed
`<commit>:<path>` lines. 244 entries cost one process, not 244.

### 4. `due`: a pinned read is stale when its object changes, not when it ages

**The catalog clock** measures a pinned location by change instead of by age. A pinned entry is
due when the object at `commit` differs from the object at the **reference revision**:

- for this repository, `HEAD`;
- for a dependency, its `rev` in `tonpa.toml`. That is the revision the corpus has agreed to
  read, so a sibling's new commits are owed nothing until the pin moves.

`ttl_days` does not apply to a pinned location: a read of unchanged code is current however old
it is. The clock's line names the change, not the age:

```
ce-game — crates/game/src/lib.rs changed in 4 commits since 86ba7d0 (now 1d062a6d)
```

No eighth clock is added. "Is this source still what was read?" is the catalog clock's question,
and a pin changes only how it is answered for one kind of source.

**The superseded clock** treats a re-read as a new edition when, and only when, the object
changed. An edit that moves `commit` from `86ba7d0` to `1d062a6d` supersedes the earlier read if
the two object ids differ. The nodes that cite the entry and were last touched before that commit
are then owed a re-read, by the clock's existing rule. If the ids match, the move supersedes
nothing. Re-dating a read whose code did not change no longer asks anything of the nodes that
cite it, and the catalog clock never asked for that re-read in the first place.

This is what replaces the hand-written `refresh:`. The person still re-reads, but only when `due`
names an entry whose code changed. When they do, the nodes that rest on that entry are listed
for them.

### 5. A citation in a node

A node already cites a catalog entry structurally, through an edge's `source:` and through a
markdown link into `catalog/`. This RFC adds nothing to the node. The read date that nodes write
in prose today ("read at 5d50eabf") is the entry's `commit`, and the superseded clock already
dates each node against the entry's history (§4). A second pin on the edge would be a second
place for the same fact, and the two would drift apart.

A node that genuinely rests on an *older* read than the entry's current one is the case the
superseded clock reports, and the remedy is to re-read, not to pin the node to the old read.

### 6. A registry kept as code: two extensions to `[[due.coverage]]`

```toml
[[due.coverage]]
source   = "ce-gameplay-defaults"   # a catalog entry; may now be a pinned `file` location
select   = "$.entry[*].id"          # the existing JSON-path selector, now also over a TOML file
class    = "engine-choice"
property = "key"
orphans  = true                     # new: also report instances whose key no member names
```

- **A pinned source needs no fetch.** The universe is read from the file at the pin's `commit`,
  so the coverage figure and the catalog clock describe the same read. A registry that changed
  after its read is reported by §4 as stale, and this coverage is reported as *stale coverage*,
  the state #1491 already defines for a source past its TTL.
- **`select` reads TOML.** A file whose path ends in `.toml` is parsed as TOML and walked with
  the same path syntax `select` already uses for JSON (`$`, `.key`, `['key']`, `[n]`, `[*]`).
  No new selector field is added.
- **`orphans = true`** adds the reverse direction: an instance of `class` whose `property` names
  no member. It respects `scope` and `matches` (#1495). Against city-engine it would report the
  10 keys with no node and, in the same report, the nodes whose keys the registry has dropped.
  yidam has no retired standing, and city-engine marks a retired choice only in its label, so
  how an orphan that was retired on purpose is told apart is open question 5.

### 7. `yidam migrate pins`

The migration writes the pins that 108 of city-engine's entries already state in prose. For each
`kind: file` location with no `commit`:

1. Find the *last* "read at (commit) &lt;hex&gt;" or "read again at &lt;hex&gt;" in the entry's body.
2. Keep it only if the commit resolves in this repository (or in exactly one dependency, which
   then becomes `repo`) and `value` exists at it.
3. Write `commit:` (and `repo:`), and change nothing in the prose.

Like the other `migrate` subcommands, `yidam migrate --dry-run pins` prints what it would write
and changes nothing. A body that names no commit, or one that resolves nowhere, is listed and left
unpinned. The migration does not guess. It does not touch node prose: `prose-commit-unpinned`
lists those sentences, and rewriting them is an author's edit, not a mechanical one.

## What this does not touch

- **`artifacts:` and SHA-256 digests.** Bytes a corpus fetched keep RFC-0023's record. A pin is
  for bytes the corpus reads through git and never copies.
- **`#L` line citations.** They stay what they are: a range in a file in the working tree. A pin
  dates a read. It does not locate a passage in it.
- **`used-by:` on a catalog entry.** Deriving it from citations is #1498, a sibling under #1513.
  The `used_by` strings inside city-engine's own TOML are the engine's data, not the corpus's,
  so this RFC checks a symbol only where a catalog location names it.
- **Computed figures.** A value computed from a pinned read and pasted into prose is #1503.
- **Reading a dependency's code at all.** `tonpa` materializes only a dependency's
  `.yidam/corpus`. A pin into a dependency is resolved through `git cat-file` against the
  dependency's checkout, and nothing new is copied into `.yidam/tonpa/`.

## Migration & compatibility

- **Older binaries** read a pinned entry and ignore the three fields, because `CatalogLocation`
  does not deny unknown fields. A corpus can write pins before its pin moves to a binary that
  reads them, and lose nothing but the checks.
- **No existing entry changes meaning.** A `file` location without `commit` is resolved,
  fetched and aged exactly as today.
- **`migrate pins`** is opt-in, and its dry run is the measurement: run against city-engine, it
  should pin at most the 108 dated entries.
- **Template.** `guidelines/directories.md` documents the three fields beside the location
  kinds, and says that a code read is dated by `commit`, not by a sentence.

## Alternatives considered

**A new `code` location kind.** This would be cleaner to read, but every binary up to this one
would refuse the entry outright, and 244 of city-engine's entries would need their kind rewritten
as well as pinned. RFC-0048 closed the kind set to stop it growing per source. Here the source is
still a file; only the way it is dated is new.

**Store the digest (`sha256:` or `oid:`) beside `commit`.** That would let a clone without git
history check the pin. But the object id is a function of `(repo, commit, value)`, so a stored
copy can only agree or be wrong. A shallow clone gets `catalog-pin-unchecked`, which is what it
has earned.

**SHA-256 of the file's bytes, as tonpa's `tree_digest` does.** This would be uniform with
`artifacts:`, but it means reading every pinned file at two commits on every `due`. Git has
already hashed them. A tree object also gives a directory read for free.

**A pin on each edge (`source:` plus `at:`).** Rejected in §5. The superseded clock already dates
a node against the entry's history, and a per-edge pin would put 2,144 copies of the entry's
commit into nodes.

**A registry class.** The issue suggested a class that names a keyed registry as its object.
The coverage clock already holds the class, the property and the universe. What it lacked was a
TOML reader and a second direction, and §6 adds only those.

## Open questions

1. **Symbol-scoped staleness.** A pin on `crates/game/src/lib.rs` goes stale on any change to the
   file, including one far from the symbol read. Narrowing staleness to a symbol needs a language
   parser. Is a file-grained clock good enough, given that city-engine re-reads at file grain
   already?
2. **The reference revision for this repository.** `HEAD` makes a feature branch's code changes
   show as due on that branch. Is that right (the branch changed the code), or should it be the
   default branch?
3. **`prose-commit-unpinned` in non-corpus prose.** `docs/` in a derived repository cites
   commits too. Scan it or not? The reference data is all in `.yidam/`.
4. **Abbreviated commits.** Should `commit:` require the full 40 digits once written by
   `migrate pins`, so that an abbreviation can never become ambiguous as history grows?
5. **A deliberate orphan.** `orphans` needs a way to leave out an instance that stays in the
   corpus after its key was removed on purpose, like city-engine's "(retired)" engine choices.
   One option is an `except` filter on a property value. Another is a standing that yidam does
   not have yet. Which one?
