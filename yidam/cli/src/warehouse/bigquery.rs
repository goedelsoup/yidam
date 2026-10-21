//! RFC-0049 §2 in BigQuery's dialect: the DDL that creates the contract's tables.

use std::fmt::Write as _;

use super::contract::{Column, ColumnType, Contract, Table, TableKind};
use super::MARKER;

fn type_name(ty: ColumnType) -> &'static str {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::warehouse::contract::{derive, tests::class};

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
}
