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

pub mod check;
pub mod exec;
pub mod lang;

use anyhow::Result;

use crate::paths::{repo_root, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_ont_files};

/// Fields a projection may name, beyond `properties.<name>`.
const KNOWN_FIELDS: &[&str] = &["node", "class", "label", "description", "body"];

pub const DEFAULT_SELECT: &str = "node,class,label";
pub const DEFAULT_LIMIT: usize = 50;

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
        diagnostics: Vec::new(),
        results: Vec::new(),
        matched: 0,
        returned: 0,
        cost: exec::Cost::default(),
        unschematised: false,
    }
}

/// Build the report. Separated from [`query`] so tests can run it against a fixture root.
pub fn run(root: &std::path::Path, text: &str, select: &[String], limit: usize) -> QueryReport {
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

    // The anchor parses and typechecks — the class is required, so the first hop's verdict
    // is static — but resolving it needs the index, and exposing it is #263's issue. Said
    // rather than silently ignored: a query whose anchor was dropped would return the whole
    // class and look like it had worked.
    if let Some(step) = parsed.steps.iter().position(|s| s.anchor.is_some()) {
        return rejected_report(
            text,
            check::Rejection {
                step: Some(step),
                code: "anchor-unavailable",
                message: "a similarity anchor needs the vector index and lands with #263; \
                          the class predicate and the typed hops work today"
                    .to_string(),
            },
        );
    }

    if let Some(bad) = select
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

    let corpus_dir = yidam_corpus_dir(root);
    let overlay = crate::cmd::lint::Overlay::default();
    let nodes =
        crate::cmd::lint::checks::load_nodes(root, &walk_corpus_instances(&corpus_dir), &overlay);
    let classes =
        crate::cmd::lint::checks::load_classes(root, &walk_ont_files(&corpus_dir), &overlay);
    let universal = crate::universal::Universal::load(root);
    let schema = check::Schema {
        classes: &classes,
        universal: &universal,
        authored: exec::authored(&nodes),
    };

    let checked = match check::check(&parsed, &schema) {
        Ok(checked) => checked,
        Err(rejection) => return rejected_report(text, rejection),
    };

    let rel_corpus = corpus_dir
        .strip_prefix(root)
        .unwrap_or(&corpus_dir)
        .to_string_lossy()
        .replace('\\', "/");
    let outcome = exec::execute(&parsed, &checked, &nodes, &rel_corpus);
    let matched = outcome.matched.len();
    let shown: Vec<String> = outcome.matched.into_iter().take(limit).collect();
    let (results, chars) = exec::project(&shown, &nodes, &rel_corpus, select);

    QueryReport {
        query: text.to_string(),
        scope: "local",
        steps: view(&parsed, &checked),
        rejected: None,
        diagnostics: checked.diagnostics,
        returned: results.len(),
        results,
        matched,
        cost: exec::Cost {
            chars,
            tokens: chars / 4,
            ..outcome.cost
        },
        unschematised: classes.is_empty(),
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
    format: crate::report::Format,
) -> Result<()> {
    let root = repo_root()?;
    let select: Vec<String> = select
        .unwrap_or_else(|| DEFAULT_SELECT.to_string())
        .split(',')
        .map(|f| f.trim().to_string())
        .filter(|f| !f.is_empty())
        .collect();
    let report = run(&root, text, &select, limit);
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

    fn select() -> Vec<String> {
        DEFAULT_SELECT.split(',').map(String::from).collect()
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
        let report = run(dir.path(), "reach -measured-by-> gage", &select(), 50);
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
        let report = run(dir.path(), "gauge", &select(), 50);
        let rejection = report.rejected.expect("must be rejected");
        assert_eq!(rejection.code, "unknown-class");
        assert!(rejection.message.contains("gage"), "{}", rejection.message);
        assert_eq!(report.matched, 0);
    }

    #[test]
    fn a_query_that_matches_nothing_is_not_a_rejection() {
        let dir = fixture();
        let report = run(dir.path(), "reach[regulated=no]", &select(), 50);
        assert!(report.rejected.is_none());
        assert_eq!(report.matched, 0);
    }

    #[test]
    fn a_parse_error_is_reported_on_the_contract_rather_than_as_prose() {
        let dir = fixture();
        let report = run(dir.path(), "reach -measured-by->", &select(), 50);
        assert_eq!(report.rejected.unwrap().code, "parse");
    }

    /// Dropping an anchor silently would return the whole class and look like it worked.
    #[test]
    fn an_anchored_query_says_the_anchor_is_not_available_yet() {
        let dir = fixture();
        let report = run(dir.path(), r#"reach~"below the dam""#, &select(), 50);
        let rejection = report.rejected.expect("must be rejected");
        assert_eq!(rejection.code, "anchor-unavailable");
        assert!(rejection.message.contains("#263"));
    }

    #[test]
    fn an_unknown_projection_field_is_refused_with_the_known_ones() {
        let dir = fixture();
        let report = run(dir.path(), "reach", &["colour".to_string()], 50);
        let rejection = report.rejected.unwrap();
        assert_eq!(rejection.code, "unknown-field");
        assert!(rejection.message.contains("properties.<name>"));
    }

    /// `--limit` bounds the projection and not the traversal, so `matched` stays true.
    #[test]
    fn a_limit_caps_the_projection_and_not_the_count() {
        let dir = fixture();
        let report = run(dir.path(), "*", &select(), 1);
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
        let report = run(dir.path(), "thing", &select(), 50);
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
            &["node".into(), "properties.regulated".into()],
            50,
        );
        assert!(render(&report).contains("properties.regulated=yes — outlet works"));
    }

    #[test]
    fn the_report_echoes_the_parsed_query_as_structure() {
        let dir = fixture();
        let report = run(dir.path(), "reach -measured-by-> gage", &select(), 50);
        assert_eq!(report.steps[0].class, "reach");
        assert_eq!(
            report.steps[0].hop.as_ref().unwrap().relationship,
            "measured-by"
        );
        assert_eq!(report.steps[0].hop.as_ref().unwrap().direction, "out");
        assert!(report.steps[1].hop.is_none());
    }
}
