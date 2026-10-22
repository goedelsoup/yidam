---
name: map-an-inventory-source
description: How to write a warehouse feed for a cloud inventory source so its rows land in this corpus's tables and its edges join.
status: built
---

# Mapping an inventory source

A feed is a set of SQL expressions over a landing table. The export checks the mapping
against the ontology before writing anything; it cannot check that the rows join. That part
is this.

## 1. Key on the identifier other records use to point at the resource

The `id` is what an edge's `dst_id` has to equal. Pick it by asking how *other* resources
refer to this one, not by what the row calls itself. Cloud Asset Inventory keys an asset by
its full resource name and refers to it by self-link; AWS Config keys by ARN and refers by
bare ID. Both feeds here rewrite the reference, not the key.

## 2. Make a non-matching edge null, not wrong

A `dst_id` that comes out null is dropped. Write the expression so anything that is not the
target class yields null — `IF(resourceType = …, …, NULL)` — rather than filtering it some
other way. A wrong ID loads silently and joins to nothing.

## 3. Filter out what has gone

A snapshot feed deletes the rows its landing table no longer holds. A deleted resource that
is still *in* the landing table with a deleted status is not absent, so the `where:` has to
exclude it: `TERMINATED` in GCP, `ResourceDeleted` and `terminated` in AWS.

## 4. Never point a snapshot at an empty table

An empty landing table is a snapshot of nothing, and the load deletes every row this feed
ever wrote. Check the landing row count before the scripts run, in the pipeline that runs
them.

## 5. Constants are expressions

`provider: "'gcp'"` is SQL, so the string needs its own quotes inside YAML's.
