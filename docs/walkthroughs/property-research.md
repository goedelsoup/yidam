# Property research walkthrough

*A title examiner can find every deed affecting a parcel and still not be able to say, from
the folder, where the chain of ownership actually breaks.*

The corpus is [`examples/property/`](../../examples/property/README.md) — twelve instances
across three classes. Every transcript on this page is output from a run against it.

## The work, before a corpus

The deliverable of a title examination is not a list of documents. It is a chain: the grantee
of one instrument is the grantor of the next, back from the present deed until the joining
stops. What the client pays for is the point where it stops, and what kind of stop it is.

The working material is a folder of scans, named by book and page, sorted by date. It is a
faithful record of everything that was found, and it has one structural problem: **the thing
you are looking for is the document that is not in it.** A gap between two deeds looks exactly
like two deeds recorded six days apart. Nothing in the folder distinguishes:

- the chain is genuinely at its root;
- the next instrument exists and was not found — a name variant, a year misread, a search
  that ended when the budget did;
- it exists, was found, and cannot be read.

Only the third can be established by evidence. The first is almost never established at all,
and a title abstract that presents "the oldest deed I found" as a root of title is asserting
something it did not do the work for.

Practitioners handle this with prose: a paragraph at the front of the abstract saying what was
searched and what was not. That paragraph is not machine-readable, is written last, and is the
first thing dropped when the abstract is summarised for somebody who wants the answer.

## The ontology dialogue

Bootstrap asks what the irreducible kinds are, what relates them, and what is out of scope.
This domain's answers:

**What are the irreducible kinds?** A `parcel` — the land, which persists across every
conveyance of it. An `instrument` — one recorded document, identified by where it was recorded
rather than by what it is believed to have done. A `party` — a person or entity as an
instrument names them.

**What relates them?** An instrument `conveys` a parcel, and has a `grantor` and a `grantee`.
That is all three edges. The chain is not an edge: it is what the grantor and grantee edges
*entail* when a grantee of one instrument is the grantor of another.

**The class that was rejected: `owner`.**

The dialogue proposed it immediately, because it is the question a client actually asks. When
that was set aside it came back as an `owner` property on the parcel. Both were refused, and
[`decisions/owner-is-not-a-class`](../../examples/property/.yidam/decisions/owner-is-not-a-class.yml)
records why:

> An owner node is a place to assert a conclusion without the instrument that supports it. It
> would be written once, at the time of the examination, and it would then look exactly the
> same on the day a later deed is recorded, on the day the 1961 volume is found, and on the
> day somebody notices the grantor of the 1974 quitclaim is not the grantee of the 1948
> warranty. The chain would be wrong and the node would still be there, tagged and confident.

The cost is real and worth stating plainly: the question the client asks now has no field to
read. Answering it means running a traversal and reading what comes back — including where it
stopped. That is the trade, and it is the same shape as streamflow's rejected `observation`
class: **a stored conclusion cannot fail visibly, and a computed one can.**

## The corpus

```text
.yidam/corpus/
  parcel.ont.yml        instrument.ont.yml            party.ont.yml
  parcel/               instrument/                   party/
    lot-14-brightwater    1948-warranty-deed            ada-renwick
                          1961-indexed-conveyance       thomas-calloway
                          1974-quitclaim-deed           ruth-calloway
                          1993-warranty-deed            harlan-voss
                          2014-warranty-deed            brightwater-holdings
                                                        petra-osei
```

It gates clean:

```console
$ yidam graph-check
Checked 12 instances across 3 classes — all clean.

$ yidam lint
lint: 0 finding(s), no errors
```

The chain, as the graph holds it:

```console
$ yidam query 'parcel <-conveys- instrument' --select label,properties.dated
5 result(s)
  1948 warranty deed — Renwick to Calloway  ()  properties.dated=1948-06-11
  1961 conveyance — indexed, not located  ()  properties.dated=1961
  1974 quitclaim deed — Calloway to Voss  ()  properties.dated=1974-03-02
  1993 warranty deed — Voss to Brightwater Holdings  ()  properties.dated=1993-09-30
  2014 warranty deed — Brightwater Holdings to Osei  ()  properties.dated=2014-05-19
2 step(s), 5 edge(s) walked, 6 of 12 node(s) read, ~111 token(s)
```

Five instruments, in order, which is what the folder gives you too. Nothing so far is worth a
corpus.

## Claims, honestly tagged

The three tags mean specific things here, and the domain is unusually good at showing why the
distinction has to be structural rather than a matter of tone.

**`[verified]`** — what an instrument says, or what the index shows. *"Grantee of the 1948
warranty deed."* *"The index carries a 1961 entry under Calloway, Thomas."* These rest on a
document that was read, and every node carrying one cites
[the recorder](../../examples/property/.yidam/catalog/vantry-county-recorder.md) — the `lint`
check `verified-unsourced` is what makes that non-optional.

**`[inference]`** — what the instruments entail. The record owner is an inference, not a fact
any deed states:

> "Record owner" is an entailment of the instruments in this corpus, not a fact any of them
> states — no deed says who owns the lot now, only what each grantor conveyed to each grantee.

**`[open]`** — a question the evidence does not close. There are seven:

```console
$ yidam open-questions
- [1948 warranty deed — Renwick to Calloway](.yidam/corpus/instrument/1948-warranty-deed.yml)
- [1961 conveyance — indexed, not located](.yidam/corpus/instrument/1961-indexed-conveyance.yml)
- [2014 warranty deed — Brightwater Holdings to Osei](.yidam/corpus/instrument/2014-warranty-deed.yml)
- [Ada Renwick](.yidam/corpus/party/ada-renwick.yml)
- [Brightwater Holdings LLC](.yidam/corpus/party/brightwater-holdings.yml)
- [Ruth Calloway](.yidam/corpus/party/ruth-calloway.yml)
- [Thomas Calloway](.yidam/corpus/party/thomas-calloway.yml)
```

Two of those are the interesting ones, and they are open for **different reasons**.
`party/ada-renwick` is open because the search stopped there — a boundary of the work, not a
finding about the record. `party/ruth-calloway` is open because an instrument is missing. A
prose abstract writes both as "chain not traced further"; here they are different nodes saying
different things.

## The catalog, and what it does not answer

One source: the county recorder. Its entry spends more space on its limits than on its
contents, and each limit is load-bearing somewhere in the corpus.

**It is keyed by name, so a spelling variant hides an entry.** "R. A. Calloway" and "Ruth
Calloway" are adjacent to a reader and are different keys. This is the most common way a chain
appears to have a gap it does not have — which means an apparent gap is *first* a claim about
the search, and only then a claim about the record.

**Recording is not validity.** The office records what is presented to it in recordable form.
A recorded instrument may be void, or executed by somebody with no interest to convey.
`instrument/1974-quitclaim-deed` is where that does real work: a quitclaim conveys whatever
the grantor had, including nothing, so recording it establishes that it was presented and
nothing about its effect.

**Unrecorded interests leave no trace.** A lease, an unrecorded deed, rights held by somebody
in possession. The index is the wrong instrument for that question rather than a weak one, and
no amount of care with it will surface them.

## The question a folder cannot answer

Walk backwards. At each instrument, take the grantor and ask which instrument named them as a
grantee. Where the chain holds, you get the previous link:

```console
$ yidam query 'party~"Harlan Voss" <-grantee- instrument' --select label
1 result(s)
  1974 quitclaim deed — Calloway to Voss  ()
  anchored on party/harlan-voss.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
2 step(s), 1 edge(s) walked, 7 of 12 node(s) read, ~13 token(s)
```

Where it breaks, you get this:

```console
$ yidam query 'party~"Ruth Calloway" <-grantee- instrument' --select label
0 result(s)
  anchored on party/ruth-calloway.yml (1.00) — keyword search, not similarity (no_index); run `yidam embed && yidam index-build` to build one
  [absent] step 2: `grantee` is authored in this corpus, by `instrument`, and none of the 1 node(s) that reached the previous step has one pointing at it. The relationship is in use; it is not in use here. (no-edge-from-here)
2 step(s), 0 edge(s) walked, 6 of 12 node(s) read, ~0 token(s)
```

**That is the deliverable.** Not "no results" — a reasoned absence. The query says the
relationship exists and is used elsewhere in this corpus, and that nothing reaching this point
carries one. Ruth Calloway granted the 1974 deed, and no located instrument granted anything
to her.

The folder cannot produce this. It holds the 1974 deed and the 1948 deed, both correctly
filed, and the fact that they do not join is not written anywhere in it.

And the gap is *not* silence. The index carries a 1961 entry under Thomas Calloway pointing
into a volume the office's own damage register lists as water-damaged, so the corpus can say
an instrument was recorded and cannot say what it conveyed —
[`instrument/1961-indexed-conveyance`](../../examples/property/.yidam/corpus/instrument/1961-indexed-conveyance.yml).
It has a `grantor` edge and no `grantee` edge, because that is exactly the state of the
evidence. The missing edge *is* the finding, and
[`decisions/an-unlocated-instrument-is-a-node`](../../examples/property/.yidam/decisions/an-unlocated-instrument-is-a-node.yml)
is the argument for why it is a node rather than a footnote:

> *No instrument was recorded* and *an instrument was recorded and cannot be read* are
> opposite findings: the first says the chain is continuous and the second says it is broken
> at a known point by a known cause.

### What recording the gap bought

The chain itself is one query — every instrument with a locatable predecessor:

```console
$ yidam query 'instrument -grantor-> party <-grantee- instrument' --select label
3 result(s)
  1948 warranty deed — Renwick to Calloway  ()
  1974 quitclaim deed — Calloway to Voss  ()
  1993 warranty deed — Voss to Brightwater Holdings  ()
3 step(s), 8 edge(s) walked, 10 of 12 node(s) read, ~43 token(s)
```

And the same question, asked of the corpus as it stood one commit earlier, before the 1961
entry was written down:

```console
$ yidam query --at HEAD~1 'instrument -grantor-> party <-grantee- instrument' --select label
2 result(s) at 2b0d7733 (2026-07-21T10:05:00Z)
  1974 quitclaim deed — Calloway to Voss  ()
  1993 warranty deed — Voss to Brightwater Holdings  ()
3 step(s), 6 edge(s) walked, 8 of 11 node(s) read, ~29 token(s)
```

**The 1948 deed was in the corpus the whole time and was not in the chain.** What put it there
was recording an instrument nobody can read. The index entry names Thomas Calloway as the
grantor of *something* in 1961, Thomas Calloway is the 1948 deed's grantee, and that is the
join — the earliest deed reaches the rest of the chain through a document that cannot be
produced.

An absence noted in prose would have added no edge. That is the argument in
`an-unlocated-instrument-is-a-node`, and this is the difference it makes, one commit wide.

And if the film turns up: the `grantee` edge lands on a node that already exists, the chain
query returns four, and nothing else in the corpus changes. The finding, when it comes, is an
edit rather than a rewrite — which is the second reason the gap is a node.

`--at` is what makes any of that visible, and it is the flag's reason to exist: an abstract is
a claim about what was known at a date, and reproducing the state of the chain as of a commit
is the difference between a record of an examination and a snapshot of the current best guess.

## What this example does not show

**It does not resolve the parcel.** Lot-and-block description means the plat fixes the
boundary once and every instrument can refer to it. A metes-and-bounds parcel puts a second
question under every conveyance — *is the land described the same land* — and this corpus
would need a fourth class and a different set of arguments to hold it.

**It does not model interests short of fee.** No mortgages, liens, easements, life estates or
leases. A real examination is largely about those, and they do not fit the three edges here:
an easement is not conveyed *to* a party so much as burdened *onto* a parcel.

**It does not handle a split or a merger.** Every instrument conveys the whole lot. A parcel
subdivided into two is one node becoming two, and what the chain of the parent means for each
child is a genuine modelling question this corpus does not face.

**It does not decide the identity question.** `party/ruth-calloway` and the 1961 index entry
may name one person; the corpus notes the question and does not answer it. Deciding two
spellings are one party is title work, and a corpus that did it silently would be doing the
thing `owner` was rejected for.

**Nothing here is real.** Invented county, parcel, parties and instruments, carrying real
conventions. See
[what is illustrative and what is real](../../examples/property/README.md#what-is-illustrative-and-what-is-real).
