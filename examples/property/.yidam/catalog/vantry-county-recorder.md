---
name: vantry-county-recorder
description: The Vantry County Recorder of Deeds — the grantor–grantee index and the deed book series it points into.
type: archive
obtained: true
location:
  - kind: address
    value: Vantry County Recorder of Deeds, Vantry County Courthouse
    description: |
      Consulted in person. The grantor-grantee index and the deed books are both here and
      are not the same source: the index is a name index, the books hold the instruments.
used-by:
  - ../corpus/party/ada-renwick.yml
  - ../corpus/party/thomas-calloway.yml
  - ../corpus/party/ruth-calloway.yml
  - ../corpus/party/harlan-voss.yml
  - ../corpus/party/brightwater-holdings.yml
  - ../corpus/parcel/lot-14-brightwater.yml
  - ../corpus/instrument/1948-warranty-deed.yml
  - ../corpus/instrument/1961-indexed-conveyance.yml
  - ../corpus/instrument/1974-quitclaim-deed.yml
  - ../corpus/instrument/1993-warranty-deed.yml
  - ../corpus/instrument/2014-warranty-deed.yml
---

# Vantry County Recorder of Deeds

The office of record for instruments affecting land in the county. Two things live here and
they are not the same thing: the **grantor–grantee index**, which is a name index, and the
**deed books**, which hold the instruments themselves. The index tells you a document exists
and where it is bound. Only the document tells you what it did.

An entry is reached by name and year, not by parcel. Searching a chain therefore means
alternating: take a grantee from one instrument, look them up as a grantor, find the next
instrument, repeat. The chain is a sequence of those joins, and it ends wherever the next
join cannot be made.

## What this corpus takes from it

The **conventions**: the grantor–grantee index structure, book-and-page citation form, the
distinction between the date an instrument bears and the date it was recorded, and the
denominations instruments carry — warranty deed, quitclaim deed, decree.

The county, the parcel, the parties and the instruments are **invented**. No entry described
here corresponds to a real record in a real office, and no statement here is a claim about
anybody's title to anything.

## What it does not answer

**It is indexed by name, so a name variant hides an entry.** "R. A. Calloway" and "Ruth
Calloway" are adjacent to a reader and are different keys. An examination that finds nothing
under one spelling has established nothing, and this is the most common way a chain appears
to have a gap it does not have.

**Recording is not validity.** The office records what is presented to it in recordable form.
A recorded instrument may be void, forged, or executed by somebody with no interest to
convey; the record shows only that it was presented and accepted. `instrument/1974-quitclaim-deed`
is the node where that distinction does real work.

**Unrecorded interests do not appear at all.** A lease, an unrecorded deed, an interest
arising by adverse possession, rights held by someone in possession — none of these are here,
and none of them would leave a trace to be found. The index is the wrong instrument for that
question, not a weak one.

**The books have physical condition.** Volume 168 is listed in the office's damage register
as water-damaged in 1979, with the affected pages enumerated. That the corpus can say *an
instrument was recorded and cannot say what it was* is a fact about this archive, and it is
why `instrument/1961-indexed-conveyance` exists as a node.
