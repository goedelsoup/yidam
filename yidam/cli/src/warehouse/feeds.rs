//! RFC-0049 §3: feeds, the declarations that say how rows from elsewhere load into the contract.
//!
//! A feed is `.yidam/warehouse/feeds/<feed>.yml`. This module parses one, checks it against
//! the contract (§3.2) and resolves it into a [`Feed`]: a load plan a dialect renderer turns
//! into SQL. The expressions in a feed are authored SQL fragments. They are carried verbatim
//! and never parsed; the one thing done to them is expanding `${landing}` to the feed's
//! `landing:`, when it has one. Every other `${...}` is a run-time parameter, listed under
//! `parameters:`, and survives into the SQL.

use std::collections::{BTreeMap, BTreeSet};

use super::contract::{ident, ColumnType, Contract, Table, TableKind, PRECISION_SUFFIX};

/// The parameter a feed's `landing:` defines, and the one this module expands.
const LANDING: &str = "landing";

/// The origin `--with-corpus` writes (§3.4), which no feed may take.
pub(crate) const CORPUS_ORIGIN: &str = "corpus";

/// A feed file as found on disk.
pub(crate) struct FeedSource {
    /// Corpus-relative path, for naming the file in a refusal.
    pub rel: String,
    /// The file's stem, which must be its `feed:`.
    pub stem: String,
    pub text: String,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FeedFile {
    feed: String,
    #[serde(default)]
    description: String,
    mode: Mode,
    #[serde(default)]
    landing: Option<String>,
    /// Run-time parameter → what it is. Every `${...}` but `${landing}` must be listed here.
    #[serde(default)]
    parameters: BTreeMap<String, String>,
    #[serde(default)]
    classes: BTreeMap<String, ClassMap>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ClassMap {
    from: String,
    #[serde(default, rename = "where")]
    filter: Option<String>,
    id: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    columns: BTreeMap<String, String>,
    #[serde(default)]
    edges: BTreeMap<String, EdgeMap>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgeMap {
    #[serde(default)]
    unnest: Option<String>,
    /// The name the unnested element is read by. Without one, BigQuery exposes the element's
    /// fields bare, and a field that shares a name with the row's (CAI's `name`) is ambiguous.
    #[serde(default, rename = "as")]
    alias: Option<String>,
    dst_id: String,
}

/// Whether a load deletes what landing no longer holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Mode {
    /// Landing is the whole truth for this origin: rows absent from it are deleted.
    Snapshot,
    /// Rows are upserted and never deleted.
    Append,
}

/// One feed, checked and resolved against the contract.
#[derive(Debug)]
pub(crate) struct Feed {
    pub name: String,
    pub description: String,
    pub mode: Mode,
    pub parameters: BTreeMap<String, String>,
    pub classes: Vec<ClassLoad>,
}

/// How one class's rows load: its table, the rows' source and each mapped column.
#[derive(Debug)]
pub(crate) struct ClassLoad {
    pub class: String,
    pub table: String,
    pub from: String,
    pub filter: Option<String>,
    pub id: String,
    pub source: Option<String>,
    /// In the contract's column order, not the file's.
    pub columns: Vec<MappedColumn>,
    pub edges: Vec<EdgeLoad>,
}

#[derive(Debug)]
pub(crate) struct MappedColumn {
    pub column: String,
    pub ty: ColumnType,
    pub expr: String,
    /// The `vocab_` table this column's values must be in.
    pub values: Option<String>,
    /// A `_precision` column, whose values must be `year`, `month` or `day`.
    pub precision: bool,
}

#[derive(Debug)]
pub(crate) struct EdgeLoad {
    pub relationship: String,
    pub table: String,
    pub unnest: Option<String>,
    pub alias: Option<String>,
    pub dst_id: String,
}

/// The precision a feed writes for a `date` it maps and whose precision it does not (§2.5).
pub(crate) const DEFAULT_PRECISION: &str = "day";

/// Every `${name}` in `expr`, or the reason one is malformed.
fn parameters_in(expr: &str) -> Result<Vec<&str>, String> {
    let mut names = Vec::new();
    let mut rest = expr;
    while let Some(start) = rest.find("${") {
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(format!("`{expr}` opens a `${{` it never closes"));
        };
        let name = &after[..end];
        let well_formed = name
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
        if !well_formed {
            return Err(format!("`${{{name}}}` is not a parameter name"));
        }
        names.push(name);
        rest = &after[end + 1..];
    }
    Ok(names)
}

/// Resolve every feed, or every reason one cannot be resolved.
///
/// `catalog` is the corpus's catalog slugs. The checks are §3.2's, and each refusal names
/// the file and the key it is about.
pub(crate) fn resolve(
    sources: &[FeedSource],
    contract: &Contract,
    catalog: &BTreeSet<String>,
) -> Result<Vec<Feed>, Vec<String>> {
    let mut refusals = Vec::new();
    let mut feeds = Vec::new();
    for source in sources {
        match resolve_one(source, contract, catalog) {
            Ok(feed) => feeds.push(feed),
            Err(mut r) => refusals.append(&mut r),
        }
    }
    if refusals.is_empty() {
        feeds.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(feeds)
    } else {
        Err(refusals)
    }
}

fn resolve_one(
    source: &FeedSource,
    contract: &Contract,
    catalog: &BTreeSet<String>,
) -> Result<Feed, Vec<String>> {
    let rel = &source.rel;
    let file: FeedFile = serde_yaml::from_str(&source.text)
        .map_err(|e| vec![format!("{rel}: the feed does not parse ({e})")])?;
    let mut refusals = Vec::new();

    let well_named = file
        .feed
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_lowercase())
        && file
            .feed
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !well_named {
        refusals.push(format!(
            "{rel}: feed: `{}` is not a feed name; it becomes `_origin` and a directory, so it \
             is lowercase letters, digits and `-`",
            file.feed
        ));
    } else if file.feed != source.stem {
        refusals.push(format!(
            "{rel}: feed: `{}` differs from the file's name `{}`; the two must agree",
            file.feed, source.stem
        ));
    } else if file.feed == CORPUS_ORIGIN {
        refusals.push(format!(
            "{rel}: feed: `{CORPUS_ORIGIN}` is the origin `--with-corpus` writes"
        ));
    }

    // The parameters an expression may use, and a check that each one it uses is listed.
    let mut check_parameters =
        |key: &str, expr: &str, allowed_landing: bool| match parameters_in(expr) {
            Err(why) => refusals.push(format!("{rel}: {key}: {why}")),
            Ok(names) => {
                for name in names {
                    // `${landing}` is expanded when the feed declares `landing:`; otherwise it
                    // is a run-time parameter like any other, and is listed like one.
                    let expanded = name == LANDING && allowed_landing && file.landing.is_some();
                    if !expanded && !file.parameters.contains_key(name) {
                        let hint = if name == LANDING && allowed_landing {
                            "; declare `landing:` to have it expanded, or list it"
                        } else {
                            ""
                        };
                        refusals.push(format!(
                            "{rel}: {key}: `${{{name}}}` is not listed under `parameters:`{hint}"
                        ));
                    }
                }
            }
        };
    if let Some(landing) = &file.landing {
        check_parameters("landing", landing, false);
    }
    for (class, map) in &file.classes {
        let at = format!("classes.{class}");
        check_parameters(&format!("{at}.from"), &map.from, true);
        check_parameters(&format!("{at}.id"), &map.id, true);
        if let Some(filter) = &map.filter {
            check_parameters(&format!("{at}.where"), filter, true);
        }
        for (key, expr) in &map.columns {
            check_parameters(&format!("{at}.columns.{key}"), expr, true);
        }
        for (rel_name, edge) in &map.edges {
            let at = format!("{at}.edges.{rel_name}");
            if let Some(unnest) = &edge.unnest {
                check_parameters(&format!("{at}.unnest"), unnest, true);
            }
            check_parameters(&format!("{at}.dst_id"), &edge.dst_id, true);
        }
    }

    let expand = |expr: &str| match &file.landing {
        Some(landing) => expr.replace(&format!("${{{LANDING}}}"), landing),
        None => expr.to_string(),
    };

    let mut classes = Vec::new();
    for (class, map) in &file.classes {
        let at = format!("{rel}: classes.{class}");
        let Some(table) = class_table(contract, class) else {
            refusals.push(format!("{at}: no `.ont.yml` declares class `{class}`"));
            continue;
        };
        if let Some(slug) = &map.source {
            if !catalog.contains(slug) {
                refusals.push(format!(
                    "{at}.source: `{slug}` is not a catalog entry (no `.yidam/catalog/{slug}.md`)"
                ));
            }
        }

        // Each mapped key, resolved to the contract column it names.
        let mut mapped: BTreeMap<String, (&str, &str)> = BTreeMap::new();
        for (key, expr) in &map.columns {
            let wanted = ident(key).to_ascii_lowercase();
            let Some(column) = table
                .columns
                .iter()
                .find(|c| c.name.to_ascii_lowercase() == wanted)
            else {
                refusals.push(format!(
                    "{at}.columns.{key}: `{class}` has no column `{key}`; a `claim` or \
                     `quotation` property has none, and every other property's column is named \
                     in contract.json"
                ));
                continue;
            };
            if column.property.is_none() {
                refusals.push(format!(
                    "{at}.columns.{key}: `{}` is a provenance column, which the feed SQL sets \
                     itself",
                    column.name
                ));
                continue;
            }
            if let Some((prev, _)) = mapped.insert(column.name.clone(), (key, expr)) {
                refusals.push(format!(
                    "{at}.columns: `{prev}` and `{key}` both map column `{}`",
                    column.name
                ));
            }
        }

        let mut columns = Vec::new();
        for column in table.columns.iter().filter(|c| c.property.is_some()) {
            let precision = column.name.ends_with(PRECISION_SUFFIX)
                && column.ty == ColumnType::String
                && table.columns.iter().any(|d| {
                    d.ty == ColumnType::Date
                        && format!("{}{PRECISION_SUFFIX}", d.name) == column.name
                });
            let expr = match mapped.get(&column.name) {
                Some((_, expr)) => expand(expr),
                None if precision
                    && column
                        .name
                        .strip_suffix(PRECISION_SUFFIX)
                        .is_some_and(|date| mapped.contains_key(date)) =>
                {
                    format!("\"{DEFAULT_PRECISION}\"")
                }
                None => {
                    if column.required {
                        refusals.push(format!(
                            "{at}.columns: required property `{}` is not mapped (column `{}`)",
                            column.property.as_deref().unwrap_or_default(),
                            column.name
                        ));
                    }
                    continue;
                }
            };
            columns.push(MappedColumn {
                column: column.name.clone(),
                ty: column.ty,
                expr,
                values: column.values.clone(),
                precision,
            });
        }

        let mut edges = Vec::new();
        for (relationship, edge) in &map.edges {
            let Some(link) = edge_table(contract, class, relationship) else {
                refusals.push(format!(
                    "{at}.edges.{relationship}: `{class}` authors no edge `{relationship}`; an \
                     `in` edge is mapped under the class at its other end"
                ));
                continue;
            };
            if let Some(alias) = &edge.alias {
                let identifier = alias
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
                    && alias.chars().all(|c| c.is_ascii_alphanumeric() || c == '_');
                if edge.unnest.is_none() {
                    refusals.push(format!(
                        "{at}.edges.{relationship}.as: names an unnested element, and the edge \
                         declares no `unnest:`"
                    ));
                } else if !identifier {
                    refusals.push(format!(
                        "{at}.edges.{relationship}.as: `{alias}` is not an identifier"
                    ));
                }
            }
            edges.push(EdgeLoad {
                relationship: relationship.clone(),
                table: link.name.clone(),
                unnest: edge.unnest.as_deref().map(expand),
                alias: edge.alias.clone(),
                dst_id: expand(&edge.dst_id),
            });
        }

        classes.push(ClassLoad {
            class: class.clone(),
            table: table.name.clone(),
            from: expand(&map.from),
            filter: map.filter.as_deref().map(expand),
            id: expand(&map.id),
            source: map.source.clone(),
            columns,
            edges,
        });
    }

    if !refusals.is_empty() {
        return Err(refusals);
    }
    Ok(Feed {
        name: file.feed,
        description: file.description.trim().to_string(),
        mode: file.mode,
        parameters: file.parameters,
        classes,
    })
}

fn class_table<'c>(contract: &'c Contract, class: &str) -> Option<&'c Table> {
    contract
        .tables
        .iter()
        .find(|t| matches!(&t.kind, TableKind::Class { class: c } if c == class))
}

/// The link table `class` authors for `relationship`, matched as the table name matches it.
fn edge_table<'c>(contract: &'c Contract, class: &str, relationship: &str) -> Option<&'c Table> {
    let wanted = ident(relationship).to_ascii_lowercase();
    contract.tables.iter().find(|t| {
        matches!(&t.kind, TableKind::Edge { class: c, relationship: r, .. }
            if c == class && ident(r).to_ascii_lowercase() == wanted)
    })
}

/// The classes no feed maps, by name. Coverage is reported, never refused (§3.2).
pub(crate) fn unmapped(contract: &Contract, feeds: &[Feed]) -> Vec<String> {
    let mapped: BTreeSet<&str> = feeds
        .iter()
        .flat_map(|f| f.classes.iter().map(|c| c.class.as_str()))
        .collect();
    contract
        .tables
        .iter()
        .filter_map(|t| match &t.kind {
            TableKind::Class { class } if !mapped.contains(class.as_str()) => Some(class.clone()),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::warehouse::contract::{derive, tests::class};

    /// The cloud-inventory fixture: a value set, a date, a claim and one edge each way.
    pub(crate) fn contract() -> Contract {
        derive(&[
            class(
                "compute-instance",
                "properties:\n\
                 \x20 - name: provider\n    type: string\n    required: true\n    \
                 values: [gcp, aws]\n\
                 \x20 - name: machine-type\n    type: string\n\
                 \x20 - name: created\n    type: date\n\
                 \x20 - name: standing\n    type: claim\n\
                 edges:\n\
                 \x20 - relationship: attached-to\n    target: network\n    direction: out\n",
            ),
            class(
                "network",
                "properties:\n  - name: cidr\n    type: string\n\
                 edges:\n\
                 \x20 - relationship: attached-to\n    target: compute-instance\n    \
                 direction: in\n",
            ),
        ])
        .unwrap()
    }

    /// A feed modelled on Cloud Asset Inventory's per-asset-type BigQuery export.
    pub(crate) const CAI: &str = "\
feed: gcp-cai
description: Cloud Asset Inventory export, per-asset-type tables, daily.
mode: snapshot
landing: \"${project}.${landing_dataset}\"
parameters:
  project: The GCP project holding the landing dataset.
  landing_dataset: The dataset CAI exports into.
classes:
  compute-instance:
    from: \"`${landing}.cai_compute_googleapis_com_Instance`\"
    where: \"resource.data.status != 'TERMINATED'\"
    id: name
    source: gcp-cloud-asset-inventory
    columns:
      provider: \"'gcp'\"
      machine_type: resource.data.machineType
      created: DATE(TIMESTAMP(resource.data.creationTimestamp))
    edges:
      attached-to:
        unnest: resource.data.networkInterfaces
        as: nic
        dst_id: nic.network
";

    pub(crate) fn catalog() -> BTreeSet<String> {
        ["gcp-cloud-asset-inventory".to_string()].into()
    }

    pub(crate) fn source(stem: &str, text: &str) -> FeedSource {
        FeedSource {
            rel: format!(".yidam/warehouse/feeds/{stem}.yml"),
            stem: stem.into(),
            text: text.into(),
        }
    }

    fn refusals(text: &str) -> Vec<String> {
        let stem = text
            .lines()
            .find_map(|l| l.strip_prefix("feed: "))
            .unwrap_or("x");
        resolve(&[source(stem, text)], &contract(), &catalog()).unwrap_err()
    }

    /// `CAI` with one substring replaced, asserting the replacement happened.
    fn cai_with(from: &str, to: &str) -> String {
        assert!(CAI.contains(from), "the fixture has no `{from}`");
        CAI.replacen(from, to, 1)
    }

    fn only(r: Vec<String>, needle: &str) {
        assert_eq!(r.len(), 1, "{r:#?}");
        assert!(r[0].contains(needle), "{r:#?}");
    }

    #[test]
    fn the_cai_feed_resolves() {
        let feeds = resolve(&[source("gcp-cai", CAI)], &contract(), &catalog()).unwrap();
        let ci = &feeds[0].classes[0];
        assert_eq!(ci.table, "compute_instance");
        let columns: Vec<&str> = ci.columns.iter().map(|c| c.column.as_str()).collect();
        assert_eq!(
            columns,
            ["provider", "machine_type", "created", "created_precision"]
        );
        assert_eq!(ci.edges[0].table, "edge_compute_instance__attached_to");
    }

    #[test]
    fn landing_is_expanded_and_every_other_parameter_survives() {
        let feeds = resolve(&[source("gcp-cai", CAI)], &contract(), &catalog()).unwrap();
        assert_eq!(
            feeds[0].classes[0].from,
            "`${project}.${landing_dataset}.cai_compute_googleapis_com_Instance`"
        );
    }

    #[test]
    fn a_date_mapped_without_its_precision_is_written_as_day() {
        let feeds = resolve(&[source("gcp-cai", CAI)], &contract(), &catalog()).unwrap();
        let p = feeds[0].classes[0]
            .columns
            .iter()
            .find(|c| c.column == "created_precision")
            .unwrap();
        assert_eq!(p.expr, "\"day\"");
        assert!(p.precision);
    }

    #[test]
    fn an_undeclared_class_is_refused() {
        only(
            refusals(&cai_with("  compute-instance:\n", "  vm:\n")),
            "classes.vm: no `.ont.yml` declares class `vm`",
        );
    }

    #[test]
    fn an_undeclared_property_is_refused() {
        only(
            refusals(&cai_with("      machine_type:", "      zone:")),
            "classes.compute-instance.columns.zone",
        );
    }

    #[test]
    fn a_claim_property_has_no_column_to_map() {
        only(
            refusals(&cai_with("      machine_type:", "      standing:")),
            "has no column `standing`",
        );
    }

    #[test]
    fn an_undeclared_edge_is_refused() {
        only(
            refusals(&cai_with("      attached-to:", "      runs-on:")),
            "classes.compute-instance.edges.runs-on",
        );
    }

    #[test]
    fn an_unmapped_required_property_is_refused() {
        only(
            refusals(&cai_with("      provider: \"'gcp'\"\n", "")),
            "required property `provider` is not mapped",
        );
    }

    #[test]
    fn a_column_mapped_twice_is_refused() {
        only(
            refusals(&cai_with(
                "      machine_type:",
                "      machine-type: x\n      machine_type:",
            )),
            "`machine-type` and `machine_type` both map column `machine_type`",
        );
    }

    #[test]
    fn a_source_that_is_no_catalog_entry_is_refused() {
        only(
            refusals(&cai_with(
                "source: gcp-cloud-asset-inventory",
                "source: gcp-cai-export",
            )),
            "classes.compute-instance.source: `gcp-cai-export` is not a catalog entry",
        );
    }

    #[test]
    fn an_unlisted_parameter_is_refused() {
        only(
            refusals(&cai_with("status != 'TERMINATED'", "status != '${status}'")),
            "classes.compute-instance.where: `${status}` is not listed under `parameters:`",
        );
    }

    #[test]
    fn landing_without_a_landing_key_is_a_parameter_to_list() {
        let text = cai_with("landing: \"${project}.${landing_dataset}\"\n", "");
        only(
            refusals(&text),
            "classes.compute-instance.from: `${landing}` is not listed",
        );
        let listed = text.replace("parameters:\n", "parameters:\n  landing: The dataset.\n");
        let feeds = resolve(&[source("gcp-cai", &listed)], &contract(), &catalog()).unwrap();
        assert!(feeds[0].classes[0].from.contains("${landing}"));
    }

    #[test]
    fn a_provenance_column_is_not_the_feeds_to_map() {
        only(
            refusals(&cai_with("      machine_type:", "      _loaded_at:")),
            "is a provenance column",
        );
    }

    #[test]
    fn a_feed_name_must_agree_with_its_file() {
        let r = resolve(&[source("cai", CAI)], &contract(), &catalog()).unwrap_err();
        only(r, "differs from the file's name `cai`");
    }

    #[test]
    fn an_unknown_key_is_refused_rather_than_ignored() {
        only(
            refusals(&cai_with("    id: name\n", "    id: name\n    key: name\n")),
            "does not parse",
        );
    }

    #[test]
    fn every_refusal_in_a_file_is_reported_at_once() {
        let text = cai_with("      machine_type:", "      zone:");
        let text = text.replace("source: gcp-cloud-asset-inventory", "source: nowhere");
        assert_eq!(refusals(&text).len(), 2);
    }

    #[test]
    fn an_alias_without_an_unnest_is_refused() {
        only(
            refusals(&cai_with(
                "        unnest: resource.data.networkInterfaces\n",
                "",
            )),
            "edges.attached-to.as: names an unnested element",
        );
    }

    #[test]
    fn a_class_no_feed_maps_is_reported_as_coverage() {
        let c = contract();
        let feeds = resolve(&[source("gcp-cai", CAI)], &c, &catalog()).unwrap();
        assert_eq!(unmapped(&c, &feeds), ["network"]);
    }

    #[test]
    fn a_parameter_name_is_read_whole() {
        assert_eq!(parameters_in("${a}.${b_2}.t").unwrap(), ["a", "b_2"]);
        assert!(parameters_in("${a").is_err());
        assert!(parameters_in("${a-b}").is_err());
        assert!(parameters_in("no parameters").unwrap().is_empty());
    }
}
