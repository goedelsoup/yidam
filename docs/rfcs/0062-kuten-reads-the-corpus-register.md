# RFC-0062 — `kuten check` reads the corpus register, and no second profile ships

- **Status:** Draft
- **Track:** I45
- **Relates to:**
  - #1507 (asks for an engineering profile; the measurement here answers no), parent epic
    #1513
  - #1464 (`kuten check` reads an object-coupled corpus's artifact commits as its practice;
    this RFC closes it)
  - RFC-0028 §4 "As built" (the A3 decision that `kuten check` is not scoped by the register,
    which this RFC supersedes)
  - #1468 (phase types that are all one type, which stays there)
- **Versioning layers touched:**
  - **Kuten.** `kuten check` and `yidam practice` read the vocabulary and phases bands over
    corpus-register commits whenever `[object] paths` is declared. No band and no profile
    changes, and `inquiry` stays at revision 2.
  - **Registers.** A path under `.yidam/` is always in the corpus register. That holds for
    `lint --commits`, `kuten check`, `practice` and coupling alike.
  - **Reports.** The `kuten check` measurement gains four object-only counts, and its
    `commits`, `phase_commits`, `off_vocabulary_commits` and `suffixed_commits` change meaning:
    they count the scoped population, where today they count every authored commit (§2.4).
    The new counts default to 0, so a fixture or recorded row without them still parses.
  - **Regenerated files.** `yidam practice` writes the `PRACTICE.md` REGEN block, and scoping
    moves its shares. city-engine keeps one, so its next `regen --check` after the upgrade
    reports the block stale until `yidam regen` runs.
  - **The prelude.** The kuten `README.md` gains one paragraph on the negative measurement.
  - **The bump.** It is minor for the CLI. No authored file in any corpus moves and no config
    changes, so no migration is owed. An upgrade note is owed for the regenerated block
    above.
- **Downstream reference cases:** measured read-only on 2026-11-24 at the checked-out head of
  every derived repository on the measuring machine. Three of them declare `[object] paths`:
  city-engine, allen-county-ohio and matt-huffman.

## Summary

#1507 asks for an engineering kuten profile. Two corpora chose `inquiry` because it was the
only profile, and both diverge from its bands. The measurement finds that the divergence
comes from the reading, not from the practice. `kuten check` counts every authored commit,
including commits that touch only the declared object. `lint --commits` already leaves those
out. Read over the corpus register, city-engine falls inside both `inquiry` bands that it
fails today.

The measurement finds no second practice to write down either. The share of build verbs
runs from 0.03 to 0.26 across the corpora, with no gap to draw a band in. About 95% of phases
are Investigation in every corpus that types them, engineering corpora included. A profile
whose bands cannot be measured cannot be written down, and every band in this layer is
measured first.

So this RFC changes the reading and ships no profile. `kuten check` and `yidam practice`
judge both bands over corpus-register commits whenever `[object] paths` is declared. They
report the unscoped numbers and the object-only counts beside the verdict, so the
declaration's effect is always visible. A path under `.yidam/` is always corpus, so no
declaration can move the corpus out of the corpus register. #1507 closes on the finding, and
phase-type degeneracy stays with #1468.

## Problem

### 1.1 What was measured

The commit reading replays `kuten::commit_measurement` in a script: the verb is the text
before `: `, git-generated merge subjects are left out as `is_merge` leaves them out, and
the glob matcher and the object-only rule are the ones
`Registers` uses. Each corpus is read twice: over every authored commit, and over the commits
that are not object-only.

| Corpus | Reading | Commits | Phase share | Off-vocabulary share |
|---|---|---|---|---|
| city-engine | every authored commit | 5,048 | 0.098 | 0.286 |
| city-engine | corpus register (1,432 object-only left out) | 3,616 | **0.136** | **0.004** |
| allen-county-ohio | corpus register | 1,422 | 0.138 | 0.006 |
| matt-huffman | corpus register | 659 | 0.005 | 0.173 |
| ohio-education-funding (declares no object) | every authored commit | 559 | 0.174 | 0.377 |

The `inquiry` bands are 0.12–0.27 for phase share and 0.00–0.02 for off-vocabulary share.
city-engine fails both today and passes both over the corpus register. allen-county-ohio
passes both over the corpus register. #1464 reached the same split on city-engine at an
earlier head.

matt-huffman stays divergent on both bands after scoping. Of its 192 off-vocabulary commits,
106 list no paths, and 104 of those are hand-written `merge …` subjects on merge commits.
`git log --name-only` lists no paths for a merge, so they stay in the corpus register under
`Touch::None` (§2.1), not because they touch the corpus. Its phase share is a practice, not a
reading. Scoping is not a pass: it removes only what the corpus says is not corpus.

### 1.2 What an engineering profile would have to measure

A profile names a practice that a cluster of corpora share and others do not. Two
candidates were measured.

- **Build verbs.** The share of `implement`, `build`, `fix` and `scaffold` commits runs 0.09
  in city-engine, 0.17 in allen-county-ohio and 0.26 in ohio-education-funding, which is a
  research corpus. Across all corpora it runs from 0.03 to 0.26 with no gap. No band separates
  the engineering corpora from the rest.
- **Phase types.** About 95% of phases are Investigation everywhere: 632 of 636 in
  traffic-design, 406 of 428 in city-engine, 132 of 141 in ohio-legislation-tracker and 89 of
  92 in land-use-activity. Engineering corpora type their phases the same way research corpora
  do, so a profile that lists different types would describe a practice nobody has. That one
  type covers almost every phase is #1468's question.

Once the reading is fixed, the two corpora that prompted #1507 do not diverge in a way that a
second profile would explain. The issue's premise was the reading.

### 1.3 Why A3 no longer holds

RFC-0028 §4 decided that `kuten check` is not scoped by the register. It gave two reasons.
First, zero commits changed register in any measured corpus, so scoping bought nothing.
Second, scoping would make a band-checked quantity settable from a corpus's own config.

The first reason is now false. Three corpora declare `[object] paths`, and in city-engine
1,432 of 5,048 authored commits are object-only.

The second reason is real, and A3 measured it wrongly. §2.3 measures the lever again and
bounds it from both sides.

## Design

### 2.1 The population

`kuten check` and `yidam practice` read their commit counts over the commits that are not
`Touch::ObjectOnly` under `Registers::of_repo`. That is the population `lint --commits`
already reports on.

- A corpus that declares no object reads exactly what it reads today, because every commit is
  corpus.
- A commit with no paths, such as a merge, stays in the corpus register under `Touch::None`,
  as `lint --commits` treats it.
- A commit touching both registers stays in the corpus register.
- Both shares are taken over the scoped population, numerator and denominator alike.

`practice` keeps its own population, the authored non-`regen:` commits, and scopes it the
same way. One range then gets one verdict whichever surface reads it. The issue also notes
that `practice` and `kuten check` differ on `regen:`. That difference is by design, because
`practice` reads a stretch of work and a regeneration is not work, and this RFC leaves it.

### 2.2 `.yidam/` is always corpus

`Registers::register_of` answers `Corpus` for any path under `.yidam/`, even when an object
glob covers it. Without this rule, `paths = ["**"]` would make every commit object-only and
empty the reading.

No corpus measured declares a glob that covers `.yidam/`, so the rule changes no number in §1.
It is a rule of `Registers`, not of kuten, so `lint --commits` and coupling read it the same
way. A config that declares a glob over `.yidam/` keeps that glob for every other path it
covers.

### 2.3 The lever, measured again

The widest declaration is not the extreme. Declaring more paths removes on-vocabulary and
non-phase commits as well, so a declaration chosen to remove only the commits that hurt a
share moves it further than declaring everything does. The bound therefore takes the best
case for the corpus. Only a commit whose every path lies outside `.yidam/` can become
object-only (§2.2). The floor removes every such off-vocabulary commit and keeps every other
commit. The ceiling removes every such non-phase commit and keeps every phase commit. No glob
set can do better than either, and a real one may not reach them, because a file that an
off-vocabulary commit touches is often touched by an on-vocabulary one too.

| Corpus | Off-vocabulary: declared, widest, floor | Phase share: declared, widest, ceiling |
|---|---|---|
| city-engine | 0.004, 0.000, 0.000 | 0.136, 0.166, 0.166 |
| allen-county-ohio | 0.006, 0.006, 0.005 | 0.138, 0.149, 0.149 |
| matt-huffman | 0.173, 0.186, **0.137** | 0.005, 0.005, 0.005 |
| ohio-education-funding (no object declared) | 0.377, 0.384, **0.285** | 0.174, 0.238, **0.260** |
| traffic-design (no object declared) | 0.000, 0.000, 0.000 | 0.151, 0.166, 0.166 |

Off-vocabulary share can move, by up to 0.092 in ohio-education-funding, and only a
declaration aimed at it moves it down. An off-vocabulary commit that touches `.yidam/` or
lists no paths cannot be declared away. A3's counterfactual, 0.4930 to 0.3248 for
ohio-education-funding, removed object-only commits from the numerator alone. §2.1 removes
them from both, which is why the widest declaration raises the share and only a targeted one
lowers it.

Phase share moves by up to 0.086 in ohio-education-funding. In four of the five corpora every
phase commit touches `.yidam/`, so the ceiling is the widest declaration. In
ohio-education-funding 11 of its 97 phase commits do not touch `.yidam/`, so a declaration
that spares them reaches 0.260 where the widest reaches 0.238.

No measured corpus can declare its way into a band that its held declaration fails.
matt-huffman's floor, 0.137, and ohio-education-funding's, 0.285, both stay above the 0.02
off-vocabulary ceiling. matt-huffman's phase share cannot reach 0.12.
ohio-education-funding's phase share is already inside its band, and its ceiling stays inside
it. city-engine and allen-county-ohio pass both bands over the declarations they already hold.

That is a lever all the same, and three things bound it:

- The declaration is a committed file. In city-engine it is backed by a decision record.
  `lint --commits` already trusts it.
- The verdict prints the unscoped share and the object-only count beside the scoped share, so
  a reader sees how far the declaration moved the reading.
- The floor and ceiling above are the most any declaration can do, and none of them crosses
  a band.

A conformance that a corpus can move without saying so is not a measurement. One that it can
move only in a committed file, with the effect printed beside the verdict, is a reading of
what that file says.

### 2.4 The report

The measurement gains four counts:

- `object_only_commits`, the object-only commits left out;
- `object_only_phase_commits`, the phase commits among them;
- `object_only_off_vocabulary_commits`, the off-vocabulary commits among them;
- `object_only_suffixed_commits`, the suffixed commits among those, so `suffixed_commits`
  stays a subset of `off_vocabulary_commits` on both readings.

**Four existing fields change meaning.** `commits`, `phase_commits`, `off_vocabulary_commits`
and `suffixed_commits` count the scoped population where they count every authored commit
today, and the `Measurement` doc comment that calls the off-vocabulary count "deliberately
not register-scoped" is rewritten. In a corpus that declares no object the two readings are
equal. In one that does, a consumer of `kuten check --format json` that reads `commits` as
every authored commit reads the scoped count, 3,616 rather than 5,048 in city-engine. Each
unscoped figure is the scoped field plus its object-only count. The field names stay, because
the bands read these fields and the bands now read the scoped figure.

Each new count has `#[serde(default)]`, so a `Measurement` deserialized without them, such as
a `tests/fixtures/kuten/*.toml` control, parses with 0. The `measured.members` rows in
`yidam/prelude/kuten/inquiry/kuten.yml` are read through the `Member` mapping in
`tests/suite/kuten_cluster.rs`, which builds a `Measurement` field by field and sets the new
counts to 0. All six members declare no object, so no row changes. The report schema and the
`kuten-check.json` golden gain the four fields, and a golden must reach each one non-null.

The text verdict adds one line when any commit was left out:

```
register: 1,432 object-only commits left out ([object] paths); unscoped: phase 0.098, off-vocabulary 0.286
```

With no object declared, the report reads exactly as it does today.

### 2.5 What `inquiry` and the README say

The bands do not change. The kuten `README.md` gains one paragraph: an engineering profile was
measured for in 2026-11, the build-verb share and the phase-type mix gave no band, and the
divergence that prompted it came from reading object-only commits. RFC-0028 §4's A3 bullet is
annotated as superseded by this RFC when it is implemented.

## What this does not do

- **It ships no engineering profile.** §1.2 is the reason. A later profile needs a measured
  cluster first.
- **It does not make phases optional or derived.** That would be a different profile.
- **It does not change phase types.** One type covering 95% of phases is #1468.
- **It does not scope `score`.** `score` keeps its own measure, which no band reads.
- **It does not change bootstrap.** Bootstrap already offers a choice when more than one
  profile exists. With one profile there is nothing to choose.

## Alternatives considered

- **Ship an engineering profile with wider bands.** Its bands would be the union of what the
  corpora do, so it would pass everything and measure nothing.
- **Keep A3 and document the divergence.** city-engine would show two questions for a person
  on every run, both answered in a decision record. That is the cost #1464 reports.
- **Scope `kuten check` and leave `practice` unscoped.** One range would get two verdicts on
  the same band, which is the second inconsistency #1464 names.
- **Judge the band on both readings and diverge if either fails.** An object-coupled corpus
  would keep failing on the unscoped reading, and nothing would change for it.

## Migration

None. A corpus with no `[object] paths` reads exactly as before. A corpus that declares them
sees its reading move to what `lint --commits` already reports.

## Falsifier

This RFC is wrong if an object-coupled corpus is found whose object-only commits carry its
practice, so that leaving them out hides a divergence a person should see. The check is the
object-only counts in §2.4: a corpus whose object-only commits include most of its phases
would show it. None of the three measured does. Their 1,886 object-only commits include no
phase commit at all.

## Unresolved

- Whether `score` should read the same population. It is not band-checked, so this RFC leaves
  it alone.
