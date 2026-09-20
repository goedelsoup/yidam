# RFC-0034 — A claim resting on a node beside it (`cites:` without a package)

- **Status:** Draft
- **Track:** I29
- **Relates to:**
  - RFC-0019 (which made `cites:` a field and a citation into a *dependency* checkable; this is the other half of the same join, and it reuses that module's span comparison rather than writing a second one)
  - RFC-0017 (whose claim vocabulary — `[verified]` / `[inference]` / `[open]` — is what a local citation's `tag:` is held against)
  - RFC-0031 (whose declared prose set decides which fields a claim can be read out of, and which this consumes through `ClaimFields`)
  - RFC-0032 (whose `references:` is the field for a thing that is *not* a node; `cites:` keeps its narrower meaning of a node, at a standing, by span)
- **Versioning layers touched:** template (a new grammar in `agent-conduct.md`) / tooling (four `yidam lint` checks). **No on-disk format changes and no SDK change:** `ExternalCitation::package` was already `Option<String>`, so every released parser already accepts the shape this gives a meaning.
- **Downstream reference case:** a private derived repository whose `crates/platform` implements this check locally over a document class it invented (73 citations across 8 files). Its fault taxonomy is quoted below and its design decisions are the strongest evidence in this document.

## Summary

yidam checks a citation into a corpus it cannot revise, and a citation into a source file it
owns, and not the join a corpus performs constantly: **one node resting on a verbatim span of
another node in the same tree.** `graph-check` reads `links:`, and a citation is deliberately
not a link. So a node can state that it rests on a sentence in the node beside it, that sentence
can be rewritten by the next commit, and nothing goes red.

The obvious fix — read the quote beside the markdown link, the way `slid-line-citation` reads
the quote beside a source citation — does not survive contact with the corpora, and §2 is the
measurement that killed it. **The anchor has to be declared.** `cites:` is the declaration the
prelude already documents; it has simply never been able to name a node in the corpus it was
written in, because `package:` was required. This RFC gives the package-less shape a meaning and
four checks, and changes nothing else.

The four are safe to ship at Error because their population is **empty by construction** in a
corpus that writes no local citation. Nothing has to be argued about a severity, and no
existing corpus can be put in debt by the change: there is no practice here that a repository
could already have been doing wrong.

## Problem

### 1.1 The direction nothing reads

Three surfaces read a citation, and they leave one hole between them.

[`cmd/lint/citations.rs`](../../yidam/cli/src/cmd/lint/citations.rs) (RFC-0019) resolves a
`cites:` entry against `.yidam/tonpa/<pkg>/`. Its module doc states the boundary exactly:

> So what is checked is what is knowable: **the node is in the bundle installed here, the span still appears in it, and the bundle is the one the citation says it read.**

[`cmd/lint/line_citations.rs`](../../yidam/cli/src/cmd/lint/line_citations.rs) resolves a `#L`
fragment against a file this repository owns.

And `broken-prose-link` resolves the *path* of every markdown link under `.yidam/` and `docs/` —
12,506 of the 13,557 below — and checks the one thing about a link that a rename breaks and a
rewrite does not.

Nothing resolves a claim in this corpus against a span of this corpus. And that is the direction
the corpora write in. Across seventeen repositories with a `.yidam/corpus/`, **13,557 markdown
links resolve into `.yidam/corpus/*.yml`**, and 86.3% of them are written from one node to
another:

| where the citing file lives | links into `.yidam/corpus/*.yml` | share |
|---|---:|---:|
| `.yidam/corpus/` (node → node) | 11,697 | 86.3% |
| repository `README.md` | 639 | 4.7% |
| `.yidam/catalog/` | 582 | 4.3% |
| `.yidam/decisions/` | 291 | 2.1% |
| `.yidam/skills/` | 120 | 0.9% |
| `.yidam/sangha/` | 76 | 0.6% |
| `crates/`, `docs/`, `agents/`, and six smaller | 152 | 1.1% |
| **total** | **13,557** | |

Every one inside that walk is checked for one thing — that the file resolves — and for nothing
else. The 1,051 outside it, chiefly the repository `README.md` and `crates/`, are not read at
all.

### 1.2 The grammar exists and has never been reachable

`prelude/guidelines/agent-conduct.md` states the outbound rule and the inbound rule in the same
words, and gives a structured form for only one of them:

> **Cite a span, not a node.** An external assertion names a **verbatim span** of the corpus node it rests on, and the gate asserts that span appears there character-for-character.

The structured form under *"When claims arrive from another repository"* is `cites:`, whose
`package` field names a dependency. `ExternalCitation::package` is an `Option<String>`, and
`citations::findings` reported the absent case as an error:

> "citation with no `package:` or no `node:`"

So the field a corpus would reach for to say *this claim rests on that span of that node* has
always existed, always parsed, and always been a lint error when used that way.

Its intended population is empty and always has been. Measured across the same seventeen
repositories: **zero nodes carry a `cites:` block, and no repository has a `.yidam/tonpa.toml`
at all.** RFC-0019's four checks have never had a subject in a real corpus, because the only
citation they can read requires a dependency nobody has declared. Making the field reachable
without one is the cheapest thing that could give the grammar a subject.

## 2. The convention could not be inferred, and here is the measurement that says so

The check this RFC does *not* propose is the interesting half, because it was the obvious one.

`line_citations.rs` already finds the quote beside a link in four house forms — quotation marks
before it, a quoted span after it introduced by a colon or dash, a blockquote whose attribution
line carries it, and a fenced block whose tag names the target's language. Pointing that
machinery at the 13,557 corpus-node links would have needed no convention and no new field.

It was run over them, under exactly those rules. **222 of the 12,506 links inside the lint walk
carry something the detector reads as a quotation, producing 237 candidate quotes. 26 of the 237
are text that is actually in the cited node.** Split by the form the quote was written in:

| form | in the node | not in it | precision |
|---|---:|---:|---:|
| quoted span after the link, introduced by a colon or dash | 11 | 3 | 79% |
| italic span after the link | 10 | 12 | 45% |
| quoted span immediately before the link | 3 | 4 | 43% |
| **italic span immediately before the link** | **2** | **192** | **1%** |

The bottom row is the whole answer. An italic span before a corpus-node link is emphasis, not
transcription: node prose is written with heavy emphasis and links sit mid-sentence, so
`*The road not taken is a real one.*` followed by a link to `legislation/hb-33-2023.yml` is one
sentence of the citing node, not a quotation of the cited one.

The failures in the other rows are not near-misses either. They are a **different act**: a
blockquote beside a `person/` link is an interview being attributed to whoever said it. The
link names the speaker; the words are from a source the corpus catalogued, and they are
correctly not in the node.

So the ambient link carries no claim about the target's text, and a check reading one into it
would have reported **211 drifts of which 26 were real** — against a population no corpus opted
into. This is the same conclusion `line_citations.rs` reached for a different population and
recorded in its own doc, arriving from the other side: there, a citation whose prose says
nothing about the passage is left at `unverified-line-citation`, Info, because *"the document
holds no claim about the target at all."*

The anchor has to be declared. The only question left is what the declaration looks like.

## Proposal

### 3.1 A `cites:` entry with no `package:` names a node in this corpus

```yaml
cites:
  - node: reach/tailwater      # <class>/<name> in this corpus; `.yml` may be written or not
    tag: inference             # the standing this corpus holds that span at, and it must agree
    span: >-                   # verbatim text from that node
      Discharge below the dam tracks the release schedule within a day
```

No new field, no new file, no new marker syntax. `commit:` has no meaning here and is not read —
the node is in this tree, so git already records which state it was in.

**The absence of `package:` is the whole of the test, and it is a partition.** Every `cites:`
entry is read by exactly one module:
[`local_citations::is_local`](../../yidam/cli/src/cmd/lint/local_citations.rs) is the only place
the rule is written, and `citations::all` filters on it. One population moves: a citation naming
neither a package nor a node was `external-citation-unresolved`'s finding and is now
`local-citation-unresolved`'s, at the same severity, saying which field is missing.

### 3.2 Four checks, mirroring the external four

| check | severity | fires when |
|---|---|---|
| `local-citation-unresolved` | Error | no `node:`, or this corpus holds no such node |
| `local-citation-span-drift` | Error | no `span:`, or the span is no longer in that node |
| `local-citation-tag-drift` | Error | the span is there and the claim governing it does not carry the declared standing |
| `local-citation-untagged` | Info | the citation records a span and no `tag:` |

The severities follow `line_citations.rs`'s reasoning rather than its values: **Error where the
document makes a claim about the target that a person here can repair, Info where the document
holds no claim at all.** `local-citation-untagged` is the residue — the span is anchored, which
is most of the value; what is missing is what it was worth, and supplying it is a judgement
about the citing node rather than a defect in the corpus. That is the same verdict
`external-citation-unpinned` reaches about a citation that records what was read and not which
state it was read from.

**Whitespace is normalized on both sides and nothing else is** — `citations::flatten`, the same
function the external span check uses, not `line_citations`'s stronger word reduction. A YAML
folded scalar rewraps on read and the cited node's prose is wrapped too, so raw bytes would fail
every citation written the readable way; case, punctuation, emphasis and wording are compared as
written, because those are the changes a span exists to catch. This is what satisfies "even
ignoring line wrapping" without also forgiving a rewording.

### 3.3 The tag check exists locally and cannot exist externally

This is the one finding with no external analogue, and the reason is in the prelude:

> **A foreign tag is the producer's tag.** `[verified]` in a dependency means *that* corpus's electors accepted *that* provenance. It does not transfer, and you cannot check it.

Which is why RFC-0019's nearest sibling, `external-citation-pin-moved`, is a **Warn** that
reports a moved pin rather than a judgement about a tag.

Inside one corpus the producer is this corpus. A citation declaring `[verified]` over a
paragraph this corpus tags `[inference]` asserts something its own corpus denies, in the same
tree, decidable now rather than across an update. That is an Error.

The standing governing a span is read from `claims::claims_in_node` — the claim model
RFC-0017 already serves — taking **the weakest standing among the claims that overlap the
span, in either direction of containment.** Both directions, because a cited span is as often
two sentences as half of one: requiring the claim to contain the span reports "nothing licenses
this" against a citation that quotes a whole tagged paragraph, and requiring the span to contain
the claim misses one that quotes a clause. Weakest, because that is the direction
`agent-conduct.md` computes a derived assertion's tier in, and the only direction that cannot
flatter a citation.

Citing an `[open]` span is **legal**. The corpus is allowed to rest on its own open questions;
what it may not do is call one something else. `tag: open` over an `[open]` paragraph passes,
and `tag: verified` over it is the finding — which is the downstream's `OpenCited` reached
through the tag rule rather than beside it, for a reason §5 gives.

### 3.4 Opt-in is structural

A corpus that writes no `cites:` produces no violations from any of the four. There is no
config key to set, no ratchet entry to seed, and no adoption cliff: the population starts empty
in every repository that exists today and grows one citation at a time as authors write them.

Measured with the binary that carries the checks rather than with a proxy — `yidam lint
--format json` over six derived corpora, read from `git archive` copies so nothing touched
their trees:

| corpus | findings, all checks | of which local-citation |
|---|---:|---:|
| `goedelsoup/allen-county-ohio` (public) | 1,634 | 0 |
| B | 969 | 0 |
| C | 191 | 0 |
| `goedelsoup/ohio-education-funding` (public) | 110 | 0 |
| E | 93 | 0 |
| F | 8 | 0 |

The two public repositories are named so the aggregate has a falsifier; the other four are
private or have no remote and are labelled by rank. Appending one deliberately wrong local
citation to a node in F produced exactly one finding, `local-citation-span-drift`, naming the
node and the span. The checks are live; the population is empty because nothing has declared
one yet.

This is the property that makes Error the right severity, and it is the one condition under
which this tree gates at all. `node-too-long` is the worked precedent: a ceiling every corpus
was already over was declined because *"gating there would enforce a contract nobody wrote"* —
and because *"calling four fifths of it debt is the check that gets switched off"*. The answer
there was to move the number into a declaration the class makes about itself. The answer here
is the same shape and cheaper, because the declaration and the citation are the same act:
writing the `cites:` entry **is** opting in, so the check enforces a contract the author wrote
one line above it.

`docs/post-genesis-measurement.md` measured the other end of this and found both instrumented
repositories reporting *"`lint … no regression` while a third of one corpus is unreachable by
traversal"*, because *"the signal lies outside the severity it governs"*. A citation defect
sits inside it, and can, precisely because nobody inherits one.

## 4. The downstream reference case

A private derived repository built this check for itself, in `crates/platform`, over a document
class it had to invent first — `platform/`, the only directory in that repository permitted to
say what *should* be done. Its module doc states why it exists in terms yidam's own prelude
supplies:

> the prelude's rule for a directory it does not name is that a directory outside `graph-check` is a directory nothing checks, and that where its contents derive from the corpus, **the gate is the derivation**.

It runs over 73 citations in 8 files and its fault enum has thirteen variants. The evidence it
contributes is not the list, it is which of them generalize — and **five do not**, because they
belong to that repository's *plank* document model rather than to the citation join:
`NoFrontmatter`, `MissingField`, `EmptyLedger`, `DuplicateId` and `StandingTooLow` all read a
frontmatter block declaring `id`, `title`, `ask`, `actor`, `rests_on` and `status`. yidam has no
such document class and §5 declines to invent one.

Of the rest, three are adopted outright — `NodeNotFound`, `SpanNotFound` and `TagMismatch`,
whose message forms are the ones this RFC's checks carry, including `SpanNotFound`'s insistence
on saying *"even ignoring line wrapping"* and `TagMismatch`'s two arms: a paragraph at the wrong
standing, and a paragraph at none, where *"nothing licenses"* what was declared.

Two are declined with reasons, in §5.

Its strongest contribution is a **negative** one, and it corroborates §2 from the other
direction. That repository did not read the markdown links its corpus was already full of. It
introduced an explicit `<!-- cite: policy/x.yml [verified] -->` marker followed by a blockquote,
and put it in a directory that did not exist before — which is what building a citation gate
against an ambient convention looks like when somebody tries it: they find there is no ambient
convention to build against, and declare one. This RFC reaches the same place by a shorter
route, because yidam already has a declaration and only ever needed to make it reachable.

## What this does not touch

**Markdown documents.** `.yidam/catalog/`, `.yidam/decisions/`, `.yidam/sangha/` and `docs/`
together are 7.3% of the citing population, and none of them has a frontmatter convention that
declares a standing for a quotation. Giving them one is a template change with a document model
attached, and §2 says that model cannot be inferred from what they write today. A corpus that
wants this now can put the claim in a node and cite it from the document, which is what
`agent-conduct.md` already prescribes for material arriving from anywhere else.

**`links:`.** A citation is not a relationship and must never enter a traversal. Nothing here
makes a cited node an edge target, and `graph-check` is untouched.

**The MCP `check_citation` tool.** It requires `package`, so it never constructs a
package-less citation and `citations::findings` keeps the shape the frozen contract describes.
A local equivalent is worth having and is deliberately not in this RFC: the contract in
`prelude/sdks/parity/mcp/tools.json` has a version, a bump is not a local decision, and this
change does not need one.

**`references:`.** RFC-0032's field is for a thing that is not a node. `cites:` keeps its
narrower meaning — a node, at a standing, by span — in both directions.

## Migration & compatibility

**Nothing breaks.** `ExternalCitation::package` has been `Option<String>` since RFC-0019, so
every released binary and every SDK already parses a package-less `cites:` entry. An older
binary meeting one reports `external-citation-unresolved` where a current one reports
`local-citation-unresolved`: a different check id for the same bytes, and the older one is
wrong about what it is looking at rather than broken by it.

The baseline ratchet needs nothing, and the one case where it would is measured empty. A
baseline entry keys on a check id, so a citation naming neither a package nor a node — the one
population that changes check — would leave its old entry over-listing. `baseline-unmet` treats
an entry that no longer fires exactly as it treats a new break, *"because a list permitted to be
wrong drifts and one that over-lists silently re-permits whatever it over-lists"*. That cannot
happen here: **no corpus carries a `cites:` block at all**, so no baseline can hold an
`external-citation-unresolved` entry to be stranded. A repository adopting a newer binary sees
no new findings unless it has written a local citation, and one that writes a wrong citation
sees a finding it can act on the same commit.

Four check ids are added to `lint --format json` and to the report goldens. A consumer
enumerating checks by name sees four it does not know, which is the same event every check
addition has been.

## Alternatives considered

**Read the quote beside the markdown link.** §2. 10% precision against a population nobody
opted into, and 1% for the single most common form.

**A `<!-- cite: node.yml [tag] -->` marker, as downstream built.** It works — it is unambiguous,
invisible when rendered, and measured at zero collisions against 13,557 ambient links. It is
declined because it would be a *second* citation grammar in a repository that already has one,
and the second would be the one that could not express a dependency. The downstream invented it
because no declaration existed there to reach for. One exists here.

**A new field — `local_cites:` or similar.** Two fields meaning one thing, distinguished by
which of them carries a package, which is what `package:`'s presence already distinguishes.

**Gate the whole thing behind a config key.** Unnecessary. Opt-in is already structural (§3.4),
and a key would be a second switch for the same fact — with the failure mode that a corpus
writing local citations and forgetting the key gets a check that silently does nothing.

**`TravelledTooFar` — the document claims more than its citations license.** Declined. It is
sound and it needs a declared document-level standing to compare against; `rests_on` is the
plank model. The node-level substitute — *this node makes a `[verified]` claim somewhere and
cites an `[inference]` span somewhere* — is a guess, because nothing says which claim rests on
which citation. It would fire on every node that cites one open span and states one verified
fact about something else, which is most nodes. RFC-0031's finding records may make this
decidable later; it is not decidable now.

**`UnacknowledgedOpen` — the document leans on a node without saying what that node does not
know.** Declined for the same reason: it is defined against a `## What this does not know`
heading in a document class yidam does not have.

**`OpenCited` as a separate check.** Folded into `local-citation-tag-drift`. Downstream keeps it
separate because its rule is a *publication* rule — `[open]` material does not leave the
repository — and `platform/` is a publication boundary. yidam's template has no such boundary,
and a node resting on the corpus's own open question is a legitimate and useful thing to write
down. The defect is only ever calling it something else, which is exactly what the tag check
decides.

## Open questions

**Should a document class get this too, and which one?** §"What this does not touch" scopes it
out on a measurement of what documents write today, not on a judgement that they should not.
`.yidam/catalog/` is the most likely first candidate — 582 citations, already has frontmatter,
and `catalog-used-by-drift` already reads it — but a catalog entry cites a node to say *this
source was used here*, which is a different claim from *this text supports that*. Somebody
should measure which of the 582 are which before a convention is designed for them.

**Does anyone write one?** The honest risk. RFC-0019's grammar has zero adoption after being
shipped and documented, and the reason measured here is that it was unreachable without a
dependency — but unreachability is a sufficient explanation, not a proven one. The falsifier is
cheap and should be run: count local `cites:` entries across the derived corpora one quarter
after this ships. If the answer is still zero, the problem was never the `package:` field, and
this is a second surface with no consumer rather than the first one with a reachable subject.
