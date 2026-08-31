# Investigative journalism walkthrough

*A newsroom can hold every document a story rests on and still not be able to say, before
publication, which findings would collapse if one source turned out to be wrong.*

The corpus is [`examples/journalism/`](../../examples/journalism/README.md) — eleven instances
across four classes, and a two-vault configuration. Every transcript here is output from a run
against it.

**Nothing in it is real.** Ostreza Freight Holdings, its officers, the regulator and every
finding are invented. That line is drawn harder in this domain than anywhere else in yidam's
examples, and [the corpus README says why](../../examples/journalism/README.md#nothing-here-is-real-and-in-this-domain-that-line-is-drawn-hard).

## The work, before a corpus

Two questions have to be answered before a story runs, and neither is answered by the folder
the material is in.

**What does each finding rest on?** Not "do we have documents" — every finding has documents.
Which ones, how many, and whether they are independent. A finding supported by two documents
from the same office is closer to a finding with one document than to a finding with two.

**What may leave the building?** Some material is public. Some was provided on terms
permitting reporting and not republication. The distinction lives in somebody's memory of a
conversation, and it has to survive the person who had it, the departure of the reporter, and
an automated pipeline that packages an archive.

The usual answer to both is a spreadsheet, maintained alongside the work and consulted at the
end. It goes stale in the direction that matters: a new document is added to the folder and
the row is written later, or not at all.

## The ontology dialogue

**What are the irreducible kinds?** An `entity` — a company, agency or person. A `document` —
one document and the terms it arrived under. A `finding` — something the reporting asserts.
A `thread` — a line of inquiry, which outlives any particular finding.

**What relates them?** A finding is `supported-by` documents and is `about` an entity. A
document is `obtained-from` an entity. A thread `pursues` findings.

**One entity class, for sources and subjects both.** `entity/state-transport-board` is both at
once — the board's own conduct is a live question here *and* a third of the documentary record
came from it. Two classes would have put the interesting node in neither.

**The class that was rejected: `allegation`.**

It is the natural shape. A newsroom knows things it cannot print, and they feel like a
different kind of object from a finding.
[`decisions/allegation-is-not-a-class`](../../examples/journalism/.yidam/decisions/allegation-is-not-a-class.yml)
refuses it:

> An allegation class is a place to put assertions with no provenance, and having such a place
> is the failure this corpus exists to prevent. The nodes in it would be exactly the ones most
> in need of the discipline the rest of the graph is under — and they would be outside it, in a
> class whose whole definition is that the usual requirements do not apply.

So an allegation with a document is a finding; one without is a finding at `[open]` whose
`supported-by` list is empty. The cost is that `[open]` now covers two distinguishable states,
and the count of `supported-by` edges is what distinguishes them — which is the traversal the
whole corpus is arranged around.

## The corpus

```console
$ yidam graph-check
Checked 11 instances across 4 classes — all clean.

$ yidam lint
lint: 0 finding(s), no errors
```

## Claims, honestly tagged

The tiers map onto newsroom standards without translation. `[verified]` means two independent
documents support it. `[inference]` means it follows from verified facts and nobody said it.
`[open]` is what is still being chased.

`finding/officer-tenure-overlap` is the one worth reading, because it is the tier that gets
promoted by accident:

> It follows from two verified facts — the governance section gives the tenure, the inspection
> file gives the order's period — and no document says it. That is exactly what the inference
> tier is for, and promoting it because both inputs are verified is the most common way a
> corpus like this degrades: **an inference from verified facts is still an inference.**

And the second decision record is about a conflation that would move a tag for the wrong
reason:
[`hosting-and-standing-are-separate`](../../examples/journalism/.yidam/decisions/hosting-and-standing-are-separate.yml).
Whether a document may be republished says nothing about how well it supports a finding. A
finding may stand at the verified tier and rest entirely on material this corpus may never
publish; that is the ordinary condition of the work.

## The catalog, and what it does not answer

Three sources. Each entry carries an artifact record with a digest and a `redistributable`
flag, and each says what it cannot answer.

**A filing is the registrant's account of itself.** It is a primary source for what the
company said and a weak one for what happened. An absence in Item 3 is evidence of what was
disclosed, not of what exists — which is why `finding/undisclosed-consent-order` needs the
inspection file too and would be unsupportable on the filing alone.

**A withholding is not an absence.** The records response was released *in part*, citing
5 U.S.C. § 552(b)(4) for confidential commercial information and § 552(b)(6) for personal
privacy. A (b)(4) citation says the agency **has** the record and treated something in it as
confidential — a fact about the record that survives its contents being unavailable, and a
location that can be appealed. That is why `finding/deferred-maintenance` is `[open]` rather
than unsupported: there is a known place where the answer probably is.

**Terms are not a licence you can look up.** `catalog/confidential-material` records material
provided on terms permitting reporting and not republication. They were agreed with a person
and are recorded in prose. `redistributable: false` is the machine-actionable half; the
paragraph beside it is the half a person has to read.

## The question a folder cannot answer

Ask what a finding rests on, and then ask where those documents came from.

```console
$ yidam query 'finding~"consent order was not described" -supported-by-> document' --select label
2 result(s)
  Annual report, Item 3 — Legal Proceedings  ()
  Inspection file — records-request response  ()

$ yidam query 'finding~"consent order was not described" -supported-by-> document -obtained-from-> entity' --select label
2 result(s)
  Ostreza Freight Holdings  ()
  State Transport Board  ()
```

Two documents, two entities. The finding is corroborated in the sense that matters: the
company's own account and the regulator's disagree, and the disagreement is the finding.

Now the same two questions of the other one:

```console
$ yidam query 'finding~"Maintenance was deferred" -supported-by-> document' --select label
1 result(s)
  Internal maintenance memo  ()

$ yidam query 'finding~"Maintenance was deferred" -supported-by-> document -obtained-from-> entity' --select label
1 result(s)
  Ostreza Freight Holdings  ()
```

**One document, and it came from the subject of the finding.** That is the pre-publication
conversation, and it is two facts rather than one — a finding with a single source, and that
source being the party the finding is about. Neither is visible in a folder, and the second is
not visible even in a list of documents unless somebody remembers where each came from.

The count is the real one, not the page: `--limit` bounds the projection and never the
traversal.

## What may leave the building

The corpus declares two stores, because a newsroom's derived output and the documents its
reporting rests on have different readerships:

```console
$ yidam vault list
default
  url       s3://ostreza-newsroom-public/yidam
  audience  Anyone who can read this corpus. Derived output only — index, embeddings, bundles.
  holds     index, embeddings, bundle
  routed    0 artifacts the corpus names

sources
  url       s3://ostreza-newsroom-sources/yidam
  audience  The newsroom. Documents obtained under terms permitting reporting, not hosting.
  holds     catalog
  routed    3 artifacts the corpus names
```

`audience` is required and nothing can check it. It is not a security control; it is a
statement of intent that lives in the repository and outlasts the person who made it.

Then ask what would happen if you pushed. `--dry-run` sends nothing and needs no access to
either store — the credentials below are the published example values, and a dry run signs a
request it never makes:

```console
$ yidam vault push --dry-run
sources — The newsroom. Documents obtained under terms permitting reporting, not hosting.
  → s3://ostreza-newsroom-sources/yidam
  would send 4135b43f2085935849297878c77b3250dc4c1bc8b19e921bd386610bab112155 (.yidam/catalog/edgar-filings.md)
      PUT https://ostreza-newsroom-sources.s3.us-east-1.amazonaws.com/yidam/sha256/41/4135b43f…

      PUT
      /yidam/sha256/41/4135b43f2085935849297878c77b3250dc4c1bc8b19e921bd386610bab112155

      host:ostreza-newsroom-sources.s3.us-east-1.amazonaws.com
      x-amz-content-sha256:4135b43f2085935849297878c77b3250dc4c1bc8b19e921bd386610bab112155
      x-amz-date:20260831T120627Z

      host;x-amz-content-sha256;x-amz-date
      4135b43f2085935849297878c77b3250dc4c1bc8b19e921bd386610bab112155

  not cached, nothing to send: 576345cde063d82ba9a1e0c3b8e6563a0f72a8fe3052f70a2187c8b4cdf2788d (.yidam/catalog/transport-board-records.md)

3 artifacts named by the corpus; 1 would be sent; 0 already stored; 1 not cached; 1 refused

Refused:
  sources — The newsroom. Documents obtained under terms permitting reporting, not hosting.
    ff40055a0b9a10eef324dc61916f7825703ef2932cfb0d53995217b81c3dc2b3 — .yidam/catalog/confidential-material.md records `redistributable: false` — licensed to read, not to host
```

Three states in one command. One artifact would go. One is named by the corpus and not held
locally. **And one refuses, by name, under the audience of the store it was headed for** — so
the reader learns what they were about to publish to as well as what stopped.

Nothing about that refusal depends on anybody remembering the terms at the moment of the push.
The terms were recorded when the document arrived, in a committed file, and the refusal is a
consequence of the record rather than of anybody's attention.

## What this example does not show

**It does not model a source who is a person.** A confidential human source is the thing a
newsroom protects hardest, and the correct handling is almost certainly that they are not in
the corpus at all. Working out what *is* safe to record — a reference that means something to
two people and nothing to anybody else — is a real design problem this example does not
attempt.

**It does not do redaction.** `vault push` refuses a whole artifact. A document that is
publishable with three paragraphs removed is a common case and has no representation here;
the redacted version would be a second artifact with its own digest and its own record.

**It does not show the legal review.** Pre-publication review asks questions this corpus can
inform and not answer — whether a finding is defensible, whether an inference is fair comment.
The traversal tells you what rests on one source; it does not tell you whether to run it.

**It does not cross corpus boundaries.** *Who owns the building the shell company operates
from* is a question for the property corpus, and answering it from here needs
`query --across` — which is [#456](https://github.com/goedelsoup/yidam/issues/456) and not
this page.

**Its findings are invented, and the company does not exist.** See the corpus README.
