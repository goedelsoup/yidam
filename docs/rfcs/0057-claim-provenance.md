# RFC-0057 — A refusal is read without an argument, and a ruled non-refusal is declared once

- **Status:** Implemented
- **Track:** I40
- **Relates to:**
  - #1502 (claims, edge provenance and refusals live in prose; the evidence this answers), parent epic #1513
  - RFC-0045 (`derive check`, `refuses:` and `refusal-span-drift`, which this keeps and gives a second reader)
  - #1067 (edge `source:` resolved against the catalog, closed 2026-09-29)
  - #1382 and #1391 (a nested `claim_tag` misread, and a per-entry standing read by nothing; both closed)
- **Versioning layers touched:** CLI, prelude and all three SDKs. `lint` gains one check, `refusal-undeclared`. It is Info, and it runs only where `universal.yml` asks for it. A node gains `not_refusals:`, beside `refuses:`. `refusal-span-drift` and `derive check` read it. `refuses:` is a named field in the Rust, Python and TypeScript SDKs, so `not_refusals:` is named in all three, with a `parse_instance` parity fixture beside `refusals-are-declared-on-the-node.toml`. `yidam-core` gains one public field, so it takes a minor bump. No change to the edge keys, the claim grammar or the node-level standing. No report or MCP contract change.
- **Downstream reference cases:** measured read-only on 2026-11-22 at `origin/main` of every derived repository on the measuring machine (§1). That is 21 repositories, 20 of them with a corpus. These include ohio-campaign-finance-2025-2026 (e91e01c), matt-huffman (7fcec05) and allen-county-ohio (7193553). allen-county-ohio-democratic-party and cellular-automata are not checked out there. The issue's figures for them are cited, not re-measured.

## Summary

The issue asks for three declared shapes: a claim, an edge's provenance and a refusal. Two of
them shipped before the issue was filed, and the measurement says how each one is doing.

**Edge provenance shipped and is used.** A link has carried `claim_tag:` and `source:` since
2026-09-21. 27,612 links in 15 of 20 corpora carry `claim_tag`, all of them in the three graded
spellings. The issue reads ohio-campaign-finance's 2,319 tagged links as invented fields. They are
the recognised keys, on a pin that reads them. This RFC adds nothing to edges.

**A sentence-level claim is not a shape the corpora write.** Of 48,820 `[verified]` tags, 7.7%
cite a catalog entry in their own sentence. 78% cite one somewhere in the same field. One corpus
in twenty writes the tag-then-source shape. A span list would duplicate 72,000 tags of prose, so
this RFC adds no claim shape (§2.1).

**Refusals shipped and are read by nothing.** `refuses:` has existed since 2026-09-29, and 0 of
20 corpora declare one. Its only reader is `derive check`, and 0 of 21 repositories set
`[derive] paths`. Meanwhile matt-huffman keeps its own recogniser and a 65-row ledger of rulings
outside the graph. 28 of those rulings say a paragraph is *not* a refusal. yidam has no place to
write that down.

This RFC gives `refuses:` a reader that needs no argument (§2.2). It adds `not_refusals:`, so a
ruling in either direction is declared once, in the node it rules on (§2.3).

## Problem

### 1.1 What was measured

One script read every derived repository's `origin/HEAD` through `git show`. It never read a
working tree. It parsed each `.yidam/corpus/**/*.yml` node with PyYAML and fenced code blocks
removed. The totals are 11,263 nodes and 59,884 links.

| Measure | Count |
|---|---|
| Inline `[verified]` / `[inference]` / `[open]` | 48,820 / 17,603 / 5,651 |
| Near misses such as `[verified — …]` | 5 |
| Links carrying `claim_tag:` | 27,612, in 15 corpora |
| Links carrying `source:` | 25,936 |
| `claim_tag` values on links: `verified` / `inference` / `open` | 25,522 / 2,005 / 85 |
| Other keys on links | `note:` 664 (two corpora), `because:` 28 (one) |
| Corpora declaring a `type: claim` property | 15 of 20 |
| Nodes with a `refuses:` entry | 0 |
| Repositories setting `[derive] paths` | 0 of 21 |

The inline grammar is healthy. Five near misses in 72,074 tags is what `claim-tag-malformed`
leaves behind.

### 1.2 Edge provenance is not the gap the issue reads

The issue's figure for ohio-campaign-finance is "about 2,197" per-link fields "no check
recognises". Today the corpus carries 2,319 `claim_tag:` and 2,147 `source:` on its links. Its
template pin is f5a359c0, from 2026-11-02. That pin already carries the edge keys, which arrived
in 26bbc52b on 2026-09-21. Four checks read them: `edge-verified-unsourced`, `edge-untagged`,
`edge-standing-unheld` and `edge-source-unresolved`.

The one corpus whose edge tags predate its pin is allen-county-ohio. It tags 1,674 links and pins
v0.5.0, from 2026-09-07. Its next template update reads them. That needs no change here.

Two corpora write prose on an edge, under `note:`, and one under `because:`. Nothing reads either.
That is a separate question from provenance and is left open (§6).

### 1.3 Where a `[verified]` tag's source is

For each inline `[verified]`, the script took its sentence. That is the text from the previous
sentence boundary through the tag, plus any parenthetical or link right after it. Then it asked
where the nearest catalog link sits.

| A catalog link is in the tag's… | Tags | Share |
|---|---|---|
| sentence, including a link right after the tag | 3,769 | 7.7% |
| paragraph | 23,753 | 48.7% |
| field | 38,288 | 78.4% |
| node's `links:` | 12,268 | 25.1% |

The tag-then-source shape is one corpus's convention. ohio-campaign-finance writes
`[verified] [SoS bulk 2025](../../catalog/ohio-sos-bulk-2025.md)` on 34% of its tags. No other
corpus writes it. Three more cite a catalog entry in the tag's sentence on 20–28% of their tags.
Six corpora source at the node instead, through `links:`, on 98–100% of their tags.

A sentence is also not a claim. 13,981 tags share a sentence with an earlier tag, so one sentence
often holds two standings.

### 1.4 `refuses:` has no reader

RFC-0045 added `refuses:` as `derive check`'s input. `refusal-span-drift` holds a span to the
prose, but it only fires on a span that exists. Every other reader of a refusal is an argument
under `[derive] paths`.

No repository sets that key. Nine vendor the `[derive]` table from the scaffold, with `paths`
commented out. So in every corpus a refusal declared today changes nothing. No corpus has
declared one.

Fourteen of the sixteen readable pins carry `refuses:`. This is not a corpus behind the template.
It is a template field with no consumer.

### 1.5 The refusals are there

`derive`'s proposer, `proposes_refusal`, flags a paragraph that holds `[inference]` or `[open]`
and a negation. It runs only over paragraphs an argument cites. Run over every paragraph, it
flags 8,780 of 135,200, in 3,994 nodes. None is declared.

How many of the 8,780 are refusals is an estimate, not a measurement. RFC-0045 measured the
proposer's precision at 0.593, against corpus A's 62 ruled paragraphs. Those paragraphs are one
corpus's, picked by that corpus's own recogniser. At that precision about 5,200 would be
refusals. Precision over the other nineteen corpora's paragraphs has not been measured, and
could be lower.

### 1.6 What a corpus that needed this built

matt-huffman has no `[derive] paths` either. It built the same machinery outside the graph:

- `crates/dossier/examples/recogniser.rs`, a refusal recogniser with its own negation list;
- `data/refusal-adjudications.tsv`, 65 rulings, "authoritative in both directions";
- `data/refusal-candidates.tsv`, about 245 proposed verdicts awaiting a person;
- two ratchets in its own gate, over the recogniser's misses and false flags.

The ledger rules 33 paragraphs `refusal`, 28 `not-a-refusal` and 4 `unread`. The commits the
issue cites reword prose so that recogniser reads it. That pressure came from the corpus's own
heuristic, not yidam's. It exists because yidam gives a ruling nowhere to live.

`refuses:` could hold the 33. Nothing can hold the 28. Without them, any proposer re-flags the
same false positives on every run. That is the reason a corpus keeps its ledger in a TSV.

## Proposal

### 2.1 No claim shape, and why

The issue asks for "a sentence or span with its own tag and source". The measurement says three
things against it.

- **The unit is wrong.** A sentence often holds two standings (§1.3). `derive check` does not
  read a sentence's standing either. It grades a cited span at the weakest tag the span
  overlaps, and it refuses a span that crosses a blank line. The paragraph bounds a citation,
  and the tags inside it set the grade.
- **The corpora source at the field and the node.** 38,288 tags have a catalog link in their
  field, and 3,769 of those have it in their sentence. A sentence-level `claim-unsourced` would
  therefore fire on at least 34,500 tags that are sourced in the same field, just not
  adjacently. Tags sourced only through `links:` would add to that.
- **A span list duplicates prose.** `refuses:` is that design, and it went unadopted. A `claims:`
  list would quote 72,000 tags back out of the prose they sit in, and drift from all of them.

What yidam reads already covers the rest. Inline tags are counted, and near misses fail.
`verified-unsourced` asks the node-level question. `type: claim` declares a structured standing,
and 15 corpora use it.

The tag-then-link shape stays a style a corpus may write. yidam does not bind it, and nothing
asks the other nineteen to adopt it.

### 2.2 `refusal-undeclared` — the proposer, run by `lint`

A corpus opts in from `.yidam/corpus/universal.yml`:

```yaml
refusals:
  propose: true
```

Then `lint` runs `proposes_refusal` over every paragraph of every node. It uses the split
`derive check` already makes: each string the node holds, cut at a blank line, with `refuses:`,
`not_refusals:` and `cites:` left out. A paragraph is skipped when it holds a `refuses:` span or a
`not_refusals:` span. Each other flagged paragraph is one finding, quoting its first words.

| Check | Fires on | Gates |
|---|---|---|
| `refusal-undeclared` | a paragraph the proposer flags that no `refuses:` or `not_refusals:` span rules on | no — Info |

**Opt-in, because the default cannot be otherwise.** 8,780 findings on upgrade, perhaps four in
ten of them wrong (§1.5), is the gate arriving in a corpus that never agreed to it. `edge_claims: required:`
is opt-in for the same reason.

**Info, never more.** The proposer's precision is 0.593. Deciding which paragraphs refuse is a
person's ruling, and Article V does not delegate it. The finding names both repairs and imposes
neither.

**The same predicate.** The check calls `derive`'s `proposes_refusal` and `prose_fields`. It does
not copy them, so a widened negation list widens both readers at once.

### 2.3 `not_refusals:` — the other direction of a ruling

A node that rules a flagged paragraph declines nothing says so beside the prose:

```yaml
not_refusals:
  - span: The publisher does not label the motions; the Journal does.
```

`span` quotes the node's own prose, whitespace aside, exactly as `refuses:` does. Three readers
honour it:

- `refusal-undeclared` skips a paragraph holding the span (§2.2);
- `derive check` does not raise `derive-refusal-candidate` over it;
- `refusal-span-drift` holds it to the prose, as it holds a `refuses:` span.

**A ruling is per paragraph, so its span sits in one paragraph.** A span on either list that
crosses a blank line is an Error under `refusal-span-drift`. Today that check matches a span
against the whole field, so such a span passes, but it silences no paragraph. `refusal-undeclared`
would go on flagging its paragraph with nothing to say why. No corpus declares a refusal yet
(§1.1), so tightening the check for `refuses:` fails no repository.

**One paragraph cannot be ruled both ways.** A paragraph that holds a span from each list is an
Error under `refusal-span-drift`, whether the spans are the same text or two different sentences
of it. Comparing spans alone would let one paragraph be a refusal that `derive check` must answer
and a non-refusal that the proposer skips, with no finding.

**A separate key, not a field on `refuses:`.** A `refuses:` entry is something an argument must
answer. A ruled non-refusal is the opposite, and `derive check` reads the list by name. One list
that meant both would make every reader check a flag first. Two lists keep each reader's question
to one key.

**Neither list is prose.** Both are excluded from the paragraph split, as `refuses:` is now. A
span found only there is a declaration, not the node's words.

matt-huffman's `unread` verdict has no home here. §6 says why that is left open.

### 2.4 The prelude

`GRAPH.md`'s refusal section says `refuses:` is read without an argument when the corpus opts in.
It documents `not_refusals:` and the check. `agent-conduct.md` teaches the declaration at the
point a refusal is written, not only where an argument cites it. `GRAPH.evidence.md` carries §1.3
and §1.5's figures under the new check's id.

### 2.5 Tests

Lib tests on `refusal-undeclared`:

- a flagged paragraph is a finding, and only with `propose: true`;
- a `refuses:` span or a `not_refusals:` span silences its paragraph;
- a paragraph without `[inference]` or `[open]` is never a finding.

On `refusal-span-drift`, each of these is an Error:

- a drifted `not_refusals:` span;
- a span on either list that crosses a blank line;
- one paragraph holding a span from each list, with the same text and with two different
  sentences.

On `derive check`, a `not_refusals:` span suppresses `derive-refusal-candidate`.

A `parse_instance` parity fixture pins `not_refusals:` as a named field in all three SDKs. The new
check goes through the usual registrations, the lint golden among them.

## What this does not touch

- The edge keys and their four checks.
- The inline claim grammar, `claim-tag-malformed` and `verified-unsourced`.
- `type: claim` and every reader of a node's standing.
- `derive check`'s rules, apart from the suppression in §2.3.
- The MCP contract and the report schema.

## Migration & compatibility

Nothing changes for a corpus that does not opt in. A pin that predates this RFC ignores the
`refusals:` block in `universal.yml`.

An old pin parses a node with `not_refusals:`, but it does not read it unchanged. Its `NOT_PROSE`
holds only `refuses` and `cites`, so it reads the `not_refusals:` strings as the node's prose.
That has three effects:

- old `refusal-span-drift` passes a `refuses:` span whose text is gone from the prose but still
  quoted under `not_refusals:`;
- old `derive check` accepts a citation whose span appears only in that declaration;
- old `derive check` may propose a declared non-refusal as `derive-refusal-candidate`.

None of these fails a gate that passed before, and all three end at the corpus's next template
update. A corpus that declares `not_refusals:` should update its pin in the same commit.

A corpus with a ledger moves it into its nodes. Each `refusal` row becomes a `refuses:` entry,
and each `not-a-refusal` row becomes a `not_refusals:` entry. That is the corpus's commit, and
yidam ships no migration for it. The rows are keyed by a paragraph's opening words, not a span a
tool could quote safely.

## Alternatives considered

### 4.1 A `claims:` list of spans, each with a tag and a source

This is the issue's first proposal. §2.1 is the answer: the sentence is not the unit, and a
quoted list drifts. `refuses:` is the measured precedent for a span list nobody writes.

### 4.2 Bind the tag-then-link shape and gate an unresolved one

A catalog link after a tag would become that tag's source, checked against the catalog. It
serves one corpus in twenty. The broken-link checks already fail a link that resolves nowhere.

### 4.3 Turn `refusal-undeclared` on by default

That is 8,780 Info findings across these corpora. At corpus A's precision about 3,600 of them
would be wrong (§1.5). A corpus would learn to
ignore the check before it learned the key.

### 4.4 A ruling field on each `refuses:` entry

§2.3 says why not. `derive check` would have to read a flag before knowing whether an entry is
owed an answer.

### 4.5 Ship matt-huffman's recogniser in place of `proposes_refusal`

Its widened negation list is already in `derive`'s, which credits it as corpus A. What it adds is
the corpus's own adjudication workflow. That belongs to the corpus.

## Expected effect, and the falsifier

A corpus that opts in sees every refusal-shaped paragraph once. Each ruling, either way, silences
its paragraph for good. When every candidate is ruled, the check reports nothing, and the corpus
can run `derive check` over declared refusals.

**The falsifier is matt-huffman's ledger.** Moved into its nodes, its 61 settled rulings should
silence 61 paragraphs and leave the rest. If a ruling cannot be quoted as a span, or a quoted
span still leaves its paragraph flagged, §2.3 is the wrong shape.

## Open questions

1. **`unread`.** matt-huffman records a ruling made only by the paragraph's author. It vetoes
   until a second reader rules. yidam has no notion of who ruled, so this RFC leaves the third
   verdict in the corpus. Should a `refuses:` entry carry a `ruled_by:`?
2. **Prose on an edge.** `note:` (664 links) and `because:` (28) are read by nothing. Edge prose
   is not provenance, so it is out of scope here. It may deserve a declared key of its own.
