//! `yidam query` — typed pattern execution over the resolved graph (RFC-0018, #261).
//!
//! Six export formats exist so the graph can be queried somewhere that is not yidam. Until
//! this, the entire traversal surface here was `neighbors --depth N`, which chains outbound
//! and inbound edges unconditionally and filters on neither relationship nor direction —
//! carrying both out as *labels* on the result and reading neither as an input. E1 typed the
//! graph and nothing traversed by any of it; `unlicensed-edge`'s own rationale names the gap
//! it left, that "a traversal that walks by relationship will not find it".
//!
//! # `query` never gates
//!
//! Exit 1 here says the *query* was wrong, never that the corpus is. A query that runs and
//! matches nothing exits 0. It appears in `--help` without the `*` that marks the commands
//! which write, and it writes nothing.
//!
//! Exit 2 is not available and is not borrowed: its only site is clap's pre-dispatch arm for
//! an unrecognised subcommand, which `tests/binary_pin.rs` pins. A rejection therefore
//! **emits its report and then exits 1**, the shape `doctor`, `regen`, `rename` and
//! `index-verify` already have — returning `Err` instead would print `Error: {:?}` with no
//! envelope, and every rejection this surface specifies would be invisible to a JSON
//! consumer.

pub mod anchor;
pub mod check;
pub mod exec;
pub mod lang;

use anyhow::Result;

use crate::paths::{repo_root, yidam_corpus_dir};
use crate::retrieval::Retrieval;
use crate::walk::{walk_corpus_instances, walk_ont_files};

/// Fields a projection may name, beyond `properties.<name>`.
const KNOWN_FIELDS: &[&str] = &["node", "class", "label", "description", "body"];

pub const DEFAULT_SELECT: &str = "node,class,label";
pub const DEFAULT_LIMIT: usize = 50;

/// How wide a similarity anchor opens, by default.
///
/// **One.** An anchor is a starting point, not an answer: a five-wide anchor followed by a
/// two-hop walk is a flood wearing a type, and the whole claim under test is that entering at
/// the right node and walking typed edges beats reading five and hoping. `retrieve`'s own
/// default of 5 is right for retrieval, where the caller *is* the ranking, and wrong here.
/// The report lists the entry nodes with their scores, so a `k` that was too narrow is
/// visible rather than inferred.
pub const DEFAULT_ANCHOR_K: usize = 1;

/// What a query run is asked for, beyond the query itself.
pub struct Options {
    pub select: Vec<String>,
    pub limit: usize,
    pub anchor_k: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            select: DEFAULT_SELECT.split(',').map(String::from).collect(),
            limit: DEFAULT_LIMIT,
            anchor_k: DEFAULT_ANCHOR_K,
        }
    }
}

/// Everything a query reads about the corpus, loaded once.
///
/// Separated from [`run`] for the MCP server, which answers many queries against one corpus
/// and must not re-walk `.yidam/corpus` per call. The CLI loads one and drops it.
pub struct Graph {
    pub nodes: Vec<crate::cmd::lint::checks::Node>,
    pub classes: Vec<crate::cmd::lint::checks::Class>,
    pub universal: crate::universal::Universal,
    /// Repo-relative, e.g. `.yidam/corpus` — the prefix node ids are stripped of.
    pub corpus_dir: String,
}

impl Graph {
    pub fn load(root: &std::path::Path) -> Graph {
        let corpus_dir = yidam_corpus_dir(root);
        let overlay = crate::cmd::lint::Overlay::default();
        let nodes = crate::cmd::lint::checks::load_nodes(
            root,
            &walk_corpus_instances(&corpus_dir),
            &overlay,
        );
        let classes =
            crate::cmd::lint::checks::load_classes(root, &walk_ont_files(&corpus_dir), &overlay);
        let rel = corpus_dir
            .strip_prefix(root)
            .unwrap_or(&corpus_dir)
            .to_string_lossy()
            .replace('\\', "/");
        Graph {
            nodes,
            classes,
            universal: crate::universal::Universal::load(root),
            corpus_dir: rel,
        }
    }
}

/// A step, echoed back as structure.
///
/// The report carries the *parsed* query as well as the string, so a programmatic consumer
/// can read back what it asked without re-parsing — which is the reason RFC-0018 does not
/// also offer a structured input form.
#[derive(Debug, serde::Serialize)]
pub struct StepView {
    pub class: String,
    pub anchor: Option<String>,
    pub predicates: Vec<String>,
    /// Classes this step may match after `*` narrowing.
    pub classes: Vec<String>,
    /// The hop that *leaves* this step, if there is one.
    pub hop: Option<HopView>,
}

#[derive(Debug, serde::Serialize)]
pub struct HopView {
    pub relationship: String,
    pub direction: &'static str,
}

#[derive(Debug, serde::Serialize)]
pub struct QueryReport {
    pub query: String,
    /// `local` always, for now. Traversing a package boundary is E3 (#251), and a client
    /// must never have to infer the scope of an answer.
    pub scope: &'static str,
    pub steps: Vec<StepView>,
    /// Present and null when the query ran, so a consumer testing the key does not have to
    /// distinguish "accepted" from "a binary too old to say".
    pub rejected: Option<check::Rejection>,
    /// What the similarity anchor did, or null when the query had none.
    pub anchor: Option<anchor::Anchor>,
    pub diagnostics: Vec<check::Diagnostic>,
    /// Nodes satisfying the final step, capped by `--limit`.
    pub results: Vec<exec::Row>,
    /// How many satisfied it. Always the full count — see the note on `--limit` in
    /// [`exec::Cost`]'s docs: computing this requires the full candidate walk, so `--limit`
    /// bounds the projection and not the traversal.
    pub matched: usize,
    pub returned: usize,
    pub cost: exec::Cost,
    /// True when the corpus declares no classes at all, in which case class names were not
    /// checked — the carve-out `unknown_class` itself makes.
    pub unschematised: bool,
}

fn view(query: &lang::Query, checked: &check::Checked) -> Vec<StepView> {
    query
        .steps
        .iter()
        .enumerate()
        .map(|(index, step)| StepView {
            class: step.class.clone(),
            anchor: step.anchor.clone(),
            predicates: step
                .filter
                .iter()
                .map(|p| format!("{}{}{}", p.prop, p.op.as_str(), p.value))
                .collect(),
            classes: checked.narrowed.get(index).cloned().unwrap_or_default(),
            hop: query.hops.get(index).map(|h| HopView {
                relationship: h.relationship.clone(),
                direction: match h.direction {
                    lang::Dir::Out => "out",
                    lang::Dir::In => "in",
                },
            }),
        })
        .collect()
}

fn rejected_report(query: &str, rejection: check::Rejection) -> QueryReport {
    QueryReport {
        query: query.to_string(),
        scope: "local",
        steps: Vec::new(),
        rejected: Some(rejection),
        anchor: None,
        diagnostics: Vec::new(),
        results: Vec::new(),
        matched: 0,
        returned: 0,
        cost: exec::Cost::default(),
        unschematised: false,
    }
}

/// Build the report, loading the corpus — and, only if the query anchors, the index.
///
/// The index is loaded behind the parse rather than beside it: decoding the vector table is
/// the most expensive thing this command can do, and the overwhelming majority of queries
/// never anchor. A parse that fails twice costs nothing worth measuring; a corpus that decodes
/// its embeddings on every `yidam query reach` does.
pub fn run(root: &std::path::Path, text: &str, opts: &Options) -> QueryReport {
    let graph = Graph::load(root);
    let anchored = lang::parse(text).is_ok_and(|q| q.steps.iter().any(|s| s.anchor.is_some()));
    if !anchored {
        return run_on(&graph, None, text, opts);
    }
    match load_index(root) {
        Ok(retrieval) => run_on(&graph, Some(&retrieval), text, opts),
        // Not a degraded anchor: a degraded one loaded fine and will answer badly. This is an
        // index that could not be read at all, and answering the class scan instead would be
        // answering a different question under the query the caller typed.
        Err(e) => rejected_report(
            text,
            check::Rejection {
                step: None,
                code: "anchor-unresolvable",
                message: format!("the similarity anchor needs the index, and it did not load: {e}"),
            },
        ),
    }
}

fn load_index(root: &std::path::Path) -> Result<Retrieval> {
    let model = crate::model::load_domain_model(root)?;
    Ok(crate::retrieval::load(&model)?.0)
}

/// Build the report against an already-loaded corpus.
///
/// `retrieval` is `None` when the caller knows the query does not anchor. An anchored query
/// arriving here with `None` is rejected rather than silently run unanchored — dropping an
/// anchor would return the whole class and look exactly like a query that worked.
pub fn run_on(
    graph: &Graph,
    retrieval: Option<&Retrieval>,
    text: &str,
    opts: &Options,
) -> QueryReport {
    let parsed = match lang::parse(text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return rejected_report(
                text,
                check::Rejection {
                    step: e.token,
                    code: "parse",
                    message: e.message,
                },
            )
        }
    };

    // **An anchor is an entry, and only the first step is entered.** The grammar allows one
    // anywhere; a later step is *arrived at* by a hop, and ranking a set that a typed edge
    // already produced is a similarity filter — a different operation, with a different cost
    // model and a different meaning for `matched`. RFC-0018 specifies the entry form and only
    // that, so the other form is refused with the reason rather than quietly reinterpreted.
    if let Some(step) = parsed
        .steps
        .iter()
        .skip(1)
        .position(|s| s.anchor.is_some())
        .map(|i| i + 1)
    {
        return rejected_report(
            text,
            check::Rejection {
                step: Some(step),
                code: "anchor-not-entry",
                message: "an anchor enters the graph, and only the first step is entered — \
                          this step is reached by a hop. Anchor the first step instead, or \
                          filter this one with a predicate."
                    .to_string(),
            },
        );
    }

    if let Some(bad) = opts
        .select
        .iter()
        .find(|f| !KNOWN_FIELDS.contains(&f.as_str()) && !f.starts_with("properties."))
    {
        return rejected_report(
            text,
            check::Rejection {
                step: None,
                code: "unknown-field",
                message: format!(
                    "`{bad}` is not a projectable field — {} or `properties.<name>`",
                    KNOWN_FIELDS.join(", ")
                ),
            },
        );
    }

    let schema = check::Schema {
        classes: &graph.classes,
        universal: &graph.universal,
        authored: exec::authored(&graph.nodes),
    };

    let checked = match check::check(&parsed, &schema) {
        Ok(checked) => checked,
        Err(rejection) => return rejected_report(text, rejection),
    };

    // Resolved after the typecheck, never before. Retrieval is the expensive half and a
    // rejected query must not pay for it — and the narrowed class set the anchor filters on
    // is something only the check knows.
    let resolved = match parsed.steps[0].anchor.as_deref() {
        None => None,
        Some(text_anchor) => {
            let Some(retrieval) = retrieval else {
                return rejected_report(
                    text,
                    check::Rejection {
                        step: Some(0),
                        code: "anchor-unavailable",
                        message: "this caller supplied no index, so a similarity anchor \
                                  cannot be resolved"
                            .to_string(),
                    },
                );
            };
            let empty = Vec::new();
            let classes = checked.narrowed.first().unwrap_or(&empty);
            match anchor::resolve(
                retrieval,
                0,
                text_anchor,
                classes,
                opts.anchor_k,
                &graph.nodes,
                &graph.corpus_dir,
            ) {
                Ok(resolved) => Some(resolved),
                Err(message) => {
                    return rejected_report(
                        text,
                        check::Rejection {
                            step: Some(0),
                            code: "anchor-unresolvable",
                            message,
                        },
                    )
                }
            }
        }
    };

    let outcome = exec::execute(
        &parsed,
        &checked,
        &graph.nodes,
        &graph.corpus_dir,
        resolved.as_ref(),
    );
    let matched = outcome.matched.len();
    let shown: Vec<String> = outcome.matched.into_iter().take(opts.limit).collect();
    let (results, chars) = exec::project(&shown, &graph.nodes, &graph.corpus_dir, &opts.select);

    QueryReport {
        query: text.to_string(),
        scope: "local",
        steps: view(&parsed, &checked),
        rejected: None,
        anchor: resolved.map(|r| r.anchor),
        diagnostics: checked.diagnostics,
        returned: results.len(),
        results,
        matched,
        cost: exec::Cost {
            chars,
            tokens: chars / 4,
            ..outcome.cost
        },
        unschematised: graph.classes.is_empty(),
    }
}

pub fn render(report: &QueryReport) -> String {
    if let Some(rejection) = &report.rejected {
        return format!(
            "rejected ({}){}: {}",
            rejection.code,
            match rejection.step {
                Some(step) => format!(" at step {}", step + 1),
                None => String::new(),
            },
            rejection.message
        );
    }
    let mut out = format!(
        "{} result(s){}\n",
        report.matched,
        match report.matched > report.returned {
            true => format!(" — showing {}", report.returned),
            false => String::new(),
        }
    );
    for row in &report.results {
        let field = |name: &str| {
            row.get(name)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };
        let (node, label) = (field("node"), field("label"));
        out.push_str(&format!(
            "  {}{}\n",
            match label.is_empty() {
                true => node.clone(),
                false => format!("{label}  ({node})"),
            },
            // Anything the caller selected beyond the default is worth showing, or
            // `--select` would silently do nothing in text mode.
            row.iter()
                .filter(|(k, _)| !["node", "class", "label"].contains(&k.as_str()))
                .map(|(k, v)| format!("  {k}={}", v.as_str().unwrap_or("—")))
                .collect::<String>()
        ));
    }
    if let Some(a) = &report.anchor {
        // Which nodes it entered on, always — an answer that surprises is usually an anchor
        // that landed somewhere else, and that is one line away rather than a `--format json`
        // away.
        out.push_str(&format!(
            "  anchored on {} — {}\n",
            match a.entries.is_empty() {
                true => "nothing".to_string(),
                false => a
                    .entries
                    .iter()
                    .map(|e| format!("{} ({:.2})", e.node, e.score))
                    .collect::<Vec<_>>()
                    .join(", "),
            },
            match (a.degraded_reason, a.repair) {
                (Some(reason), Some(repair)) =>
                    format!("keyword search, not similarity ({reason}); {repair}"),
                _ => "semantic search".to_string(),
            }
        ));
    }
    for d in &report.diagnostics {
        out.push_str(&format!(
            "  [{}] step {}: {}\n",
            d.level,
            d.step + 1,
            d.message
        ));
    }
    if report.unschematised {
        out.push_str("  [info] this corpus declares no classes, so class names were not checked\n");
    }
    let c = &report.cost;
    out.push_str(&format!(
        "{} step(s), {} edge(s) walked, {} of {} node(s) read, ~{} token(s)\n",
        c.steps, c.edges_walked, c.nodes_read, c.corpus_nodes, c.tokens
    ));
    out.trim_end().to_string()
}

/// Execute a query against the resolved graph.
pub fn query(
    text: &str,
    select: Option<String>,
    limit: usize,
    anchor_k: usize,
    format: crate::report::Format,
) -> Result<()> {
    let root = repo_root()?;
    let opts = Options {
        select: select
            .unwrap_or_else(|| DEFAULT_SELECT.to_string())
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect(),
        limit,
        anchor_k: anchor_k.max(1),
    };
    let report = run(&root, text, &opts);
    let rejected = report.rejected.is_some();
    if format.is_json() {
        crate::report::emit(&root, report)?;
    } else {
        println!("{}", render(&report));
    }
    if rejected {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        Options::default()
    }

    fn with(select: &[&str], limit: usize) -> Options {
        Options {
            select: select.iter().map(|f| f.to_string()).collect(),
            limit,
            ..Options::default()
        }
    }

    /// A repository with a corpus, materialized so the loaders have something to read.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("reach")).unwrap();
        std::fs::create_dir_all(corpus.join("gage")).unwrap();
        std::fs::write(
            corpus.join("reach.ont.yml"),
            "class: reach\nproperties:\n  - name: regulated\n    type: string\nedges:\n  \
             - relationship: measured-by\n    target: gage\n    direction: out\n",
        )
        .unwrap();
        std::fs::write(corpus.join("gage.ont.yml"), "class: gage\n").unwrap();
        std::fs::write(
            corpus.join("reach/tailwater.yml"),
            "class: reach\nlabel: Tailwater\nproperties:\n  regulated: \"yes — outlet works\"\n\
             links:\n  - target: ../gage/canyon.yml\n    relationship: measured-by\n",
        )
        .unwrap();
        std::fs::write(
            corpus.join("gage/canyon.yml"),
            "class: gage\nlabel: Canyon Outlet\n",
        )
        .unwrap();
        dir
    }

    #[test]
    fn a_typed_hop_answers_and_reports_its_cost() {
        let dir = fixture();
        let report = run(dir.path(), "reach -measured-by-> gage", &opts());
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        assert_eq!(report.matched, 1);
        assert_eq!(
            report.results[0]["label"],
            serde_json::json!("Canyon Outlet")
        );
        assert_eq!(report.cost.steps, 2);
        assert_eq!(report.cost.edges_walked, 1);
        assert!(report.cost.tokens > 0);
        assert_eq!(report.scope, "local");
    }

    /// #261's acceptance: an unknown name fails with a diagnosis, never as an empty result.
    #[test]
    fn an_unknown_class_is_a_diagnosis_and_not_an_empty_result() {
        let dir = fixture();
        let report = run(dir.path(), "gauge", &opts());
        let rejection = report.rejected.expect("must be rejected");
        assert_eq!(rejection.code, "unknown-class");
        assert!(rejection.message.contains("gage"), "{}", rejection.message);
        assert_eq!(report.matched, 0);
    }

    #[test]
    fn a_query_that_matches_nothing_is_not_a_rejection() {
        let dir = fixture();
        let report = run(dir.path(), "reach[regulated=no]", &opts());
        assert!(report.rejected.is_none());
        assert_eq!(report.matched, 0);
    }

    #[test]
    fn a_parse_error_is_reported_on_the_contract_rather_than_as_prose() {
        let dir = fixture();
        let report = run(dir.path(), "reach -measured-by->", &opts());
        assert_eq!(report.rejected.unwrap().code, "parse");
    }

    /// #263: the anchor resolves, the walk stays typed, and the report says which path
    /// retrieval took. No index in this fixture, so it is the keyword one — and the point of
    /// the assertion is that it *says so* rather than that it degraded.
    #[test]
    fn an_anchored_query_enters_by_similarity_and_says_how_it_resolved() {
        let dir = fixture();
        let report = run(
            dir.path(),
            r#"reach~"outlet works" -measured-by-> gage"#,
            &opts(),
        );
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        let anchor = report
            .anchor
            .clone()
            .expect("an anchored query reports its anchor");
        assert_eq!(anchor.step, 0);
        assert_eq!(anchor.k, DEFAULT_ANCHOR_K);
        assert_eq!(anchor.entries.len(), 1);
        assert_eq!(anchor.entries[0].node, "reach/tailwater.yml");
        assert!(anchor.degraded);
        assert_eq!(anchor.degraded_reason, Some("no_index"));
        assert_eq!(report.matched, 1);
        assert!(render(&report).contains("anchored on reach/tailwater.yml"));
    }

    /// An anchor is an entry. Ranking a set a typed edge already produced is a different
    /// operation, and refusing it beats reinterpreting it.
    #[test]
    fn an_anchor_on_a_later_step_is_refused_with_the_reason() {
        let dir = fixture();
        let report = run(dir.path(), r#"reach -measured-by-> gage~"canyon""#, &opts());
        let rejection = report.rejected.expect("must be rejected");
        assert_eq!(rejection.code, "anchor-not-entry");
        assert_eq!(rejection.step, Some(1));
    }

    /// The anchored entry replaces the class scan; that substitution is the whole mechanism.
    /// Same class, one anchored query and one not, over a class with more than one instance.
    #[test]
    fn an_anchor_enters_at_one_node_where_the_class_scan_enters_at_all_of_them() {
        let dir = fixture();
        std::fs::write(
            dir.path().join(".yidam/corpus/reach/canyon.yml"),
            "class: reach\nlabel: Canyon\nproperties:\n  regulated: \"no\"\n",
        )
        .unwrap();
        let scanned = run(dir.path(), "reach", &opts());
        let anchored = run(dir.path(), r#"reach~"tailwater outlet""#, &opts());
        assert_eq!(scanned.matched, 2);
        assert_eq!(anchored.matched, 1);
        assert_eq!(
            anchored.results[0]["node"],
            serde_json::json!("reach/tailwater.yml")
        );

        // And what it cost. A *degraded* anchor still reads every candidate to score it — the
        // narrowing arrives with the index, not with the syntax — so this asserts the honest
        // number rather than the flattering one. `bench` refuses to publish a figure from
        // this path for exactly that reason.
        assert_eq!(scanned.cost.nodes_read, 2);
        assert_eq!(anchored.cost.nodes_read, 2);
    }

    #[test]
    fn an_unknown_projection_field_is_refused_with_the_known_ones() {
        let dir = fixture();
        let report = run(dir.path(), "reach", &with(&["colour"], 50));
        let rejection = report.rejected.unwrap();
        assert_eq!(rejection.code, "unknown-field");
        assert!(rejection.message.contains("properties.<name>"));
    }

    /// `--limit` bounds the projection and not the traversal, so `matched` stays true.
    #[test]
    fn a_limit_caps_the_projection_and_not_the_count() {
        let dir = fixture();
        let report = run(dir.path(), "*", &with(&["node", "class", "label"], 1));
        assert_eq!(report.matched, 2);
        assert_eq!(report.returned, 1);
        assert!(render(&report).contains("showing 1"));
    }

    #[test]
    fn a_corpus_with_no_ontology_says_so_rather_than_rejecting_every_name() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".yidam/corpus/thing")).unwrap();
        std::fs::write(
            dir.path().join(".yidam/corpus/thing/one.yml"),
            "class: thing\nlabel: One\n",
        )
        .unwrap();
        let report = run(dir.path(), "thing", &opts());
        assert!(report.rejected.is_none());
        assert!(report.unschematised);
        assert!(render(&report).contains("declares no classes"));
    }

    #[test]
    fn the_text_report_shows_a_selected_property() {
        let dir = fixture();
        let report = run(
            dir.path(),
            "reach",
            &with(&["node", "properties.regulated"], 50),
        );
        assert!(render(&report).contains("properties.regulated=yes — outlet works"));
    }

    #[test]
    fn the_report_echoes_the_parsed_query_as_structure() {
        let dir = fixture();
        let report = run(dir.path(), "reach -measured-by-> gage", &opts());
        assert_eq!(report.steps[0].class, "reach");
        assert_eq!(
            report.steps[0].hop.as_ref().unwrap().relationship,
            "measured-by"
        );
        assert_eq!(report.steps[0].hop.as_ref().unwrap().direction, "out");
        assert!(report.steps[1].hop.is_none());
    }
}
