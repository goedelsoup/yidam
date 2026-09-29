# RFC-0045 — A refusal is declared, and a derivation is checked against it (`refuses:` and `yidam derive check`)

- **Status:** Draft
- **Track:** I33
- **Commands:** `derive`
- **Relates to:**
  - RFC-0034 (whose package-less `cites:` entry is the citation this reuses, and whose "What this does not touch" declined to invent the document class this RFC invents)
  - RFC-0017 (whose claim model, through `claims_in_node`, is what a cited span's standing is read out of)
  - RFC-0031 (whose node prose model is why a span may sit in a property and not only in `description`)
  - RFC-0032 (the precedent for promoting a key that survived in `extra` to a named field, and the reason it was done)
  - RFC-0037 (the precedent for declaring a marker rather than inferring one, and for measuring the population before choosing its shape)
- **Versioning layers touched:** template (`agent-conduct.md`'s third outbound rule gains its grammar; `GRAPH.md` documents `refuses:`) / tooling (one `derive check` command, one `yidam lint` check) / SDK (`refuses:` becomes a named field on the node in all three SDKs — a yidam-core minor bump and a parity fixture). **No MCP contract bump:** no tool reads either field in this RFC.
- **Downstream reference case:** five artifact layers in four derived repositories — 937 artifacts and 1,552 citations — and the tracked corpora of seven repositories, 4,630 nodes, measured read-only on 2026-09-29 at the commits in §2.

## Summary

`agent-conduct.md` states three rules for a claim that leaves the repository, and nothing in the
template runs them. So four derived repositories each built the gate themselves: two `dossier/`
directories, a `platform/` and a `ledger/`, and a publish crate. All four agree on the first two
rules. They share a citation model — a node, a verbatim span, the weakest tag beneath it — which
RFC-0034 already gives yidam as a `cites:` entry.

They do not agree on the third rule, and they cannot: **the third rule has no subject the tooling
can read.** A refusal is a sentence of English prose, and every repository that tried to find
one wrote a lexicon. Measured against the only human rulings that exist, the detector one
repository gates on finds 12 of 33 refusals. The widest finds 32 and flags 22 paragraphs that are
not refusals. Another finds 0. A fourth repository declined to build the rule at all.

This RFC does two things.

- **It declares the refusal.** A node lists the refusal sentences it carries under a `refuses:`
  key, each quoted verbatim from its own prose. A detector may propose one; only a declaration
  gates.
- **It runs the derivation.** `yidam derive check` reads artifacts from declared paths and
  checks their `cites:` entries against the corpus. A span must be verbatim in one paragraph.
  The artifact's tier is computed as the weakest standing beneath its spans and held to the reach
  it declares. A declared refusal in a cited paragraph must be answered.

The rule's scope — the cited paragraph, not the whole node — is chosen by the population, and
§4 is the measurement that chose it.

## Problem

### 1.1 Four gates, one rule

`directories.md` says of a directory outside `graph-check` that *"if what it holds derives from
the corpus, the gate is the derivation"*. It names no tool that runs one. Four repositories
wrote their own.

| Layer | Repository | Language | Artifacts | Citation syntax | Refusal rule |
|---|---|---|---:|---|---|
| `dossier/` | corpus A | Rust | 4 | frontmatter `rests_on: [{node, quote}]` | any refusal in a cited paragraph, detected by shape plus a file of human rulings |
| `dossier/` | corpus B | Python | 10 | YAML `rests_on: [{node, span}]` | an uncited refusal sentence between two cited spans |
| `platform/` | corpus C | Rust | 8 | `<!-- cite: path [tag] -->` then a blockquote | **not implemented, deliberately** |
| `ledger/` | corpus C | Rust | 25 | YAML `claims[].cites[]{node, tag, span}` | none |
| `crates/publish` | `goedelsoup/allen-county-ohio` | Rust | 890 | Rust constants, `support!(path, span)` | any refusal in a cited node, answered by name |

Four syntaxes, and one object underneath them: a node, a verbatim span, and the standing that
span has. Every layer computes the tier as the minimum over the spans, weakest first.

### 1.2 Two of the filing issue's premises move

#1053 was written from a review. The measurement corrects it in two places. The correction
changes how rule 3 is argued, not what is built.

| #1053 says | Measured |
|---|---|
| two repositories "re-implemented a YAML frontmatter reader in Python" | **One did.** Corpus B's is Python. Corpus C's is a restricted YAML reader in Rust |
| three repositories record that refusal detection is "a lexicon, not a parser" | **One does.** Corpus B's README and decision record say it. Corpus A argues the opposite: its detector's doc says *"A refusal is a negation next to an epistemic verb, not a phrase from a list"*, and a human ruling overrides it in both directions. `allen-county-ohio` keeps a 12-phrase list with no record. Corpus C's decision record rejects a keyword list outright, because one matched *"426 nodes on 'is not a' alone"* |

The disagreement is itself the evidence. Four teams read one sentence of the prelude and chose
four detection methods. One of them chose not to detect.

### 1.3 No lexicon is a gate

Corpus A holds the only human rulings on refusals that exist: 62 paragraphs, 33 ruled a refusal
and 29 ruled not one. Each repository's detector was reimplemented from its source and run over
those paragraphs. Corpus A's own row reproduces the gap and false-positive counts its test suite
pins (21 and 6), which is the check on the reimplementation.

| Detector | Found | Missed | False | Precision | Recall |
|---|---:|---:|---:|---:|---:|
| corpus A, negation + epistemic verb | 12 | 21 | 6 | 0.667 | 0.364 |
| corpus A, wide proposer (any negation in a tagged paragraph) | 32 | 1 | 22 | 0.593 | 0.970 |
| corpus B, `[open]` sentence or cue regex | 25 | 8 | 19 | 0.568 | 0.758 |
| corpus B, cue regex alone | 7 | 26 | 7 | 0.500 | 0.212 |
| corpus B, `[open]` sentence alone | 22 | 11 | 15 | 0.595 | 0.667 |
| `allen-county-ohio`, 12 phrases | 0 | 33 | 0 | — | 0.000 |

**No row is usable as a gate.** Every row that reaches three-quarters recall is wrong about more
than two paragraphs in five. The row that is never wrong finds nothing, because its phrases were
written against a different corpus's prose.

The sample is not a random one: it is the paragraphs someone proposed or cited, so it is biased
toward hard cases. That makes these rates a statement about the paragraphs a gate actually has to
decide, which is the population that matters.

### 1.4 The lexicons disagree with each other

Over every tracked node description in seven repositories, the three detectors do not flag the
same sentences.

| Repository | Sentences | A | B | `allen-county` | A ∩ B | Union |
|---|---:|---:|---:|---:|---:|---:|
| `goedelsoup/allen-county-ohio` | 17,163 | 808 | 1,216 | 214 | 386 | 1,653 |
| corpus C | 6,231 | 255 | 236 | 16 | 109 | 383 |
| corpus A | 3,855 | 405 | 465 | 53 | 173 | 700 |
| corpus B | 13,700 | 1,032 | 1,159 | 8 | 787 | 1,405 |
| corpus D | 8,816 | 1,076 | 1,162 | 50 | 399 | 1,841 |
| `goedelsoup/ohio-education-funding` | 1,766 | 73 | 88 | 1 | 22 | 139 |
| corpus E | 765 | 63 | 87 | 4 | 35 | 116 |

Of the sentences A or B flags, the two agree on between 16% and 30% in six of seven corpora.
Corpus B, whose detector was tuned on its own prose, is the exception, at 56%. A template that
shipped any one of these would be shipping one corpus's idea of a refusal to all of them.

`allen-county-ohio`'s own module comment names the failure that makes this matter: three of its
twenty-six refusals were invisible to its detector because a phrase straddled a hard wrap, and
*"it failed silently, because a refusal nobody detects looks identical to a node with none."*

## 2. The measurement

Read-only, through `git ls-tree` and `git cat-file` at pinned commits, so an untracked projection
counts for nothing. HEAD moved in two repositories during the run; every figure is at these
commits.

| Repository | Commit | Nodes |
|---|---|---:|
| `goedelsoup/allen-county-ohio` | `7193553` | 724 |
| corpus C | `3ca9707` | 1,590 |
| corpus A | `c7b4dc8` | 106 |
| corpus B | `afbc49f` | 1,716 |
| corpus D | `586640c` | 303 |
| `goedelsoup/ohio-education-funding` | `2016712` | 129 |
| corpus E | `43f886d` | 62 |

A paragraph is a blank-line block of one field's prose, which is the definition every one of the
four layers uses. A ruling in corpus A is keyed on the first 60 normalized characters of its
paragraph, and all 62 matched a live paragraph. Sentences are split with corpus B's splitter,
vendored unchanged, so the §1.4 columns count the same units.

The artifact inventory:

| Layer | Artifacts | Citations | Found with whitespace collapse only | In a field other than `description` | Crosses a paragraph | Artifact-to-artifact edges |
|---|---:|---:|---:|---:|---:|---:|
| corpus A `dossier/` | 4 | 15 | 11 | 0 | 0 | 0 |
| corpus B `dossier/` | 10 | 64 | 62 | 2 | 0 | 0 |
| corpus C `platform/` | 8 | 84 | 84 | **65** | 0 | 0 |
| corpus C `ledger/` | 25 | 80 | 80 | **40** | 0 | 0 |
| `allen-county-ohio` publish | 890 | 1,309 | 1,309 | 0 | 0 | 0 |
| **total** | **937** | **1,552** | **1,546** | **107** | **0** | **0** |

Every citation resolves. The six that need more than whitespace collapse are four in corpus A
that differ from the node by emphasis or link markup, and two in corpus B that carry a
blockquote `>`. The 107 outside `description` are in properties, except corpus B's two, which
quote `source_note`, a top-level key every one of its nodes carries.

## Proposal

### 3.1 A refusal is declared on the node that makes it

```yaml
# .yidam/corpus/period/depopulation.yml
class: period
label: Depopulation, 2020–2024
description: |
  Eleven of the county's thirteen civil subdivisions lost population between 2020 and 2024.
  [verified] It does not establish that the decline has ended. [open]
refuses:
  - span: >-                      # verbatim from this node's prose, compared as `cites:` compares
      It does not establish that the decline has ended.
    inference: the decline has ended    # optional; what the sentence declines to conclude
```

**A declaration, not a detection.** §1.3 is the case against detection. A refusal is a statement
about what the node's own author declines to conclude, and the author is the only party who can
make it. A declaration makes a refusal something the tooling can see, which a sentence of prose
never was: it can be counted, reviewed in a diff, and held to its text by a lint check.

**A span, not an index.** The entry quotes the sentence rather than numbering it, for the
reason `local-citation-span-drift` gives: a reference alone *"rots invisibly, because the node
keeps its name while its content is rewritten"*. A sentence number is such a reference.
`yidam lint` gains **`refusal-span-drift`** (Error). A `refuses:` span no longer in its node's
prose is a declaration about text that is gone. The population starts empty in every corpus, so
the check can ship at Error without seeding a baseline.

**A field, not a fourth tag.** `agent-conduct.md` names three claim tags and pins that there is
no fourth, and a refusal is not a standing. The sentence in the example is `[open]`: it is a
claim with a standing, and it is also a refusal. Corpus B's detector treats every `[open]`
sentence as a refusal, and 15 of the 37 it flags that way are ruled not to be (§1.3). The two
properties are independent, so they are held in different places.

**Named in the SDKs.** `refuses:` already survives every parse through `CorpusInstance.extra`.
It is promoted to a named field in the Rust, Python and TypeScript SDKs, as `references:` was, for
the reason RFC-0032 gave: a field that is named is one a check can read and a migration can
write.

### 3.2 An artifact is a file under a declared path, and cites as a node does

```toml
# .yidam/config.toml
[derive]
paths = ["dossier/**", "ledger/entries/**"]
```

An artifact is a `.yml` file, or a `.md` file with YAML frontmatter, under a `[derive] paths`
glob. The globs take `[object] paths`' grammar. The fields are:

```yaml
# dossier/decline-is-not-suburbanization.yml
claim: >-
  The suburban ring is shrinking too. Eleven of the county's thirteen civil subdivisions lost
  population between 2020 and 2024.
reach: public                     # public | attributed | internal
cites:
  - node: period/depopulation     # RFC-0034's entry: <class>/<name>, `.yml` optional
    tag: verified                 # optional; held to the standing the corpus gives the span
    span: >-
      Eleven of the county's thirteen civil subdivisions lost population between 2020 and 2024.
answers:
  - node: period/depopulation
    refusal: >-
      It does not establish that the decline has ended.
    answer: >-
      The assertion reports the loss and says nothing of its trend.
```

**`cites:` is RFC-0034's entry, unchanged.** It is one grammar for one object: a node, at a
standing, by span. The predicate that checks a local citation — resolve, find the span, read its
governing standing — is the predicate that checks this one. It is called, not copied. Three of
the four layers call the list `rests_on`. It is renamed here because a second key name for the
same entry would be a second grammar.

**Every other key is ignored.** Corpus A's `examined_not_used`, corpus C's `figure` and
benchmarks, and `allen-county-ohio`'s `topic` are theirs. An artifact is a document the
repository owns. `derive check` reads the four fields above and asserts nothing about the rest.

**`reach:`, not a declared tier.** `agent-conduct.md` says the tier is *"computed rather than
declared"*, and names where each tier may go. So an artifact declares where it is going, and the
check computes whether it may:

| `reach:` | Weakest standing admitted | `agent-conduct.md` |
|---|---|---|
| `public` | `verified` | "`[verified]` may reach public material" |
| `attributed` | `inference` | "`[inference]` reaches attributed memos and backgrounders" |
| `internal` | `open` | "`[open]` does not leave the repository" |

Each layer already has a field that says this, under its own name. Corpus A's messages carry
`publication_tier`, corpus C's planks carry `status: draft | publish`, and `allen-county-ohio`
holds its whole feed to an `Inference` ceiling. Corpus B writes the computed tier back into the
file and checks the copy. A declared tier that must equal the computed one is a cache. A declared
reach is a decision the tooling can hold the evidence to.

### 3.3 Where a span may be

**In any string the node holds, not only `description`.** Corpus C's planks cite a property in
65 of 84 citations, and its ledger in 40 of 80: `fiscal_effect`, `authority_basis`,
`counter_account`. Corpus B cites its top-level `source_note` twice. A rule that read
`description` alone would fail 107 citations that are correct. A span is found in `description`,
in any string-valued property, or in any other top-level string. RFC-0031 is the argument that a
node's prose is not one field, and this is that argument applied to a citation.

**Within one paragraph of one field.** A span that crosses a blank line has no single paragraph,
so it has no single governing standing and no single refusal scope. Two layers already enforce
this, and **0 of 1,552 citations cross one.**

**Compared after whitespace collapse, and nothing else.** This is RFC-0034's rule and
`local-citation-span-drift`'s comparison. Case, punctuation, emphasis and markup are compared as
written, because those are the changes a span exists to catch. 1,546 of 1,552 citations already
pass it. The six that do not are listed in §2, and each one is a re-quote.

**A span's standing is the weakest claim overlapping it, or `open` if none does.**
`local_citations::governing` already computes the first half. The second half is the direction
that cannot flatter an artifact: corpus A reads an untagged paragraph as `Open`, and
`allen-county-ohio` refuses one outright. The artifact's tier is the weakest over its spans.

### 3.4 A declared refusal in a cited paragraph must be answered

For each span an artifact cites, take the paragraph it sits in. Every `refuses:` entry of that
node whose span lies in the same paragraph must be named by an `answers:` entry of the artifact,
by node and by refusal text. An answer is prose, and the check does not read it. It checks that
the answer exists — the prelude's *"the author must answer it rather than route around it"* —
which puts the refusal and the reply beside each other where a reviewer reads both.

The scope is the paragraph because the prelude says *"a refusal in the cited block"*, and §4 says
the other two readings are worse.

A refusal the artifact itself quotes, inside one of its own spans, needs no answer. The
artifact is then resting on the refusal, not routing around it, and the refusal travels with it.

An answer that names no refusal declared in a cited paragraph is a finding too. That is
`allen-county-ohio`'s `StaleAnswer`: an answer outliving its refusal is a caveat about text that
is gone.

### 3.5 `yidam derive check`

A read-only gate in the "Checks and gates" group. It takes `--format text | json` like every
report, and exits nonzero on any Error.

| Finding | Severity | Fires when |
|---|---|---|
| `derive-unresolved` | Error | a `cites:` entry names no node, or one this corpus does not hold |
| `derive-span-drift` | Error | no `span:`, or the span is in no prose field of that node |
| `derive-span-crosses-paragraph` | Error | the span is found only across a blank line |
| `derive-tag-drift` | Error | a `tag:` is written, and the span's governing standing differs |
| `derive-beyond-reach` | Error | the computed tier is weaker than `reach:` admits |
| `derive-unanswered-refusal` | Error | a declared refusal in a cited paragraph has no `answers:` entry |
| `derive-stale-answer` | Error | an `answers:` entry names no declared refusal in a cited paragraph |
| `derive-no-reach` | Error | the artifact declares no `reach:`, or one outside the three |
| `derive-refusal-candidate` | Info | §3.6 |

`derive-unresolved`, `derive-span-drift` and `derive-tag-drift` are `local-citation-*`'s
predicate reached from a new caller. Their messages keep RFC-0034's wording, including *"even
ignoring line wrapping"*.

The JSON report lists every artifact with its computed tier, its reach, and each cited span's
standing. Corpus B runs a separate task to write that tier back into each artifact. It can
render the tier from the report instead, and stop storing a copy.

### 3.6 A detector proposes, and never gates

A cited paragraph that carries no `refuses:` declaration and matches a proposer is reported as
`derive-refusal-candidate` (Info). The proposer is corpus A's wide shape: any negation in a
paragraph tagged `[inference]` or `[open]`. §1.3 measures it at 0.970 recall and 0.593
precision. That precision disqualifies it as a gate. The recall is what an advisory needs,
because an advisory that misses a refusal is the silent failure `allen-county-ohio` described.

It runs only over cited paragraphs, which bounds the noise to the paragraphs an artifact rests
on. Measured over every field:

| Layer | Cited paragraphs | Proposed |
|---|---:|---:|
| corpus A `dossier/` | 15 | 1 |
| corpus B `dossier/` | 47 | 1 |
| corpus C `platform/` | 82 | 23 |
| corpus C `ledger/` | 68 | 7 |
| `goedelsoup/allen-county-ohio` | 1,269 | 209 |

Each one is resolved by a declaration, or by reading it and moving on. It never changes the exit
code. `allen-county-ohio`'s 209 is the cost of a proposer tuned on a different corpus. A
paragraph read and found not to refuse anything stays on the list, and that number is why
"Open questions" asks whether it should be silenced.

This is corpus A's architecture, and the reason it is adopted rather than invented. Its detector's
doc says *"This proposes; `data/refusal-adjudications.tsv` decides."* `refuses:` is that adjudication file, moved into
the node, where every repository can read it and a diff shows who ruled.

## 4. Rule 3's scope, decided by the population

The three readings of *"citing across a refusal"*, run over every citation in every layer with
each layer's own detector. Each cell is failing citations over failing artifacts.

| Layer | (i) an uncited refusal between two cited spans | (ii) a refusal in a cited paragraph | (iii) a refusal anywhere in a cited node |
|---|---|---|---|
| corpus A, with its rulings | 0 / 0 | 0 / 0 | 13 / 4 |
| corpus B, its detector, every field | 0 / 0 | 5 / 1 | 61 / 10 |
| corpus C `platform/`, corpus A's detector, every field | 0 / 0 | 12 / 5 | 84 / 8 |
| corpus C `ledger/`, corpus A's detector, every field | 0 / 0 | 11 / 5 | 63 / 17 |
| `allen-county-ohio`, its detector, before answers | 0 / 0 | 9 / 9 | 841 / 721 |

**Reading (i) never fires.** It needs one artifact citing two spans in one paragraph. That
happens in 13 paragraphs across all five layers, and never with a refusal between the spans. It
is corpus B's rule, and corpus B's own gate reports zero under it. A rule whose population is
empty cannot be the one the prelude meant.

**Reading (iii) fails nearly everything.** Corpus C's planks would fail 84 of 84 citations. Every
plank cites a node that refuses something somewhere, because refusing is what a careful node
does. `allen-county-ohio` built (iii) and paid for it. Without its answers, 721 of its 890
assertions would fail. It wrote 771 `answers:` entries, and **749 of them — 97% — answer a
refusal in a paragraph the assertion does not cite.** 161 distinct answer strings do that work,
copied from assertion to assertion. That is a rule producing boilerplate, and boilerplate is how
a caveat stops being read.

**Reading (ii) is the prelude's words and the one with a population.** It fires on 0 to 12
citations per layer — not on none everywhere, and not on nearly all. It keeps the case the rule was written for —
*"a node stating a fact at `[verified]` and refusing the inference from it one sentence later"*
— because one sentence later is the same paragraph.

## What this does not touch

**Artifact-to-artifact chains.** Two layers built one, and **0 edges exist in any layer**:
corpus A's messages and briefs are empty, and corpus B has written no `{assertion:}` edge. The
prelude's *"the whole supporting chain"* is honoured one level down. A span's standing is already
the weakest claim beneath it in the corpus. A chain between artifacts is left until something
writes one, for the reason RFC-0037 declined to build for Article V: n = 0.

**Figures.** `allen-county-ohio` holds 2,012 figure literals to its cited spans, and corpus C
checks 25 ledger figures against a node's stored value. Both are real checks of a different
claim — *this number is that number*. RFC-0043's `count` generator and the numeric property type
are where that belongs.

**The selection ledger.** Four of the five layers keep a list of what was examined and not used: corpus A's
`examined_not_used` (48 entries), corpus B's `not_asserted`, corpus C's `## What was examined and
not used` and `examined-and-not-used.yml` (36). Three repositories invented one list. It is a candidate
for the next RFC and not this one, because nothing in the three rules reads it.

**Artifacts that are not files.** `allen-county-ohio`'s 890 assertions are Rust constants in a
16,000-line `derived.rs`, and a command that reads files cannot read them. That repository can
emit its table as YAML under a `[derive]` path and let the command check it. Or it can keep its
crate, which runs today. Nothing forces the move.

**Normalization beyond whitespace.** Corpus A strips emphasis and reduces a link to its text
before matching, and refuses any quote containing a link. Adopting either would forgive the
changes a span exists to catch. Four of corpus A's fifteen citations would need re-quoting from
the node as written.

**An MCP tool.** A `check_derivation` tool is worth having, and a contract bump is not a local
decision.

## Migration & compatibility

**Nothing breaks.** A repository with no `[derive]` section has no artifacts, and a node with no
`refuses:` declares nothing. `derive check` reports an empty set and exits zero. Every released
SDK already carries `refuses:` through `extra`, so a corpus that writes one is readable by every
binary that exists.

**What a repository that moves pays:**

- **Re-keying citations.** `rests_on` → `cites`, and corpus A's `quote` → `span`. This is
  mechanical, one key per citation. Corpus C's ledger citations already have the entry's shape.
- **Re-quoting six spans** (§2).
- **Declaring the refusals its artifacts rest on — not the corpus's.** Declaring every refusal the
  detectors propose would mean 55 to 909 nodes per repository, and nothing requires it. Rule 3
  reads only cited paragraphs, so the declarations a repository owes are the ones in §4's column
  (ii): from 0 to 12 per layer. The rest can be declared as they are cited. `derive-refusal-candidate`
  names each one as it becomes due.
- **Retiring an answers list.** `allen-county-ohio`'s 749 answers to refusals outside a cited
  paragraph are no longer owed. The 22 inside one carry over as `answers:` entries, fewer where
the assertion's own span quotes the refusal (§3.4).

Corpus A's 62 paragraph rulings cannot become `refuses:` entries mechanically. A ruling covers a
paragraph and a declaration quotes a sentence, so each of the 33 positive rulings needs one
reading to pick its sentence.

**The SDK change is additive:** a new optional field, so a yidam-core minor bump under the
semver gate, with a parity fixture so the three SDKs read `refuses:` identically.

## Alternatives considered

**Ship a lexicon.** §1.3 and §1.4 are the case against. The best recall comes with 41% false
positives. The one detector with no false positives has no recall. And the two detectors that
find anything agree on at most 30% of what either flags, in every corpus except corpus B, whose
own detector is one of the two.

**`[open]` is the refusal marker.** It needs no new syntax, and it is exact where it is
written. Corpus B does this, and 15 of the 37 paragraphs it flags that way are ruled not to be
refusals. It also misses the motivating case, a `[verified]` fact whose inference is refused
without the refusal being `[open]`. A standing and a refusal are different properties.

**A fourth inline tag, `[refuses]`.** It is readable in place, but `agent-conduct.md` names three
tags and pins the count. The tag parser would also need a narration rule to tell a mark from a
mention, which corpus A had to build with its own override file. A structured key has neither
problem.

**Keep `rests_on:` as the key.** Three of four layers use it. But a second name for RFC-0034's
entry would make two grammars for one object. The migration cost is a key rename.

**Reading (iii), refusal anywhere in the node.** This is `allen-county-ohio`'s rule. §4 is the
case against: 97% of the answers it produced do not answer anything the assertion rests on.

**A declared tier checked for equality.** This is corpus B's rule. It stores what the check can
compute, and it can only drift. Corpus C's check is closer to this RFC's: its declared
`rests_on` may not be stronger than the weakest tag cited, which is a reach under another name. `reach:` stores the decision the check cannot make.

## Open questions

**Is `derive` the right word?** "Derived repository" already means a fork of this template, and
the help group "Deriving and maintaining a repository" uses that sense. `agent-conduct.md` and
`directories.md` use "derivation" in this RFC's sense. #1053 asked for `derive check`. A reviewer
who finds the collision worse than the fit should say so before the command ships, because the
name is frozen once it does.

**Should a candidate be silenced?** §3.6 leaves an Info standing until a refusal is declared.
A paragraph read and ruled *not* a refusal keeps reporting. Corpus A's rulings file carries
both verdicts for this reason. A `not_refusal:` entry on the node would carry the negative
ruling, and whether the noise earns a second key is a question for the first repository that
adopts this.

**Does anyone move?** RFC-0034 recorded the honest risk: a grammar with no consumer is a surface
with no subject. The falsifier is the same shape and just as cheap. One quarter after this ships,
count the repositories with a `[derive]` section and the nodes with a `refuses:` entry. At zero,
the four local gates were the right answer and this was a fifth.
