# From a corpus to a warehouse and a semantic layer

*A corpus that has settled what the things in a domain are can hand that shape to the systems
an organization already queries — and let its pipelines fill it, instead of the corpus.*

The corpus is [`examples/cloud-inventory/`](../../examples/cloud-inventory/README.md) — six
instances across three classes, and the only example that declares **warehouse feeds**. Every
transcript is output from a run against it. The design is
[RFC-0049](../rfcs/0049-warehouse-contract.md); this page is the route through it.

## The work, before a corpus

A team running on GCP and AWS has two inventories. Cloud Asset Inventory can export every
project, network and machine to BigQuery, one table per asset type, shaped like each
resource's API. AWS Config delivers snapshots to S3, one configuration item per resource,
shaped like a different API. Both are complete and neither answers *how many machines do we
run*. The answer is a union across two schemas that an analyst has to know to write, and a
semantic layer — TextQL's Ana, here — can only describe the tables it is given.

So the hard part is not loading data. It is deciding what a machine *is*, once, in a form both
pipelines and the semantic layer can be held to.

## The ontology is the schema of record

That decision is what a corpus already contains. Its ontology is three classes —
`account`, `network`, `compute-instance` — and
[`decisions/one-class-per-kind-not-per-cloud`](../../examples/cloud-inventory/.yidam/decisions/one-class-per-kind-not-per-cloud.yml)
records why they are not six:

> The classes become warehouse tables and the tables become what an analyst asks questions
> of. Per-cloud classes would have made "how many machines do we run" a union the analyst has
> to know to write, and every new cloud a new set of tables.

The export turns the ontology into a **contract**: one BigQuery table per class, one link table
per declared edge, a value table per closed value set, and the DDL to create them.

```console
$ yidam export --format warehouse --out warehouse
Warehouse contract written → warehouse: 14 written, 0 unchanged, 0 removed
  migration: ddl/bigquery/0001_init.sql
  feeds: aws-config, gcp-cai
```

Every class table carries five provenance columns beside the properties: `id`, `_origin`
(which feed wrote the row), `_loaded_at`, `_source` (a catalog slug) and `_corpus_commit`. Rows
are keyed on `(id, _origin)`, so two pipelines describing the same resource never overwrite
each other. A `date` property gets a second column, `<name>_precision`, because `2024-03`
and `2024-03-01` are different statements. `claim` and `quotation` properties get no column at
all: they are the corpus's argument, and the argument stays in the Markdown layer.

Run `ddl/bigquery/0001_init.sql` once, with the contract's dataset as the job's default
dataset. A later change to the ontology does not rewrite it. The next export writes
`0002_*.sql` beside it, or refuses and writes the change to `ddl/bigquery/breaking/` if
running it would lose data — see [RFC-0049 §6](../rfcs/0049-warehouse-contract.md#6-schema-evolution).

## Feeds: the pipelines fill the contract

[`decisions/the-fleet-arrives-by-feed`](../../examples/cloud-inventory/.yidam/decisions/the-fleet-arrives-by-feed.yml)
is the other half: the corpus holds the six resources somebody has something to say about,
and the thousands of others arrive from the inventories. A feed declares how a landing table
becomes contract rows. Two ship with the example:

- [`gcp-cai.yml`](../../examples/cloud-inventory/.yidam/warehouse/feeds/gcp-cai.yml) reads
  Cloud Asset Inventory's per-asset-type export.
- [`aws-config.yml`](../../examples/cloud-inventory/.yidam/warehouse/feeds/aws-config.yml)
  reads AWS Config configuration items, loaded one per row.

A feed is a map from class to SQL expressions over the landing table. This is the edge that
says which network an instance is attached to, from the GCP feed:

```yaml
    edges:
      attached-to:
        unnest: "resource.data.networkInterfaces"
        as: nic
        dst_id: "CONCAT('//compute.googleapis.com/', REGEXP_EXTRACT(nic.network, r'projects/.+$'))"
```

The rewrite is the whole lesson of
[`skills/map-an-inventory-source`](../../examples/cloud-inventory/.yidam/skills/map-an-inventory-source.md).
An asset is keyed by its full resource name, and an interface names its network by self-link
URL, so an edge written straight from the data would load and join to nothing. The AWS feed
has the same problem in a different shape: a relationship names a VPC by bare ID, and the
feed rebuilds the ARN the VPC rows are keyed on.

The export checks each feed against the contract before writing anything: every mapped class,
property and edge is declared, every required property is mapped, no column is mapped twice,
every `source:` is a catalog entry, and every `${…}` is listed under `parameters:`. A feed
that fails any of these writes nothing. What it cannot check is the landing table, which it
never sees.

For each class and edge a feed maps, the export writes one script under
`feeds/<feed>/<table>.sql`. A script casts each expression to the contract's type, fails the
job naming the value if a closed value set is violated or an `id` occurs twice, and `MERGE`s
into the contract table under the feed's `_origin`. Under `mode: snapshot` it also deletes
that origin's rows the landing table no longer holds.

**Never run a snapshot over an empty landing table.** An empty table is a snapshot of nothing,
and the script deletes every row that feed ever wrote. Nothing in the generated SQL can tell an
empty inventory from a failed sync. The pipeline can, so the check belongs there.

## Running the scripts from Airflow

yidam ships no DAG: the scripts are plain BigQuery SQL and run anywhere it does. This one
runs the AWS feed after the Airbyte sync that lands the Config snapshot, and the GCP feed after
the daily Cloud Asset Inventory export. Each feed's scripts wait on a check that its landing
table is not empty.

```python
"""Load the cloud-inventory contract from both inventories."""
from pathlib import Path
from string import Template

import pendulum
from airflow import DAG
from airflow.providers.airbyte.operators.airbyte import AirbyteTriggerSyncOperator
from airflow.providers.google.cloud.operators.bigquery import (
    BigQueryCheckOperator,
    BigQueryInsertJobOperator,
)

# `yidam export --format warehouse --out` writes here, shipped with the DAG.
WAREHOUSE = Path(__file__).parent / "warehouse"
PARAMS = {"project": "acme-landing", "landing_dataset": "inventory"}
CONTRACT = {"projectId": "acme-analytics", "datasetId": "cloud_inventory"}
# One landing table per feed whose emptiness means the sync failed.
LANDING = {
    "gcp-cai": "cai_compute_googleapis_com_Instance",
    "aws-config": "aws_config_items",
}


def script(feed: str, table: str) -> str:
    sql = (WAREHOUSE / "feeds" / feed / f"{table}.sql").read_text()
    # safe_substitute, not substitute: `'$.cidrBlock'` is a JSON path, not a parameter.
    return Template(sql).safe_substitute(PARAMS)


with DAG(
    "cloud_inventory_warehouse",
    schedule="@daily",
    start_date=pendulum.datetime(2026, 1, 1, tz="UTC"),
    catchup=False,
):
    aws_sync = AirbyteTriggerSyncOperator(
        task_id="sync_aws_config",
        airbyte_conn_id="airbyte",
        connection_id="<the S3 → BigQuery connection>",
    )
    for feed, landing in LANDING.items():
        guard = BigQueryCheckOperator(
            task_id=f"{feed}__landing_not_empty",
            sql=f"SELECT COUNT(*) > 0 FROM `{PARAMS['project']}.{PARAMS['landing_dataset']}.{landing}`",
            use_legacy_sql=False,
        )
        loads = [
            BigQueryInsertJobOperator(
                task_id=f"{feed}__{sql.stem}",
                configuration={
                    "query": {
                        "query": script(feed, sql.stem),
                        "useLegacySql": False,
                        "defaultDataset": CONTRACT,
                    }
                },
            )
            for sql in sorted((WAREHOUSE / "feeds" / feed).glob("*.sql"))
        ]
        if feed == "aws-config":
            aws_sync >> guard
        guard >> loads
```

Three things in it are not incidental. `defaultDataset` is the contract's dataset, because the
scripts name contract tables unqualified. The parameters are substituted with
`safe_substitute`, because a feed's SQL can hold a literal `$`. And the scripts of one feed have
no order among them: each loads one table, and a link row does not require its endpoints to
exist.

## The corpus as one more origin

The six nodes are rows too, and `--with-corpus` writes them as a feed named `corpus` — NDJSON
beside a BigQuery schema for each table, the same `MERGE` scripts, and a `load.sh` that stages
each file with `bq load` before running its script.

```console
$ yidam export --format warehouse --out warehouse --with-corpus
Warehouse contract written → warehouse: 19 written, 13 unchanged, 0 removed
  migration: none, the contract is unchanged
  feeds: aws-config, gcp-cai
  corpus: 12 rows across 6 tables, loaded by feeds/corpus/load.sh
    not carried: 6 link(s) whose relationship their class does not declare as an edge
```

A node's `id` is its `yidam://` reference, so a hand-written row never collides with an
inventoried one and says which corpus it came from. The six links not carried are the
`instance-of` links, which the class tables already say. Without the flag nothing from the
nodes is written, and a re-export without it removes what an earlier one wrote.

## The semantic layer

TextQL's Ontology is a git-synced folder of Markdown and CSV. The `textql` format writes one
page per class naming its table, its columns and the edges it joins on, and a CSV per closed
value set.

```console
$ yidam export --format textql --out textql
TextQL folder written → textql: 8 written, 0 unchanged, 0 removed
  ANA.md is not written; it is loaded into every chat, so adding to it is the TextQL owner's call. The row to add:
  | Account, Compute instance, Network | `textql/README.md` |
```

Commit the folder to the repository TextQL syncs. The class pages carry each class's
description from the ontology, so the meaning an analyst's question is answered in is the one
the corpus settled, not one reconstructed from column names.

## What has and has not been run

Everything above the warehouse is checked here: the transcripts on this page are re-run on
every change, both feeds pass the export's checks in the test suite, and the SQL the
generator writes is unit-tested statement by statement. The scripts have **not** been run against BigQuery
from this repository, and the feeds have not been run against a live Cloud Asset Inventory
export or AWS Config snapshot. They follow each provider's documented schema field for field;
the first run against a real one is where a mismatch would show.
