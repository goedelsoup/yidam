---
name: aws-config
description: AWS Config — recorded configuration items for the resources in an account, delivered to S3 as periodic snapshots.
type: api
obtained: true
location:
  - kind: url
    value: https://docs.aws.amazon.com/config/latest/developerguide/deliver-snapshot-cli.html
    description: Delivering a configuration snapshot to the delivery channel's S3 bucket.
  - kind: url
    value: https://docs.aws.amazon.com/config/latest/APIReference/API_ConfigurationItem.html
    description: The configuration item, the record each row of the landing table holds.
used-by:
  - ../corpus/account/workloads-prod.yml
  - ../corpus/network/workloads-main.yml
  - ../corpus/compute-instance/api-host.yml
---

# AWS Config

The record of what each resource in an account looks like and what it is related to. A
snapshot is one JSON file per delivery whose `configurationItems` array holds one
configuration item per recorded resource.

## What this corpus takes from it

**The shape, and through the feed, the fleet.** The `aws-config` feed reads a landing table
holding one configuration item per row: its top-level fields as `STRING` columns, and
`configuration` and `relationships` as `JSON`. How the file reaches that table — Airbyte's
S3 source, or `jq '.configurationItems[]'` and `bq load` — is the pipeline's business, and the
feed reads only the table.

A configuration item names its resource by `ARN`, and that is the `id` the feed keys on. A
relationship names the other resource by type and bare ID, not by ARN, so the feed rebuilds
the VPC's ARN from the item's region and account to make `attached-to` join.

## What it does not answer

Anything Config is not recording. A recorder set to a list of resource types records only
those, and a type it skips is absent rather than empty. Nor is an account a recorded resource
here: the feed derives the `account` rows from the account numbers the items carry.
