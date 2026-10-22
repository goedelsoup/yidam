//! RFC-0049 §2 in BigQuery's dialect: the DDL that creates the contract's tables.

use std::fmt::Write as _;

use super::contract::{Column, ColumnType, Contract, Table, TableKind};
use super::evolution::{Additive, Breaking, Diff};
use super::feeds::{ClassLoad, EdgeLoad, Feed, Mode};
use super::MARKER;

pub(crate) fn type_name(ty: ColumnType) -> &'static str {
    match ty {
        ColumnType::String => "STRING",
        ColumnType::Float => "FLOAT64",
        ColumnType::Date => "DATE",
        ColumnType::Timestamp => "TIMESTAMP",
    }
}

/// A GoogleSQL double-quoted string literal.
pub(crate) fn string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn options(description: &str) -> String {
    if description.is_empty() {
        String::new()
    } else {
        format!(" OPTIONS(description={})", string_literal(description))
    }
}

fn column(c: &Column) -> String {
    let not_null = if c.required { " NOT NULL" } else { "" };
    format!(
        "  `{}` {}{not_null}{}",
        c.name,
        type_name(c.ty),
        options(&c.description)
    )
}

fn create_table(out: &mut String, t: &Table) {
    let _ = writeln!(out, "CREATE TABLE IF NOT EXISTS `{}` (", t.name);
    for c in &t.columns {
        let _ = writeln!(out, "{},", column(c));
    }
    let key: Vec<String> = t.primary_key.iter().map(|k| format!("`{k}`")).collect();
    // BigQuery records a key and does not enforce it; the feed SQL's MERGE is what keeps it.
    let _ = writeln!(out, "  PRIMARY KEY ({}) NOT ENFORCED", key.join(", "));
    let _ = writeln!(out, "){};", options(&t.description));
}

/// Insert the declared values a vocab table does not already hold, so a re-run adds nothing.
fn seed_vocab(out: &mut String, t: &Table, values: &[String]) {
    let list: Vec<String> = values.iter().map(|v| string_literal(v)).collect();
    let _ = writeln!(
        out,
        "INSERT INTO `{name}` (`value`)\nSELECT v FROM UNNEST([{list}]) AS v\n\
         WHERE v NOT IN (SELECT `value` FROM `{name}`);",
        name = t.name,
        list = list.join(", ")
    );
}

/// The initial DDL: every table, then every value set's rows.
///
/// Identifiers are unqualified, so the script runs against whichever dataset its job names as
/// the default. One file then serves every environment, the same reason feed SQL leaves the
/// dataset to a parameter.
pub(crate) fn render_init(contract: &Contract) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "-- {MARKER}");
    out.push_str(
        "--\n\
         -- The RFC-0049 warehouse contract, BigQuery dialect. Identifiers are unqualified: run\n\
         -- this with the target dataset as the job's default dataset, for example\n\
         --   bq query --use_legacy_sql=false --dataset_id=<project>:<dataset> < 0001_init.sql\n",
    );
    for t in &contract.tables {
        out.push('\n');
        create_table(&mut out, t);
    }
    for t in &contract.tables {
        if let TableKind::Vocab { values, .. } = &t.kind {
            out.push('\n');
            seed_vocab(&mut out, t, values);
        }
    }
    out
}

/// An additive migration (§6): the statements that bring a dataset at the last contract to
/// this one without touching a row it holds. Each is guarded so a second run adds nothing.
pub(crate) fn render_migration(name: &str, diff: &Diff) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "-- {MARKER}");
    let _ = writeln!(
        out,
        "--\n-- Migration {name}: run it after every earlier file in this directory, in order.\n\
         -- What changed in the ontology since the last export:"
    );
    for change in &diff.additive {
        let _ = writeln!(out, "--   {}", change.describe());
    }
    for change in &diff.additive {
        out.push('\n');
        match change {
            Additive::CreateTable(t) => {
                create_table(&mut out, t);
                if let TableKind::Vocab { values, .. } = &t.kind {
                    seed_vocab(&mut out, t, values);
                }
            }
            Additive::AddColumn { table, column: c } => {
                let _ = writeln!(
                    out,
                    "ALTER TABLE `{table}` ADD COLUMN IF NOT EXISTS {};",
                    column(c).trim_start()
                );
            }
            Additive::DropNotNull { table, column } => {
                let _ = writeln!(
                    out,
                    "ALTER TABLE `{table}` ALTER COLUMN `{column}` DROP NOT NULL;"
                );
            }
            Additive::AddValues { table, values } => {
                let list: Vec<String> = values.iter().map(|v| string_literal(v)).collect();
                let _ = writeln!(
                    out,
                    "INSERT INTO `{table}` (`value`)\nSELECT v FROM UNNEST([{list}]) AS v\n\
                     WHERE v NOT IN (SELECT `value` FROM `{table}`);",
                    list = list.join(", ")
                );
            }
        }
    }
    out
}

/// The breaking statements `--allow-breaking` writes for review (§6). yidam never runs them,
/// and nothing in a pipeline should: each one alters or discards rows somebody loaded.
pub(crate) fn render_breaking(name: &str, breaking: &[Breaking]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "-- {MARKER}");
    let _ = writeln!(
        out,
        "--\n-- Breaking changes {name}, written because the export was run with \
         --allow-breaking.\n\
         -- Review each statement against the rows the table holds before running any of it.\n\
         -- A rename arrives as a removal here and an addition in the migration of the same\n\
         -- number; to keep the data, copy the old column into the new before dropping it."
    );
    for change in breaking {
        let _ = writeln!(out, "\n-- {}", change.describe());
        match change {
            Breaking::DropTable { table } => {
                let _ = writeln!(out, "DROP TABLE IF EXISTS `{table}`;");
            }
            Breaking::DropColumn { table, column } => {
                let _ = writeln!(
                    out,
                    "ALTER TABLE `{table}` DROP COLUMN IF EXISTS `{column}`;"
                );
            }
            Breaking::TypeChange {
                table, column, to, ..
            } => {
                let _ = writeln!(
                    out,
                    "-- BigQuery converts only a widening type in place. If this statement \
                     fails,\n-- add a new column, copy the converted values, and drop this one.\n\
                     ALTER TABLE `{table}` ALTER COLUMN `{column}` SET DATA TYPE {};",
                    type_name(*to)
                );
            }
            Breaking::Tightened { table, column } => {
                let _ = writeln!(
                    out,
                    "-- BigQuery cannot make an existing column NOT NULL, so it stays nullable.\n\
                     -- The rows that hold no value for it:\n\
                     -- SELECT COUNT(*) FROM `{table}` WHERE `{column}` IS NULL;"
                );
            }
            Breaking::RequiredAdded { table, column: c } => {
                let mut nullable = c.clone();
                nullable.required = false;
                let _ = writeln!(
                    out,
                    "-- BigQuery cannot add a NOT NULL column to a table: it is added nullable, \
                     and\n-- the rows already loaded have no value for it until a feed \
                     supplies one.\n\
                     ALTER TABLE `{table}` ADD COLUMN IF NOT EXISTS {};",
                    column(&nullable).trim_start()
                );
            }
            Breaking::ValueRemoved { table, value } => {
                let _ = writeln!(
                    out,
                    "DELETE FROM `{table}` WHERE `value` = {};",
                    string_literal(value)
                );
            }
        }
    }
    out
}

/// The clause that deletes this origin's rows landing no longer holds (§3.3, step 4).
///
/// The one place the snapshot delete is written. P0 (#1395) runs it at the other end, and if
/// BigQuery wants a different form this is the only line that changes.
fn snapshot_delete(origin: &str) -> String {
    format!(
        "WHEN NOT MATCHED BY SOURCE AND T.`_origin` = {} THEN\n  DELETE",
        string_literal(origin)
    )
}

/// The header every feed script opens with: what it loads, and what must be substituted.
fn feed_header(out: &mut String, feed: &Feed, what: &str) {
    let _ = writeln!(out, "-- {MARKER}");
    let _ = writeln!(out, "--");
    let _ = writeln!(out, "-- Feed `{}`: {what}.", feed.name);
    if !feed.description.is_empty() {
        let about = feed
            .description
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(out, "-- {about}");
    }
    let mode = match feed.mode {
        Mode::Snapshot => "snapshot: rows of this origin that landing no longer holds are deleted",
        Mode::Append => "append: rows are upserted and never deleted",
    };
    let _ = writeln!(out, "-- Mode: {mode}.");
    if feed.parameters.is_empty() {
        let _ = writeln!(out, "-- Parameters: none.");
    } else {
        let _ = writeln!(
            out,
            "-- Parameters, to substitute before running (nothing else here is a template):"
        );
        for (name, about) in &feed.parameters {
            let about = about.split_whitespace().collect::<Vec<_>>().join(" ");
            let _ = writeln!(out, "--   ${{{name}}}  {about}");
        }
    }
    let _ = writeln!(
        out,
        "-- Run with the contract's dataset as the job's default dataset, as 0001_init.sql is."
    );
    out.push('\n');
    // One timestamp for every row this run writes, so a load reads as one event.
    out.push_str("DECLARE loaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP();\n\n");
}

fn from_clause(c: &ClassLoad, edge: Option<&EdgeLoad>) -> String {
    let mut s = format!("FROM {}", c.from);
    if let Some(unnest) = edge.and_then(|e| e.unnest.as_deref()) {
        let _ = write!(s, ",\n  UNNEST({unnest})");
        if let Some(alias) = edge.and_then(|e| e.alias.as_deref()) {
            let _ = write!(s, " AS {alias}");
        }
    }
    if let Some(filter) = &c.filter {
        let _ = write!(s, "\nWHERE ({filter})");
    }
    s
}

/// Fail the job if any key in `_staged` occurs twice, naming it: `MERGE` would fail anyway,
/// without saying which row.
fn duplicate_check(out: &mut String, table: &str, key: &[&str]) {
    let cols: Vec<String> = key.iter().map(|k| format!("`{k}`")).collect();
    let shown: Vec<String> = key.iter().map(|k| format!("`{k}`")).collect();
    let _ = writeln!(
        out,
        "SELECT ERROR(FORMAT({}, {}))\nFROM _staged\nGROUP BY {}\nHAVING COUNT(*) > 1\nLIMIT 1;\n",
        string_literal(&format!(
            "{table}: landing holds {} more than once ({})",
            key.iter().map(|_| "%T").collect::<Vec<_>>().join(", "),
            shown.join(", ")
        )),
        cols.join(", "),
        cols.join(", "),
    );
}

fn merge(
    out: &mut String,
    feed: &Feed,
    table: &str,
    key: &[&str],
    set: &[&str],
    source: Option<&str>,
) {
    let origin = string_literal(&feed.name);
    let source = source.map_or("NULL".to_string(), string_literal);
    let on: Vec<String> = key.iter().map(|k| format!("T.`{k}` = S.`{k}`")).collect();
    let mut update: Vec<String> = set.iter().map(|c| format!("`{c}` = S.`{c}`")).collect();
    update.push("`_loaded_at` = loaded_at".into());
    update.push(format!("`_source` = {source}"));
    let mut insert: Vec<String> = key.iter().chain(set).map(|c| format!("`{c}`")).collect();
    insert.extend(["`_origin`", "`_loaded_at`", "`_source`"].map(String::from));
    let mut values: Vec<String> = key.iter().chain(set).map(|c| format!("S.`{c}`")).collect();
    values.extend([origin.clone(), "loaded_at".into(), source]);

    let _ = writeln!(out, "MERGE `{table}` AS T\nUSING _staged AS S");
    let _ = writeln!(out, "ON {} AND T.`_origin` = {origin}", on.join(" AND "));
    let _ = writeln!(out, "WHEN MATCHED THEN\n  UPDATE SET {}", update.join(", "));
    let _ = writeln!(
        out,
        "WHEN NOT MATCHED THEN\n  INSERT ({})\n  VALUES ({})",
        insert.join(", "),
        values.join(", ")
    );
    if feed.mode == Mode::Snapshot {
        let _ = writeln!(out, "{}", snapshot_delete(&feed.name));
    }
    out.truncate(out.trim_end().len());
    out.push_str(";\n");
}

fn class_script(feed: &Feed, c: &ClassLoad) -> String {
    let mut out = String::new();
    feed_header(
        &mut out,
        feed,
        &format!("class `{}` into `{}`", c.class, c.table),
    );

    // 1. Cast each expression to its contract type.
    let mut select = vec![format!("  CAST({} AS STRING) AS `id`", c.id)];
    for m in &c.columns {
        select.push(format!(
            "  CAST({} AS {}) AS `{}`",
            m.expr,
            type_name(m.ty),
            m.column
        ));
    }
    let _ = writeln!(
        out,
        "CREATE TEMP TABLE _staged AS\nSELECT\n{}\n{};\n",
        select.join(",\n"),
        from_clause(c, None)
    );
    duplicate_check(&mut out, &c.table, &["id"]);

    // 2. Fail on a value outside its declared set, naming it.
    for m in &c.columns {
        let allowed = if let Some(vocab) = &m.values {
            format!("(SELECT `value` FROM `{vocab}`)")
        } else if m.precision {
            "(\"year\", \"month\", \"day\")".to_string()
        } else {
            continue;
        };
        let _ = writeln!(
            out,
            "SELECT ERROR(FORMAT({}, v))\nFROM (SELECT DISTINCT `{col}` AS v FROM _staged)\n\
             WHERE v IS NOT NULL AND v NOT IN {allowed}\nLIMIT 1;\n",
            string_literal(&format!(
                "{}.{}: %T is not a declared value",
                c.table, m.column
            )),
            col = m.column,
        );
    }

    // 3. Merge on (id, _origin); 4. under snapshot, delete what landing no longer holds.
    let set: Vec<&str> = c.columns.iter().map(|m| m.column.as_str()).collect();
    merge(&mut out, feed, &c.table, &["id"], &set, c.source.as_deref());
    out
}

fn edge_script(feed: &Feed, c: &ClassLoad, e: &EdgeLoad) -> String {
    let mut out = String::new();
    feed_header(
        &mut out,
        feed,
        &format!(
            "edge `{}` of class `{}` into `{}`",
            e.relationship, c.class, e.table
        ),
    );
    // A row whose `dst_id` is null links to nothing, so it is not a link.
    let _ = writeln!(
        out,
        "CREATE TEMP TABLE _staged AS\nSELECT DISTINCT * FROM (\nSELECT\n  CAST({} AS STRING) AS \
         `src_id`,\n  CAST({} AS STRING) AS `dst_id`\n{}\n)\nWHERE `src_id` IS NOT NULL AND \
         `dst_id` IS NOT NULL;\n",
        c.id,
        e.dst_id,
        from_clause(c, Some(e))
    );
    merge(
        &mut out,
        feed,
        &e.table,
        &["src_id", "dst_id"],
        &[],
        c.source.as_deref(),
    );
    out
}

/// Every feed's load scripts, as output-relative path and text: one per mapped class and one
/// per mapped edge, under `feeds/<feed>/<table>.sql`.
pub(crate) fn render_feeds(feeds: &[Feed]) -> Vec<(String, String)> {
    let mut files = Vec::new();
    for feed in feeds {
        for c in &feed.classes {
            files.push((
                format!("feeds/{}/{}.sql", feed.name, c.table),
                class_script(feed, c),
            ));
            for e in &c.edges {
                files.push((
                    format!("feeds/{}/{}.sql", feed.name, e.table),
                    edge_script(feed, c, e),
                ));
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warehouse::contract::{derive, tests::class};
    use crate::warehouse::feeds::{resolve, tests as feed};

    fn cai(text: &str) -> Vec<(String, String)> {
        let feeds = resolve(
            &[feed::source("gcp-cai", text)],
            &feed::contract(),
            &feed::catalog(),
        )
        .unwrap();
        render_feeds(&feeds)
    }

    #[test]
    fn a_feed_writes_one_script_per_mapped_class_and_edge() {
        let paths: Vec<String> = cai(feed::CAI).into_iter().map(|(p, _)| p).collect();
        assert_eq!(
            paths,
            [
                "feeds/gcp-cai/compute_instance.sql",
                "feeds/gcp-cai/edge_compute_instance__attached_to.sql"
            ]
        );
    }

    #[test]
    fn a_snapshot_feed_deletes_and_an_append_feed_does_not() {
        for (_, sql) in cai(feed::CAI) {
            assert!(sql.contains("WHEN NOT MATCHED BY SOURCE AND T.`_origin` = \"gcp-cai\""));
        }
        let append = feed::CAI.replacen("mode: snapshot", "mode: append", 1);
        for (path, sql) in cai(&append) {
            assert!(!sql.contains("NOT MATCHED BY SOURCE"), "{path}");
            assert!(!sql.contains("DELETE"), "{path}");
        }
    }

    #[test]
    fn run_time_parameters_reach_the_sql_untouched() {
        let (_, sql) = &cai(feed::CAI)[0];
        assert!(
            sql.contains(
                "FROM `${project}.${landing_dataset}.cai_compute_googleapis_com_Instance`"
            ),
            "{sql}"
        );
        assert!(sql.contains("--   ${project}  The GCP project"), "{sql}");
        assert!(!sql.contains("${landing}"), "{sql}");
    }

    #[test]
    fn a_value_set_column_is_checked_against_its_vocab_table() {
        let (_, sql) = &cai(feed::CAI)[0];
        assert!(
            sql.contains(
                "WHERE v IS NOT NULL AND v NOT IN (SELECT `value` FROM \
                 `vocab_compute_instance__provider`)"
            ),
            "{sql}"
        );
    }

    #[test]
    fn a_string_literal_escapes_what_would_end_or_break_it() {
        assert_eq!(string_literal(r#"a "b" \c"#), r#""a \"b\" \\c""#);
        assert_eq!(string_literal("one\ntwo"), r#""one\ntwo""#);
    }

    #[test]
    fn each_contract_type_has_its_bigquery_name() {
        assert_eq!(type_name(ColumnType::String), "STRING");
        assert_eq!(type_name(ColumnType::Float), "FLOAT64");
        assert_eq!(type_name(ColumnType::Date), "DATE");
        assert_eq!(type_name(ColumnType::Timestamp), "TIMESTAMP");
    }

    #[test]
    fn the_init_script_carries_the_marker_on_its_first_line() {
        let sql = render_init(&derive(&[class("thing", "{}\n")]).unwrap());
        assert_eq!(sql.lines().next(), Some(format!("-- {MARKER}").as_str()));
    }

    #[test]
    fn a_required_column_is_not_null_and_the_key_is_not_enforced() {
        let sql = render_init(
            &derive(&[class(
                "thing",
                "properties:\n  - name: size\n    type: number\n    required: true\n    \
                 description: Disk size in \"GB\"\n",
            )])
            .unwrap(),
        );
        assert!(
            sql.contains(
                "  `size` FLOAT64 NOT NULL OPTIONS(description=\"Disk size in \\\"GB\\\"\"),"
            ),
            "{sql}"
        );
        assert!(
            sql.contains("  PRIMARY KEY (`id`, `_origin`) NOT ENFORCED\n"),
            "{sql}"
        );
    }

    #[test]
    fn a_vocab_seed_is_idempotent_by_construction() {
        let sql = render_init(
            &derive(&[class(
                "thing",
                "properties:\n  - name: s\n    type: string\n    values: [a, b]\n",
            )])
            .unwrap(),
        );
        assert!(
            sql.contains(
                "INSERT INTO `vocab_thing__s` (`value`)\nSELECT v FROM UNNEST([\"a\", \"b\"]) \
                 AS v\nWHERE v NOT IN (SELECT `value` FROM `vocab_thing__s`);"
            ),
            "{sql}"
        );
    }

    #[test]
    fn each_breaking_change_has_its_statement_or_its_reason_for_none() {
        use crate::warehouse::evolution::diff;
        let old = derive(&[
            class(
                "thing",
                "properties:\n  - name: a\n    type: string\n  - name: b\n    type: number\n\
                 \x20 - name: c\n    type: string\n    values: [x, y]\n",
            ),
            class("gone", "{}\n"),
        ])
        .unwrap();
        let new = derive(&[class(
            "thing",
            "properties:\n  - name: a\n    type: string\n    required: true\n\
             \x20 - name: c\n    type: string\n    values: [x]\n\
             \x20 - name: d\n    type: date\n    required: true\n",
        )])
        .unwrap();
        let d = diff(&old, &new);
        let sql = render_breaking("0002", &d.breaking);
        for want in [
            "-- column `thing.a` becomes required\n-- BigQuery cannot make an existing column NOT NULL",
            "ALTER TABLE `thing` ADD COLUMN IF NOT EXISTS `d` DATE",
            "ALTER TABLE `thing` DROP COLUMN IF EXISTS `b`;",
            "DELETE FROM `vocab_thing__c` WHERE `value` = \"y\";",
            "DROP TABLE IF EXISTS `gone`;",
        ] {
            assert!(sql.contains(want), "missing {want:?} in\n{sql}");
        }
        assert!(!sql.contains("`d` DATE NOT NULL"), "{sql}");
        assert!(sql.starts_with(&format!("-- {MARKER}")), "{sql}");
    }

    #[test]
    fn a_migration_creates_a_new_vocab_table_and_seeds_it() {
        use crate::warehouse::evolution::diff;
        let old = derive(&[class("thing", "{}\n")]).unwrap();
        let new = derive(&[class(
            "thing",
            "properties:\n  - name: s\n    type: string\n    values: [a]\n",
        )])
        .unwrap();
        let sql = render_migration("0002", &diff(&old, &new));
        assert!(
            sql.contains("ALTER TABLE `thing` ADD COLUMN IF NOT EXISTS `s` STRING;"),
            "{sql}"
        );
        assert!(
            sql.contains("CREATE TABLE IF NOT EXISTS `vocab_thing__s`"),
            "{sql}"
        );
        assert!(sql.contains("INSERT INTO `vocab_thing__s`"), "{sql}");
    }
}
