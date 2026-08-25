//! Executing a checked query over the resolved graph.
//!
//! # One resolver, one answer
//!
//! `cmd/graph.rs` states the rule this inherits: *"A consumer resolving edges itself would be
//! re-deriving it — and would disagree with the gate about which edges are broken, silently,
//! in the direction of 'looks fine here'."* So the edge set here is
//! [`crate::cmd::lint::checks::instance_links`] — the gate's own — and not a second walk.
//!
//! That is deliberately **narrower** than what `neighbors` reports. `graph.rs` keeps a link
//! whose target resolves inside the corpus but names no file, because an editor wants to see
//! a broken edge. A query cannot walk an edge to a file that is not there, and
//! `instance_links` drops exactly those, along with the `instance-of` link to the class file
//! and citations into the catalog. RFC-0018 picks the narrower reader on purpose rather than
//! by accident.
//!
//! # `nodes_read` is not process I/O
//!
//! The executor loads the whole corpus to resolve edges and always will. An agent consuming
//! the result does not. `nodes_read` counts the nodes whose content was *evaluated by a
//! predicate, tested for a hop's class, or returned in the projection* — the agent's cost,
//! which is the only one the benchmark's arms can be compared on.

use std::collections::{BTreeMap, BTreeSet};

use super::check::Checked;
use super::lang::{Dir, Op, Pred, Query, Step};
use crate::cmd::lint::checks::{class_of, instance_links, Node};

/// What an arm of the query cost, in the units #264 compares on.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct Cost {
    /// Path length asked for. **Not `hops`**: `graph.rs:277-282` reserves that name for hops
    /// actually taken, having already had this collision once in one report.
    pub steps: usize,
    pub edges_walked: usize,
    pub nodes_read: usize,
    pub chars: usize,
    pub tokens: usize,
    pub corpus_nodes: usize,
}

/// One node, projected.
pub type Row = BTreeMap<String, serde_json::Value>;

pub struct Outcome {
    pub matched: Vec<String>,
    pub cost: Cost,
}

/// The corpus-relative id of a node — `class/name.yml`, as `corpus-index` reports it.
pub fn id_of(node: &Node, corpus_dir: &str) -> String {
    node.rel
        .strip_prefix(&format!("{corpus_dir}/"))
        .unwrap_or(&node.rel)
        .to_string()
}

fn property<'a>(node: &'a Node, name: &str) -> Option<&'a serde_yaml::Value> {
    node.inst
        .properties
        .as_ref()?
        .get(serde_yaml::Value::String(name.to_string()))
}

/// A property value as text, for comparison.
///
/// A sequence is *not* flattened into one string: `claim_tag: [open]` is legal YAML that the
/// claim counter reads as one claim per element, and `property_type_violation` accepts it.
/// A predicate matches if **any** element matches, so the caller gets the elements.
fn scalars(value: &serde_yaml::Value) -> Vec<String> {
    match value {
        serde_yaml::Value::Sequence(items) => items.iter().flat_map(scalars).collect(),
        serde_yaml::Value::String(s) => vec![s.clone()],
        serde_yaml::Value::Bool(b) => vec![b.to_string()],
        serde_yaml::Value::Number(n) => vec![n.to_string()],
        serde_yaml::Value::Null => vec![],
        _ => vec![],
    }
}

/// Whether one predicate holds of one node.
///
/// **An absent property never matches, for any operator including `!=`.** A reach with no
/// `claim_tag` is not in `reach[claim_tag!=maybe]`. The alternative — three-valued logic —
/// buys nothing here and makes `!=` mean two different things depending on the corpus.
fn pred_holds(node: &Node, pred: &Pred) -> bool {
    let Some(raw) = property(node, &pred.prop) else {
        return false;
    };
    let values = scalars(raw);
    if values.is_empty() {
        return false;
    }
    let wanted = pred.value.to_lowercase();
    let matches_one = |v: &String| match pred.op {
        // `=` on a `date` compares at the precision written, so `observed_on=2026-08`
        // matches every day in that month. Textual prefix on a `-` boundary is exactly that
        // and needs no date parsing.
        Op::Eq => {
            v == &pred.value
                || v.strip_prefix(&pred.value)
                    .is_some_and(|r| r.starts_with('-'))
        }
        Op::Ne => v != &pred.value,
        Op::Contains => v.to_lowercase().contains(&wanted),
    };
    match pred.op {
        // Any element may satisfy `=` or `~`; `!=` has to hold of all of them, or
        // `claim_tag: [open, verified]` would satisfy `claim_tag != open`.
        Op::Ne => values.iter().all(matches_one),
        _ => values.iter().any(matches_one),
    }
}

/// Whether a node satisfies a step: its class, then its predicates.
fn step_holds(node: &Node, step: &Step, allowed: &[String]) -> bool {
    let class = class_of(node);
    // `allowed` is the check's narrowing — for `*` with a predicate, the classes that
    // actually declare it. For a named class it is that class.
    if !allowed.contains(&class) {
        return false;
    }
    step.filter.iter().all(|p| pred_holds(node, p))
}

/// Run a checked query.
///
/// `entry` is the resolved similarity anchor when the first step had one. It replaces the
/// entry scan rather than filtering it: that substitution *is* hybrid anchoring, and it is
/// where the cost difference the benchmark measures actually comes from.
pub fn execute(
    query: &Query,
    checked: &Checked,
    nodes: &[Node],
    corpus_dir: &str,
    entry: Option<&super::anchor::Resolved>,
) -> Outcome {
    let by_path = crate::cmd::lint::checks::nodes_by_path(nodes);
    let id = |n: &Node| id_of(n, corpus_dir);

    // Every traversable edge once: (from id, relationship, to id). The gate's own set.
    let mut edges: Vec<(String, String, String)> = Vec::new();
    for node in nodes {
        for (link, to) in instance_links(node, &by_path) {
            edges.push((
                id(node),
                link.relationship.clone().unwrap_or_default(),
                id(to),
            ));
        }
    }

    let mut read: BTreeSet<String> = BTreeSet::new();
    let mut edges_walked = 0usize;

    // The entry step: every node whose class and predicates hold. Corpus order, which
    // `walk_corpus_instances` already sorts, so the result order is specified rather than
    // incidental — golden fixtures depend on it.
    let empty = Vec::new();
    let allowed = |index: usize| checked.narrowed.get(index).unwrap_or(&empty);
    let mut current: Vec<String> = Vec::new();
    match entry {
        // Anchored: the entry set is what retrieval returned, in score order — the one place
        // in this surface where the order is not corpus order. The step's predicates still
        // apply, so `reach~"…"[claim_tag=open]` means what it reads as; what the anchor
        // replaces is *which nodes were considered*, not what qualifies them.
        Some(resolved) => {
            for id in &resolved.read {
                read.insert(id.clone());
            }
            for want in &resolved.entries {
                let Some(node) = nodes.iter().find(|n| id(n) == *want) else {
                    continue;
                };
                read.insert(want.clone());
                if step_holds(node, &query.steps[0], allowed(0)) {
                    current.push(want.clone());
                }
            }
        }
        None => {
            for node in nodes {
                // **Class narrowing is a directory listing, not a read.** A corpus stores
                // each class in its own directory, so an agent that knows the class never
                // opens the others — and charging the entry step for the whole corpus would
                // make an anchored lookup cost exactly what a full scan costs, which is the
                // comparison #264 exists to make.
                //
                // Every node of a *candidate* class is charged, matching or not: a predicate
                // has to be evaluated against a node to reject it, and charging only for the
                // matches would make a filter that rejects everything look free.
                if !allowed(0).contains(&class_of(node)) {
                    continue;
                }
                read.insert(id(node));
                if step_holds(node, &query.steps[0], allowed(0)) {
                    current.push(id(node));
                }
            }
        }
    }

    for (index, hop) in query.hops.iter().enumerate() {
        let landing = &query.steps[index + 1];
        let mut next: Vec<String> = Vec::new();
        for (from, relationship, to) in &edges {
            if relationship != &hop.relationship {
                continue;
            }
            // `-rel->` walks the edge in its authoring direction; `<-rel-` walks it
            // backwards. The edge itself is stored once, on the node that wrote it.
            let (source, target) = match hop.direction {
                Dir::Out => (from, to),
                Dir::In => (to, from),
            };
            if !current.contains(source) {
                continue;
            }
            edges_walked += 1;
            let Some(node) = nodes.iter().find(|n| id(n) == *target) else {
                continue;
            };
            read.insert(target.clone());
            if step_holds(node, landing, allowed(index + 1)) && !next.contains(target) {
                next.push(target.clone());
            }
        }
        // Corpus order again, not discovery order: two runs must agree.
        next.sort_by_key(|target| nodes.iter().position(|n| id(n) == *target).unwrap_or(0));
        current = next;
    }

    Outcome {
        matched: current,
        cost: Cost {
            steps: query.steps.len(),
            edges_walked,
            nodes_read: read.len(),
            corpus_nodes: nodes.len(),
            ..Cost::default()
        },
    }
}

/// Project the matched nodes onto the selected fields.
///
/// `chars` is the serialized size of what comes back, so the token count is the cost of the
/// *answer* rather than of the walk — which is where a benchmark's budget belongs.
pub fn project(
    matched: &[String],
    nodes: &[Node],
    corpus_dir: &str,
    select: &[String],
) -> (Vec<Row>, usize) {
    let mut rows = Vec::new();
    for want in matched {
        let Some(node) = nodes.iter().find(|n| id_of(n, corpus_dir) == *want) else {
            continue;
        };
        let mut row = Row::new();
        for field in select {
            let value = match field.as_str() {
                "node" => Some(serde_json::Value::String(want.clone())),
                "class" => Some(serde_json::Value::String(class_of(node))),
                "label" => node.inst.label.clone().map(serde_json::Value::String),
                "description" => node.inst.description.clone().map(serde_json::Value::String),
                // The raw instance text. For a YAML corpus that is the whole node, which is
                // what an agent asked for `body` actually wants.
                //
                // From the node rather than from `node.path`. A query at a past commit holds
                // nodes whose paths name today's files — or name nothing at all — and reading
                // the working tree there would answer with the wrong revision's prose under a
                // report that says which commit it is about.
                "body" => Some(serde_json::Value::String(node.text.clone())),
                other => other.strip_prefix("properties.").map(|name| {
                    property(node, name)
                        .map(|v| serde_json::Value::String(scalars(v).join(", ")))
                        .unwrap_or(serde_json::Value::Null)
                }),
            };
            row.insert(field.clone(), value.unwrap_or(serde_json::Value::Null));
        }
        rows.push(row);
    }
    let chars = serde_json::to_string(&rows).map(|s| s.len()).unwrap_or(0);
    (rows, chars)
}

/// Every relationship the corpus authors, and the classes that author it.
///
/// **Wider than the traversable set on purpose**: citations and dangling links count. A name
/// written on a link into the catalog is still a name this corpus uses, and the diagnostic
/// that says "no node authors it" would be false if this only counted edges.
pub fn authored(nodes: &[Node]) -> BTreeMap<String, BTreeSet<String>> {
    let mut out: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for node in nodes {
        for link in node.inst.links.as_deref().unwrap_or(&[]) {
            let Some(relationship) = link.relationship.as_deref() else {
                continue;
            };
            out.entry(relationship.to_string())
                .or_default()
                .insert(class_of(node));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::{CorpusInstance, CorpusLink};
    use std::path::PathBuf;

    fn node(
        rel: &str,
        class: &str,
        props: &[(&str, serde_yaml::Value)],
        links: &[(&str, &str)],
    ) -> Node {
        let mut mapping = serde_yaml::Mapping::new();
        for (name, value) in props {
            mapping.insert(serde_yaml::Value::String(name.to_string()), value.clone());
        }
        Node {
            path: PathBuf::from(format!("/r/.yidam/corpus/{rel}")),
            rel: format!(".yidam/corpus/{rel}"),
            inst: CorpusInstance {
                class: Some(class.to_string()),
                label: Some(rel.to_string()),
                description: None,
                properties: Some(mapping),
                links: Some(
                    links
                        .iter()
                        .map(|(target, relationship)| CorpusLink {
                            target: Some(target.to_string()),
                            relationship: Some(relationship.to_string()),
                        })
                        .collect(),
                ),
            },
            // These fixtures build the instance directly rather than from YAML, so there is
            // no source text to keep. `--select body` is the only reader, and no case here
            // selects it.
            text: String::new(),
        }
    }

    fn text(s: &str) -> serde_yaml::Value {
        serde_yaml::Value::String(s.to_string())
    }

    /// Two reaches, two gages, one concept — streamflow's shape in miniature.
    fn corpus() -> Vec<Node> {
        vec![
            node(
                "concept/low-flow.yml",
                "concept",
                &[("claim_tag", text("inference"))],
                &[],
            ),
            node(
                "gage/canyon.yml",
                "gage",
                &[("parameter", text("00060"))],
                &[
                    ("../concept/low-flow.yml", "sources-from"),
                    ("../../catalog/x.md", "sourced-from"),
                ],
            ),
            node(
                "gage/valley.yml",
                "gage",
                &[("parameter", text("00065"))],
                &[],
            ),
            node(
                "reach/tailwater.yml",
                "reach",
                &[
                    ("regulated", text("yes — outlet works")),
                    ("claim_tag", text("open")),
                ],
                &[
                    ("../gage/canyon.yml", "measured-by"),
                    ("../gage/missing.yml", "measured-by"),
                ],
            ),
        ]
    }

    fn run(query: &str, narrowed: Vec<Vec<&str>>) -> Outcome {
        let nodes = corpus();
        let q = super::super::lang::parse(query).unwrap();
        let checked = Checked {
            diagnostics: vec![],
            narrowed: narrowed
                .into_iter()
                .map(|v| v.into_iter().map(String::from).collect())
                .collect(),
        };
        execute(&q, &checked, &nodes, ".yidam/corpus", None)
    }

    #[test]
    fn a_bare_class_matches_its_instances_in_corpus_order() {
        let out = run("gage", vec![vec!["gage"]]);
        assert_eq!(out.matched, vec!["gage/canyon.yml", "gage/valley.yml"]);
    }

    #[test]
    fn a_typed_hop_follows_only_its_own_relationship() {
        let out = run(
            "reach -measured-by-> gage",
            vec![vec!["reach"], vec!["gage"]],
        );
        assert_eq!(out.matched, vec!["gage/canyon.yml"]);
    }

    /// The narrower reader, pinned. `reach/tailwater` authors two `measured-by` links and
    /// one of them names a file that is not there; `graph.rs` would keep it, and a query
    /// cannot walk it.
    #[test]
    fn an_edge_to_a_file_that_is_not_there_is_not_traversable() {
        let out = run(
            "reach -measured-by-> gage",
            vec![vec!["reach"], vec!["gage"]],
        );
        assert_eq!(
            out.cost.edges_walked, 1,
            "the dangling edge must not be walked"
        );
    }

    /// `instance-of` points at the class file and `sourced-from` into the catalog. Neither
    /// is an ontology edge, and `unlicensed-edge` says so in as many words.
    #[test]
    fn a_citation_is_not_traversable_but_is_still_an_authored_name() {
        let nodes = corpus();
        let out = run(
            "gage -sourced-from-> concept",
            vec![vec!["gage"], vec!["concept"]],
        );
        assert!(out.matched.is_empty());
        assert_eq!(out.cost.edges_walked, 0);
        // ...and the name is still one the corpus uses, so a diagnostic must not claim
        // nobody authors it.
        assert!(authored(&nodes).contains_key("sourced-from"));
    }

    #[test]
    fn a_backward_hop_walks_the_edge_from_the_other_end() {
        let out = run(
            "gage <-measured-by- reach",
            vec![vec!["gage"], vec!["reach"]],
        );
        assert_eq!(out.matched, vec!["reach/tailwater.yml"]);
    }

    #[test]
    fn a_two_hop_path_composes() {
        let out = run(
            "reach -measured-by-> gage -sources-from-> concept",
            vec![vec!["reach"], vec!["gage"], vec!["concept"]],
        );
        assert_eq!(out.matched, vec!["concept/low-flow.yml"]);
        assert_eq!(out.cost.steps, 3);
        assert_eq!(out.cost.edges_walked, 2);
    }

    // ── predicates ────────────────────────────────────────────────────────────

    /// The example that shipped wrong in the RFC's first draft: `regulated` holds prose, so
    /// `=` matches nothing and `~` is the working form.
    #[test]
    fn equality_is_exact_and_containment_is_not() {
        assert!(run("reach[regulated=yes]", vec![vec!["reach"]])
            .matched
            .is_empty());
        assert_eq!(
            run("reach[regulated~yes]", vec![vec!["reach"]]).matched,
            vec!["reach/tailwater.yml"]
        );
    }

    #[test]
    fn an_absent_property_never_matches_even_under_inequality() {
        // `gage` carries no `claim_tag` at all.
        assert!(run("gage[claim_tag!=open]", vec![vec!["gage"]])
            .matched
            .is_empty());
    }

    #[test]
    fn a_list_valued_property_matches_if_any_element_does() {
        let mut nodes = corpus();
        let mut mapping = serde_yaml::Mapping::new();
        mapping.insert(
            text("claim_tag"),
            serde_yaml::Value::Sequence(vec![text("open"), text("verified")]),
        );
        nodes[0].inst.properties = Some(mapping);
        let q = super::super::lang::parse("concept[claim_tag=open]").unwrap();
        let checked = Checked {
            diagnostics: vec![],
            narrowed: vec![vec!["concept".to_string()]],
        };
        assert_eq!(
            execute(&q, &checked, &nodes, ".yidam/corpus", None).matched,
            vec!["concept/low-flow.yml"]
        );
        // ...and `!=` has to hold of every element, or a node tagged both would satisfy it.
        let q = super::super::lang::parse("concept[claim_tag!=open]").unwrap();
        assert!(execute(&q, &checked, &nodes, ".yidam/corpus", None)
            .matched
            .is_empty());
    }

    /// A `date` predicate at the precision it was written.
    #[test]
    fn equality_on_a_date_compares_at_the_precision_given() {
        let nodes = vec![node(
            "note/a.yml",
            "note",
            &[("observed_on", text("2026-08-23"))],
            &[],
        )];
        let checked = Checked {
            diagnostics: vec![],
            narrowed: vec![vec!["note".to_string()]],
        };
        for query in ["note[observed_on=2026-08]", "note[observed_on=2026-08-23]"] {
            let q = super::super::lang::parse(query).unwrap();
            assert_eq!(
                execute(&q, &checked, &nodes, ".yidam/corpus", None)
                    .matched
                    .len(),
                1,
                "{query}"
            );
        }
        let q = super::super::lang::parse("note[observed_on=2026-09]").unwrap();
        assert!(execute(&q, &checked, &nodes, ".yidam/corpus", None)
            .matched
            .is_empty());
    }

    // ── `*` ───────────────────────────────────────────────────────────────────

    #[test]
    fn a_star_matches_only_the_classes_the_check_narrowed_to() {
        let out = run("*[claim_tag=open]", vec![vec!["concept", "reach"]]);
        assert_eq!(out.matched, vec!["reach/tailwater.yml"]);
    }

    // ── cost ──────────────────────────────────────────────────────────────────

    /// `nodes_read` is the agent's cost, not the process's: every node tested plus every
    /// node landed on. Charging only for matches would make a filter that rejects everything
    /// look free.
    /// Every node of a candidate class is charged, matching or not — a predicate has to be
    /// evaluated to reject one. Nodes of *other* classes are not: narrowing by class is a
    /// directory listing, and charging for them would make an anchored lookup cost exactly
    /// what a full scan costs.
    #[test]
    fn nodes_read_counts_the_candidate_class_and_not_the_whole_corpus() {
        let out = run("gage[parameter=00060]", vec![vec!["gage"]]);
        assert_eq!(out.matched.len(), 1);
        assert_eq!(
            out.cost.nodes_read, 2,
            "both gages tested, neither concept nor reach"
        );
        assert_eq!(out.cost.corpus_nodes, 4);
    }

    /// The narrowing is what the check decided, so `*` with no predicate pays for
    /// everything and a narrowed `*` does not.
    #[test]
    fn a_star_with_no_narrowing_pays_for_the_whole_corpus() {
        let all = vec!["concept", "gage", "reach"];
        assert_eq!(run("*", vec![all.clone()]).cost.nodes_read, 4);
        assert_eq!(
            run("*[claim_tag=open]", vec![vec!["reach"]])
                .cost
                .nodes_read,
            1
        );
    }

    #[test]
    fn the_projection_carries_the_selected_fields_and_its_own_size() {
        let nodes = corpus();
        let (rows, chars) = project(
            &["gage/canyon.yml".to_string()],
            &nodes,
            ".yidam/corpus",
            &["node".into(), "class".into(), "properties.parameter".into()],
        );
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0]["class"], serde_json::json!("gage"));
        assert_eq!(rows[0]["properties.parameter"], serde_json::json!("00060"));
        assert!(chars > 0);
    }

    #[test]
    fn a_selected_property_a_node_lacks_is_null_rather_than_missing() {
        let nodes = corpus();
        let (rows, _) = project(
            &["gage/canyon.yml".to_string()],
            &nodes,
            ".yidam/corpus",
            &["properties.claim_tag".into()],
        );
        assert_eq!(rows[0]["properties.claim_tag"], serde_json::Value::Null);
    }
}
