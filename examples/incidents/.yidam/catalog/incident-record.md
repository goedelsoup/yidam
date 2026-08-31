---
name: incident-record
description: The organisation's own incident record — declarations, timelines, and the review document for each.
type: database
obtained: true
retrieved: 2026-08-01
location:
  - kind: address
    value: Internal incident record
    description: The system of record for declarations, timelines and reviews.
used-by:
  - ../corpus/service/checkout-api.yml
  - ../corpus/service/session-store.yml
  - ../corpus/service/image-resize.yml
  - ../corpus/incident/2026-03-checkout-saturation.yml
  - ../corpus/incident/2026-05-session-store-eviction.yml
  - ../corpus/incident/2026-06-resize-queue-memory.yml
  - ../corpus/incident/2026-07-partial-checkout-degradation.yml
  - ../corpus/contributing-factor/unbounded-retry.yml
  - ../corpus/contributing-factor/cache-stampede.yml
  - ../corpus/contributing-factor/no-backpressure.yml
  - ../corpus/remediation/request-coalescing.yml
  - ../corpus/remediation/retry-budget.yml
  - ../corpus/remediation/load-shed.yml
---

# The incident record

Declarations, timelines and one review document per incident. The conventions are the ordinary
ones: a **severity ladder** where SEV1 is customer-visible loss of a core function and the
numbers rise as impact falls; a **timeline** of timestamped entries from detection through
mitigation to resolution; and a **review document** with a fixed section set — summary, impact,
timeline, contributing factors, action items.

The services, incidents and reviews are **invented**. The conventions are not.

## What this corpus takes from it

The contributing factors and the remediations, lifted out of the review documents and made
nodes so they can be counted across incidents. The timelines are **not** here: they are
per-incident, they are long, and nothing this corpus asks needs them.

## What it does not answer

**It records what was declared, not what happened.** An incident that nobody declared is not in
it. So a factor's recurrence count is a count across *reviewed* incidents, and it measures the
system only to the extent that declaring is consistent — which is why
`incident/2026-07-partial-checkout-degradation` is here despite being minor.

**The action items are not remediations.** A review's action item is a proposal, sometimes with
an owner. Whether it shipped is in the change log, and whether it *worked* is usually nowhere
at all — which is the gap `remediation/load-shed` is `[open]` about, and the reason its tier is
a fact this corpus has to assert rather than import.

**It has no memory across incidents.** Each review is a document about one event. Two reviews
concluding the same factor produce two paragraphs in two documents and no link, and nothing in
the record notices. That absence is the whole reason for this corpus.
