# RFC-0065 — A pin names its release, and `due` counts the releases since

- **Status:** Draft
- **Track:** I48
- **Relates to:**
  - #1511 (nothing reports template age), parent epic #1513
  - #1076 (`.yidam.toml`'s `cli` field, which this keeps and stops overloading)
  - #1308 (re-vendor to the newest `cli/v*` release, whose tag lookup this reuses)
  - #1442 (a remedy is a command or a judgement, which the new clock's remedy follows)
  - RFC-0064 §5 (a projection has no vendoring surface, so it has no pin to age)
- **Versioning layers touched:**
  - **Template.** `.yidam.toml` gains a sixth field, `release`. `template` changes what it
    holds for a pin between template tags (§2). The task layer's `yidam-vendor-update`
    writes both, and `yidam clone` and `yidam overlay` write both.
  - **Config.** `.yidam/config.toml` gains `[due] template_after`, a count of releases.
    Absent by default, like every other `[due]` interval.
  - **CLI.** `yidam due` gains an eighth clock, `template`. It is the first clock that reads
    the network (§3), and `due`'s "read-only, offline" contract becomes "read-only, and
    offline but for one `ls-remote`".
  - **Reports.** `due --format json` carries the new clock. The report schema gains its id.
  - **Prelude.** The bootstrap skill's provenance step gains a stale-template warning (§4).
  - **The bump.** It is minor for the CLI and for the template. No authored file moves and no
    migration is owed. An upgrade note is owed when this is implemented: the field is new,
    `template` reads differently, and `due` now touches the network.
- **Downstream reference case:** ohio-campaign-finance-2025-2026, whose `.yidam.toml` was read
  through the GitHub API on 2026-11-24. Nothing was written to it.

## Summary

A derived repository cannot tell how far behind its template is. `.yidam.toml` records the
pinned commit and that commit's date. Nothing compares either against what upstream has
released since, and two of the fields mislead a reader who tries.

This RFC makes three changes:

- `.yidam.toml` gains `release`, the `cli/v*` release the pin installs (§1);
- `template` names the nearest template tag at or before the pin, so a pin at a `cli/v*`
  release no longer reads `untagged` (§2);
- `yidam due` gains a `template` clock: how many `cli/v*` releases the origin has cut past
  the pin, against a declared `[due] template_after` (§3).

Bootstrap warns when the template it is about to vendor is already behind (§4). `cli` keeps
its meaning from #1076: the binary that wrote the file.

## Problem

### 0.1 What was measured

The 2026-11-18 cross-corpus review read six derived pins. Upstream stood at `cli/v0.26.0` and
template `v0.16.0`.

- **ohio-campaign-finance-2025-2026** pins `f5a359c0`, which carries `cli/v0.23.0`. Its
  `.yidam.toml`, read again for this RFC, says:

  ```toml
  commit    = "f5a359c0dbca77b5c9215d00b60108ca5bf4c4cc"
  template  = "untagged"
  committed = "2026-11-02"
  cli       = "0.23.0 (f5a359c)"
  ```

  The pin is a release, and nothing in the file says which. `git describe --match 'v[0-9]*'`
  at that commit answers `v0.13.0-6`: the template layer had moved six commits past
  `v0.13.0`, and `untagged` hides that too. Three releases have been cut since,
  `cli/v0.24.0` through `cli/v0.26.0`, 83 commits in all. The review found the same repository
  had vendored a commit already 24 days old at its genesis.
- **cellular-automata** records `cli = "0.21.0 (e2c011a)"`, `template = "v0.14.0"`, and
  installs 0.24.0. Three fields, three versions. The `cli` value is the binary that *ran* the
  re-vendor, which is the one being replaced.
- **watermark-directory** records `template = "untagged"` at a pin that is `cli/v0.23.0`.
  Its own comment calls the pin a tagged release.
- Two repositories sit at `v0.14.0`, one at `v0.15.0`, one at `v0.16.0`, and two read
  `untagged`.

### 0.2 Why each field misleads

**`template` is exact or nothing.** `Provenance::read` and the re-vendor task both ask
`git describe --exact-match --match 'v[0-9]*'`. The glob is right. VERSIONING.md says a
commit carrying several layers' tags must not answer for the template layer with another
layer's tag. But `--exact-match` turns every pin between template tags into `untagged`.
Since #1308 the re-vendor pins `cli/v*` releases, and template tags are cut less often.
So `untagged` is now the common reading of a correct pin.

**`cli` is read as the installed binary, and it is not that.** #1076 added the field to
attribute a bad copy to the binary that made it, and for `yidam clone` it does. The
re-vendor task records `yidam --version` before the new `[tools]` entry is installed. So
it names the binary the update is about to replace. Nothing records what the pin installs.

**`committed` is an age with nothing to compare it to.** A pin dated 2026-11-02 is current
if nothing was released since, and stale if three releases were. The date cannot say which.

### 0.3 What already measures this, and why it is not enough

`mise run yidam-vendor-status` already asks the origin for its newest `cli/v*` release. It
says whether the pin is behind and counts the commits between them. It answers only when
someone runs it, and the review found nobody had. `yidam due` is what a corpus reads on a
cadence. It has seven clocks and none of them covers the template.

## Design

### 1. `release` names what the pin installs

`.yidam.toml` gains a sixth field:

```toml
[yidam]
origin    = "git@github.com:goedelsoup/yidam.git"
commit    = "f5a359c0dbca77b5c9215d00b60108ca5bf4c4cc"
template  = "v0.13.0+6"
committed = "2026-11-02"
release   = "cli/v0.23.0"
cli       = "0.23.0 (f5a359c)"
```

`release` is the nearest `cli/v*` tag at or before `commit`, asked with
`git describe --tags --match 'cli/v*'`. A pin exactly at a release records the tag bare.
A pin past one, taken with `YIDAM_REF=HEAD`, records `cli/v0.23.0+5`. That is five commits
past the release, which `yidam-build` compiles rather than downloads. A tree with no
`cli/v*` tag in its history records `unreleased`.

The tag carries its `cli/` prefix. The field is about one layer, and a bare `v0.23.0` is the
spelling of a different layer's tag.

**`cli` keeps #1076's meaning.** It is the binary that wrote this file, and the comment
`render` writes says so in those words. The re-vendor task still records the binary it
resolves at write time. That binary is now named for what it is, and not mistaken for the
target. The target is `release`.

### 2. `template` is the nearest template tag

`template` drops `--exact-match`. It asks `git describe --tags --match 'v[0-9]*'` and
renders the answer as `<tag>` when exact and `<tag>+<n>` when `n` commits past. Only a tree
with no `v*` tag in its history records `untagged`.

The `+<n>` spelling is chosen over git's own `v0.13.0-6-gf5a359c0`. That form repeats the
commit already in `commit`, and `-6` reads as a semver pre-release.

`doctor`'s provenance line reads the field verbatim, as it does today, so it now prints
`pinned f5a359c0dbca (v0.13.0+6)`.

### 3. The `template` clock

**The question:** *How many releases has the origin cut past this pin?*

**The measurement.** `due` reads `origin` and `release` from `.yidam.toml`. It lists the
origin's tags once with `git ls-remote --tags <origin> 'refs/tags/cli/v*'`, by the
`newest_release` rules: peeled lines win, and only `cli/vX.Y.Z` counts. It counts the
releases whose version sorts above the pin's. By version, not by date, for the reason
`newest_release` gives.

A pin at `cli/v0.23.0+5` counts from `cli/v0.23.0`. If no release was cut since, it is zero
behind, however far past the release it sits.

**The interval** is `[due] template_after`, in releases:

```toml
[due]
template_after = 2
```

Releases, not days or commits. A release is the unit a derived repository adopts. A
fortnight with no release costs nothing, and a single release can carry a vocabulary change.

**The states**, using the enum that exists:

| Case | State | Detail |
|---|---|---|
| No `.yidam.toml` (the template itself, a projection, a hand copy) | `Undeclared` | not a derived repository, so it has no pin to age |
| `release` absent (a pin written before this change) | measured from `commit` | the tag whose peeled commit equals `commit`, if one does |
| Neither `release` nor `commit` resolves to a release | `Unmeasurable` | re-vendor to record `release` |
| Origin unreachable, or `ls-remote` fails | `Unmeasurable` | names the origin, and that it could not be read |
| No interval, origin read | `Undeclared` | `3 release(s) past cli/v0.23.0; newest cli/v0.26.0` |
| Interval declared, behind `>= template_after` | `Due` | the same, with the interval |
| Interval declared, fewer behind | `Ok` | the same |

The remedy is a command: `mise run yidam-vendor-update`. Under #1442's distinction it is
the one clock beside `index` whose discharge is mechanical. Adopting the release is a
judgement, and the update task already refuses when that judgement matters: a rollback, or
a source pack whose range the new commit cannot satisfy.

**The network, bounded.** This is the clock that breaks `due`'s offline contract, so the
breach is narrow:

- one `ls-remote`, and no clone or fetch. Counting commits behind is `vendor-status`'s job.
  It is a clone, and a schedule should not pay for one;
- run with `GIT_TERMINAL_PROMPT=0` and `ssh -o BatchMode=yes`, so CI without credentials
  fails fast instead of prompting;
- a failure is `Unmeasurable`, never an error. `due` still exits zero unless `--strict`.
  Under `--strict` an `Unmeasurable` clock gates the same way it does today.

The module comment's "no network, and none is possible from here" becomes true of seven
clocks, and says which one is the exception and why.

**Declining it.** `[due.declined] template = "<record>"` works as for every clock. A
repository pinned on purpose, a frozen fork say, writes down why.

### 4. Bootstrap warns

The bootstrap skill's provenance step runs after `yidam clone` has written `.yidam.toml`.
It gains one command:

```
mise run yidam-vendor-status
```

If the task reports a newer release, the skill tells the user which release, and that the
template it is vendoring is already behind. It names `mise run yidam-vendor-update`, to run
after the genesis commit. It does not stop. A user bootstrapping from a deliberate older
template has a reason the skill cannot see. If the origin is unreachable, the skill says it
could not check and continues.

`vendor-status`'s own report gains the `release` field beside `template` in its `pinned`
line.

## What this does not do

- **No age in days.** `committed` stays as written, and nothing reads it as a clock. A date
  cannot tell a quiet upstream from a stale pin (§0.2).
- **No commits-behind in `due`.** That needs a clone. `vendor-status` still reports it on
  demand.
- **No automatic re-vendor.** The clock says a release is owed and names the command. Taking
  it stays a person's act.
- **No refusal at bootstrap.** See §4.
- **`cli` is not redefined.** #1076's attribution of a copy to the binary that made it is
  kept (§1).
- **No other layer's age.** The bootstrap protocol, the SDKs and the editor have their own
  trains, and a derived repository does not pin them in `.yidam.toml`.

## Alternatives considered

**Days since `committed`, offline.** This keeps `due` offline, and was the first design
offered. It was rejected because days measure the wrong thing. A corpus pinned to a release
that is still the newest would go due on a calendar, and a corpus three releases behind
within a week would not.

**A `--remote` flag, offline by default.** A clock that reads nothing unless asked is the
clock the review found nobody reading. `vendor-status` is already that, on demand.

**Redefine `cli` as the target.** This matches what #1511's readers assumed the field meant.
It loses #1076's evidence: the binary that copied a tree wrongly would go unrecorded. A new
field costs one line of `.yidam.toml`.

**`template` names any layer's tag on the commit.** That makes `cli/v0.23.0` a template
version. It is the four-layers-in-one-list confusion VERSIONING.md's glob exists to prevent.

**Refuse a stale bootstrap.** A refusal at the first command a newcomer runs, over a fact
that does not make the corpus wrong, teaches the override.

## Migration

None is owed. Every existing `.yidam.toml` stays valid:

- an absent `release` is measured from `commit` (§3), and the next re-vendor writes it;
- an existing `template = "untagged"` is read verbatim until the next re-vendor rewrites it;
- `[due] template_after` is absent, so the clock reports and calls nothing due.

The upgrade note says three things. `due` now reads the origin once. `release` is the field
that names the installed version. And `template` reads `<tag>+<n>` after the next re-vendor.

## Falsifier

This is wrong if any of these is observed after it ships:

- a derived repository three or more releases behind, with `template_after` declared, whose
  `due` does not report the clock due;
- `due` in a CI job with no network or credentials that hangs, prompts or exits nonzero
  without `--strict`;
- a re-vendor to `cli/vX.Y.Z` whose `.yidam.toml` does not record `release = "cli/vX.Y.Z"`;
- a reader again taking `cli` for the installed version, when `release` is beside it. If
  this one happens, the comment `render` writes is not doing its job.

## Unresolved

- **Should `doctor` print `release`?** Its provenance line prints `template`. Adding
  `release` is one more word on a line people read, and this RFC leaves it to the
  implementation.
- **A forked origin.** A fork that cuts no `cli/v*` tags reads `Unmeasurable` on every run.
  Declining the clock is the answer this RFC gives. A fork that cuts its own tags on another
  numbering would be measured against its own, which is right.
