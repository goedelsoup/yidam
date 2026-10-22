//! RFC-0049 §3.4: the corpus as one more feed.
//!
//! `--with-corpus` reads the nodes and writes, per contract table, the rows they become as
//! NDJSON beside a BigQuery schema file. The SQL that merges them is the feed SQL's, so the
//! corpus takes the same checks as any other origin. A node the contract cannot place exactly
//! is a refusal naming it, since a row that fails at `bq load` names nothing.

use std::collections::{BTreeMap, BTreeSet};

use super::contract::{ColumnType, Contract, Table, TableKind, PRECISION_SUFFIX};
use super::feeds::CORPUS_ORIGIN;
use crate::cmd::lint::checks::{
    declared_value_violation, iso_date_parts, numeric_value, property_type_violation,
};
use crate::corpus::{Class, ClassProperty};
use crate::model::{file_stem, resolve_link_target, DomainModel};
use crate::parse::CorpusInstance;

/// The columns a staged row carries: every contract column but the two the `MERGE` writes.
pub(crate) fn staged_columns(table: &Table) -> impl Iterator<Item = &super::contract::Column> {
    table
        .columns
        .iter()
        .filter(|c| c.name != "_origin" && c.name != "_loaded_at")
}

/// One table's rows, each in [`staged_columns`] order.
pub(crate) struct TableRows<'a> {
    pub table: &'a Table,
    pub rows: Vec<Vec<serde_json::Value>>,
}

/// What the corpus becomes: rows for every class and link table, and what was left out.
pub(crate) struct CorpusRows<'a> {
    pub tables: Vec<TableRows<'a>>,
    /// The commit the rows were read at, which every class row carries.
    pub commit: String,
    /// What no table holds, one line per reason, with a count. Reported, never refused.
    pub skipped: Vec<String>,
}

/// The corpus component of a row's `id`: the genesis hash, short, as the RDF export names its
/// subjects (`cmd/export_rdf.rs`, `corpus_component`).
///
/// Refused rather than substituted, for that function's reason: two corpora loaded into one
/// warehouse under one constant would key `concept/foo` the same, and each `MERGE` would
/// overwrite the other's row.
fn corpus_component(model: &DomainModel) -> Result<String, String> {
    match &model.provenance.genesis_hash {
        Some(hash) => Ok(hash.chars().take(12).collect()),
        None => Err(
            "--with-corpus keys each row on the node's reference, which names the corpus by its \
             genesis commit, and this corpus has none: it is not a git repository, or it is a \
             directory inside one. Give the corpus a repository of its own, or export without \
             --with-corpus."
                .to_string(),
        ),
    }
}

/// The node's reference (RFC-0032), which is a `corpus` row's `id` (§2.2).
fn reference(corpus: &str, node: &str) -> String {
    yidam_core::uri::render_reference(&yidam_core::uri::Reference {
        corpus: Some(corpus.to_string()),
        kind: yidam_core::uri::Kind::Node,
        path: node.to_string(),
        rev: None,
        fragment: None,
    })
}

/// The catalog slug a link target names, when it names a catalog entry: the one shape a
/// target escaping the corpus takes, `../../catalog/<slug>.md` (see `export_rdf`'s
/// `reference_for`).
fn catalog_slug(target: &str) -> Option<&str> {
    match target.rsplit_once("catalog/") {
        Some((prefix, entry)) if prefix.contains("..") || prefix.is_empty() => {
            Some(entry.strip_suffix(".md").unwrap_or(entry))
        }
        _ => None,
    }
}

/// A `date` value as the first day of the interval it states, and that interval's precision
/// (§2.5). `None` for a date the calendar does not have, which lint's structural check admits
/// and BigQuery's `DATE` does not.
fn first_day(s: &str) -> Option<(String, &'static str)> {
    let parts = iso_date_parts(s.trim())?;
    let y: i64 = parts[0].parse().ok()?;
    let m: u32 = parts.get(1).map_or(Some(1), |p| p.parse().ok())?;
    let d: u32 = parts.get(2).map_or(Some(1), |p| p.parse().ok())?;
    if crate::dates::civil_from_days(crate::dates::days_from_civil(y, m, d)) != (y, m, d) {
        return None;
    }
    let precision = ["year", "month", "day"][parts.len() - 1];
    Some((format!("{y:04}-{m:02}-{d:02}"), precision))
}

/// The cells one property contributes, by column name, or why it cannot.
fn property_cells(
    property: &ClassProperty,
    column: &str,
    ty: ColumnType,
    value: &serde_yaml::Value,
) -> Result<Vec<(String, serde_json::Value)>, String> {
    // The gate's own predicates, so the export refuses nothing lint accepts for a reason
    // lint would not give, beyond what the warehouse's types add.
    if let Some(why) = property_type_violation(&property.r#type, value) {
        return Err(why);
    }
    if let Some(why) = declared_value_violation(&property.values, value) {
        return Err(why);
    }
    let text = || value.as_str().map(str::to_string);
    match ty {
        ColumnType::String => match text() {
            Some(s) => Ok(vec![(column.to_string(), s.into())]),
            None => Err("is not text".to_string()),
        },
        ColumnType::Float => {
            let n = match value {
                serde_yaml::Value::Number(n) => numeric_value(&n.to_string()),
                _ => None,
            };
            match n.and_then(serde_json::Number::from_f64) {
                Some(n) => Ok(vec![(column.to_string(), serde_json::Value::Number(n))]),
                None => Err("is not a finite number".to_string()),
            }
        }
        ColumnType::Date => {
            let s = text().unwrap_or_default();
            match first_day(&s) {
                Some((day, precision)) => Ok(vec![
                    (column.to_string(), day.into()),
                    (format!("{column}{PRECISION_SUFFIX}"), precision.into()),
                ]),
                None => Err(format!("`{}` is not a date the calendar has", s.trim())),
            }
        }
        // No property column is a timestamp; the contract gives one only to `_loaded_at`.
        ColumnType::Timestamp => Err("maps to a timestamp, which no property does".to_string()),
    }
}

struct Node {
    id: String,
    class: String,
    rel: String,
    parsed: CorpusInstance,
}

/// Every node's row and link rows, or every reason one cannot be placed.
pub(crate) fn rows<'a>(
    model: &DomainModel,
    classes: &[Class],
    contract: &'a Contract,
) -> Result<CorpusRows<'a>, Vec<String>> {
    let corpus = corpus_component(model).map_err(|e| vec![e])?;
    let mut refusals = Vec::new();

    let mut nodes: Vec<Node> = Vec::new();
    for inst in &model.instances {
        let rel = format!(".yidam/corpus/{}/{}", inst.class, inst.filename);
        let (parsed, malformed) = crate::corpus::parse_or_default::<CorpusInstance>(
            &String::from_utf8_lossy(&inst.content),
        );
        if let Some(why) = malformed {
            refusals.push(format!("{rel}: the node does not parse ({why})"));
            continue;
        }
        nodes.push(Node {
            id: format!("{}/{}", inst.class, file_stem(&inst.filename)),
            class: inst.class.clone(),
            rel,
            parsed,
        });
    }
    nodes.sort_by(|a, b| a.id.cmp(&b.id));
    let known: BTreeSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let declared: BTreeMap<&str, &Class> = classes.iter().map(|c| (c.name.as_str(), c)).collect();
    let mut rows: BTreeMap<&str, Vec<Vec<serde_json::Value>>> = BTreeMap::new();
    let mut class_tables: BTreeMap<&str, &Table> = BTreeMap::new();
    let mut edge_tables: BTreeMap<(&str, &str), &Table> = BTreeMap::new();
    for table in &contract.tables {
        match &table.kind {
            TableKind::Class { class } => {
                class_tables.insert(class.as_str(), table);
            }
            TableKind::Edge {
                class,
                relationship,
                ..
            } => {
                edge_tables.insert((class.as_str(), relationship.as_str()), table);
            }
            TableKind::Vocab { .. } => continue,
        }
        rows.insert(table.name.as_str(), Vec::new());
    }

    let mut undeclared_class: BTreeMap<&str, usize> = BTreeMap::new();
    let (mut undeclared_property, mut no_edge, mut no_node, mut many_sources) = (0, 0, 0, 0);
    for node in &nodes {
        let (Some(table), Some(class)) = (
            class_tables.get(node.class.as_str()),
            declared.get(node.class.as_str()),
        ) else {
            *undeclared_class.entry(&node.class).or_default() += 1;
            continue;
        };
        let links = node.parsed.links.as_deref().unwrap_or_default();
        let slugs: BTreeSet<&str> = links
            .iter()
            .filter_map(|l| l.target.as_deref().and_then(catalog_slug))
            .collect();
        if slugs.len() > 1 {
            many_sources += 1;
        }
        let source = match slugs.len() {
            1 => slugs.iter().next().map(|s| (*s).into()),
            _ => None,
        }
        .unwrap_or(serde_json::Value::Null);
        let id = reference(&corpus, &node.id);

        let empty = serde_yaml::Mapping::new();
        let properties = node.parsed.properties.as_ref().unwrap_or(&empty);
        let mut cells: BTreeMap<String, serde_json::Value> = BTreeMap::new();
        for column in table.columns.iter().filter(|c| c.property.is_some()) {
            let name = column.property.as_deref().unwrap_or_default();
            let Some(property) = class.properties.iter().find(|p| p.name == name) else {
                continue;
            };
            // A date's `_precision` column is written with the date's own.
            if property.r#type == "date" && column.ty != ColumnType::Date {
                continue;
            }
            match properties.get(name) {
                None | Some(serde_yaml::Value::Null) if column.required => refusals.push(format!(
                    "{}: `{name}` is required by class `{}` and absent",
                    node.rel, class.name
                )),
                None | Some(serde_yaml::Value::Null) => {}
                Some(value) => match property_cells(property, &column.name, column.ty, value) {
                    Ok(c) => cells.extend(c),
                    Err(why) => refusals.push(format!("{}: `{name}` {why}", node.rel)),
                },
            }
        }
        undeclared_property += properties
            .keys()
            .filter_map(|k| k.as_str())
            .filter(|k| !class.properties.iter().any(|p| p.name == *k))
            .count();
        let row = staged_columns(table)
            .map(|c| match c.name.as_str() {
                "id" => id.clone().into(),
                "_source" => source.clone(),
                "_corpus_commit" => model.provenance.commit.clone().into(),
                other => cells.remove(other).unwrap_or(serde_json::Value::Null),
            })
            .collect();
        if let Some(r) = rows.get_mut(table.name.as_str()) {
            r.push(row);
        }

        for link in links {
            let Some(target) = link.target.as_deref() else {
                continue;
            };
            if catalog_slug(target).is_some() {
                continue;
            }
            let relationship = link.relationship.as_deref().unwrap_or("link");
            let Some(edge) = edge_tables.get(&(node.class.as_str(), relationship)) else {
                no_edge += 1;
                continue;
            };
            let dst = resolve_link_target(&node.class, target);
            let dst_class = dst.split('/').next().unwrap_or_default();
            let TableKind::Edge { targets, .. } = &edge.kind else {
                continue;
            };
            if !known.contains(dst.as_str()) || !targets.contains(dst_class) {
                no_node += 1;
                continue;
            }
            if let Some(r) = rows.get_mut(edge.name.as_str()) {
                r.push(vec![
                    id.clone().into(),
                    reference(&corpus, &dst).into(),
                    source.clone(),
                ]);
            }
        }
    }
    if !refusals.is_empty() {
        return Err(refusals);
    }

    let mut skipped: Vec<String> = undeclared_class
        .iter()
        .map(|(class, n)| format!("{n} node(s) of `{class}`, which no `.ont.yml` declares"))
        .collect();
    for (n, what) in [
        (
            undeclared_property,
            "property value(s) their class does not declare",
        ),
        (
            no_edge,
            "link(s) whose relationship their class does not declare as an edge",
        ),
        (
            no_node,
            "link(s) to no node of a class the edge declares as its target",
        ),
        (
            many_sources,
            "node(s) linking more than one catalog entry, so `_source` is null",
        ),
    ] {
        if n > 0 {
            skipped.push(format!("{n} {what}"));
        }
    }

    let tables = contract
        .tables
        .iter()
        .filter_map(|table| {
            let mut r = rows.remove(table.name.as_str())?;
            r.sort_by_key(|row| serde_json::to_string(row).unwrap_or_default());
            r.dedup();
            Some(TableRows { table, rows: r })
        })
        .collect();
    Ok(CorpusRows {
        tables,
        commit: model.provenance.commit.clone(),
        skipped,
    })
}

/// One table's rows as NDJSON: an object per line, every staged column present, null or not.
pub(crate) fn ndjson(rows: &TableRows) -> String {
    let names: Vec<String> = staged_columns(rows.table)
        .map(|c| serde_json::Value::from(c.name.as_str()).to_string())
        .collect();
    let mut out = String::new();
    for row in &rows.rows {
        let fields: Vec<String> = names
            .iter()
            .zip(row)
            .map(|(name, value)| format!("{name}:{value}"))
            .collect();
        out.push('{');
        out.push_str(&fields.join(","));
        out.push_str("}\n");
    }
    out
}

/// The BigQuery schema file `bq load` reads the NDJSON with: the staged columns, in order.
pub(crate) fn schema(table: &Table) -> String {
    let fields: Vec<serde_json::Value> = staged_columns(table)
        .map(|c| {
            serde_json::json!({
                "name": c.name,
                "type": super::bigquery::type_name(c.ty),
                "mode": if c.required { "REQUIRED" } else { "NULLABLE" },
            })
        })
        .collect();
    let mut out = serde_json::to_string_pretty(&fields).unwrap_or_default();
    out.push('\n');
    out
}

/// Every file `--with-corpus` adds, under `feeds/corpus/`.
pub(crate) fn render(rows: &CorpusRows) -> Vec<(String, String)> {
    let dir = format!("feeds/{CORPUS_ORIGIN}");
    let mut files = Vec::new();
    for t in &rows.tables {
        let name = &t.table.name;
        if !t.rows.is_empty() {
            files.push((format!("{dir}/{name}.ndjson"), ndjson(t)));
            files.push((format!("{dir}/{name}.schema.json"), schema(t.table)));
        }
        files.push((
            format!("{dir}/{name}.sql"),
            super::bigquery::corpus_script(t.table, &rows.commit, !t.rows.is_empty()),
        ));
    }
    files.push((
        format!("{dir}/load.sh"),
        super::bigquery::corpus_runner(&rows.tables),
    ));
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InstanceFile, OntClass, RenderedViews};
    use crate::warehouse::contract::derive;

    const RESOURCE: &str = "\
properties:
  - name: kind
    type: string
    required: true
    values: [bucket, instance]
  - name: created
    type: date
  - name: size-gb
    type: number
  - name: claim_tag
    type: claim
edges:
  - relationship: stored-in
    target: region
";

    fn model(nodes: &[(&str, &str, &str)]) -> DomainModel {
        let mut provenance = crate::model::test_provenance();
        provenance.genesis_hash = Some("0123456789abcdef0123".into());
        DomainModel {
            classes: [("resource", RESOURCE), ("region", "properties: []\n")]
                .iter()
                .map(|(name, yaml)| OntClass {
                    filename: format!("{name}.ont.yml"),
                    content: yaml.as_bytes().to_vec(),
                })
                .collect(),
            instances: nodes
                .iter()
                .map(|(class, name, yaml)| InstanceFile {
                    class: (*class).into(),
                    filename: format!("{name}.yml"),
                    content: yaml.as_bytes().to_vec(),
                })
                .collect(),
            skills: vec![],
            decisions: vec![],
            index: None,
            provenance,
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    fn export(nodes: &[(&str, &str, &str)]) -> Result<BTreeMap<String, String>, Vec<String>> {
        let m = model(nodes);
        let classes = crate::warehouse::classes(&m);
        let contract = derive(&classes).unwrap();
        let r = rows(&m, &classes, &contract)?;
        Ok(render(&r).into_iter().collect())
    }

    fn lines(files: &BTreeMap<String, String>, table: &str) -> Vec<serde_json::Value> {
        files[&format!("feeds/corpus/{table}.ndjson")]
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect()
    }

    const BUCKET: &str = "\
class: resource
label: Logs bucket
properties:
  kind: bucket
  created: \"2024-05\"
  size-gb: 12.5
  claim_tag: verified
links:
  - target: ../region/us-east1.yml
    relationship: stored-in
  - target: ../../catalog/gcp-inventory.md
    relationship: sourced-from
";

    #[test]
    fn every_line_carries_exactly_the_contract_columns_but_the_two_the_merge_writes() {
        let files = export(&[
            ("resource", "logs", BUCKET),
            ("region", "us-east1", "class: region\n"),
        ])
        .unwrap();
        let m = model(&[]);
        let contract = derive(&crate::warehouse::classes(&m)).unwrap();
        for table in ["resource", "region", "edge_resource__stored_in"] {
            let t = contract.tables.iter().find(|t| t.name == table).unwrap();
            let want: Vec<&str> = t
                .columns
                .iter()
                .map(|c| c.name.as_str())
                .filter(|c| *c != "_origin" && *c != "_loaded_at")
                .collect();
            for line in lines(&files, table) {
                let got: BTreeSet<&str> = line
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(|k| k.as_str())
                    .collect();
                assert_eq!(got, want.iter().copied().collect(), "{table}");
            }
            let schema: Vec<serde_json::Value> =
                serde_json::from_str(&files[&format!("feeds/corpus/{table}.schema.json")]).unwrap();
            let names: Vec<&str> = schema.iter().map(|f| f["name"].as_str().unwrap()).collect();
            assert_eq!(names, want, "{table}");
        }
    }

    #[test]
    fn a_row_is_keyed_on_the_reference_and_carries_its_commit_and_catalog_slug() {
        let files = export(&[
            ("resource", "logs", BUCKET),
            ("region", "us-east1", "class: region\n"),
        ])
        .unwrap();
        let row = &lines(&files, "resource")[0];
        assert_eq!(row["id"], "yidam://0123456789ab/node/resource/logs");
        assert_eq!(row["_corpus_commit"], "abc1234");
        assert_eq!(row["_source"], "gcp-inventory");
        assert_eq!(row["kind"], "bucket");
        assert_eq!(row["size_gb"], 12.5);
        let edge = &lines(&files, "edge_resource__stored_in")[0];
        assert_eq!(edge["src_id"], row["id"]);
        assert_eq!(edge["dst_id"], "yidam://0123456789ab/node/region/us-east1");
        assert_eq!(edge["_source"], "gcp-inventory");
    }

    #[test]
    fn a_claim_value_is_carried_by_no_column() {
        let files = export(&[("resource", "logs", BUCKET)]).unwrap();
        let ndjson = &files["feeds/corpus/resource.ndjson"];
        assert!(
            !ndjson.contains("claim_tag") && !ndjson.contains("verified"),
            "{ndjson}"
        );
    }

    #[test]
    fn each_date_precision_round_trips_to_its_first_day() {
        for (written, day, precision) in [
            ("\"2024\"", "2024-01-01", "year"),
            ("\"2024-05\"", "2024-05-01", "month"),
            ("\"2024-05-17\"", "2024-05-17", "day"),
        ] {
            let node = format!("properties:\n  kind: bucket\n  created: {written}\n");
            let files = export(&[("resource", "logs", &node)]).unwrap();
            let row = &lines(&files, "resource")[0];
            assert_eq!(row["created"], day, "{written}");
            assert_eq!(row["created_precision"], precision, "{written}");
        }
    }

    #[test]
    fn a_node_the_contract_cannot_place_is_refused_by_name() {
        let refusals = export(&[
            ("resource", "absent", "properties:\n  size-gb: 3\n"),
            ("resource", "outside", "properties:\n  kind: disk\n"),
            (
                "resource",
                "feb",
                "properties:\n  kind: bucket\n  created: \"2024-02-31\"\n",
            ),
            (
                "resource",
                "text",
                "properties:\n  kind: bucket\n  size-gb: \"3\"\n",
            ),
        ])
        .unwrap_err();
        let all = refusals.join("\n");
        for want in [
            "resource/absent.yml: `kind` is required by class `resource` and absent",
            "resource/outside.yml: `kind` `disk` is not one of the values",
            "resource/feb.yml: `created` `2024-02-31` is not a date the calendar has",
            "resource/text.yml: `size-gb` `3` is text, not a number",
        ] {
            assert!(all.contains(want), "missing {want:?} in:\n{all}");
        }
    }

    #[test]
    fn a_corpus_with_no_genesis_of_its_own_is_refused() {
        let mut m = model(&[]);
        m.provenance.genesis_hash = None;
        let classes = crate::warehouse::classes(&m);
        let contract = derive(&classes).unwrap();
        let why = rows(&m, &classes, &contract).err().unwrap().join("");
        assert!(why.contains("genesis commit"), "{why}");
    }

    #[test]
    fn a_link_with_no_link_table_is_skipped_and_counted() {
        let node = "properties:\n  kind: bucket\nlinks:\n  - target: ../resource.ont.yml\n    \
                    relationship: instance-of\n  - target: ../region/nowhere.yml\n    \
                    relationship: stored-in\n";
        let m = model(&[("resource", "logs", node)]);
        let classes = crate::warehouse::classes(&m);
        let contract = derive(&classes).unwrap();
        let r = rows(&m, &classes, &contract).unwrap();
        let skipped = r.skipped.join("\n");
        assert!(
            skipped.contains("1 link(s) whose relationship"),
            "{skipped}"
        );
        assert!(skipped.contains("1 link(s) to no node"), "{skipped}");
    }

    #[test]
    fn a_table_with_no_rows_deletes_its_corpus_rows_and_loads_nothing() {
        let files = export(&[("resource", "logs", "properties:\n  kind: bucket\n")]).unwrap();
        assert!(!files.contains_key("feeds/corpus/region.ndjson"));
        let sql = &files["feeds/corpus/region.sql"];
        assert!(
            sql.contains("DELETE FROM `region` WHERE `_origin` = \"corpus\""),
            "{sql}"
        );
        let runner = &files["feeds/corpus/load.sh"];
        assert!(!runner.contains("region.ndjson"), "{runner}");
        assert!(runner.contains("region.sql"), "{runner}");
    }
}
