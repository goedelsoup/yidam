---
name: assess-a-finding
description: What to check before a finding is published, and which of those checks the corpus can answer for you.
---

# Assessing a finding

## 1. Count the documents, and read the count as a fact about exposure

`yidam query 'finding -supported-by-> document'` reports what each finding rests on. One
document is not a failure — much reporting rests on one — but it is a specific exposure that
should be a decision rather than an accident. The reported count is the full traversal;
`--limit` bounds only the projection.

## 2. Ask where the documents came from, not just how many there are

Two documents from the same entity are closer to one document than to two. Walk
`finding -supported-by-> document -obtained-from-> entity` and look at the entity set, not the
document count. A finding with three documents all released by the same agency has one point
of failure.

## 3. Do not let hosting move the tag

See `decisions/hosting-and-standing-are-separate`. Whether the newsroom may republish a
document is unrelated to how well it supports a finding. Record it in the artifact record and
leave the claim tag to the evidence.

## 4. Distinguish a withholding from an absence

A cited exemption says the agency has the record and is not releasing it. That is a location,
and it is appealable. An absence from a response says only that nothing in scope was released.
See `catalog/transport-board-records`.

## 5. Check what the finding does not assert

The most common failure here is not an unsupported finding but a supported one carrying an
unsupported implication — a date overlap presented as a relationship, a filing omission
presented as a breach. Write the boundary into the node, as
`finding/undisclosed-consent-order` and `finding/officer-tenure-overlap` both do.

## 6. Before publication, run the push

`yidam vault push --dry-run`. It reports what would leave and what refuses, grouped by the
store each was headed for and printed under that store's own audience. It is the last check
that does not depend on anybody remembering the terms.
