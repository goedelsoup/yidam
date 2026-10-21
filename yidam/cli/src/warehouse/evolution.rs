//! RFC-0049 §6: what changed between the contract the last export committed and this one.
//!
//! An additive change becomes a migration a pipeline can run unattended. A breaking change is
//! refused, because the tables it would alter hold rows somebody loaded: dropping a column
//! the ontology no longer declares is a decision about that data, and not the export's.
//!
//! A rename arrives here as a removal and an addition, since the ontology records no class or
//! property rename. That is why a rename is refused rather than guessed: the two halves are
//! reported together, and the author decides whether they are one change.

use std::collections::BTreeMap;

use super::contract::{Column, ColumnType, Contract, Table, TableKind};

/// A change a migration applies without touching a loaded row.
#[derive(Debug, Clone)]
pub(crate) enum Additive {
    /// A table the last contract did not have; a vocab table is seeded with its values.
    CreateTable(Table),
    AddColumn {
        table: String,
        column: Column,
    },
    /// A column that was required no longer is.
    DropNotNull {
        table: String,
        column: String,
    },
    AddValues {
        table: String,
        values: Vec<String>,
    },
}

/// A change that alters or discards what a table already holds.
#[derive(Debug, Clone)]
pub(crate) enum Breaking {
    DropTable {
        table: String,
    },
    DropColumn {
        table: String,
        column: String,
    },
    TypeChange {
        table: String,
        column: String,
        from: ColumnType,
        to: ColumnType,
    },
    /// A column that was optional is now required. Existing rows may hold nulls.
    Tightened {
        table: String,
        column: String,
    },
    /// A new column that is required on a table that already exists. BigQuery cannot add a
    /// `NOT NULL` column to a table, and the rows already there would have no value for it.
    RequiredAdded {
        table: String,
        column: Column,
    },
    ValueRemoved {
        table: String,
        value: String,
    },
}

impl Additive {
    /// The change as one line of the export's report.
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::CreateTable(t) => format!("table `{}` added", t.name),
            Self::AddColumn { table, column } => {
                format!("column `{table}.{}` added", column.name)
            }
            Self::DropNotNull { table, column } => {
                format!("column `{table}.{column}` is no longer required")
            }
            Self::AddValues { table, values } => {
                format!("`{table}` gains {}", quoted(values))
            }
        }
    }

    fn slug(&self) -> String {
        match self {
            Self::CreateTable(t) => format!("create_{}", t.name),
            Self::AddColumn { table, column } => format!("add_{table}__{}", column.name),
            Self::DropNotNull { table, column } => format!("optional_{table}__{column}"),
            Self::AddValues { table, .. } => format!("values_{table}"),
        }
    }
}

impl Breaking {
    pub(crate) fn describe(&self) -> String {
        match self {
            Self::DropTable { table } => format!("table `{table}` removed"),
            Self::DropColumn { table, column } => format!("column `{table}.{column}` removed"),
            Self::TypeChange {
                table,
                column,
                from,
                to,
            } => format!(
                "column `{table}.{column}` changes type from {} to {}",
                super::bigquery::type_name(*from),
                super::bigquery::type_name(*to)
            ),
            Self::Tightened { table, column } => {
                format!("column `{table}.{column}` becomes required")
            }
            Self::RequiredAdded { table, column } => format!(
                "column `{table}.{}` is added as required to a table that already has rows",
                column.name
            ),
            Self::ValueRemoved { table, value } => {
                format!("value `{value}` removed from `{table}`")
            }
        }
    }
}

fn quoted(values: &[String]) -> String {
    values
        .iter()
        .map(|v| format!("`{v}`"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[derive(Debug, Default)]
pub(crate) struct Diff {
    pub additive: Vec<Additive>,
    pub breaking: Vec<Breaking>,
}

impl Diff {
    /// The migration's file-name slug: the change, when there is one, and `evolve` otherwise.
    pub(crate) fn slug(&self) -> String {
        match self.additive.as_slice() {
            [one] => one.slug(),
            _ => "evolve".into(),
        }
    }
}

/// Compare `old` (the committed contract) with `new`, table by table in `new`'s order and
/// then the tables only `old` has, so two runs over one pair report in one order.
///
/// Descriptions and a link table's targets are not schema: neither is compared.
pub(crate) fn diff(old: &Contract, new: &Contract) -> Diff {
    let before: BTreeMap<&str, &Table> = old.tables.iter().map(|t| (t.name.as_str(), t)).collect();
    let after: BTreeMap<&str, &Table> = new.tables.iter().map(|t| (t.name.as_str(), t)).collect();
    let mut d = Diff::default();

    for table in &new.tables {
        let Some(prev) = before.get(table.name.as_str()) else {
            d.additive.push(Additive::CreateTable(table.clone()));
            continue;
        };
        columns(prev, table, &mut d);
        values(prev, table, &mut d);
    }
    for table in &old.tables {
        if !after.contains_key(table.name.as_str()) {
            d.breaking.push(Breaking::DropTable {
                table: table.name.clone(),
            });
        }
    }
    d
}

fn columns(prev: &Table, table: &Table, d: &mut Diff) {
    let before: BTreeMap<&str, &Column> =
        prev.columns.iter().map(|c| (c.name.as_str(), c)).collect();
    let name = || table.name.clone();
    for column in &table.columns {
        match before.get(column.name.as_str()) {
            None if column.required => d.breaking.push(Breaking::RequiredAdded {
                table: name(),
                column: column.clone(),
            }),
            None => d.additive.push(Additive::AddColumn {
                table: name(),
                column: column.clone(),
            }),
            Some(old) => {
                if old.ty != column.ty {
                    d.breaking.push(Breaking::TypeChange {
                        table: name(),
                        column: column.name.clone(),
                        from: old.ty,
                        to: column.ty,
                    });
                }
                match (old.required, column.required) {
                    (false, true) => d.breaking.push(Breaking::Tightened {
                        table: name(),
                        column: column.name.clone(),
                    }),
                    (true, false) => d.additive.push(Additive::DropNotNull {
                        table: name(),
                        column: column.name.clone(),
                    }),
                    _ => {}
                }
            }
        }
    }
    for old in &prev.columns {
        if !table.columns.iter().any(|c| c.name == old.name) {
            d.breaking.push(Breaking::DropColumn {
                table: name(),
                column: old.name.clone(),
            });
        }
    }
}

fn values(prev: &Table, table: &Table, d: &mut Diff) {
    let (TableKind::Vocab { values: before, .. }, TableKind::Vocab { values: after, .. }) =
        (&prev.kind, &table.kind)
    else {
        return;
    };
    let added: Vec<String> = after
        .iter()
        .filter(|v| !before.contains(v))
        .cloned()
        .collect();
    if !added.is_empty() {
        d.additive.push(Additive::AddValues {
            table: table.name.clone(),
            values: added,
        });
    }
    for v in before.iter().filter(|v| !after.contains(v)) {
        d.breaking.push(Breaking::ValueRemoved {
            table: table.name.clone(),
            value: v.clone(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::Class;
    use crate::warehouse::contract::derive;

    fn contract(classes: &[(&str, &str)]) -> Contract {
        let parsed: Vec<Class> = classes
            .iter()
            .map(|(n, y)| Class::parse(format!(".yidam/corpus/{n}.ont.yml"), *y))
            .collect();
        derive(&parsed).unwrap()
    }

    const BASE: &str = "properties:\n\
                        \x20 - name: provider\n    type: string\n    values: [gcp, aws]\n\
                        \x20 - name: zone\n    type: string\n\
                        \x20 - name: vcpus\n    type: number\n";

    fn base() -> Contract {
        contract(&[("instance", BASE)])
    }

    fn changed(yaml: &str) -> Diff {
        diff(&base(), &contract(&[("instance", yaml)]))
    }

    fn described(d: &Diff) -> (Vec<String>, Vec<String>) {
        (
            d.additive.iter().map(Additive::describe).collect(),
            d.breaking.iter().map(Breaking::describe).collect(),
        )
    }

    #[test]
    fn no_change_is_an_empty_diff() {
        let d = diff(&base(), &base());
        assert!(d.additive.is_empty() && d.breaking.is_empty(), "{d:?}");
    }

    #[test]
    fn a_description_is_not_schema() {
        let d = changed(&format!("description: now described\n{BASE}"));
        assert!(d.additive.is_empty() && d.breaking.is_empty(), "{d:?}");
    }

    #[test]
    fn an_added_class_creates_its_table() {
        let d = diff(
            &base(),
            &contract(&[("instance", BASE), ("network", "{}\n")]),
        );
        assert_eq!(described(&d).0, ["table `network` added"]);
        assert!(d.breaking.is_empty());
    }

    #[test]
    fn an_added_property_adds_its_column() {
        let d = changed(&format!("{BASE}  - name: region\n    type: string\n"));
        assert_eq!(described(&d).0, ["column `instance.region` added"]);
    }

    #[test]
    fn an_added_edge_creates_its_link_table() {
        let d = diff(
            &contract(&[("instance", BASE), ("network", "{}\n")]),
            &contract(&[
                (
                    "instance",
                    &format!("{BASE}edges:\n  - relationship: attached-to\n    target: network\n"),
                ),
                ("network", "{}\n"),
            ]),
        );
        assert_eq!(
            described(&d).0,
            ["table `edge_instance__attached_to` added"]
        );
    }

    #[test]
    fn an_added_value_is_inserted() {
        let d = changed(&BASE.replace("[gcp, aws]", "[gcp, aws, azure]"));
        assert_eq!(
            described(&d).0,
            ["`vocab_instance__provider` gains `azure`"]
        );
        assert!(d.breaking.is_empty());
    }

    #[test]
    fn a_loosened_requirement_drops_not_null() {
        let required = BASE.replace(
            "name: zone\n    type: string\n",
            "name: zone\n    type: string\n    required: true\n",
        );
        let d = diff(&contract(&[("instance", &required)]), &base());
        assert_eq!(
            described(&d).0,
            ["column `instance.zone` is no longer required"]
        );
        assert!(d.breaking.is_empty());
    }

    #[test]
    fn a_removed_class_is_breaking() {
        let d = diff(
            &contract(&[("instance", BASE), ("network", "{}\n")]),
            &base(),
        );
        assert_eq!(described(&d).1, ["table `network` removed"]);
    }

    #[test]
    fn a_removed_property_is_breaking() {
        let d = changed(&BASE.replace("  - name: vcpus\n    type: number\n", ""));
        assert_eq!(described(&d).1, ["column `instance.vcpus` removed"]);
    }

    #[test]
    fn a_rename_is_reported_as_both_its_halves() {
        let d = changed(&BASE.replace("name: zone", "name: location"));
        let (added, broken) = described(&d);
        assert_eq!(added, ["column `instance.location` added"]);
        assert_eq!(broken, ["column `instance.zone` removed"]);
    }

    #[test]
    fn a_type_change_is_breaking() {
        let d = changed(&BASE.replace("type: number", "type: string"));
        assert_eq!(
            described(&d).1,
            ["column `instance.vcpus` changes type from FLOAT64 to STRING"]
        );
    }

    #[test]
    fn a_tightened_requirement_is_breaking() {
        let d = changed(&BASE.replace(
            "name: zone\n    type: string\n",
            "name: zone\n    type: string\n    required: true\n",
        ));
        assert_eq!(described(&d).1, ["column `instance.zone` becomes required"]);
        assert!(d.additive.is_empty());
    }

    #[test]
    fn a_removed_value_is_breaking() {
        let d = changed(&BASE.replace("[gcp, aws]", "[gcp]"));
        assert_eq!(
            described(&d).1,
            ["value `aws` removed from `vocab_instance__provider`"]
        );
    }

    #[test]
    fn a_required_property_added_to_an_existing_class_is_breaking() {
        let d = changed(&format!(
            "{BASE}  - name: region\n    type: string\n    required: true\n"
        ));
        assert!(d.additive.is_empty(), "{d:?}");
        assert_eq!(
            described(&d).1,
            ["column `instance.region` is added as required to a table that already has rows"]
        );
    }

    #[test]
    fn the_slug_names_a_single_change_and_evolve_otherwise() {
        let one = changed(&format!("{BASE}  - name: region\n    type: string\n"));
        assert_eq!(one.slug(), "add_instance__region");
        let two = changed(&format!(
            "{BASE}  - name: region\n    type: string\n  - name: os\n    type: string\n"
        ));
        assert_eq!(two.slug(), "evolve");
    }
}
