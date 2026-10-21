# RFC-0049 — The ontology is the schema a pipeline loads, and the semantic layer reads (warehouse contract and the `textql` export)

- **Status:** Draft
- **Track:** I36
- **Relates to:**
  - RFC-0018 (the ontology as the bound on a typed traversal, which this extends to the bound on a table)
  - RFC-0040, RFC-0044 (the `number` type and the declared value set, the two declarations a column type and a check constraint are read from)
  - RFC-0032 (the reference grammar, whose reference is the `id` a corpus row carries)
  - RFC-0026 (a fetch writes bytes and never a node, the boundary §3 keeps: yidam generates the SQL a pipeline runs and runs none of it)
  - RFC-0014 (node rename, the operation a class or property rename does not yet have, and why §6 refuses one)
- **Versioning layers touched:** tooling only. Two new `export` formats, `warehouse` and `textql`; one new file kind a corpus may write, `.yidam/warehouse/feeds/<feed>.yml`; one new report field, `warehouse`, in `report.schema.json`. No MCP change, no SDK parity change, no template change beyond a guideline paragraph in `directories.md`.
- **Downstream reference case:** a corpus modelling GCP and AWS cloud resources whose instances live in the providers' inventories, not in the corpus. The corpus is where the model was worked out. A BigQuery warehouse is where the facts will live. TextQL's Ana, at the owner's workplace, is where the questions will be asked. The corpus cannot reach the workplace and the workplace cannot reach the corpus.

## Summary

A corpus's ontology is the most worked-over artefact a derived repository has: classes with
descriptions, typed properties, declared value sets, edges with directions. Today it can
leave the repository as RDF (`export --format rdf`), as GraphML, or as prose. None of those
is something a data pipeline can load into, or something a semantic layer over a warehouse
can query.

This RFC makes the ontology the **schema of record** for a warehouse, and proposes three outputs. Each is generated from one in-memory contract derived from `.ont.yml`:

1. **`export --format warehouse`.** This produces DDL for the contract tables, starting with BigQuery, plus a machine-readable `contract.json` and, for each feed a corpus declares, the SQL that moves a landing table into a contract table.
2. **Feeds.** A feed is a small, source-agnostic mapping file the corpus writes. It says which landing table, which columns or JSON paths, and which id feed each class and edge. The SQL it generates is run by whatever orchestrates the pipeline, such as Airflow; yidam runs none of it.
3. **`export --format textql`.** This produces a folder of Markdown, CSV and `.tql` files laid out for TextQL's git-synced Ontology repository. The `.tql` files query the contract tables by construction, so there is nothing to bind at the other end.

The corpus's own instances can travel the same route, behind a flag, as one more feed named `corpus`.

## Problem

### 1.1 What a corpus has, and what a warehouse needs

| A warehouse table needs | The ontology already declares | Where |
|---|---|---|
| a name | the class stem | `yidam/cli/src/corpus/class.rs:74-81` |
| typed columns | `properties[].type`: `string`, `text`, `ref`, `number`, `date`, `claim`, `quotation` | `yidam/cli/src/cmd/lint/checks.rs:1469-1476` |
| `NOT NULL` | `properties[].required` (#301) | `yidam/cli/src/cmd/graph.rs:84` |
| an allowed-values constraint | `properties[].values` (RFC-0044) | `docs/rfcs/0044-declared-value-set.md` |
| join paths | `edges[]` with `relationship`, `target`, `direction` | `yidam/cli/src/cmd/graph.rs:110-119` |
| a key | — | not declared; §2.2 |
| cardinality | — | not declared; §2.3 |
| an aggregation | — | not declared; §5.3 |

Five of the eight are already declared and checked by `lint`. The other three are where this RFC has to choose a default, and each choice below fails closed.

### 1.2 Why not RDF

`export --format rdf` already emits `owl:Class` and the edges as object properties
(`yidam/cli/src/cmd/export_rdf.rs:260`). That is the right artefact for a triple store and
the wrong one for a warehouse. No pipeline tool reads OWL to decide a table's columns, and no
semantic layer over SQL reads Turtle. The contract here is a second rendering of the same
model, beside RDF, not a replacement for it.

### 1.3 Why not a connector

The obvious alternative is a yidam connector that pulls inventory from AWS and GCP. It is
refused (§Alternatives) for the reason RFC-0026 refuses a fetch that writes a node. The
pipeline already exists at the reference case's workplace (Airbyte, Airflow) and so do the
landing tables: Cloud Asset Inventory exports to BigQuery natively. What is missing is the
mapping from landing tables into the shape the ontology describes. That mapping is declarative, it is
where the corpus's knowledge applies, and it is what §3 proposes.

## What TextQL's Ontology is

*Read 2026-10-21 from docs.textql.com. This surface is a product's, and it moves. Re-read before implementing §5 and date the reading in the PR.*

- **Everything is a file in a git repository.** It syncs both ways with GitHub, GitLab, CodeCommit or any git remote. A commit to the connected branch appears in the Ontology within minutes, and edits approved in TextQL are pushed back as commits or as pull requests.
- **`.md`** holds definitions, rules and conventions. **`.csv`** holds reference data. **`.tql`** holds SQL templates with typed parameters, which TextQL calls the semantic layer. A `.tql` file is compiled to SQL against a connector with no LLM in the SQL path.
- **`ANA.md` is loaded into every chat for every user.** Folder `README.md` files with "when asked about X → go to file Y" tables are how Ana finds files on demand.
- **The recommended `.tql` layout keeps every table name in one `schema.tql`.** Relation, dimension and filter modules import it. This RFC's output follows that layout.

The consequence for this design: delivering to TextQL is committing a folder to a git
repository. yidam needs no TextQL credential, and makes no TextQL API call.

## Proposal

### 2. The contract

One internal model, `warehouse::Contract`, is derived from the loaded classes. The two
renderers below read it and nothing else, so the DDL, the feed SQL and the `.tql` cannot disagree
about a column.

#### 2.1 Classes become tables

| Ontology | BigQuery column | Note |
|---|---|---|
| — | `id STRING NOT NULL` | §2.2 |
| — | `_origin STRING NOT NULL` | the feed that wrote the row: `corpus`, or a feed's name |
| — | `_loaded_at TIMESTAMP NOT NULL` | set by the feed SQL |
| — | `_source STRING` | the catalog slug the row's facts came from, when known |
| — | `_corpus_commit STRING` | the commit a `corpus` row was exported from; null otherwise |
| `string`, `text`, `ref` | `STRING` | `ref` holds an RFC-0032 reference |
| `string` with `values:` | `STRING` + a `vocab_<set>` table | BigQuery does not enforce `CHECK`; §2.4 |
| `number` | `FLOAT64` | yidam's `number` is a finite double (`numeric_value`), and `FLOAT64` keeps it exact |
| `date` | `<name> DATE` + `<name>_precision STRING` | §2.5 |
| `claim`, `quotation` | none | prose and standing, not facts; carried in the Markdown layer |
| `required: true` | `NOT NULL` | |

Identifiers are the name with every character outside `[A-Za-z0-9_]` turned to `_`. Class
and property stems are kebab-case, but a relationship is free text: a derived corpus writes
`exposed at`. Two names that collide after that (`a-b`, `a_b`) are a refusal naming both,
not a silent merge.

#### 2.2 The key

The primary key is `(id, _origin)`, which BigQuery records as `PRIMARY KEY … NOT ENFORCED`.
- In a `corpus` row, `id` is the node's reference.
- In a feed row, `id` is whatever the feed's `id:` expression yields. For the reference case that is the resource's provider-native name: a CAI `name` or an AWS ARN, both globally unique.

Keying on the pair, not on `id` alone, lets the corpus's account of a resource and the
inventory's record of it coexist without either overwriting the other. Whether they should
then be *joined* is an open question (Q1).

#### 2.3 Edges become link tables

Each declared `(class, relationship)` becomes `edge_<class>__<relationship>(src_id, dst_id,
_origin, _loaded_at, _source)`. The ontology declares no cardinality, and a link table is the
shape that is correct for every cardinality. A one-to-one edge stored this way costs a join.
A many-to-many edge stored as a foreign key would lose rows. The default is the one that is
never wrong.

`direction: in` edges are written once, on the class that authors them, as `source_classes`
already resolves (`yidam/cli/src/cmd/graph.rs:105-109`). They are not written twice.

#### 2.4 Value sets

Each declared set becomes a `vocab_<class>__<property>` table, one row per value. The same values are also
written as a CSV for the TextQL layer. BigQuery has no enforced `CHECK`, so the constraint is
enforced where it can be:
- the feed SQL (§3.3) counts the rows outside the set and fails the job;
- the `.tql` filterables (§5.2) accept only declared values.

#### 2.5 Dates with a precision

`lint` accepts `YYYY`, `YYYY-MM` and `YYYY-MM-DD`
(`yidam/cli/src/cmd/lint/checks.rs:1469-1476`). A single `DATE` column would have to invent
a day. So a `date` becomes the interval's first day plus its precision: `1893` is stored as
`1893-01-01` and `year`. That is lossless, it sorts, and a query that cares can widen the
bound. Feed rows from a source with full timestamps write `day`.

### 3. Feeds

#### 3.1 The file

A corpus declares a feed at `.yidam/warehouse/feeds/<feed>.yml`. The format is
source-agnostic. Nothing in it names a provider, and the cloud-inventory case below is an
instance of it, not its scope.

```yaml
feed: gcp-cai
description: Cloud Asset Inventory export, per-asset-type tables, daily.
mode: snapshot            # snapshot: rows absent from landing are deleted for this _origin
                          # append:   rows are upserted and never deleted
landing: "${landing_dataset}"
classes:
  compute-instance:
    from: "${landing}.cai_compute_googleapis_com_Instance"
    where: "resource.data.status != 'TERMINATED'"
    id: "name"
    source: gcp-cloud-asset-inventory        # a catalog slug, written to _source
    columns:
      zone: "resource.location"
      machine_type: "resource.data.machineType"
      created: "DATE(resource.data.creationTimestamp)"
    edges:
      attached-to:
        unnest: "resource.data.networkInterfaces"
        dst_id: "network"
```

The expressions are authored SQL fragments, trusted the way `.tql` treats authored fragments.
They are reviewed in the corpus's own PRs, and the generator neither parses nor rewrites them.
The `${...}` parameters are the only thing left for run time. They are template parameters,
not SQL, so a dataset name that differs between environments needs no regeneration.

#### 3.2 What `export` checks before writing

Every check runs against the contract, so all of them run here, with no warehouse to hand:

- every mapped class, property and edge is declared in the ontology;
- every `required: true` property of a mapped class is mapped;
- no column is mapped twice;
- every `source:` is an existing catalog slug;
- every `${...}` parameter is listed.

Classes no feed maps are reported, not refused. Coverage is information the author wants; a
gate on it would refuse the first feed anyone writes.

#### 3.3 The SQL it generates

For each mapped class, the export writes one BigQuery script, `warehouse/feeds/<feed>/<class>.sql`. The script:
1. `CAST`s each expression to the contract type, so a landing column that arrives as `NUMERIC` or a JSON string still lands as the contract says;
2. fails the job if any value of a value-set property is outside the set, naming the value;
3. `MERGE`s into the contract table on `(id, _origin)`;
4. under `mode: snapshot`, deletes this `_origin`'s rows that are absent from landing.

Link tables get the same treatment. The scripts are plain SQL with a header naming the parameters.

They run anywhere BigQuery SQL runs: an Airflow `BigQueryInsertJobOperator`, a scheduled
query, `bq query`, or a dbt `run-operation`. yidam ships no DAG. A worked Airflow example belongs in the
guide (§7), not in the binary.

#### 3.4 The corpus as a feed

`--with-corpus` adds the feed `corpus`. It writes each class's instances as NDJSON beside a BigQuery
schema file, together with a load script that runs `bq load` into a staging table and the same `MERGE`. The
corpus is one more origin, so it takes the same path and the same checks as any other.

### 4. Ownership and re-export

The TextQL folder syncs both ways. So does any repository the warehouse tree is committed to.
A re-export must not overwrite what someone changed at the other end.

- Every generated file carries a marker in a comment, which is the `yidam schema` precedent (#1057).
- `.yidam-export.json` records each file's hash at export time.
- A file whose hash no longer matches is a refusal naming it, unless `--force` is given.
- A file without the marker was not written by yidam, and is left alone.
- `schema.tql` (§5.2) is written only when it is absent, so a work-side edit to a dataset name survives.

### 5. The `textql` export

#### 5.1 Layout

```
<prefix>/                          --prefix, so the folder sits beside the work repo's own
  README.md                        navigation: "when asked about <class label> → classes/<class>.md"
  warehouse.md                     the provenance columns, the feeds, _origin, date precision
  classes/<class>.md               description; a property table with column, type, required, values;
                                   an edge table with link table and target; alignment IRIs; claims
                                   and quotations as prose
  vocab/<class>__<property>.csv    value sets
  schema.tql                       { <class> = { backing = sql"`<project>.<dataset>.<class>`" }, … }
  relations/<class>.tql            one column fragment per contract column
  links/<class>__<relationship>.tql join fragment through the link table
  views/<class>.tql                semantic view: dimensions, filterables, metrics
```

It writes no `ANA.md`. That file is loaded into every chat in the organisation, so adding
to it is the work-side owner's call. The export prints the navigation row to add.

#### 5.2 What the `.tql` views expose

- **Dimensions:** every `string`, `ref`, `date`, and value-set column, plus `_origin`.
- **Filterables:** equality on value-set columns, accepting the declared values only; `gte`/`lte` on `number` and `date` columns; equality on `_origin`.
- **Joins:** one per edge, through its link table, onto the target class's relation.

#### 5.3 Metrics

v1 emits `count` per class and nothing else. The ontology declares no aggregation, and
inventing `sum(size_gb)` because a property is a `number` would put a definition into the
semantic layer that nobody wrote, and Ana follows definitions exactly. A `metrics:`
declaration on `.ont.yml` is Q3.

### 6. Schema evolution

`warehouse/contract.json` is committed with the export. The next export diffs the new contract against it:

| Change | Output |
|---|---|
| class, property, edge or value added | `warehouse/ddl/bigquery/<n>_<slug>.sql` with `CREATE TABLE` / `ALTER TABLE ADD COLUMN` / `INSERT` into a vocab table |
| a property's `required` loosened | `ALTER COLUMN DROP NOT NULL` |
| removal, rename, type change, `required` tightened, value removed | **refused**, naming each |

`--allow-breaking` writes the breaking statements into a separate
`warehouse/ddl/bigquery/breaking/<n>.sql` for a human to run. It never runs them, and never folds them into the additive
script.

A rename shows up as a removal plus an addition, because the ontology records no class or
property rename (RFC-0014 renames nodes). That is the reason renames are refused rather than
guessed.

### 7. Documentation

- A guide page: *From a corpus to a warehouse and a semantic layer*.
- Two reference feeds under `examples/`, both read-only against public schemas:
  - Cloud Asset Inventory per-asset-type export;
  - AWS Config snapshot, delivered to S3 and loaded to BigQuery.
- An Airflow example that runs a feed's scripts after an Airbyte sync.

## What is verified, and what is not

| Claim | Status |
|---|---|
| TextQL's Ontology is a git-synced file tree of `.md`, `.csv` and `.tql` | read 2026-10-21; not run |
| a `.tql` of the shape §5.2 describes passes TextQL's `inspect` and `render` | **unverified** — P0 |
| CAI's per-asset-type export names tables `<table>_<asset_type with non-alphanumerics as _>`, with `resource.data` as nested RECORDs | read 2026-10-21; not run |
| `MERGE … WHEN NOT MATCHED BY SOURCE AND _origin = @origin THEN DELETE` is valid BigQuery | **unverified** — P0 |
| Airbyte's BigQuery destination lands `number` as `NUMERIC` | read on a page that does not say which destination version; §3.3's `CAST` makes it moot either way |

## What this does not touch

- **The corpus's model.** No `.ont.yml` field is added. Q2 and Q3 would each add one, and each is its own RFC.
- **`export --format sqlite`.** It holds the vector index and stays that way.
- **The MCP contract.** No tool reads the warehouse.
- **Running anything.** yidam writes SQL and data files; it opens no connection to BigQuery or TextQL.

## Migration & compatibility

Nothing existing changes. A corpus with no `feeds/` directory exports a contract and the TextQL
folder. The new `warehouse` report field is additive and is added to `report.schema.json` with its reachability
case. The feature is pure text generation with no new dependency, so it ships in the default build.

## Alternatives considered

- **A yidam connector that pulls AWS and GCP inventory.** Refused. The pipeline exists, the landing data exists, and a connector would make yidam the owner of credentials, rate limits and API drift for two clouds. That ownership is the scope the owner asked not to be narrowed to, and it is the part a feed file makes unnecessary.
- **One `textql` format carrying the DDL inside it.** Refused. The contract is useful to a warehouse with no TextQL in front of it, and a second semantic layer (dbt's, Cube) should be a second renderer over the same contract, not a fork of this one.
- **Binding the `.tql` to existing work tables through a mapping file.** This was the first design, before the owner said the tables do not exist yet. It is subsumed: a feed maps landing tables into contract tables, so the `.tql` binds to the contract and never to a landing table.
- **Foreign keys for edges.** Refused, for §2.3's reason: the ontology declares no cardinality.
- **Airbyte stream schemas generated from the contract.** Deferred. Useful only to someone writing a custom Airbyte connector whose output is already contract-shaped. `contract.json` carries enough for that to be a later renderer.

## Phases

| Phase | Work | Gate |
|---|---|---|
| **P0** | Work-side, by hand, on one class: create the table from hand-written DDL, load CAI rows with a hand-written `MERGE`, push one `.tql` view and run `inspect`, `render` and `execute`. | The owner's run. Every *unverified* row above is settled here. |
| **P1** | `warehouse::Contract`, `--format warehouse` DDL for BigQuery, `contract.json`, markers and the manifest. | |
| **P2** | Feed files: parse, the §3.2 checks, and the §3.3 SQL. | |
| **P3** | `--format textql`: Markdown, vocab CSVs and the navigation README. | Useful before P4: Ana reads Markdown. |
| **P4** | `--format textql`: `schema.tql`, relations, links and views. | P0's `.tql` result. |
| **P5** | `--with-corpus`. | |
| **P6** | Schema evolution: additive migrations and the breaking-change refusal. | Before the first re-export anyone relies on. |
| **P7** | Guide, two reference feeds, the Airflow example. | |

## Open questions

1. **Should the corpus's row and a feed's row for the same resource be joined?** If both carry one provider-native `id`, a view could put the corpus's account beside the inventory's facts. That needs a precedence rule per column, and the corpus would have to carry provider ids on its nodes.
2. **Should `.ont.yml` declare cardinality?** A `cardinality: one` on an edge would let §2.3 emit a column instead of a link table. It is not needed for correctness.
3. **Should `.ont.yml` declare metrics?** A `metrics:` block (`name`, `aggregate`, `over`, `description`) would make §5.3 emit real measures. It is the one place this RFC most wants a declaration it does not have.
4. **Should dialects beyond BigQuery follow?** The contract is dialect-free and the DDL and feed renderers are per dialect. Snowflake and Postgres are the obvious next two, and nobody has asked for either yet.
5. **Should a second semantic layer follow?** A dbt `schema.yml` with tests generated from the contract is the cheapest candidate.
