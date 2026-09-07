---
name: local-registry
description: The county hydrant registry as published — a small, stable table kept beside the corpus that reads it.
type: dataset
obtained: true
retrieved: 2026-01-14
ttl_days: 365
location:
  - kind: file
    value: sources/registry-2026.csv
    description: The published table, kept in-tree because it is small and does not change between editions.
  - kind: url_template
    value: https://registry.example/api/v1/records?year={year}&format=csv
    description: The publisher's API, one edition per request.
  - kind: address
    value: Vantry County Clerk, 14 Court Street, Room 208
    description: Where the paper originals are held. Nothing fetches this.
used-by:
  - ../corpus/record/withdrawn-survey.yml
---

# The county hydrant registry

An annual table of hydrants under county maintenance, published as CSV. Each row carries an
asset identifier, the year it was placed, and the static pressure recorded at the last
inspection.

## What this corpus takes from it

The **count and its denomination**, not the individual rows. What the corpus records is how
many hydrants the county maintains and what "maintained" means in the publisher's own terms,
which is what this source is the authority on.

## What it does not answer

The registry says a hydrant exists and when it was placed. It does not say whether it was
flowing at any particular moment, and it does not carry the inspection notes that would say
why a pressure reading is low. Both are the substance of the questions this corpus is open on,
and neither is recoverable from the table.
