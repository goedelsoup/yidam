---
name: gcp-cloud-asset-inventory
description: Google Cloud Asset Inventory — a snapshot of every resource in a project, folder or organization, exportable to BigQuery one table per asset type.
type: api
obtained: true
location:
  - kind: url
    value: https://cloud.google.com/asset-inventory/docs/exporting-to-bigquery
    description: The BigQuery export, including the per-asset-type table layout this corpus's feed reads.
  - kind: url
    value: https://cloud.google.com/asset-inventory/docs/supported-asset-types
    description: The asset types, and the API resource each one's `resource.data` follows.
used-by:
  - ../corpus/account/platform-prod.yml
  - ../corpus/network/platform-default.yml
  - ../corpus/compute-instance/build-runner.yml
---

# Cloud Asset Inventory

The system of record for what exists in a GCP organization. An export with
`--per-asset-type` writes one table per asset type, named `<prefix>_<asset type>` with its
dots and slashes as underscores, and unpacks each asset's `resource.data` into typed columns
following the resource's own API schema.

## What this corpus takes from it

**The shape, and through the feed, the fleet.** The three GCP nodes here are illustrative and
say what a reader would want said about a resource. The `gcp-cai` feed under
`.yidam/warehouse/feeds/` is how the rest arrive: it reads the export's tables and loads every
project, network and instance into the same contract tables the nodes go to.

An asset's `name` is its full resource name — `//compute.googleapis.com/projects/…` — and
that is the `id` the feed keys on. A reference *to* a resource inside another asset's data
is usually a self-link URL instead, `https://www.googleapis.com/compute/v1/projects/…`, so an
edge has to be rewritten to the full resource name before it can join. The feed does this for
`attached-to`.

## What it does not answer

Who a resource is *for*. The inventory says a machine exists and what it is attached to;
the label that says which team owns it is a convention, applied unevenly, and absent on
anything older than the convention. It is also a snapshot: a resource created and deleted
between two exports never appears.
