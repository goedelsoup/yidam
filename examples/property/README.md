# property

*A worked yidam corpus: a chain of title, and the point where it stops.*

Twelve instances across three classes, one catalog source, two decision records, and one
skill. Small enough to read in ten minutes, and arranged so that the interesting question is
one the graph answers and a folder cannot.

## The domain, in one paragraph

A chain of title is built by joining recorded instruments: the grantee of one is the grantor
of the next. The examination walks backwards from the most recent deed, and the answer it
produces is not "who owns this" — it is *where the joining stops, and why*. Three different
failures look identical at that point: the chain is genuinely at its root, the next
instrument exists and was not found, or it exists and cannot be read. This corpus is built
around the third, which is the only one that can be established by evidence.

## What is illustrative and what is real

The **conventions** are real: the grantor–grantee index and the fact that it is keyed by name
rather than by parcel, book-and-page citation form, the distinction between the date an
instrument bears and the date it was recorded, and what the denominations mean — a quitclaim
conveys whatever the grantor had, a general warranty covenants against defects arising at any
time.

The **county, parcel, parties and instruments are not**. There is no Vantry County. They
carry real conventions so the shape of a real record is legible, and they reproduce no entry
from any real office. A corpus that invented plausible conveyances of a real parcel, or
attached invented findings to a real party's name, would be a fabricated record about
somebody's property — and this one is meant to be copied.

## The shape of it

```text
.yidam/
  corpus/
    parcel.ont.yml        instrument.ont.yml            party.ont.yml
    parcel/               instrument/                   party/
      lot-14-brightwater    1948-warranty-deed            ada-renwick
                            1961-indexed-conveyance       thomas-calloway
                            1974-quitclaim-deed           ruth-calloway
                            1993-warranty-deed            harlan-voss
                            2014-warranty-deed            brightwater-holdings
                                                          petra-osei
  catalog/vantry-county-recorder.md
  decisions/owner-is-not-a-class.yml
  decisions/an-unlocated-instrument-is-a-node.yml
  skills/trace-a-chain-of-title.md
history.toml              the order this corpus was written in
```

## What each piece is here to demonstrate

**The class that was rejected.**
[`decisions/owner-is-not-a-class.yml`](.yidam/decisions/owner-is-not-a-class.yml) records the
argument against the obvious modelling of the question a client actually asks. An owner node
is a place to assert a conclusion without the instrument that supports it — it would look the
same on the day the chain is walked and on the day a later deed is recorded. Ownership is an
entailment, and it is computed rather than stored.

**A gap that is a node rather than an absence.**
[`instrument/1961-indexed-conveyance`](.yidam/corpus/instrument/1961-indexed-conveyance.yml)
is an instrument the index says exists and which could not be read. It carries a `grantor`
edge and no `grantee` edge, because that is the state of the evidence: the index is keyed by
grantor and gives that name; the instrument would have given the rest. The missing edge is
the finding.
[`decisions/an-unlocated-instrument-is-a-node.yml`](.yidam/decisions/an-unlocated-instrument-is-a-node.yml)
is why it is not a footnote in its neighbours.

**A history, so the gap can be shown arriving.**
[`history.toml`](history.toml) names five commits, and the last one adds the 1961 entry. That
is what lets the walkthrough ask the chain question at `HEAD` and at `HEAD~1` and get
different answers: the 1948 deed is in the corpus in both, and is in the *chain* only in the
one where the unreadable instrument was written down. `yidam/cli/tests/example_corpus.rs`
replays these commits when it materialises this example; an example with no manifest gets a
single genesis commit.

**Two different reasons the chain ends.** It stops at Ruth Calloway because an instrument is
missing, and at Ada Renwick because the search stopped. The first is established; the second
is a boundary of the work, and `party/ada-renwick` says so rather than presenting itself as a
root of title.

**A source, and what it does not answer.**
[`catalog/vantry-county-recorder.md`](.yidam/catalog/vantry-county-recorder.md) spends more
space on what the index cannot tell you than on what it holds: it is keyed by name, so a
spelling variant hides an entry; recording establishes that a document was presented and
nothing about its effect; and unrecorded interests leave no trace to find.

## Running the gates

```sh
cp -R examples/property /tmp/property
cd /tmp/property && git init -q && git add -A && git commit -qm "genesis: property"
yidam graph-check     # 12 instances across 3 classes — all clean
yidam lint            # 0 finding(s), no errors
yidam open-questions  # seven live questions
```

The walkthrough that uses this corpus is
[docs/walkthroughs/property-research.md](../../docs/walkthroughs/property-research.md). It
walks the chain, shows the query that stops, and then finds the missing deed and asks what
the chain looked like before.
