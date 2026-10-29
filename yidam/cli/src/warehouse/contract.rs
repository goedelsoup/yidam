//! RFC-0049 §2: the warehouse contract.
//!
//! One model, derived from the loaded classes and nothing else. Every renderer reads this and
//! not the ontology, so the DDL, the feed SQL and the `.tql` cannot disagree about a column.
//! It is dialect-free: a column has a contract type, and each dialect's renderer maps it.

use std::collections::{BTreeMap, BTreeSet};

use crate::corpus::{Class, ClassProperty};

/// A contract column's type. The dialect renderers map these and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ColumnType {
    String,
    /// yidam's `number` is a finite double, which a 64-bit float holds exactly.
    Float,
    Date,
    Timestamp,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Column {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: ColumnType,
    pub required: bool,
    /// The ontology property this column carries; absent on the provenance columns.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub property: Option<String>,
    /// The `vocab_` table holding this column's declared values.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub(crate) enum TableKind {
    Class {
        class: String,
    },
    /// A link table. `class` authors the link; `targets` are the classes its `dst_id` names.
    /// More than one target arrives when several classes each declare the same `in` edge.
    Edge {
        class: String,
        relationship: String,
        targets: BTreeSet<String>,
    },
    Vocab {
        class: String,
        property: String,
        values: Vec<String>,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Table {
    pub name: String,
    #[serde(flatten)]
    pub kind: TableKind,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub description: String,
    pub columns: Vec<Column>,
    pub primary_key: Vec<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct Contract {
    /// Class tables by class name, then link tables, then value-set tables, each sorted, so
    /// two derivations of one ontology are byte-identical whatever order the files walked in.
    pub tables: Vec<Table>,
}

/// The provenance columns every class table opens with (§2.1), in order.
pub(crate) const PROVENANCE: &[(&str, ColumnType, bool, &str)] = &[
    (
        "id",
        ColumnType::String,
        true,
        "The row's identity within its origin: a node reference for `corpus`, the feed's `id:` \
         expression otherwise.",
    ),
    (
        "_origin",
        ColumnType::String,
        true,
        "The feed that wrote the row: `corpus`, or a feed's name.",
    ),
    (
        "_loaded_at",
        ColumnType::Timestamp,
        true,
        "When the feed SQL wrote the row.",
    ),
    (
        "_source",
        ColumnType::String,
        false,
        "The catalog slug the row's facts came from, when known.",
    ),
    (
        "_corpus_commit",
        ColumnType::String,
        false,
        "The commit a `corpus` row was exported from; null for every other origin.",
    ),
];

/// Column-name prefixes BigQuery reserves. A property whose column would start with one is a
/// refusal here rather than a `CREATE TABLE` that fails at the other end.
const RESERVED_PREFIXES: &[&str] = &[
    "_table_",
    "_file_",
    "_partition",
    "_row_timestamp",
    "__root__",
    "_colidentifier",
];

/// The suffix of the column that carries a `date` property's precision (§2.5).
pub(crate) const PRECISION_SUFFIX: &str = "_precision";

/// The warehouse identifier for an ontology name: every character outside `[A-Za-z0-9_]`
/// becomes `_`.
///
/// Class and property stems are kebab-case, but a relationship is free text, and real corpora
/// write `exposed at` and `rooted in`. Mapping only `-` refused those. The mapping can merge
/// two names, which is why [`derive`] refuses any two that land on one identifier.
pub(crate) fn ident(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn is_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Derive the contract, or refuse with every reason at once.
///
/// A refusal names the class and the declaration. The contract is the schema of record for
/// tables that do not exist yet, so anything it cannot place exactly is the author's to
/// settle, not this function's to guess.
pub(crate) fn derive(classes: &[Class]) -> Result<Contract, Vec<String>> {
    let mut refusals = Vec::new();
    let mut sorted: Vec<&Class> = classes.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let declared: BTreeSet<&str> = sorted.iter().map(|c| c.name.as_str()).collect();

    for class in &sorted {
        if let Some(why) = &class.malformed {
            refusals.push(format!(
                "{}: the class file does not parse ({why})",
                class.rel
            ));
        }
    }

    let mut class_tables = Vec::new();
    let mut vocab_tables = Vec::new();
    for class in &sorted {
        let (table, vocabs) = class_table(class, &mut refusals);
        class_tables.push(table);
        vocab_tables.extend(vocabs);
    }
    let edge_tables = edge_tables(&sorted, &declared, &mut refusals);

    let mut tables = class_tables;
    tables.extend(edge_tables);
    vocab_tables.sort_by(|a, b| a.name.cmp(&b.name));
    tables.extend(vocab_tables);

    // Table names are compared exactly; two stems that differ only by `-`/`_` collide here.
    let mut seen: BTreeMap<&str, &Table> = BTreeMap::new();
    for table in &tables {
        if !is_identifier(&table.name) {
            refusals.push(format!(
                "`{}` is not a warehouse identifier (from {})",
                table.name,
                origin(table)
            ));
        }
        if let Some(prev) = seen.insert(&table.name, table) {
            refusals.push(format!(
                "{} and {} both become table `{}`",
                origin(prev),
                origin(table),
                table.name
            ));
        }
    }

    if refusals.is_empty() {
        Ok(Contract { tables })
    } else {
        Err(refusals)
    }
}

/// What a table was derived from, in the ontology's own words.
fn origin(table: &Table) -> String {
    match &table.kind {
        TableKind::Class { class } => format!("class `{class}`"),
        TableKind::Edge {
            class,
            relationship,
            ..
        } => format!("edge `{relationship}` authored by `{class}`"),
        TableKind::Vocab {
            class, property, ..
        } => format!("the values of `{class}.{property}`"),
    }
}

fn class_table(class: &Class, refusals: &mut Vec<String>) -> (Table, Vec<Table>) {
    let table_name = ident(&class.name);
    let mut columns: Vec<Column> = PROVENANCE
        .iter()
        .map(|(name, ty, required, description)| Column {
            name: (*name).to_string(),
            ty: *ty,
            required: *required,
            property: None,
            values: None,
            description: (*description).to_string(),
        })
        .collect();
    let mut vocabs = Vec::new();

    for property in &class.properties {
        let at = format!("{}: property `{}`", class.rel, property.name);
        if property.name.trim().is_empty() {
            refusals.push(format!("{}: a property has no name", class.rel));
            continue;
        }
        let column = ident(&property.name);
        let string_family = matches!(property.r#type.as_str(), "string" | "text" | "ref");
        if !property.values.is_empty() && !string_family {
            refusals.push(format!(
                "{at} declares `values:` on type `{}`; the contract carries a value set only on \
                 `string`, `text` or `ref`",
                property.r#type
            ));
            continue;
        }
        match property.r#type.as_str() {
            "string" | "text" | "ref" => {
                let values = (!property.values.is_empty()).then(|| {
                    let name = format!("vocab_{table_name}__{column}");
                    vocabs.push(vocab_table(class, property, &name));
                    name
                });
                columns.push(property_column(
                    property,
                    &column,
                    ColumnType::String,
                    values,
                ));
            }
            "number" => columns.push(property_column(property, &column, ColumnType::Float, None)),
            "date" => {
                columns.push(property_column(property, &column, ColumnType::Date, None));
                columns.push(Column {
                    name: format!("{column}{PRECISION_SUFFIX}"),
                    ty: ColumnType::String,
                    required: property.required(),
                    property: Some(property.name.clone()),
                    values: None,
                    description: format!(
                        "How much of `{column}` was stated: `year`, `month` or `day`. `{column}` \
                         holds the first day of that interval."
                    ),
                });
            }
            // Prose and standing, not facts: the Markdown layer carries them (§2.1). A list
            // is several values and a column holds one; a child table is RFC-0049's to
            // design, and until it is, the list is carried where the claims are (RFC-0050).
            "claim" | "quotation" | "list" => {}
            "" => refusals.push(format!(
                "{at} declares no `type:`; the contract cannot choose a column type for it"
            )),
            other => refusals.push(format!(
                "{at} has type `{other}`, which the contract has no column type for"
            )),
        }
    }

    let mut seen: BTreeMap<String, &str> = BTreeMap::new();
    for column in &columns {
        // BigQuery column names are case-insensitive, so the comparison is too.
        let folded = column.name.to_ascii_lowercase();
        let source = column.property.as_deref().unwrap_or("a provenance column");
        if !is_identifier(&column.name) {
            refusals.push(format!(
                "{}: `{}` is not a warehouse column name (from {source})",
                class.rel, column.name
            ));
        }
        if column.property.is_some() {
            if let Some(prefix) = RESERVED_PREFIXES.iter().find(|p| folded.starts_with(*p)) {
                refusals.push(format!(
                    "{}: column `{}` (from `{source}`) starts with `{prefix}`, which BigQuery \
                     reserves",
                    class.rel, column.name
                ));
            }
        }
        if let Some(prev) = seen.insert(folded, source) {
            refusals.push(format!(
                "{}: `{prev}` and `{source}` both become column `{}`",
                class.rel, column.name
            ));
        }
    }

    let description = if class.description.trim().is_empty() {
        class.label.trim().to_string()
    } else {
        class.description.trim().to_string()
    };
    let table = Table {
        name: table_name,
        kind: TableKind::Class {
            class: class.name.clone(),
        },
        description,
        columns,
        primary_key: vec!["id".into(), "_origin".into()],
    };
    (table, vocabs)
}

fn property_column(
    property: &ClassProperty,
    column: &str,
    ty: ColumnType,
    values: Option<String>,
) -> Column {
    Column {
        name: column.to_string(),
        ty,
        required: property.required(),
        property: Some(property.name.clone()),
        values,
        description: property.description.trim().to_string(),
    }
}

fn vocab_table(class: &Class, property: &ClassProperty, name: &str) -> Table {
    let mut values = Vec::new();
    for v in &property.values {
        if !values.contains(v) {
            values.push(v.clone());
        }
    }
    Table {
        name: name.to_string(),
        kind: TableKind::Vocab {
            class: class.name.clone(),
            property: property.name.clone(),
            values,
        },
        description: format!(
            "The values `{}.{}` declares. A feed whose rows hold any other value fails.",
            class.name, property.name
        ),
        columns: vec![Column {
            name: "value".into(),
            ty: ColumnType::String,
            required: true,
            property: None,
            values: None,
            description: String::new(),
        }],
        primary_key: vec!["value".into()],
    }
}

/// One link table per `(authoring class, relationship)` (§2.3).
///
/// An `in` edge declared on `c` with target `t` is authored by `t`'s instances, which is how
/// [`crate::corpus::source_classes`] reads `direction:`, so it lands on `t`'s link table. When
/// `t` also declares the `out` side, the two declarations are one table, not two.
fn edge_tables(
    classes: &[&Class],
    declared: &BTreeSet<&str>,
    refusals: &mut Vec<String>,
) -> Vec<Table> {
    let mut links: BTreeMap<(String, String), (BTreeSet<String>, String)> = BTreeMap::new();
    for class in classes {
        for edge in &class.edges {
            if edge.relationship.trim().is_empty() || edge.target.trim().is_empty() {
                refusals.push(format!(
                    "{}: an edge needs both `relationship:` and `target:`",
                    class.rel
                ));
                continue;
            }
            let (author, target) = match edge.direction.as_deref() {
                Some("in") => (edge.target.as_str(), class.name.as_str()),
                _ => (class.name.as_str(), edge.target.as_str()),
            };
            if !declared.contains(edge.target.as_str()) {
                refusals.push(format!(
                    "{}: edge `{}` targets `{}`, which no `.ont.yml` declares",
                    class.rel, edge.relationship, edge.target
                ));
                continue;
            }
            let entry = links
                .entry((author.to_string(), edge.relationship.clone()))
                .or_default();
            entry.0.insert(target.to_string());
            if entry.1.is_empty() {
                entry.1 = edge.description.trim().to_string();
            }
        }
    }

    links
        .into_iter()
        .map(|((class, relationship), (targets, description))| {
            let column = |name: &str, ty, required, description: &str| Column {
                name: name.into(),
                ty,
                required,
                property: None,
                values: None,
                description: description.into(),
            };
            Table {
                name: format!("edge_{}__{}", ident(&class), ident(&relationship)),
                columns: vec![
                    column(
                        "src_id",
                        ColumnType::String,
                        true,
                        "The `id` of the authoring row.",
                    ),
                    column(
                        "dst_id",
                        ColumnType::String,
                        true,
                        "The `id` of the row linked to.",
                    ),
                    column("_origin", ColumnType::String, true, PROVENANCE[1].3),
                    column("_loaded_at", ColumnType::Timestamp, true, PROVENANCE[2].3),
                    column("_source", ColumnType::String, false, PROVENANCE[3].3),
                ],
                primary_key: vec!["src_id".into(), "dst_id".into(), "_origin".into()],
                kind: TableKind::Edge {
                    class,
                    relationship,
                    targets,
                },
                description,
            }
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn class(name: &str, yaml: &str) -> Class {
        Class::parse(format!(".yidam/corpus/{name}.ont.yml"), yaml)
    }

    fn table<'a>(c: &'a Contract, name: &str) -> &'a Table {
        c.tables
            .iter()
            .find(|t| t.name == name)
            .unwrap_or_else(|| panic!("no table `{name}`"))
    }

    fn column<'a>(t: &'a Table, name: &str) -> &'a Column {
        t.columns
            .iter()
            .find(|c| c.name == name)
            .unwrap_or_else(|| panic!("no column `{name}` on `{}`", t.name))
    }

    fn refusals(classes: &[Class]) -> Vec<String> {
        derive(classes).expect_err("expected a refusal")
    }

    fn one_property(ty: &str) -> Contract {
        derive(&[class(
            "thing",
            &format!("properties:\n  - name: p\n    type: {ty}\n"),
        )])
        .unwrap()
    }

    #[test]
    fn every_class_table_opens_with_the_provenance_columns_and_is_keyed_on_id_and_origin() {
        let c = derive(&[class("compute-instance", "properties: []\n")]).unwrap();
        let t = table(&c, "compute_instance");
        let names: Vec<&str> = t.columns.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(
            names,
            ["id", "_origin", "_loaded_at", "_source", "_corpus_commit"]
        );
        assert_eq!(t.primary_key, ["id", "_origin"]);
        assert!(column(t, "id").required && column(t, "_origin").required);
        assert!(!column(t, "_source").required);
        assert_eq!(column(t, "_loaded_at").ty, ColumnType::Timestamp);
    }

    #[test]
    fn string_text_and_ref_become_string() {
        for ty in ["string", "text", "ref"] {
            let c = one_property(ty);
            assert_eq!(
                column(table(&c, "thing"), "p").ty,
                ColumnType::String,
                "{ty}"
            );
        }
    }

    #[test]
    fn number_becomes_float() {
        assert_eq!(
            column(table(&one_property("number"), "thing"), "p").ty,
            ColumnType::Float
        );
    }

    #[test]
    fn a_date_becomes_a_date_and_its_precision() {
        let c = derive(&[class(
            "thing",
            "properties:\n  - name: built\n    type: date\n    required: true\n",
        )])
        .unwrap();
        let t = table(&c, "thing");
        assert_eq!(column(t, "built").ty, ColumnType::Date);
        let precision = column(t, "built_precision");
        assert_eq!(precision.ty, ColumnType::String);
        assert_eq!(precision.property.as_deref(), Some("built"));
        assert!(
            precision.required,
            "a required date's precision is required too"
        );
    }

    #[test]
    fn claims_quotations_and_lists_get_no_column() {
        // `list` was a refusal until RFC-0050 read the type: `demi-moore`'s coined one
        // failed the whole contract over two properties a column cannot hold anyway.
        for ty in ["claim", "quotation", "list"] {
            let t = table(&one_property(ty), "thing").clone();
            assert!(t.columns.iter().all(|c| c.name != "p"), "{ty}");
            assert_eq!(t.columns.len(), PROVENANCE.len(), "{ty}");
        }
    }

    #[test]
    fn required_is_carried_and_absent_means_nullable() {
        let c = derive(&[class(
            "thing",
            "properties:\n  - name: a\n    type: string\n    required: true\n\
             \x20 - name: b\n    type: string\n    required: false\n\
             \x20 - name: c\n    type: string\n",
        )])
        .unwrap();
        let t = table(&c, "thing");
        assert!(column(t, "a").required);
        assert!(!column(t, "b").required);
        assert!(!column(t, "c").required);
    }

    #[test]
    fn a_value_set_becomes_a_vocab_table_the_column_names() {
        let c = derive(&[class(
            "compute-instance",
            "properties:\n  - name: power-state\n    type: string\n    \
             values: [running, stopped, running]\n",
        )])
        .unwrap();
        let col = column(table(&c, "compute_instance"), "power_state");
        assert_eq!(
            col.values.as_deref(),
            Some("vocab_compute_instance__power_state")
        );
        let vocab = table(&c, "vocab_compute_instance__power_state");
        match &vocab.kind {
            TableKind::Vocab { values, .. } => assert_eq!(values, &["running", "stopped"]),
            other => panic!("{other:?}"),
        }
        assert_eq!(vocab.primary_key, ["value"]);
    }

    #[test]
    fn a_value_set_on_a_number_is_refused() {
        let r = refusals(&[class(
            "thing",
            "properties:\n  - name: p\n    type: number\n    values: ['1']\n",
        )]);
        assert!(r[0].contains("`values:` on type `number`"), "{r:?}");
    }

    #[test]
    fn an_untyped_or_unknown_property_is_refused_by_name() {
        let r = refusals(&[class("thing", "properties:\n  - name: p\n")]);
        assert!(r[0].contains("property `p` declares no `type:`"), "{r:?}");
        let r = refusals(&[class("thing", "properties:\n  - name: p\n    type: blob\n")]);
        assert!(r[0].contains("type `blob`"), "{r:?}");
    }

    #[test]
    fn two_stems_that_collide_after_the_hyphen_swap_are_refused_naming_both() {
        let r = refusals(&[class("a-b", "{}\n"), class("a_b", "{}\n")]);
        assert_eq!(r.len(), 1, "{r:?}");
        assert!(
            r[0].contains("`a-b`") && r[0].contains("`a_b`") && r[0].contains("table `a_b`"),
            "{r:?}"
        );
        let r = refusals(&[class(
            "thing",
            "properties:\n  - name: x-y\n    type: string\n  - name: X_y\n    type: string\n",
        )]);
        assert!(r[0].contains("`x-y` and `X_y`"), "{r:?}");
    }

    #[test]
    fn a_property_cannot_take_a_provenance_column_or_a_reserved_prefix() {
        let r = refusals(&[class(
            "thing",
            "properties:\n  - name: id\n    type: string\n",
        )]);
        assert!(r[0].contains("`a provenance column` and `id`"), "{r:?}");
        let r = refusals(&[class(
            "thing",
            "properties:\n  - name: _partition-key\n    type: string\n",
        )]);
        assert!(r.iter().any(|m| m.contains("`_partition`")), "{r:?}");
        // A date's precision column is a column like any other.
        let r = refusals(&[class(
            "thing",
            "properties:\n  - name: built\n    type: date\n  - name: built_precision\n    \
             type: string\n",
        )]);
        assert!(r[0].contains("`built` and `built_precision`"), "{r:?}");
    }

    #[test]
    fn a_relationship_written_as_words_becomes_an_identifier() {
        let c = derive(&[
            class(
                "formation",
                "edges:\n  - relationship: exposed at\n    target: site\n",
            ),
            class("site", "{}\n"),
        ])
        .unwrap();
        table(&c, "edge_formation__exposed_at");
        let r = refusals(&[class(
            "a",
            "edges:\n  - relationship: runs on\n    target: a\n  - relationship: runs-on\n    \
                 target: a\n",
        )]);
        assert!(
            r[0].contains("both become table `edge_a__runs_on`"),
            "{r:?}"
        );
    }

    #[test]
    fn an_identifier_must_start_with_a_letter_or_underscore() {
        let r = refusals(&[class("3d-model", "{}\n")]);
        assert!(
            r[0].contains("`3d_model` is not a warehouse identifier"),
            "{r:?}"
        );
    }

    #[test]
    fn an_out_edge_is_a_link_table_on_its_declaring_class() {
        let c = derive(&[
            class(
                "instance",
                "edges:\n  - relationship: attached-to\n    target: network\n    direction: out\n",
            ),
            class("network", "{}\n"),
        ])
        .unwrap();
        let t = table(&c, "edge_instance__attached_to");
        assert_eq!(t.primary_key, ["src_id", "dst_id", "_origin"]);
        match &t.kind {
            TableKind::Edge { class, targets, .. } => {
                assert_eq!(class, "instance");
                assert_eq!(targets.iter().collect::<Vec<_>>(), ["network"]);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_in_edge_is_written_once_on_the_class_that_authors_it() {
        // Both sides declare the link; one table results, named for the author.
        let c = derive(&[
            class(
                "instance",
                "edges:\n  - relationship: attached-to\n    target: network\n    direction: out\n",
            ),
            class(
                "network",
                "edges:\n  - relationship: attached-to\n    target: instance\n    direction: in\n",
            ),
        ])
        .unwrap();
        let edges: Vec<&str> = c
            .tables
            .iter()
            .filter(|t| matches!(t.kind, TableKind::Edge { .. }))
            .map(|t| t.name.as_str())
            .collect();
        assert_eq!(edges, ["edge_instance__attached_to"]);
        // Declared only as `in`, it still lands on the author.
        let c = derive(&[
            class("instance", "{}\n"),
            class(
                "network",
                "edges:\n  - relationship: attached-to\n    target: instance\n    direction: in\n",
            ),
        ])
        .unwrap();
        match &table(&c, "edge_instance__attached_to").kind {
            TableKind::Edge { targets, .. } => {
                assert_eq!(targets.iter().collect::<Vec<_>>(), ["network"])
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_edge_to_an_undeclared_class_is_refused() {
        let r = refusals(&[class(
            "instance",
            "edges:\n  - relationship: runs-on\n    target: host\n",
        )]);
        assert!(
            r[0].contains("targets `host`, which no `.ont.yml` declares"),
            "{r:?}"
        );
    }

    #[test]
    fn the_order_does_not_depend_on_the_order_classes_arrive_in() {
        let make = || {
            vec![
                class(
                    "b",
                    "properties:\n  - name: s\n    type: string\n    values: [x]\n\
                     edges:\n  - relationship: r\n    target: a\n",
                ),
                class(
                    "a",
                    "properties:\n  - name: s\n    type: string\n    values: [y]\n",
                ),
            ]
        };
        let forward = derive(&make()).unwrap();
        let mut reversed = make();
        reversed.reverse();
        let backward = derive(&reversed).unwrap();
        let names = |c: &Contract| c.tables.iter().map(|t| t.name.clone()).collect::<Vec<_>>();
        assert_eq!(names(&forward), names(&backward));
        assert_eq!(
            names(&forward),
            ["a", "b", "edge_b__r", "vocab_a__s", "vocab_b__s"]
        );
    }
}
