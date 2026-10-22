# cloud-inventory

*A worked yidam corpus: cloud resources across GCP and AWS, offboarded to a warehouse.*

Six instances across three classes, two catalog sources, two decision records, one skill,
and two **warehouse feeds**. It is the example the
[warehouse walkthrough](../../docs/walkthroughs/corpus-to-warehouse.md) runs against.

## The domain, in one paragraph

An organization running on two clouds has two inventories, each in its provider's shape:
Cloud Asset Inventory on one side, AWS Config on the other. Neither answers a question
across both. This corpus defines one shape — `account`, `network`, `compute-instance` — and
the feeds load each inventory into it, so the warehouse holds one table per kind of resource
whichever cloud it came from.

## What is illustrative and what is real

The **schemas are real**: the feeds read Cloud Asset Inventory's per-asset-type BigQuery
export and AWS Config configuration items, field for field as each provider documents them.
They have not been run against a live export from here.

The **resources are not**. There is no platform-prod project. The nodes carry real
conventions so the shape of a real inventory row is legible.

## The shape of it

```text
.yidam/
  corpus/
    account.ont.yml    network.ont.yml       compute-instance.ont.yml
    account/           network/              compute-instance/
      platform-prod      platform-default      build-runner
      workloads-prod     workloads-main        api-host
  catalog/gcp-cloud-asset-inventory.md
  catalog/aws-config.md
  decisions/one-class-per-kind-not-per-cloud.yml
  decisions/the-fleet-arrives-by-feed.yml
  skills/map-an-inventory-source.md
  warehouse/feeds/gcp-cai.yml
  warehouse/feeds/aws-config.yml
```
