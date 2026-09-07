# RFC-0031 — A node's prose is not one field, and a finding is not a sentence

- **Status:** Draft
- **Track:** I26
- **Relates to:**
  - RFC-0002 (which chose the YAML instance as the canonical graph, and whose bridge was never built)
  - RFC-0013 (which closed RFC-0002's five questions, is recorded `Implemented`, and specified two functions that do not exist)
  - RFC-0020 (whose `propose` writes findings into prose; this changes where they land, not what they may say)
  - RFC-0030 (whose `yidam edit` is the first surface that could supply node validation without an editor extension — see §3)
- **Versioning layers touched:** template / SDK+parity / tooling. **Phase 1 and 2 touch no on-disk format.** Phase 3 does, and was deliberately gated behind a measurement.
- **Amended 2026-09-07:** Phases 1 and 2 have landed (#711, #712) and #714 closed the second node model. The measurement §3.1 required has run (#713) and **Phase 3 is declined**; §3.2 carries the result and the per-corpus table it rests on. That measurement also found the prose Phase 1 cannot reach, which is now **§4**.
- **Downstream reference case:** `goedelsoup/ohio-education-funding` (#674, 129 nodes), and the repository behind #587 (1,972 links)

## Summary

A corpus node is one YAML document whose prose lives in a block scalar. Three things follow, and
all three are now measured rather than suspected. **Prose is one declared field and corpora write
three or four**, so half the checks read the field and half read the file, and the same corpus
measures 118 lines or 21 depending on which asked. **A finding is an English sentence**, spliced
into that block scalar and identified on the way back out by grepping for its own opening words —
so it has no id, no check, no commit, and cannot be counted apart from the claims a person wrote.
And **prose inside YAML makes the claim counter a prose parser and a YAML parser at once**, which
is the direct cause of its sentence segmenter, its block-scalar detector, and the grammar
heuristic that replaced a rule which once made a corpus understate its open questions fivefold on
its own README.

This RFC separates the incoherence from the format. Phase 1 makes prose a declared set rather
than one blessed key. Phase 2 makes a finding a record with an identity. Neither changes a byte
of the on-disk shape. Phase 3 then reopens the question RFC-0013 recorded as closed — whether a
node stays a YAML document or becomes Markdown with frontmatter — and it is stated here as a
decision procedure with a measurement in front of it, not as a recommendation. The reason for
that order is that Phases 1 and 2 are worth doing under either answer, and Phase 3's cost is
concentrated in a place the tree already documents.

That measurement has since run over sixteen corpora and 2,762 nodes, and it declined the phase —
for the opposite of the anticipated reason. Prose is 66% of a node's bytes, so the format is
carrying what §3 said it was. But two thirds of nodes keep prose in *nested* properties as well,
so a node in Markdown form is still a YAML document with block scalars in it: 13.25% of served
claims still depend on the block-scalar handling the move was meant to retire, against 22.68%
today. §3.2 is the record.

What it found instead is §4. Prose is not only in the keys Phase 1 taught the ontology to
declare: 68.6% of nodes keep a block scalar nested inside another key, most of it under
`properties`, and it is 4.5 times the coined top-level prose Phase 1 was built for. Everything
that reads prose misses it, including the embedder; the claim counter, which reads bytes, never
did. That is §1.2's disagreement again, one level down, and the fix is one flag on a property
declaration rather than a format.

## Problem

> **Phase 1 landed (#711).** §1.1 and §2 describe the state that prompted this RFC and are
> kept in that tense, because the argument is the evidence. What is true now: a class and
> `universal.yml` declare `prose:`, the effective set is `{description} ∪ universal ∪ class`,
> and `node-too-long`, `missing-description` and `yidam embed` all read it.
> `CorpusInstance` keeps every top-level key rather than dropping it, and `description` is no
> longer in the node schema's `required` list.
>
> **Phase 2 landed (#712).** §1.2 likewise describes what prompted the change. What is true
> now: `propose` records a finding under the node's `yidam:` key with an `id` that is a digest
> of the check and the finding's words, so a reworded question is still closable; the record is
> **not** counted among the claims the corpus makes, and **is** still an open question on the
> node (MCP contract 0.14.0). The splicing machinery is deleted; paragraphs an earlier release
> wrote are still read and still closed. §1.3 through §1.5 are unchanged and still describe
> live state.

### 1.1 The node, and the one field that is blessed

An instance is a YAML document. `class`, `label`, a `description` block scalar carrying every
paragraph the node asserts, a `properties` bag typed by the class, and `links`. The parser is
[`CorpusInstance`](../../yidam/prelude/sdks/rust/src/corpus.rs#L87), and `description` is the only prose field
it declares.

Corpora did not stay inside it. The node schema is permissive at the top level, and the comment
that made it so records why:

> Measured: with `false`, one derived repository was rejected 117 nodes of 117 (`summary`,
> `findings`, `revisions`, `unfilled` at the top level), a projecting consumer 199 of 199
>
> — [`schema.rs:78-80`](../../yidam/cli/src/cmd/schema.rs#L78-L80)

`summary` and `findings` are prose. So is `analytic_note`, which `class-asserts-purpose` tells an
author to move prose *into*. None of them is on `CorpusInstance`, so all of them are silently
dropped by every consumer of the parsed node.

That is not a tidiness complaint, because two families of check disagree as a result.
[`node_too_long`](../../yidam/cli/src/cmd/lint/checks.rs#L1321) reads the parsed field — the
comment at [`checks.rs:1305-1310`](../../yidam/cli/src/cmd/lint/checks.rs#L1305-L1310) is explicit
that this is the intent — while [`count_in_node`](../../yidam/cli/src/claims.rs#L813) takes the
file's whole text. Two definitions of *the node's prose* inside one binary, and #674 measures the
gap on a real corpus: median 118 lines read as the file, 21 read as `description`, 34 read as
`summary` + `description` + `findings`. The band that judges it was fitted on the first and the
lint check was just narrowed to the second, so the same release tells that corpus its nodes are
sprawling and that they are not.

The parse gap is the root, and it is one that adding a field will not close: the next corpus
coins the next prose key, and the checks that read the parsed node will not see it either.

**The class side of the same problem was met and solved, one file over.** A class file's prose
lives in four places, and the scanner that reads it takes the bytes rather than the parsed
fields for exactly that reason — measured across 17 corpora, with two of the four fields absent
from the struct entirely
([`checks.rs:133-139`](../../yidam/cli/src/cmd/lint/checks.rs#L133-L139)). The instance side never
got the same treatment, which is why half its checks read a field that is not all of the prose.

### 1.2 A finding is an English sentence

`yidam propose` carries a lint finding into the corpus by appending a paragraph to the
`description` block. The paragraph's identity — the only thing that lets a later run find it
again and close it — is a literal English string, [`MARKER`](../../yidam/cli/src/cmd/propose/draft.rs#L37).

Reading it back out was line arithmetic over a block scalar: one function located the block by
scanning for a key whose value is a block-scalar sigil and measuring the indent of the line
beneath it, another spliced,
[`marked`](../../yidam/cli/src/cmd/propose/draft.rs#L283) found its own paragraphs by
substring, and [`strip`](../../yidam/cli/src/cmd/propose/draft.rs#L333) removed one by index.
That machinery had already been wrong in the case that always occurs:

> Stopping only on the blank was wrong in the one case that always occurs: an appended paragraph
> is the last thing in `description:`, so the next non-blank line is `properties:` — and the
> removal took the rest of the node with it.
>
> — [`draft.rs:305-307`](../../yidam/cli/src/cmd/propose/draft.rs#L305-L307)

Four consequences follow from identity-by-sentence, and none of them is a bug in the code that
implements it:

- **A reworded finding is unclosable.** `close:` matches on the marker; an author who edits the
  paragraph into their own prose — the thing a person is most likely to do to a sentence sitting
  in the middle of their argument — orphans it permanently.
- **A finding cannot be counted apart from a claim.** The appended paragraph ends in `[open]`, so
  it enters the corpus's own open-question tally beside the questions a person raised. A corpus
  cannot ask how many of its open questions are its own.
- **Nothing can be asked of the set.** Which findings are outstanding on this node, from which
  check, opened at which commit, is a question with no answer that does not involve scanning
  prose for a sentence.
- **The corpus that broke out did it structurally.** #674's corpus has a `findings:` key. It
  reached for a field, not a paragraph, and yidam does not read it.

### 1.3 The claim counter is two parsers

Because prose lives inside YAML and the counter scans the file's bytes, it must be a prose parser
and a YAML parser at once. [`is_block_scalar_header`](../../yidam/cli/src/claims.rs#L392) exists
for one reason:

> Without this arm a claim in the first sentence of a `description:` block reaches back over the
> header and serves `class: gage label: Riffle description: |` as part of what the corpus
> asserted.
>
> — [`claims.rs:389-391`](../../yidam/cli/src/claims.rs#L389-L391)

Beside it: `starts_a_block`, telling a YAML key from a wrapped prose line;
[`stop_is_lexical`](../../yidam/cli/src/claims.rs#L454), a hand-rolled sentence segmenter with
four rules and a pinned 0.09% residue; and `is_narrated`, a grammar heuristic deciding mention
from use — adopted after the typographic rule it replaced made a corpus publish 26 open questions
against a true 72, in a generated block on its front page.

Each of those is well-argued and well-measured on its own terms. The point is that the boundary
they are all reconstructing — where structure stops and prose starts — is a boundary Markdown
states with a fence and YAML states not at all. The repository already holds the comparison: a
catalog entry is Markdown with frontmatter, and `append_to_body` handles it in a dozen lines
against `append_to_description`'s fifty.

### 1.4 Three components, three answers about a link

Neither concern that prompted this RFC named this one, and it is the sharpest instance of the
same disease.

A derived repository (#587) tags edge provenance: `claim_tag`, and `source` where verified, on
every empirical link — **1,972 links, 1,270 empirical and tagged**. It reports that every
location error it made across six phases was an edge, each one sitting beside correctly tagged
prose. Upstream gives three different answers to those two keys:

| Component | Answer |
|---|---|
| [`CorpusLink`](../../yidam/prelude/sdks/rust/src/corpus.rs#L61) | ~~dropped~~ — **kept since #714**: the struct now declares `claim_tag` and `source` beside `target` and `relationship`, carried and not interpreted. What *reads* them is still #587's question |
| the published schema, [`schema.rs:43-58`](../../yidam/cli/src/cmd/schema.rs#L43-L58) | rejected — `additionalProperties: false` on the link item |
| `yidam lint` | nothing |

So the editor underlines 1,270 links as invalid while the runtime silently discards what they
say, and no check reports either fact. Whatever is decided about prose, this one is a defect
today: the schema and the parser must give the same answer, and neither of them currently gives
the answer the corpus needs.

### 1.5 The bridge recorded as built

RFC-0002 chose the YAML instance as canonical and Markdown as an ingestion projection. RFC-0013
closed its five open questions and specified two functions: `parse_instance`, so an SDK offers
the parser the products actually use, and `project_markdown`, the doorway. Both RFCs are recorded
**Implemented**.

Neither function existed when this was written. Parity was at 0.9.0 and certified `parse_node` —
H1 title, path-derived kind, line-oriented prose claims, Markdown links — across three languages,
and no product called it. Both products that came near it wrote down that they were avoiding it:

*(Fixed by #714: `parse_instance` is the parity surface's node parser in all three SDKs, the CLI
calls it, and the Markdown model is retired. `project_markdown` is declined — see RFC-0013's
amendment.)*

> deliberately not the SDK's `extract_claims`, which is a line-oriented parser for the markdown
> node model and reads `class: gage` as a claim over a YAML instance.
>
> — [`tools.rs:656-658`](../../yidam/cli/src/cmd/serve/tools.rs#L656-L658)

and the VS Code extension declines to parse corpus YAML rather than become "a second
implementation of `parse_node`" ([`graph.ts:107-110`](../../yidam/editors/vscode/src/graph.ts#L107-L110)),
which describes a function that would not have parsed it either.

This matters to Phase 3 beyond bookkeeping. The record says the format question was settled; the
tree says the settlement was never enacted, so the parity layer has spent seven minor versions
certifying agreement about a model nothing runs. A decision to keep YAML and a decision to move
to Markdown both require this to be resolved, in opposite directions — and it cannot be resolved
by citing RFC-0013, because RFC-0013 is what is wrong.

## Proposal

### 2. Phase 1 — prose is a declared set, not a blessed key

A class declares which of its top-level keys carry prose:

```yaml
class: finding
prose: [summary, description, findings]
```

Absent, the set is `[description]` — which is every corpus written before this field existed, so
nothing changes for them. This is the shape `claim_tag` already has: the corpus says, and no key
name is blessed. It is deliberately *not* a fixed list of blessed names, for the reason
[`schema.rs:78-80`](../../yidam/cli/src/cmd/schema.rs#L78-L80) already measured — a closed set is
what sent 117 nodes of 117 to be reshaped around a validator.

Then every reader takes the declared set rather than the literal `description`:
`node-too-long`, `missing-description`, `count_in_node`, `is_open_question`, `embed`'s
`compose_text`, and `propose`'s append target. `CorpusInstance` grows a `prose: BTreeMap<String,
String>` holding them by key, so the parsed node stops being lossy and the two families of check
converge on one definition.

**`missing-description` is the one that changes meaning**, and should: a node with a `summary` and
a `findings` and no `description` is not a node with nothing said about it. It becomes *no prose
in any declared field*.

### 2.1 Phase 1 — the link, reconciled

`CorpusLink` gains `claim_tag` and `source`, both optional; the published schema opens the link
item to match. Structural relationships (`instance-of`, and whatever the corpus declares as
such) are exempt from any expectation of a tag.

Whether the two lint rules #587 asks for — untagged empirical edge, verified edge with no source —
ship in this phase is left open below. The parser and the schema agreeing is not optional and is
not a new feature; it is one fact currently stated three ways.

### 2.2 Phase 2 — a finding is a record

`propose` stops splicing sentences. A finding becomes an entry with the fields the machine
already has and throws away:

```yaml
findings:
  - id: 7f3a1c9-orphan-in
    check: orphan-in
    opened_at: 4f2a1c9
    detail: 'nothing links to this node — uncited since 2026-03-04, 3 commit(s)'
    standing: open
```

`id` is what makes `close:` possible without a substring search, and makes an author's rewording
survivable. `check` and `opened_at` are what make the set queryable — *which findings are open on
this node, from which check, since when* becomes a question `yidam query` can answer rather than
one requiring a prose scan.

**The carriage constraint is unchanged and is the reason this fits.** RFC-0020's rule is that a
proposal may assert only what the finding already asserts, enforced by quoting `detail` verbatim.
A record quotes it in a field instead of a sentence, which is strictly more faithful: there is no
framing prose to compose.

Two consequences worth stating. Tool-opened findings become countable apart from authored claims,
which the current design cannot do at all. And the fixed framing sentence — which exists only to
make an appended paragraph read as prose — is no longer needed anywhere.

A corpus that wants the finding visible in its prose can render it; that is a presentation
choice, and this RFC does not make one for it.

### 3. Phase 3 — the format, reopened

The question RFC-0013 recorded as settled: is a node a YAML document with prose fields, or a
Markdown document with YAML frontmatter?

```markdown
---
class: finding
label: Low flow
properties: { claim_tag: verified }
links:
  - { target: ../concept.ont.yml, relationship: instance-of }
---

## Summary
...

## Findings
...
```

**What it buys** is the boundary the whole of §1.3 is reconstructing. `is_block_scalar_header`,
`starts_a_block` and most of the YAML-awareness in `claims.rs` become unnecessary rather than
better; prose is what is after the fence. Phase 1's `prose:` declaration collapses into `##`
sections. `append_to_body` — which already exists and already works, because a catalog entry is
this shape — replaces the block-scalar surgery. And the parity SDKs get to certify a Markdown
node model that products would, for the first time, actually run.

**What it costs**, and this is not a rounding error:

1. **Editor validation, which the tree already documents.**

   > The catalog schema describes frontmatter inside markdown, which yaml-language-server cannot
   > apply to a .md file
   >
   > — [`schema.rs:553-555`](../../yidam/cli/src/cmd/schema.rs#L553-L555)

   Every compiled per-class schema is delivered through `yaml.schemas`. Under Markdown nodes,
   none of them reaches a node in a third-party editor. This is the strongest argument against,
   and RFC-0030 is what changes its weight: if `yidam edit` and `serve --lsp` supply diagnostics
   from the gate itself, the schema mapping stops being the only delivery path. **That makes
   Phase 3 dependent on RFC-0030 shipping, not merely adjacent to it.**

2. **Reach.** 624 occurrences of `.yml` across 61 files in the CLI alone, plus `walk_corpus_instances`,
   `migrate`, `rename`, every export, and the glob every published schema is keyed on.

3. **Every derived corpus migrates**, mechanically but not trivially — and the ones with the most
   nodes are the ones whose prose is most worth not damaging.

### 3.1 What decides it

Not this document. The measurement that must run first, over the eighteen corpora the kuten
population already names:

- **prose-to-structure ratio per node** — what fraction of a node's bytes are inside prose fields.
  #674 is one data point at roughly 34 of 118 lines; one is not a population.
- **how many corpora already grew top-level prose keys**, and which. Two are known from
  [`schema.rs:78-80`](../../yidam/cli/src/cmd/schema.rs#L78-L80) and one from #674.
- **how much of `claims.rs` is YAML-awareness** rather than claim semantics, measured by deleting
  it against a Markdown fixture set rather than estimated.

If prose is a minority of the bytes and few corpora coined prose keys, Phase 1 is the whole
answer and Phase 3 is not worth its cost. If prose dominates and corpora keep coining, the
format is fighting its content and Phase 3 is the honest fix. **The measurement is a child of
this RFC's epic and its result is reported back here before Phase 3 is accepted.**

### 3.2 What it decided — measured 2026-09-07 (#713)

The measurement ran, read-only, over the eighteen corpora on disk at that day's HEADs, with both
A0 controls applied. **Phase 3 is declined on the record**, and not for the reason §3.1
anticipated. Prose does dominate — the half of the rule that pointed toward the format. What
fails is the benefit: moving prose out of the YAML document does not retire the YAML-awareness
in `claims.rs` that was Phase 3's principal technical argument, because most of a node's prose
that is not `description` is not top-level either.

**The population, and what two corpora are excluded from.** Eighteen repositories hold a
`.yidam/`. One is a bootstrapped tree with no commits and no nodes. One — the public
`the-watermark-directory` — declares itself a non-vendoring consumer whose `.yidam/corpus/` is a
**git-ignored projection** written by its own exporter; it tracks zero corpus files. Only files
git tracks were read, so no generator's output could be counted as a practice. Sixteen corpora
and **2,762 tracked instance nodes** remain. Private and unpublished repositories are unnamed
here, as they are in the kuten profile and for the same reason; the three public ones are named
so that the aggregate has a falsifier somebody else can run.

| # | corpus | kuten | nodes | median lines | `description` %B | declared-set %B |
|---:|---|---|---:|---:|---:|---:|
| 1 | `ohio-budget` | pre | 777 | 28 | 41.8 | 41.8 |
| 2 | `allen-county-ohio` | cluster | 693 | 66 | 78.1 | 78.1 |
| 3 | *unpublished* | pre | 194 | 42 | 64.5 | 64.5 |
| 4 | *unpublished* | pre | 183 | 52 | 66.6 | 66.6 |
| 5 | *unpublished* | pre | 134 | 25 | 61.4 | 61.4 |
| 6 | `ohio-education-funding` | vintage | 129 | 118 | **21.8** | **50.5** |
| 7 | *unpublished* | cluster | 125 | 59 | 65.9 | 65.9 |
| 8 | *unpublished* | vintage | 106 | 69 | 82.2 | 82.2 |
| 9 | *unpublished* | cluster | 97 | 44 | 63.0 | 63.0 |
| 10 | *unpublished* | cluster | 81 | 35 | 62.0 | 62.0 |
| 11 | *unpublished* | cluster | 71 | 57 | 39.0 | 39.0 |
| 12 | *unpublished* | cluster | 51 | 62 | 63.3 | 63.3 |
| 13 | *unpublished* | vintage | 43 | 36 | 88.6 | 88.6 |
| 14 | *unpublished* | pre | 37 | 46 | 62.0 | 62.0 |
| 15 | *unpublished* | pre | 31 | 18 | 42.5 | 42.5 |
| 16 | *unpublished* | pre | 10 | 16 | 26.3 | 26.3 |

`cluster` is A0's six-member `inquiry` cluster, `vintage` is the other three whose vendored
prelude closes the vocabulary, `pre` vendored before it did. `%B` is the share of a node's bytes
inside the field, summed over a corpus; `declared-set` adds the free-text keys that corpus coined.

**1. Prose dominates, and it grows rather than shrinking.** Over all 2,762 nodes, `description`
alone holds **62.6% of bytes and 51.9% of lines**; under Phase 1's declared set the prose share
is **65.8% of bytes**. The per-corpus range is **0.21 to 0.89** of bytes, quoted to two decimals
and rounded outward by the estimator the kuten profile writes down. `description` is a block
scalar in fourteen of the sixteen; the two exceptions write it inline and are the two oldest
prototypes, which is what the vintage control is for, and no claim about shape rests on them.

Under the maturity control — each corpus re-measured at commit index 73 and again at HEAD, by
`ls-tree` rather than by checkout — prose share holds or rises in ten of the twelve corpora with
that much history, by as much as 17 points. One falls by under two points. The twelfth is #674's
own corpus, and it falls by 39, which is the next finding rather than a counter-example.

**2. Coining is one corpus of sixteen, and the projecting consumer has stopped.** The three data
points [`schema.rs:78-80`](../../yidam/cli/src/cmd/schema.rs#L78-L80) and #674 rest on are one
corpus and one generator. Re-measured: that corpus — `ohio-education-funding`, public and already
named upstream as the divergence canary — still coins, now on 129 nodes of 129 (`summary` on all
of them, `findings` on 91, plus `figures`, `revisions` and `unfilled`). The projecting consumer
emits `class`, `label`, `description`, `properties` and `links` across 489 nodes and coins
nothing; the *"199 of 199"* in that comment is stale. In the other fifteen corpora the only
coined key anywhere is one inline `analytic` on two nodes.

So the second half of §3.1's rule — *corpora keep coining* — is false. And the first half is
false about that corpus in a way that indicts the instrument rather than the format: measured on
`description` it is the most structure-heavy corpus in the population at 21.8%, and measured on
what it actually declares as prose it is **50.5%**, a factor of 2.3. Its prose did not shrink
between commit 73 and HEAD; it moved into keys `CorpusInstance` was dropping. **That is Phase 1's
thesis, confirmed against the one corpus that motivated the format question.**

**3. The YAML-awareness does not leave with the format.** Measured by deletion, not by reading:
`is_block_scalar_header` was stubbed to `false` and the YAML-key arm of `starts_a_block` removed,
and every tracked node was served twice — once as committed, and once projected into the
frontmatter-plus-body form §3 proposes, prose dedented and nothing reworded.

| | claims served | changed by the deletion |
|---|---:|---:|
| the node as committed | 16,045 | **3,639 (22.68%)** |
| the same node as Markdown + frontmatter | 16,045 | **2,126 (13.25%)** |

Twelve of the sixty-three `claims` unit tests fail under the deletion, all of them by serving a
block-scalar header as part of what the corpus asserted. That much was expected.

What was not: **1,942 of the 2,126 residual changes — 91% — are claims inside the frontmatter**,
after the format move. **1,892 of 2,719 nodes (69.6%) hold a nested block scalar** inside
`properties:`, a `location_description: |` or its equivalent, which the projection does not touch
because `properties` stays YAML under every version of this proposal. A node in Markdown form is
still a YAML document with block scalars in it, and `ends_statement` still has to read them.

The remaining 184 changes (1.15%) are in the Markdown body, and every one of them is the
deletion *repairing* a claim rather than breaking one — see §3.2.1.

**Why this declines Phase 3.** Its cost is unchanged: editor validation, 624 `.yml` references,
and a migration of every corpus, the largest of which now holds 693 nodes at 78% prose. Against
that, the benefit is measured at **42% of what §3 claimed** — 22.68% of served claims depend on
the YAML arms today, 13.25% still would afterwards — and the motivation is one corpus, which
Phase 1 already serves. A format break that leaves the parser it was meant to simplify still
reading YAML block scalars for two thirds of its nodes is not worth a template-layer major.

**What this does not decide.** Phases 1 and 2 stand on their own evidence and are shipped
(#711, #712). §3.3 stands: the second node model still has to go, and by the declined branch —
`parse_instance` built as RFC-0013 promised, `parse_node` retired from the parity surface,
RFC-0013's status corrected. That is #714. And the finding this measurement turned up in passing
is worth more than the phase it declined: if a corpus's prose is 66% of its bytes and two thirds
of nodes keep prose in nested properties as well, then **`prose:` wants to reach a nested key**,
which Phase 1 does not do and which is a cheaper change than a format.

#### 3.2.1 A soft-wrapped line beginning `word:` truncates a claim

Found by the deletion above and true today, in the YAML form, with no format change in prospect.
[`starts_a_block`](../../yidam/cli/src/claims.rs#L420) reads any line whose head is a bare token
followed by a colon as a new YAML key, and a wrapped prose line is often exactly that shape:

```text
… The oats outlasted the horses by
decades: there were still 22,498 acres of them in 1954. [verified] — the 1954 volume.
```

The claim is served as *"there were still 22,498 acres of them in 1954"* — the subject is on the
line above, and the boundary is a wrap column, which is the same defect
[`ends_statement`](../../yidam/cli/src/claims.rs#L530) was rewritten to remove. At least 115 of
16,045 served claims (0.72%) are truncated this way across the population, which is the same
order as the 179 lexical-stop truncations `stop_is_lexical` was given four rules to fix. Filed
separately; it is not this RFC's to fix.

### 3.3 Either way, the second model goes

Whatever Phase 3 decides, `corpus.rs`'s Markdown model cannot keep being certified by parity and
called by nothing. If Phase 3 is declined, `parse_instance` is built as RFC-0013 promised and
`parse_node` is retired from the parity surface. If Phase 3 is adopted, `parse_node` is rewritten
against the real format rather than deleted. RFC-0013's status is corrected in the same change,
because a document recorded `Implemented` that specifies two functions nobody wrote is the
premise every future reader will build on.

### 4. Phase 4 — the prose Phase 1 still cannot reach

Found by the measurement that declined Phase 3, and larger than the thing Phase 1 was built for.

**Prose is not only in top-level keys.** Over the same sixteen corpora and 2,763 tracked nodes,
**1,895 of them — 68.6% — hold a block scalar nested inside another key**, 3,625 such keys in
all. Six corpora do it on every node they have. It is 1,296,713 bytes: **14.5% of all node
bytes, and 18.2% of every block scalar a corpus writes.**

For scale against Phase 1: the coined *top-level* prose keys that motivated `prose:` —
`summary`, `findings`, one `analytic` — are **3.2%** of node bytes across the population. Nested
prose is **4.5 times that**, and it is in twelve more corpora.

| where it lives | share of measured nested prose |
|---|---|
| `properties.<key>` | **83%** |
| a key nested under a coined top-level key — `revisions.was`, `unfilled.why` | 12% |
| `links.<key>` — `note`, `because` | 5% |

The single largest is `properties.method`, a block scalar on **341 nodes** across 214 KB. Then
`properties.source_document` (322), `properties.reporting_source` (261),
`properties.seeded_because` (109), `properties.verbatim` (71).

**And it reopens §1.2's complaint one level down.** [`claims.rs`](../../yidam/cli/src/claims.rs)
scans the file's bytes, so it has always counted claims inside `properties.method`. Everything
that reads *prose* reads the declared set, which is top-level only. So the two families
disagree again, for the same reason and about a different fifth of the corpus:

- `node-too-long` measures the declared set against a class's ceiling.
- `missing-description` asks whether any declared field carries prose, and a node whose
  entire substance is a `properties.verbatim` transcription answers *no*.
- `embed` composes a node's text from its label, its declared prose, and the names of the
  nodes it links to, and from nothing else — the composition is
  [`embed.rs:85`](../../yidam/cli/src/cmd/embed.rs#L85). So `properties.method`'s 214 KB is in
  no embedding anywhere, and a query that would have matched it cannot.

#### 4.1 The declaration already exists

A class declares each property by name, type, description and `required`. Prose-ness is one
more thing a property declaration can say:

```yaml
properties:
  - name: method
    type: string
    prose: true
    description: How the figure was computed.
```

`prose: true`, absent meaning false, inheriting `required`'s argument verbatim — *"every corpus
written before this field existed was written under a schema where the question could not be
asked"* — [`ontology.rs:66-75`](../../yidam/prelude/sdks/rust/src/ontology.rs#L66-L75).
`ProseFields` gains a second axis, `prose::of` walks `properties` for the names a class flagged,
and every consumer above gets the same answer without learning a new shape.

**Not a dotted path in the existing `prose:` list.** `prose: [description, properties.method]`
is one mechanism instead of two and reaches everything — including `links.note` and
`revisions.was`, which a *class* has no standing to declare. `links` is the corpus's edge
vocabulary and #587 owns it; `revisions` is a key the class contract never described at all. A
syntax that lets a class declare prose in a structure it does not own buys 17% of the nested
bytes and an authority question this RFC has not argued.

**Not `type: prose`.** The ontology's `type` is what a value *is* — string, date, claim — and
prose-ness is orthogonal to that: `properties.method` is a string, and so is
`properties.identifier`. Folding the two would also change what `compile_class_schema` emits,
which is a parity function in three SDKs. A flag beside `type` changes nothing the schema
compiler reads.

#### 4.2 What it costs, measured

**A ceiling that has been counting a fifth of the node.** Only two of the sixteen corpora
declare `max_lines` at all. In the one that does across thirteen classes, 694 nodes are subject
to a ceiling: **60 exceed it today, and 93 would if nested prose counted** — 33 more, a 55%
increase, median +2 lines and at most +54. That is a gate arriving in a corpus that never
agreed to it, which is the objection #367 settled `max_lines` on in the first place. The answer
is the same one: the ceiling is per class and already unset by default, so a corpus adopting
`prose: true` re-reads its own ceilings in the same commit.

**Re-embedding.** Node text is what an index is built from, and `embed.rs` records the rule —
a changed composition *"would silently invalidate every existing index."* Any corpus that flags
a property re-embeds. That is the point of the change rather than a side effect, and it should
be a visible step rather than a silent drift.

**What it does not reach**, and this is stated so nobody reads the phase as complete: `links.*`
and prose nested under a coined top-level key, together 17% of measured nested prose. The first
is #587's; the second needs a class contract to describe a key it currently does not.

## What this does not touch

- **The ontology's authority.** Classes still declare properties, edges, types and `required`.
  `prose:` is one more per-class declaration in the shape the others already have.
- **The claim vocabulary.** `[verified]` / `[inference]` / `[open]`, their bare spellings, and
  `is_narrated` are unchanged. Phase 3 would let the counter stop reconstructing YAML; it does not
  change what a claim is.
- **`edge_policy`, the baseline, or either clock.** Nothing here changes what gates.
- **Governance.** No sangha, elector or resolution surface is touched.
- **The MCP contract.** Phase 2 changes what `propose` writes, not any frozen tool name or shape.
  Whether a finding record is readable over MCP is a separate contract change.

## Migration & compatibility

**Phase 1** is additive in both directions. A class with no `prose:` behaves exactly as today. A
corpus that declares one gets its existing keys read instead of dropped; no file moves. The
`CorpusLink` change accepts keys that were previously discarded, so a corpus writing them becomes
*more* valid, never less — and the schema opening is a relaxation, which cannot break a corpus
that validates today.

**Phase 2** must handle the paragraphs already in the wild. `propose` can find them — that is what
`MARKER` is for — so a one-time `yidam migrate findings` lifts each marked paragraph into a record
and removes it from the prose, using the same machinery that closes one today. A paragraph an
author has reworded past recognition stays prose, which is the correct outcome: it is theirs now.
Migration is opt-in per corpus.

**Phase 3** is a template-layer break and would be gated on RFC-0030, a `yidam migrate format`
command that rewrites a corpus in one commit, and a release that reads both forms for at least
one minor version. It is out of scope for this RFC's build; what is in scope is the measurement
that decides whether to write it.

One direction needs stating because `format_version` does not guard it: a new consumer reading a
field that a just-released CLI writes breaks against the previously released CLI, which writes
nothing there. Phase 1's fields are all optional and all read defensively, and no bundle consumer
is required to understand `prose:` in order to keep working.

## Alternatives considered

**Do nothing beyond naming the unit (#674's suggestion 1).** Cheapest, and it removes the
contradiction as a *reading* without removing it as a measurement. It leaves the parse gap, so
the next coined prose key is invisible again. Worth doing immediately and not sufficient.

**Bless a fixed list — `summary`, `description`, `findings`, `analytic_note`.** Rejected on the
evidence already in the tree: a closed set is what rejected 117 nodes of 117, and the corpus that
coins the fifth name is exactly the corpus this is meant to serve.

**A sidecar: `.yml` for structure beside `.md` for prose.** Rejected. RFC-0002 weighed two on-disk
forms and declined them, and the reasons compound here — every rename, migrate and dangling-edge
check doubles, and the two halves can disagree about which node they are.

**Keep findings in prose and give the paragraph an id in an HTML comment.** Cheaper than Phase 2
and keeps the rendering. Rejected: it leaves a finding uncountable apart from a claim, and leaves
the set unqueryable, which are two of the four consequences in §1.2.

**Go straight to Phase 3.** Rejected on sequencing, not on merit. Phases 1 and 2 are worth their
cost under either answer, Phase 3 is not yet decidable without the measurement, and it depends on
RFC-0030 shipping first.

## Open questions

1. **Does `prose:` belong on the class or in `universal.yml`?** The class is where `max_lines` and
   `required` live. But a corpus that writes `summary` on every class would declare it sixteen
   times, which is the exact argument `universal.yml` was created to answer. Probably both, with
   the class winning — this needs the same treatment universal properties already got.

2. **Do #587's two lint rules ship in Phase 1?** Reconciling the parser and the schema is not
   optional. Gating on untagged empirical edges is a new expectation arriving in corpora that
   never agreed to it, and by the argument `required` and `edge_policy` both settled, it should
   be something a class declares. Which declaration is not yet designed.

3. **Is a finding record a node-level key or its own file?** In the node, it is beside what it is
   about and travels with a rename. In `.yidam/findings/`, it does not enlarge every node that
   ever had a question asked of it, and it composes with the baseline — which is also a record of
   accepted findings, with its own clock. These may want to be one mechanism, and this RFC has
   not established that they do.

4. ~~**Does the measurement in §3.1 belong to `kuten`?**~~ **Answered no, 2026-09-07 (#713).**
   The numbers would quote as a band — prose share spreads 2.0x across the six-member cluster,
   which is inside the spread of the widest band the profile already carries. What is missing is
   a consumer. The ceiling on a node's prose is already declared per class as `max_lines:`, which
   is where #367 put it *because* 40 was a genesis norm corpora grow out of, and a practice-level
   band would be a second answer to a question a class contract already answers. Minting one
   would be the surface-with-no-consumer failure #572's own scope decision 2 names. What the run
   did find in the kuten's neighbourhood is that the member which set
   `median_node_lines: {high: 62}` measures 66 one day after the fit. That is divergence, not a
   wrong extraction, and `kuten_cluster.rs` says so in as many words — but it follows from the
   estimator rather than from a change of practice: a band quoted as the observed range has no
   headroom at either endpoint, so the extremal member diverges on its next ordinary commit.
   Filed separately as a question about the estimator.

5. **What does Phase 3 do to `.ont.yml` itself?** A class definition is prose in four places, which
   the class scanner already reads as bytes for the reason §1.1 describes
   ([`checks.rs:133-139`](../../yidam/cli/src/cmd/lint/checks.rs#L133-L139)). If nodes become Markdown
   and classes do not, the corpus has two formats again — for a defensible reason, since a class
   file really is mostly declaration, but it should be argued rather than inherited.
