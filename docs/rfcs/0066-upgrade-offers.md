# RFC-0066 — An upgrade's offers take a written answer, and a repository's own argument survives the tools

- **Status:** Draft
- **Track:** I49
- **Relates to:**
  - #1512 (upgrades are cheap and their features go unadopted), parent epic #1513
  - #1057 (repo-owned schemas, whose upgrade note this corrects)
  - #1054 (scaffold regions), #1135 (`migrate routes`) and #1444 (the loop's hooks), which
    are three of the offers §1 names
  - RFC-0065 §1 (`.yidam.toml`'s `release`, which §3 reads to date an offer)
  - `[due.declined]` (`config.rs`, `cmd/due.rs`), whose rule §2 copies: a decline names a
    record, and one naming nothing is not honoured
- **Versioning layers touched:**
  - **Config.** `.yidam/config.toml` gains `[doctor.declined]` (§2) and `[schema] owner`
    (§4). Both are absent by default.
  - **CLI.**
    - `yidam doctor` gains a fifth verdict, `declined`. Each warning that is an offer names
      the release that first made it (§1–§3).
    - `yidam schema` refuses `--force` and writes nothing when the repository declares
      that it owns `.yidam/schemas/`. Under that declaration, `--settings` maps only the
      schema files that exist (§4).
    - `yidam lint --bless` keeps the baseline's argument block and each entry's `note`.
      The baseline gains `expire_record`, and the expiry message stops suggesting a bare
      raise (§5).
  - **Reports.**
    - `doctor --format json` carries the new verdict, a `declined` count, and each offer
      with its id and `since` release.
    - The report schema's verdict enum gains `declined`.
  - **Editor.** The VS Code extension reads `doctor --format json`. Its `DoctorReport` type
    (`reports.ts`) gains `'declined'`. The Health view (`tree/model.ts`) counts a check as
    actionable when its verdict is anything other than `ok` or `skipped`, so it would show a
    declined offer as a warning. `declined` joins `ok` and `skipped` there (§2).
  - **Prelude.** None. No file that `yidam-vendor-update` copies changes shape.
  - **Docs.** The #1057 upgrade note in `docs/upgrading.md` (`cli/v0.17.0`) is corrected
    (§4). `docs/troubleshooting.md` and `docs/configuration.md` gain the new keys.
  - **The bump.** It is minor for the CLI. No authored file moves. An upgrade note is owed
    when this is implemented, because a baseline that sets `expire_after` with no record
    starts warning (§5, Migration).
- **Downstream reference cases:**
  - ohio-education-funding and matt-huffman. Their local checkouts were read with `git show`
    and `cat` on 2026-11-24. Nothing was fetched or written.
  - allen-county-ohio-democratic-party. Its evidence is quoted from #1512, since no local
    checkout was read.

## Summary

An upgrade now costs a repository very little, and what the upgrade offers is mostly left
undecided. `doctor` raises the same offers on every bump, and a repository has no way to say
it already decided. Two tools also overwrite the part of a file that holds the repository's
own argument.

This RFC makes four changes:

- `doctor` separates an **offer** (a feature a release made available) from a **defect**.
  Each offer names the release that made it (§1, §3).
- `[doctor.declined]` maps an offer to a decision record, and the offer then reads
  `declined`. A decline whose record is gone warns again (§2).
- `[schema] owner = "repository"` makes `yidam schema` leave `.yidam/schemas/` alone, even
  under `--force`. The #1057 note that sends a repository to `--force` is corrected (§4).
- `lint --bless` keeps the comments a person wrote in the baseline. `expire_after` is argued
  by a named decision record, and the expiry message says so (§5).

## Problem

### 0.1 What was measured

The 2026-11-18 cross-corpus review counted re-pins and what each one adopted.

- **Re-pins are cheap.** matt-huffman re-pinned 9 times and ohio-education-funding 15 times.
  Each touched 1–5 files outside `.vendor`.
- **The same offers are re-raised.** ohio-education-funding's 0.24.0 re-vendor (`7c1854c7`)
  lists four items as "Not adopted". They are the Claude Code hooks, `[serve] act`,
  `migrate routes` and the CI scaffold markers. The commit calls them "the same `doctor`
  warnings as before, plus `routes`". matt-huffman's re-vendor to 0.18.0 records the same
  scaffold warning, and says that `migrate scaffold` "rewrites files this repository owns".
- **Offers are decided under the pressure of the warning.** After the 0.26 re-vendor,
  allen-county-ohio-democratic-party spent a day settling doctor warnings (`781b78b`). One
  outcome was adopting `edge_policy` although "lint reported no unlicensed-edge finding
  anywhere". `contract` had asked for it.
- **A repository's own schema has no seat.** ohio-education-funding's `mise.guards.toml`
  makes the inherited `schema` task refuse. The generic schema drops `required` from five
  fields to three, flips `additionalProperties` to `true`, and adds 21 files.
- **A baseline's argument is lost on every bless.** matt-huffman's `lint-baseline.yml`
  carries 49 comment lines after the generated header. They hold "THIS REPOSITORY'S
  ARGUMENT", the case for each entry it keeps. `--bless` writes the header and the body back
  and drops the block, so it is restored by hand after each bless. When its 40-commit expiry
  ran out, the fmh worktree raised it to 80 because the debt "ran out with neither remedy
  performed".

### 0.2 What already exists, and why it is not enough

- **`doctor` has two levels of wrong, and a declined offer is neither.** `warn` is
  "actionable, and a normal state to be in". A repository that examined `migrate scaffold`
  and decided against it is not in an actionable state. Under `--strict` it fails anyway,
  and without `--strict` its warning sits beside real ones.
- **`[due.declined]` already solves this for clocks.** A clock id maps to a decision record.
  The clock reads `Declined`. A decline naming no record is not honoured, and `due` says so.
  `doctor` has no equivalent.
- **#1057 already refuses to overwrite an unmarked schema file.** The refusal is the right
  behaviour, but nothing records why a file is unmarked. The `cli/v0.17.0` upgrade note says
  that files an earlier release wrote are refused on the first run, and tells the reader to
  "replace them once" with `yidam schema --force`. The note's first paragraph does say that a
  repository compiling its own schemas keeps them. The "replace them once" paragraph that
  follows does not, and that paragraph is where a reader looks once the refusal fires. For a
  repository that compiles its own schemas, its command replaces the stricter files with the
  generic ones. The refusal text names both cases at the point of the command, and this
  paragraph does not.
- **The baseline already keeps one human-written value.** `expire_after` survives a bless
  (`baseline.rs`, `from_checks`) because it is "the one thing in this generated file a human
  wrote". The comments that argue for it are the second such thing, and they are dropped.
- **An expired entry already gates.** The ratchet fails on it. The message then offers two
  ways out: fix the finding, or "raise `expire_after` in the baseline and say in the commit
  message why". The second is the cheaper one, and the argument lands in a commit message
  that nothing reads again.

## Design

### 1. An offer is not a defect

Some `doctor` warnings mean that the repository has not adopted something a release made
available. They are **offers**. Every other warning is a **defect**: the setup is wrong in a
way a person would want fixed whatever they decided.

Each offer has a stable id and the release that first made it. The roster below covers every
current check whose warning is an offer.

| Offer id | Check | Warns when | Since |
|---|---|---|---|
| `routes` | `routes` | `AGENTS.md` has no `yidam routes` block | `cli/v0.16.0` |
| `scaffold-regions` | `scaffold` | `ci.yml` or `CLAUDE.md` has no YIDAM region | `cli/v0.17.0` |
| `scaffold-hooks` | `scaffold` | `.claude/settings.json` does not run `yidam hook` | `cli/v0.24.0` |
| `ci` | `ci` | there is no `.github/workflows/`, or none of its workflows runs a corpus gate | `cli/v0.17.0` |
| `contract-required` | `contract` | no property says `required:` | `cli/v0.17.0` |
| `contract-edge-policy` | `contract` | no class with edges says `edge_policy:` | `cli/v0.17.0` |

`check_ci` has two warn arms, and both belong to the `ci` offer. A repository with no
`.github/workflows/` usually runs its CI somewhere `doctor` cannot read. Declining `ci`
against a record that says where is the answer to both arms.

The offer ids are finer than the check ids. One check can carry two offers, and a release can
add a third. If a decline named the check, it would also silence an offer that did not exist
when the record was written. Each new offer is a new id, so no decline covers it in advance.

Every other `warn` stays a defect. That covers `binary`, `commit`, `path`, `prelude`,
`index`, `regen`, `kuten`, `vault`, `corpora` and the rest. None of them can be declined (§2).

The roster lives beside `ROSTER` in `doctor.rs` as a typed list. A gate asserts two things:

- every offer id names a check in `ROSTER`;
- every `since` parses as a `cli/v*` release.

A test also asserts that each check's warn arms map to the offers it lists.

**A check with two offers judges each one on its own.** Today `check_scaffold` returns as
soon as it finds an unmarked region. When the regions are missing, it never reports the
missing hooks. A decline built on that early return would let `scaffold-regions` hide
`scaffold-hooks`. So a check that carries offers first collects every open offer, then
applies the declines, then forms the verdict from what is left. `check_scaffold` and
`contract_answer` are restructured that way. A test covers a repository with both regions
and hooks missing and `scaffold-regions` declined. The check warns and names the hooks.

### 2. `[doctor.declined]`

```toml
[doctor.declined]
scaffold-hooks = "no-claude-hooks"     # a decision record, by its id or file stem
contract-edge-policy = "edges-are-descriptive"
```

The rule is `[due.declined]`'s, with three arms:

- **The offer is open and its record resolves.** The offer reads declined, and the check
  reads `declined` when every open offer on it is declined. The detail names the record. A
  check with one declined offer and one undecided offer still warns, naming only the
  undecided one.
- **The offer is open and its record does not resolve.** The decline is not honoured. The
  check warns as it would without it, and the detail says
  "declined against `<record>`, which this repository does not hold". The remedy is to record
  the decision or drop the key.
- **The offer was adopted.** The check reads `ok`, and the detail notes that
  `[doctor.declined]` contradicts the adoption. That way a stale key is read, not just left
  in place.

A key that names no offer is reported. That covers a typo and also an attempt to decline a
defect such as `binary`. It is listed under the summary line, and `--strict` fails on it. A
typo in a decline otherwise does nothing, and the repository would never learn it.

**Under `--only`, a key is judged only when it could name an offer on a selected check.** A
key that names a known offer on a check that was not selected is left alone. An unknown key
is reported only on a full run, because no check owns it. `yidam-vendor-update` runs
`doctor --only contract --strict` and prints the contract output when strict fails. Without
this rule, a typo such as `scaffold-hoks` would fail that run on every re-vendor and print a
contract block that reads `ok`.

`declined` is a fifth `Verdict`, serialized as `"declined"`. It is counted apart from `warn`.
It does not change the exit code, with or without `--strict`. `DoctorReport` gains a
`declined` count, and the summary line reads `… 2 warning(s), 3 declined.`

The VS Code extension's Health view treats `declined` the way it treats `ok` and `skipped`.
It does not list a declined check as actionable. Otherwise the editor would ask the question
again after the CLI stopped asking it.

`yidam-vendor-update` runs `doctor --only contract` (#1078). The run reports `declined` for a
contract the repository has argued against, so the same question stops being asked on every
re-vendor.

### 3. An undecided offer names its age

An offer that is neither adopted nor declined says when it was offered. The detail gains:

```
offered since cli/v0.17.0; this pin is cli/v0.26.0
```

The pin comes from `.yidam.toml`'s `release` (RFC-0065 §1). A pin that records no release
prints the `since` alone. `doctor` stays offline. It does not count the releases in between,
because that count needs the origin, and `due`'s `template` clock already makes that one
networked read.

This is the "dated debt" the issue asks for. It sits on the line where the offer is already
raised, not on a second clock. A ninth `due` clock was considered and rejected (Alternatives).

### 4. `[schema] owner`

```toml
[schema]
owner = "repository"   # or "yidam", the default
```

When a repository declares that it owns its schemas, `yidam schema` changes in four ways:

- **It writes nothing and exits 0.** It prints that `.yidam/schemas/` is the repository's
  own, per `[schema] owner` in `.yidam/config.toml`. The inherited `schema` task then
  succeeds and changes nothing. ohio-education-funding's guard task file stops being needed,
  and keeping it does no harm.
- **`--force` refuses** and names the key. A declared owner is the one fact the #1057
  refusal could not know, so no flag overrides it. To regenerate the generic schemas, a
  repository removes the declaration.
- **`--settings` maps only the files that exist.** Today it maps every generic file name and
  every class schema to a path under `.yidam/schemas/`, whether or not the file is there
  (`editor_settings_for`). A repository that owns the directory need not write the same set.
  ohio-education-funding's set is 21 files smaller than the generic one, so a full mapping
  would point the editor at paths that do not exist. Under `owner = "repository"`, a path is
  mapped only when its file exists. A schema the repository writes under a name yidam does
  not know is not mapped, and the output says so. With the default owner, nothing changes.
- **An unknown value fails** the run and names the two values it accepts.

With the default value, `yidam`, nothing changes. The #1057 marker refusal still protects an
unmarked file.

**The #1057 note is corrected.** The `cli/v0.17.0` section of `docs/upgrading.md` keeps its
first paragraph and its instruction for files an earlier release wrote. Only the "replace
them once" paragraph changes. A sentence is added ahead of its command, covering two points:

- if this repository writes its own schemas, the refusal is correct, and the fix is to
  declare `[schema] owner = "repository"`;
- `--force` replaces those files with the generic ones.

The refusal text in `schema.rs` names the key in the same place.

### 5. The baseline keeps its argument, and `expire_after` is argued in a record

#### 5.1 What `--bless` keeps

`--bless` keeps two things it currently drops.

- **The argument block.** This is every comment line after the generated header and before
  the first key. The header gains a fixed last line, and the block starts after it:

  ```
  # ── this repository's argument: kept by --bless, written by people ──
  ```

  In a file written before that line existed, the header is matched against the HEADER of
  each release that shipped one. Everything after the matched prefix counts as the block.
  If no known header matches, every leading comment line is kept and the new header is
  written above them, so a bless never deletes a person's words. Kept lines that match a
  line of any known HEADER are still removed, so a lightly edited old header is not carried
  forward as argument. The bless lists each line it kept from an unmatched header, so a
  person can check the block once.
- **A per-entry `note`.** An optional field, carried across blessings the way `since` is. It
  is matched to the entry by check and node. Nothing compares it.

The header's opening paragraph is rewritten to match. Today it says "do not hand-edit,
except for `expire_after`". It will name all four things a person writes: `expire_after`,
`expire_record`, each entry's `note`, and the argument block below the header's last line.
Everything else in the file stays generated.

#### 5.2 `expire_after` names a record

The baseline gains `expire_record`, the decision record that argues the number:

```yaml
expire_after: 80
expire_record: baseline-expiry-80
```

A record that names a number has to state it. The record's frontmatter carries the same key:

```yaml
---
id: baseline-expiry-80
expire_after: 80
---
```

Frontmatter is shared by every `.md` kind, so lint reads this key as an untyped value, and a
record without it does not fail to parse.

| State | What lint does |
|---|---|
| no `expire_after` | Nothing changes: entries never expire. |
| `expire_after` with a resolving `expire_record` that states the same number | The number is honoured. The summary names the record. |
| `expire_after` with a resolving `expire_record` that states another number, or none | The ratchet fails. It names the record and both numbers. |
| `expire_after` with no `expire_record` | The number is honoured. One line under the ratchet result says no record argues the expiry. This line is a **warning for one minor release** and a ratchet failure after that (Migration). |
| `expire_record` naming a record that is not held | The ratchet fails and names the record. This is the rule for a stale entry: a baseline that disagrees with the repository fails. |

The record carries the number, so a raise cannot be made in the baseline alone. If only
`expire_after` changes, the baseline and its record disagree and the ratchet fails. The raise
has to change the record too, or name a new record, so the argument lands in a reviewed file
rather than a commit message.

**The expiry message changes.** It no longer offers a bare raise. The two ways out become:

- fix the finding;
- write a decision record that argues why this corpus needs longer and states the new
  `expire_after`, raise the baseline's `expire_after` to match, and name the record in
  `expire_record`.

The lines about `--bless` not restamping `since` are kept.

## What this does not do

- **Nothing is turned on by default.** The issue asks for the offers that cost nothing to be
  turned on by default. Each offer in §1 writes or asks for a file the repository owns:
  - `AGENTS.md`;
  - `ci.yml`;
  - `CLAUDE.md`;
  - `.claude/settings.json`;
  - every `.ont.yml`.

  ohio-education-funding declined several of them as recorded choices. None of these is free,
  so none is applied unasked. The dated line in §3 is the lever instead.
- **`[serve] act` is not an offer here.** `doctor` has no check for it, and a new check is
  out of scope. The roster in §1 is where one would be added.
- **No change to lint's warn-level noise.** allen-county's 769 `missing-property` warnings,
  "none was owed", are a lint question. `contract-required` (§1) is the doctor half, and
  declining it is how a repository answers.
- **No declining defects.** A wrong binary on PATH is wrong whatever anyone decided.
- **No network in `doctor`.** §3 reads the pin's `release` and the compiled-in `since`.

## Alternatives considered

- **Decline by check id.** This matches `[due.declined]` exactly. It was rejected because
  `scaffold` already carries two offers from two releases. A decline of `scaffold` written at
  0.17 would silence the 0.24 hooks offer, which nobody examined.
- **A ninth `due` clock for undecided offers.** This gives a dated count with an interval.
  It was rejected for two reasons. The offers already have a line in `doctor`, and a second
  report would say the same thing in another place. A clock would also need an interval for
  something that has no natural cadence.
- **A mute list with no record.** An `ignore = [...]` can never go stale, because nothing
  stands behind it. The record is what lets a decline go red when it rots.
- **A layered schema override** (a repository fragment merged over the generic schema). This
  is more machinery than one key. It would also make yidam responsible for merging a schema
  it does not understand. Ownership is the question the evidence asks.
- **Comments preserved by a YAML round-trip.** `serde_yaml` does not keep comments, and a
  comment-preserving parser would be a new dependency for one block. Splitting on a fixed
  line is enough.
- **Gate a raise of `expire_after` against the committed baseline.** CI lints the committed
  tree, where the working file and HEAD agree, so a raise cannot be seen without a base ref.
  Requiring a record that states the same number is a static rule. A raise still has to
  touch the record, because otherwise the two numbers disagree (§5.2).

## Migration

- **`[doctor.declined]` and `[schema] owner`** are absent by default, and every check reads as
  before. The only difference is the `since` text on an undecided offer.
- **An existing baseline** loads unchanged. Its argument block is kept from the first bless
  under this release. That is the first release in which matt-huffman stops restoring it by
  hand.
- **A baseline with `expire_after` and no `expire_record`** prints one warning line for one
  minor release, then fails the ratchet. matt-huffman's 40 and the fmh worktree's 80 are both
  in this state. The upgrade note says to write the record with the same `expire_after` in
  its frontmatter, name it, and that nothing else is owed.
- **A repository with its own schemas** declares `[schema] owner = "repository"`. It may then
  delete its guard task file.

The upgrade note says four things:

1. `doctor` separates offers from defects and dates them;
2. `[doctor.declined]` exists, and its ids are the offer ids, not the check ids;
3. `[schema] owner` exists, and the #1057 note was wrong for repositories that write their
   own schemas;
4. `expire_after` now wants a record.

## Falsifier

This is wrong if any of these is observed after it ships:

- a repository that declined an offer against a record it holds, whose next re-vendor
  `doctor` still warns on that offer;
- a decline that silences an offer added in a later release;
- a decline of one offer on a check that hides another undecided offer on the same check,
  such as `scaffold-regions` declined while the hooks are missing;
- a typo in `[doctor.declined]` that fails `yidam-vendor-update`'s `--only contract` run;
- the editor's Health view listing a `declined` check as actionable;
- `doctor --strict` failing on a repository whose only non-`ok` lines are `declined`;
- a repository that declared `[schema] owner = "repository"`, whose `.yidam/schemas/` is
  changed by `yidam schema` with or without `--force`;
- `yidam schema --settings`, under `owner = "repository"`, mapping a path with no file;
- a `--bless` that changes any line of the argument block, or any entry's `note`, or that
  carries an old generated header forward as argument;
- a derived repository that raises `expire_after` without touching a decision record, one
  release after the warning period ends, and stays green.

## Unresolved

- **The length of the `expire_record` grace period.** One minor release is proposed. If the
  next cross-corpus review finds a baseline still unargued, the period was too short or the
  note too quiet.
- **Whether `kuten-read` and `governance` are offers.** Both warn about something a
  repository chose, but neither warning comes from an upgrade. They stay defects here, and
  the roster in §1 is where either would move.
