---
name: confidential-material
description: Material provided to the newsroom on terms permitting reporting and not republication.
type: other
obtained: true
retrieved: 2026-08-26
location:
  - kind: address
    value: Provided directly to the newsroom
    description: Terms recorded at the time of receipt; not obtained from a published location.
used-by:
  - ../corpus/document/maintenance-memo.yml
artifacts:
  - sha256: ff40055a0b9a10eef324dc61916f7825703ef2932cfb0d53995217b81c3dc2b3
    bytes: 25
    media_type: text/plain
    retrieved: 2026-08-26
    redistributable: false
---

# Material licensed to read, not to host

The terms are the point of this entry. The newsroom may report what the document establishes
and may not republish the document. Those are two permissions and the record carries both:
`obtained: true` says the bytes were retrieved, and `redistributable: false` says they may not
leave this machine.

`docs/artifact-vaults.md` states the design constraint this rests on:

> A vault stores bytes. Git stores the record of them — which bytes, and which vault they are
> allowed in.

Which is why the digest is committed and the bytes are not. The record of what supports the
reporting survives in git whether or not any vault still holds the document, and a later reader
can tell that a specific document was read even if they cannot read it themselves.

## What this corpus takes from it

The **convention**: that source material arrives under terms, that the terms are recorded when
it arrives rather than reconstructed later, and that "we have it" and "we may publish it" are
separate facts.

The material is **invented**, as is everything it is said to describe.

## What it does not answer

**It is one document.** `finding/deferred-maintenance` rests on it alone, and that is a
property of the finding rather than of the document — see the finding node, which is `[open]`
for that reason and not because the document is doubted.

**Terms are not a licence you can look up.** They were agreed with a person, they are recorded
here in prose, and nothing can check them. `redistributable: false` is the machine-actionable
half; this paragraph is the half a person has to read. If the terms change, this entry is what
gets edited, and the commit is the record of when.
