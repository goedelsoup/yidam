# RFC-0031 — A node's prose is not one field, and a finding is not a sentence

- **Status:** Draft
- **Track:** I26
- **Relates to:**
  - RFC-0002 (which chose the YAML instance as the canonical graph, and whose bridge was never built)
  - RFC-0013 (which closed RFC-0002's five questions, is recorded `Implemented`, and specified two functions that do not exist)
  - RFC-0020 (whose `propose` writes findings into prose; this changes where they land, not what they may say)
  - RFC-0030 (whose `yidam edit` is the first surface that could supply node validation without an editor extension — see §3)
- **Versioning layers touched:** template / SDK+parity / tooling. **Phase 1 and 2 touch no on-disk format.** Phase 3 does, and is deliberately gated behind a measurement.
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
[`CorpusInstance`](../../yidam/cli/src/parse.rs#L164), and `description` is the only prose field
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
[`node_too_long`](../../yidam/cli/src/cmd/lint/checks.rs#L1303) reads the parsed field — the
comment at [`checks.rs:1287-1292`](../../yidam/cli/src/cmd/lint/checks.rs#L1287-L1292) is explicit
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
| [`CorpusLink`](../../yidam/cli/src/parse.rs#L263) | dropped — the struct declares `target` and `relationship`, and sets no `deny_unknown_fields` |
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

Neither function exists. `parse_instance` and `project_markdown` appear nowhere outside those two
documents. Parity is at 0.9.0 and still certifies
[`parse_node`](../../yidam/prelude/sdks/rust/src/corpus.rs#L157) — H1 title, path-derived kind,
line-oriented prose claims, Markdown links — across three languages. No product calls it. Both
products that came near it wrote down that they were avoiding it:

> deliberately not the SDK's `extract_claims`, which is a line-oriented parser for the markdown
> node model and reads `class: gage` as a claim over a YAML instance.
>
> — [`tools.rs:640-642`](../../yidam/cli/src/cmd/serve/tools.rs#L640-L642)

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
   > — [`schema.rs:534-536`](../../yidam/cli/src/cmd/schema.rs#L534-L536)

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

### 3.2 Either way, the second model goes

Whatever Phase 3 decides, `corpus.rs`'s Markdown model cannot keep being certified by parity and
called by nothing. If Phase 3 is declined, `parse_instance` is built as RFC-0013 promised and
`parse_node` is retired from the parity surface. If Phase 3 is adopted, `parse_node` is rewritten
against the real format rather than deleted. RFC-0013's status is corrected in the same change,
because a document recorded `Implemented` that specifies two functions nobody wrote is the
premise every future reader will build on.

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

4. **Does the measurement in §3.1 belong to `kuten`?** It is a corpus-population measurement over
   the eighteen, which is what the kuten layer is for. If it does, its result becomes a band and
   Phase 3's decision has a published instrument behind it rather than a one-off study.

5. **What does Phase 3 do to `.ont.yml` itself?** A class definition is prose in four places, which
   the class scanner already reads as bytes for the reason §1.1 describes
   ([`checks.rs:133-139`](../../yidam/cli/src/cmd/lint/checks.rs#L133-L139)). If nodes become Markdown
   and classes do not, the corpus has two formats again — for a defensible reason, since a class
   file really is mostly declaration, but it should be argued rather than inherited.
