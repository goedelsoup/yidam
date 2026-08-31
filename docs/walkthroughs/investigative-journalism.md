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
Checked 12 instances across 4 classes — all clean.

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
  anchored on finding/undisclosed-consent-order.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
2 step(s), 2 edge(s) walked, 6 of 12 node(s) read, ~28 token(s)

$ yidam query 'finding~"consent order was not described" -supported-by-> document -obtained-from-> entity' --select label
2 result(s)
  Ostreza Freight Holdings  ()
  State Transport Board  ()
  anchored on finding/undisclosed-consent-order.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
3 step(s), 4 edge(s) walked, 8 of 12 node(s) read, ~18 token(s)
```

Two documents, two entities. The finding is corroborated in the sense that matters: the
company's own account and the regulator's disagree, and the disagreement is the finding.

Now the same two questions of the other one:

```console
$ yidam query 'finding~"Maintenance was deferred" -supported-by-> document' --select label
1 result(s)
  Internal maintenance memo  ()
  anchored on finding/deferred-maintenance.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
2 step(s), 1 edge(s) walked, 5 of 12 node(s) read, ~9 token(s)

$ yidam query 'finding~"Maintenance was deferred" -supported-by-> document -obtained-from-> entity' --select label
1 result(s)
  Ostreza Freight Holdings  ()
  anchored on finding/deferred-maintenance.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
3 step(s), 2 edge(s) walked, 6 of 12 node(s) read, ~9 token(s)
```

**One document, and it came from the subject of the finding.** That is the pre-publication
conversation, and it is two facts rather than one — a finding with a single source, and that
source being the party the finding is about. Neither is visible in a folder, and the second is
not visible even in a list of documents unless somebody remembers where each came from.

The count is the real one, not the page: `--limit` bounds the projection and never the
traversal.

## What may leave the building

The corpus declares two stores, because a newsroom's derived output and the documents its
reporting rests on have different readerships. Configure the one this walkthrough would push
to, and put an artifact in the local cache so there is something to push:

<!-- transcript-setup -->
```sh
# AWS's own published example key pair. `sources` is not the vault named `default`, so
# `AWS_ACCESS_KEY_ID` would not reach it: a second store exists because its readership
# differs, and silently inheriting whatever happens to be exported is the failure that
# rule is there to prevent.
export YIDAM_VAULT_SOURCES_ACCESS_KEY_ID=AKIAIOSFODNN7EXAMPLE
export YIDAM_VAULT_SOURCES_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY
# Scratch, so this section leaves nothing behind but a directory you can delete.
export YIDAM_VAULT_CACHE=.vault-cache

# One of the three artifacts the catalog names, taken into that cache. Nothing in this corpus
# is real and neither is this: the bytes stand in for the filing, and `vault put` records
# their address.
artifact=$(mktemp)
printf 'Item 3 — Legal Proceedings.\n' > "$artifact"
yidam vault put "$artifact"
```

Then ask the repository what it declares:

```console
$ yidam vault list
default
  url       s3://ostreza-newsroom-public/yidam
  audience  Anyone who can read this corpus. Derived output only — index, embeddings, bundles.
  holds     index, embeddings, bundle
  routed    0 artifacts the corpus names
  store     unusable — no credentials for vault `default`.
            Set YIDAM_VAULT_DEFAULT_ACCESS_KEY_ID and YIDAM_VAULT_DEFAULT_SECRET_ACCESS_KEY, or AWS_ACCESS_KEY_ID and AWS_SECRET_ACCESS_KEY.
            Credentials come from the environment only — `.yidam/config.toml` is committed and must never carry one.

sources
  url       s3://ostreza-newsroom-sources/yidam
  audience  The newsroom. Documents obtained under terms permitting reporting, not hosting.
  holds     catalog
  routed    3 artifacts the corpus names
  store     ready

  cache     .vault-cache
```

`audience` is required and nothing can check it. It is not a security control; it is a
statement of intent that lives in the repository and outlasts the person who made it.

`store` is the part that *is* checkable, and it is checked before anything is sent rather than
at the moment of sending. Only `sources` was configured above, so `default` reports what it
would need — and says, in the refusal itself, that `AWS_ACCESS_KEY_ID` reaches it and reaches
no other store. Two stores exist here because two readerships do, and a credential that
drifted from one to the other would move documents across exactly the line the split was
drawn on.

Then ask what would happen if you pushed. `--dry-run` reaches neither store, but it does
*sign* — and it can only report on artifacts this machine actually holds. The other two are
named by the corpus and not here, which is the state most material in a newsroom is in most of
the time; the report has to stay legible in it:

```console
$ yidam vault push --dry-run
sources — The newsroom. Documents obtained under terms permitting reporting, not hosting.
  → s3://ostreza-newsroom-sources/yidam
  would send b5f2d0bd4006ce7893b3c11d52291c2fa578e75bc7ce9616de189dcef3744862 (.yidam/catalog/edgar-filings.md)
      PUT https://ostreza-newsroom-sources.s3.us-east-1.amazonaws.com/yidam/sha256/b5/b5f2d0bd4006ce7893b3c11d52291c2fa578e75bc7ce9616de189dcef3744862

      PUT
      /yidam/sha256/b5/b5f2d0bd4006ce7893b3c11d52291c2fa578e75bc7ce9616de189dcef3744862

      host:ostreza-newsroom-sources.s3.us-east-1.amazonaws.com
      x-amz-content-sha256:b5f2d0bd4006ce7893b3c11d52291c2fa578e75bc7ce9616de189dcef3744862
      x-amz-date:20260831T153013Z

      host;x-amz-content-sha256;x-amz-date
      b5f2d0bd4006ce7893b3c11d52291c2fa578e75bc7ce9616de189dcef3744862


3 artifacts named by the corpus; 1 would be sent; 0 already stored; 1 not cached; 1 refused

Refused:
  sources — The newsroom. Documents obtained under terms permitting reporting, not hosting.
    ff40055a0b9a10eef324dc61916f7825703ef2932cfb0d53995217b81c3dc2b3 — .yidam/catalog/confidential-material.md records `redistributable: false` — licensed to read, not to host
  not cached, nothing to send: 576345cde063d82ba9a1e0c3b8e6563a0f72a8fe3052f70a2187c8b4cdf2788d (.yidam/catalog/transport-board-records.md)
```

Three states in one command. One artifact would go. One is named by the corpus and not held
locally. **And one refuses, by name, under the audience of the store it was headed for** — so
the reader learns what they were about to publish to as well as what stopped.

Nothing about that refusal depends on anybody remembering the terms at the moment of the push.
The terms were recorded when the document arrived, in a committed file, and the refusal is a
consequence of the record rather than of anybody's attention.

## The research this newsroom does not own

Who owns the land the terminals sit on is a title question, and the newsroom has not traced it.
The title research exists — it is the [property corpus](property-research.md), produced by a
different team against a different ontology — and this corpus **reads it without owning it**:

```toml
# examples/journalism/.yidam/tonpa.toml
[dependencies.property]
path = "../property"
```

A path dependency rather than a fetched bundle, which is what
[sharing a derivation](../sharing-derivations.md) documents for exactly this case. Nothing is
fetched, hashed or locked, because hashing a working tree that changes under you records
nothing:

```console
$ yidam tonpa status
  [linked]         property  → ../property  (path, unpinned)
All 1 package(s) up to date.
```

### The question, asked twice

Before publication, ask what is still unresolved in the material the story rests on. Locally:

```console
$ yidam query '*[claim_tag=open]' --select label
4 result(s)
  Maintenance was deferred at two terminals  ()
  Who owns the terminal site is not established here  ()
  What the regulator found, and what the filing said  ()
  Conditions at the terminals  ()
1 step(s), 0 edge(s) walked, 12 of 12 node(s) read, ~55 token(s)
```

And across what it depends on:

```console
$ yidam query --across '*[claim_tag=open]' --select label
5 result(s)
  Maintenance was deferred at two terminals  ()
  Who owns the terminal site is not established here  ()
  What the regulator found, and what the filing said  ()
  Conditions at the terminals  ()
  [property] 1961 conveyance — indexed, not located  ()
1 step(s), 0 edge(s) walked, 24 of 24 node(s) read, ~68 token(s) — across the dependency set
```

**Read those two lists together.** The newsroom's own open question is *who owns the terminal
site*. The title research that would answer it has an unresolved gap of its own, at 1961, and
the chain stops there. A reporter who assumed the title work was settled would have been
building on a chain with a known break in it — and the break is in a corpus nobody here wrote.

Every foreign row says whose corpus it came from. The local query returns none of them: the
boundary is not a rendering choice.

### What a cross-corpus citation is not

The [epistemic status section](../sharing-derivations.md) states the rule, and it is a claim
about the model rather than a note about the mechanism:

> - A foreign node may be **read** and **retrieved**. It is evidence an agent can consult.
> - A foreign node may **not** be an edge target. A local claim cannot rest on it structurally.

So `finding/terminal-site-ownership` has **no `supported-by` edges at all**, and stays `[open]`.
That is not a gap in the corpus; it is the boundary being honoured. The newsroom cannot make a
local claim structurally depend on research it does not own, cannot revise, and cannot stand
behind — and a claim that did would break silently the day the far side moved.

What closes that finding is a person reading the title research and writing down, *here*, what
it establishes. The dependency makes the reading cheap. It does not make the writing optional.

Reports stay local, which is the same property from the other direction:

```console
$ yidam graph-check
Checked 12 instances across 4 classes — all clean.
```

Twelve, not twenty-four. A `graph-check` or `lint` that silently counted another repository's
nodes would make every corpus metric in every derived repository meaningless.

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

**Its findings are invented, and the company does not exist.** See the corpus README.
