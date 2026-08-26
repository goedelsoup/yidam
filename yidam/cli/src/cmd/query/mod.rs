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

pub mod absence;
pub mod anchor;
pub mod at;
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
    /// The repository this corpus was loaded from.
    ///
    /// Carried so an *absent* answer can ask a question a present one never needs to: whether
    /// an installed dependency holds what this corpus does not (#283). The look is lazy and
    /// happens only when the answer came back empty, which is the one time it is worth a
    /// directory walk per package.
    pub root: std::path::PathBuf,
    /// Installed dependencies, when the caller asked to query across them (`--across`, #268).
    ///
    /// Empty by default, and that is the boundary: a query that did not ask cannot see a
    /// foreign node, and one that did sees it labelled.
    pub across: Vec<Foreign>,
}

/// A dependency's corpus, as a query sees it.
///
/// **Its own everything.** Its own nodes, its own classes, its own `universal.yml`, and its
/// own execution — the query runs once per corpus rather than once over a merged node list.
/// That is not a performance choice: it is what makes it *structurally impossible* for a hop
/// to cross the boundary, because no execution ever holds two corpora's nodes at once. The
/// alternative — merge and rely on relative paths not resolving across directories — is true
/// today and is a property of the filesystem layout rather than of this code.
pub struct Foreign {
    pub package: String,
    pub nodes: Vec<crate::cmd::lint::checks::Node>,
    pub classes: Vec<crate::cmd::lint::checks::Class>,
    pub universal: crate::universal::Universal,
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
            root: root.to_path_buf(),
            across: Vec::new(),
        }
    }

    /// The same, plus every installed dependency's corpus.
    ///
    /// A dependency's `universal.yml` is read from its own bundle, not inherited from here:
    /// a corpus-wide property declaration is that corpus's, and applying this repository's to
    /// a dependency's nodes would accept property names it never declared.
    pub fn across(root: &std::path::Path) -> Graph {
        let overlay = crate::cmd::lint::Overlay::default();
        let across = crate::deps::resolved(root)
            .into_iter()
            .map(|dep| {
                let dir = dep.corpus_dir;
                // The dependency's root is its corpus dir's parent — `.yidam/tonpa/<pkg>/`
                // for a fetched one, `<sibling>/.yidam/` for a path one. `rel_of` strips it,
                // so ids come out as `<class>/<name>.yml` on both sides of the boundary and
                // the package name is what distinguishes them.
                let owner = dir.parent().unwrap_or(&dir).to_path_buf();
                Foreign {
                    nodes: crate::cmd::lint::checks::load_nodes(
                        &owner,
                        &walk_corpus_instances(&dir),
                        &overlay,
                    ),
                    classes: crate::cmd::lint::checks::load_classes(
                        &owner,
                        &walk_ont_files(&dir),
                        &overlay,
                    ),
                    universal: crate::universal::Universal::parse(
                        &std::fs::read_to_string(dir.join("universal.yml")).unwrap_or_default(),
                    ),
                    corpus_dir: "corpus".to_string(),
                    package: dep.name,
                }
            })
            .collect();
        Graph {
            across,
            ..Graph::load(root)
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
    /// `local`, or `across` when the query ran over the dependency set too (#268).
    ///
    /// A client must never have to infer the scope of an answer, which is why this is a field
    /// and not something read off the presence of qualified ids: a corpus with no
    /// dependencies installed answers `across` with the same rows a `local` run gives, and
    /// the difference between "nothing foreign matched" and "nothing foreign was looked at"
    /// is the whole point.
    pub scope: &'static str,
    pub steps: Vec<StepView>,
    /// Present and null when the query ran, so a consumer testing the key does not have to
    /// distinguish "accepted" from "a binary too old to say".
    pub rejected: Option<check::Rejection>,
    /// What the similarity anchor did, or null when the query had none.
    pub anchor: Option<anchor::Anchor>,
    /// Why the answer is empty, when it is — and null when the query matched something.
    ///
    /// Present-and-null rather than absent, for the reason every other field on this report
    /// is: a client testing the key must not have to distinguish "the query found something"
    /// from "a binary too old to say why it did not". #283's whole finding is that an empty
    /// result is where an agent invents, so the one thing this key must never be is ambiguous.
    pub absence: Option<absence::Absence>,
    /// The commit this answer is about, or null for the working tree.
    ///
    /// Present-and-null rather than absent: an answer about a past commit and an answer about
    /// now are different claims, and a consumer must never have to infer which it holds from
    /// the absence of a key.
    pub at: Option<at::Revision>,
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

pub(crate) fn rejected_report(query: &str, rejection: check::Rejection) -> QueryReport {
    QueryReport {
        query: query.to_string(),
        scope: "local",
        steps: Vec::new(),
        rejected: Some(rejection),
        anchor: None,
        // A rejected query never ran, so there is no absence to diagnose. The two are
        // deliberately not merged: `rejected` says the query is wrong, `absence` says the
        // query is right and the corpus is quiet, and a surface that collapsed them would
        // tell an agent its typo was a true negative.
        absence: None,
        at: None,
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
    run_scoped(root, text, opts, false)
}

/// [`run`], over every installed dependency's corpus as well (`--across`, #268).
pub fn run_across(root: &std::path::Path, text: &str, opts: &Options) -> QueryReport {
    run_scoped(root, text, opts, true)
}

/// Two named entry points rather than a boolean at every call site: `run(root, q, o)` and
/// `run(root, q, o, true)` read the same at a glance and mean opposite things about whether a
/// foreign node can appear in the answer.
fn run_scoped(root: &std::path::Path, text: &str, opts: &Options, across: bool) -> QueryReport {
    let graph = match across {
        true => Graph::across(root),
        false => Graph::load(root),
    };
    let anchored = lang::parse(text).is_ok_and(|q| q.steps.iter().any(|s| s.anchor.is_some()));
    if !anchored {
        return run_on(&Context::now(&graph, None), text, opts);
    }
    match load_index(root) {
        Ok(retrieval) => run_on(&Context::now(&graph, Some(&retrieval)), text, opts),
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

/// Build the report against the corpus as it stood at one commit.
///
/// HEAD's corpus is loaded too, and only to answer one question: would this query be judged
/// differently today? #262 asks for that in a line — *"where the two disagree, say so rather
/// than silently using either"* — and the cost of saying so is one extra walk of the working
/// tree, which is the cheapest half of this command.
pub fn run_at(
    root: &std::path::Path,
    rev: &str,
    text: &str,
    opts: &Options,
) -> Result<QueryReport> {
    let revision = at::resolve(root, rev)?;
    let graph = Graph::at(root, &revision.commit)?;
    let head = Graph::load(root);
    let ctx = Context {
        graph: &graph,
        // No index. A similarity anchor at a past commit is refused inside `run_on`, with the
        // reason — not by handing it an absent index and letting it report `no_index`, which
        // would be true of nothing and would send a reader off to run `index-build`.
        retrieval: None,
        at: Some(&revision),
        head: Some(&head),
    };
    Ok(run_on(&ctx, text, opts))
}

/// One commit's answer, in a series.
#[derive(Debug, serde::Serialize)]
pub struct SeriesRow {
    pub commit: String,
    pub date: String,
    /// Null when the query typechecked at this commit.
    pub rejected: Option<check::Rejection>,
    pub diagnostics: Vec<check::Diagnostic>,
    pub results: Vec<exec::Row>,
    pub matched: usize,
    pub returned: usize,
    pub cost: exec::Cost,
}

#[derive(Debug, serde::Serialize)]
pub struct SeriesReport {
    pub query: String,
    pub scope: &'static str,
    /// The range as written.
    pub range: String,
    /// One row per commit in the range that touched the corpus, oldest first.
    ///
    /// Filtered by path exactly as `replay`'s own walk is: a row per commit in a range would
    /// be mostly commits where nothing about the corpus moved, and the reader is looking for
    /// the one where the answer did.
    pub series: Vec<SeriesRow>,
}

/// The answer at every commit in a range that touched the corpus.
///
/// A separate report type rather than a `series` field on [`QueryReport`], because a series
/// has no single `matched` and no single `cost` — and a report carrying both shapes would
/// have to leave one half meaningless, which a consumer reads as zero.
pub fn run_between(
    root: &std::path::Path,
    range: &str,
    text: &str,
    opts: &Options,
) -> Result<SeriesReport> {
    let commits = at::commits_in(root, range)?;
    // Once, not once per commit: it is the same tree for every row, and walking it N times
    // would make the divergence note the most expensive thing in the command.
    let head = Graph::load(root);
    // Carried across the whole range rather than rebuilt per commit — see `at::Blobs`. It is
    // the difference between reading the corpus once per commit and reading each object once.
    let mut blobs = at::Blobs::default();
    let mut series = Vec::new();
    for revision in commits {
        let graph = Graph::at_with(root, &revision.commit, &mut blobs)?;
        let ctx = Context {
            graph: &graph,
            retrieval: None,
            at: Some(&revision),
            head: Some(&head),
        };
        let report = run_on(&ctx, text, opts);
        series.push(SeriesRow {
            commit: revision.commit,
            date: revision.date,
            rejected: report.rejected,
            diagnostics: report.diagnostics,
            results: report.results,
            matched: report.matched,
            returned: report.returned,
            cost: report.cost,
        });
    }
    Ok(SeriesReport {
        query: text.to_string(),
        scope: "local",
        range: range.to_string(),
        series,
    })
}

pub(crate) fn load_index(root: &std::path::Path) -> Result<Retrieval> {
    let model = crate::model::load_domain_model(root)?;
    Ok(crate::retrieval::load(&model)?.0)
}

/// What a query runs against.
///
/// A struct rather than four positional arguments, because two of the four exist only for the
/// historical path and passing `None, None` at every present-tense call site is how the
/// meaning of the third one gets lost.
pub struct Context<'a> {
    pub graph: &'a Graph,
    /// `None` when the caller knows the query does not anchor. An anchored query arriving with
    /// `None` is rejected rather than silently run unanchored — dropping an anchor would
    /// return the whole class and look exactly like a query that worked.
    pub retrieval: Option<&'a Retrieval>,
    /// The commit `graph` was reconstructed at, or `None` for the working tree.
    pub at: Option<&'a at::Revision>,
    /// HEAD's corpus, present only alongside `at`.
    ///
    /// Carried so the report can say where the same query would be judged differently today.
    /// The historical ontology decides — it is the schema that commit's data obeys — and this
    /// is what keeps "decides" from meaning "silently picks".
    pub head: Option<&'a Graph>,
}

impl<'a> Context<'a> {
    /// The working tree.
    pub fn now(graph: &'a Graph, retrieval: Option<&'a Retrieval>) -> Self {
        Context {
            graph,
            retrieval,
            at: None,
            head: None,
        }
    }
}

/// Build the report against an already-loaded corpus.
pub fn run_on(ctx: &Context, text: &str, opts: &Options) -> QueryReport {
    let graph = ctx.graph;
    let retrieval = ctx.retrieval;
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

    // **An anchor cannot be evaluated as of a past commit.** The index is built from one
    // commit's text — `index/meta.json` records which — and anchoring at another would enter
    // the graph through today's embeddings and then walk that commit's edges. Degrading to
    // keyword search over the historical text is well-defined and is *a different retrieval
    // than the same query gets at HEAD*, which would make a series where the answer changed
    // because the arm changed. Refused with the reason rather than silently either.
    if ctx.at.is_some() && parsed.steps.iter().any(|s| s.anchor.is_some()) {
        return rejected_report(
            text,
            check::Rejection {
                step: Some(0),
                code: "anchor-at-revision",
                message: "the vector index is built from one commit's text, so a similarity \
                          anchor cannot be resolved as of another. Enter at the class and \
                          filter with a predicate, or drop the revision."
                    .to_string(),
            },
        );
    }

    // **An anchored `--across` is refused, and the reason is the one #268 gives.** The vector
    // index is built from this repository's corpus alone — `embed` walks `.yidam/corpus`, and
    // `IndexConfig::merge_imported_index` names a merge nothing performs — so an anchor would
    // enter through local text and then hand back rows from a dependency's scan beside it.
    // The keyword fallback *could* span, and that is worse rather than better: anchoring would
    // then reach further in the build without the index than in the build with it. Refused
    // uniformly, because a boundary that is invisible at the moment an agent is about to write
    // a claim is the failure this flag exists to prevent.
    if !ctx.graph.across.is_empty() && parsed.steps.iter().any(|s| s.anchor.is_some()) {
        return rejected_report(
            text,
            check::Rejection {
                step: Some(0),
                code: "anchor-across",
                message: "the vector index covers this repository's corpus and not its \
                          dependencies, so an anchored query cannot span them without \
                          entering through local text and answering with foreign rows. Enter \
                          at the class and filter with a predicate, or drop `--across`."
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

    let verdict = check::check(&parsed, &schema);

    // Both ontologies are in hand exactly when the query is about a past commit, so this is
    // where the two verdicts get compared. Computed before the historical one is unwrapped:
    // a rejection deserves the note as much as an answer does, and more, because a reader
    // looking at a refusal wants to know whether it is the query or the year that is wrong.
    let moved: Vec<check::Diagnostic> = match (ctx.at, ctx.head) {
        (Some(revision), Some(head)) => {
            let head_schema = check::Schema {
                classes: &head.classes,
                universal: &head.universal,
                authored: exec::authored(&head.nodes),
            };
            at::divergence(&verdict, &check::check(&parsed, &head_schema), revision)
        }
        _ => Vec::new(),
    };

    let mut checked = match verdict {
        Ok(checked) => checked,
        Err(rejection) => {
            let mut report = rejected_report(text, rejection);
            report.at = ctx.at.cloned();
            report.diagnostics = moved;
            return report;
        }
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
    let mut matched = outcome.matched.len();
    let shown: Vec<String> = outcome.matched.iter().take(opts.limit).cloned().collect();
    let (mut results, mut chars) =
        exec::project(&shown, &graph.nodes, &graph.corpus_dir, &opts.select);
    for row in &mut results {
        // Present and null for this repository's own nodes rather than absent — the
        // convention `retrieve` already follows. A consumer testing the key must not have to
        // distinguish "local" from "a binary too old to say", and #268's requirement is that
        // a result be attributed *at every point of presentation*, which includes the one
        // where everything happens to be local.
        row.insert("origin".into(), serde_json::Value::Null);
    }
    let mut cost = outcome.cost.clone();
    // Taken by value here and the rest of `checked` kept: `view` still needs `narrowed`, and
    // the foreign corpora append to this list.
    let mut diagnostics = std::mem::take(&mut checked.diagnostics);

    for foreign in &graph.across {
        let schema = check::Schema {
            classes: &foreign.classes,
            universal: &foreign.universal,
            authored: exec::authored(&foreign.nodes),
        };
        // Checked against **its own** ontology, and a corpus that cannot answer the query is
        // excluded with the reason rather than failing the run. A shared class name is not
        // agreement — `concept` here and `concept` there are two names — so a dependency that
        // declares neither is not a defect in the query, and a dependency that declares one
        // and licenses the hop differently is answering a question about itself.
        let checked = match check::check(&parsed, &schema) {
            Ok(checked) => checked,
            Err(rejection) => {
                diagnostics.push(check::Diagnostic {
                    level: "info",
                    step: rejection.step.unwrap_or(0),
                    code: "corpus-excluded",
                    message: format!(
                        "`{}` was not queried: {} ({})",
                        foreign.package, rejection.message, rejection.code
                    ),
                });
                continue;
            }
        };
        let outcome = exec::execute(
            &parsed,
            &checked,
            &foreign.nodes,
            &foreign.corpus_dir,
            // No anchor: an anchored `--across` is refused above. Passing `None` here is not
            // a silent downgrade, it is the arm that cannot be reached.
            None,
        );
        matched += outcome.matched.len();
        let shown: Vec<String> = outcome
            .matched
            .into_iter()
            .take(opts.limit.saturating_sub(results.len()))
            .collect();
        let (rows, foreign_chars) =
            exec::project(&shown, &foreign.nodes, &foreign.corpus_dir, &opts.select);
        chars += foreign_chars;
        cost.edges_walked += outcome.cost.edges_walked;
        cost.nodes_read += outcome.cost.nodes_read;
        cost.corpus_nodes += outcome.cost.corpus_nodes;
        for mut row in rows {
            // Qualified at the point the row is built, not at render time. A consumer reading
            // `node` must get an id it can hand back to `get_node`, and an unqualified one
            // from a dependency names a local node that may not exist — or, worse, one that
            // does and is a different thing.
            if let Some(node) = row.get("node").and_then(|v| v.as_str()) {
                let qualified = format!("{}::{node}", foreign.package);
                row.insert("node".into(), serde_json::Value::String(qualified));
            }
            row.insert(
                "origin".into(),
                serde_json::Value::String(foreign.package.clone()),
            );
            results.push(row);
        }
    }

    // Diagnosed only on an empty answer, and only after the dependency loop has had its say:
    // a `--across` run that found a foreign row matched, and the local corpus being quiet is
    // then a fact about the boundary rather than about the answer.
    //
    // The look next door is a directory read per installed package. It is the reason this is
    // behind the emptiness test rather than computed alongside `matched` — an answer with rows
    // has no question to ask, and that is the overwhelming majority of them.
    let absence = match matched {
        0 => Some(absence::diagnose(
            ctx,
            &parsed,
            &checked,
            &schema.authored,
            &outcome,
            resolved.as_ref().map(|r| &r.anchor),
        )),
        _ => None,
    };

    QueryReport {
        query: text.to_string(),
        // Not inferred from whether any foreign row came back. A repository with no
        // dependencies installed answers `across` with exactly the rows a `local` run gives,
        // and "nothing foreign matched" must stay distinguishable from "nothing foreign was
        // looked at".
        scope: match graph.across.is_empty() {
            true => "local",
            false => "across",
        },
        steps: view(&parsed, &checked),
        rejected: None,
        anchor: resolved.map(|r| r.anchor),
        absence,
        at: ctx.at.cloned(),
        diagnostics: diagnostics.into_iter().chain(moved).collect(),
        returned: results.len(),
        results,
        matched,
        cost: exec::Cost {
            chars,
            tokens: chars / 4,
            ..cost
        },
        unschematised: graph.classes.is_empty(),
    }
}

/// A commit, short enough to read and long enough to paste.
fn short(commit: &str) -> &str {
    &commit[..commit.len().min(8)]
}

/// The series, one line per commit, with the answer's *size* rather than its contents.
///
/// A series exists to show where an answer changed, and printing every row's nodes buries
/// that under the rows where nothing moved. The nodes are one line further in — under
/// `--format json`, or under `--at` on the commit the reader has now identified.
pub fn render_series(report: &SeriesReport) -> String {
    let mut out = format!("{} commit(s) touching the corpus\n", report.series.len());
    let mut previous: Option<String> = None;
    for row in &report.series {
        let answer = match &row.rejected {
            Some(rejection) => format!("rejected ({})", rejection.code),
            None => format!("{} result(s)", row.matched),
        };
        // The marker is the point of the report. Two hundred rows of "3 result(s)" and one
        // "4 result(s)" is a series a reader scans; the same rows with the changed one
        // unmarked is a series a reader gives up on.
        let changed = previous.as_deref().is_some_and(|p| p != answer);
        out.push_str(&format!(
            "  {} {}  {}{}\n",
            short(&row.commit),
            row.date,
            answer,
            match changed {
                true => "  ← changed",
                false => "",
            }
        ));
        previous = Some(answer);
    }
    if report.series.is_empty() {
        out.push_str("  no commit in this range touched .yidam/corpus\n");
    }
    out.trim_end().to_string()
}

pub fn render(report: &QueryReport) -> String {
    if let Some(rejection) = &report.rejected {
        let mut out = format!(
            "rejected ({}){}: {}",
            rejection.code,
            match rejection.step {
                Some(step) => format!(" at step {}", step + 1),
                None => String::new(),
            },
            rejection.message
        );
        // A rejection carries diagnostics in exactly one case, and it is the case they matter
        // most in: a query refused at a past commit that would be accepted today. Returning
        // early here printed the refusal and swallowed the reason it is not the user's typo.
        for d in &report.diagnostics {
            out.push_str(&format!(
                "\n  [{}] step {}: {}",
                d.level,
                d.step + 1,
                d.message
            ));
        }
        return out;
    }
    let mut out = format!(
        "{} result(s){}{}\n",
        report.matched,
        match report.matched > report.returned {
            true => format!(" — showing {}", report.returned),
            false => String::new(),
        },
        // Which commit this is about, on the first line. An answer about a past commit that
        // reads like an answer about now is the failure mode of the whole flag.
        match &report.at {
            Some(rev) => format!(" at {} ({})", short(&rev.commit), rev.date),
            None => String::new(),
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
            "  {}{}{}\n",
            // #268's requirement is that a foreign result be distinguishable *at every point
            // of presentation*. The qualified id carries the attribution and sits at the end
            // of the line, where a reader scanning a list does not look; this puts it where
            // they do. An agent about to write a claim must not have to parse an id to notice
            // that the answer came from a corpus this repository merely cites.
            match field("origin").as_str() {
                "" => String::new(),
                pkg => format!("[{pkg}] "),
            },
            match label.is_empty() {
                true => node.clone(),
                false => format!("{label}  ({node})"),
            },
            // Anything the caller selected beyond the default is worth showing, or
            // `--select` would silently do nothing in text mode.
            row.iter()
                .filter(|(k, _)| !["node", "class", "label", "origin"].contains(&k.as_str()))
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
    // Before the diagnostics rather than after: this is the answer to the question a reader
    // of an empty result is actually holding, and burying it under a list of notes about
    // steps that ran fine is how it gets skimmed past.
    if let Some(a) = &report.absence {
        out.push_str(&format!(
            "  [absent] step {}: {} ({})\n",
            a.step + 1,
            a.message,
            a.code
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
        "{} step(s), {} edge(s) walked, {} of {} node(s) read, ~{} token(s){}\n",
        c.steps,
        c.edges_walked,
        c.nodes_read,
        c.corpus_nodes,
        c.tokens,
        // Said on every `--across` run, including the one where nothing foreign matched.
        // Otherwise a spanning query over a corpus with no dependencies installed is
        // indistinguishable from a local one, which is the difference the flag is for.
        match report.scope {
            "across" => " — across the dependency set",
            _ => "",
        }
    ));
    out.trim_end().to_string()
}

/// Execute a query against the resolved graph.
#[allow(clippy::too_many_arguments)]
pub fn query(
    text: &str,
    select: Option<String>,
    limit: usize,
    anchor_k: usize,
    at: Option<String>,
    between: Option<String>,
    across: bool,
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

    if let Some(range) = between {
        let report = run_between(&root, &range, text, &opts)?;
        // A series does not gate. One rejected row in a range is the ordinary case — the
        // class did not exist yet — and exiting 1 for it would make `--between` unusable on
        // any query about a class the corpus grew into.
        if format.is_json() {
            crate::report::emit(&root, report)?;
        } else {
            println!("{}", render_series(&report));
        }
        return Ok(());
    }

    let report = match at {
        Some(rev) => run_at(&root, &rev, text, &opts)?,
        None if across => run_across(&root, text, &opts),
        None => run(&root, text, &opts),
    };
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

    // ── --across (#268) ───────────────────────────────────────────────────────

    /// A repository and one installed dependency, both declaring `concept` and both holding
    /// a node called `hydrology`. The collision is the fixture's whole point: a shared class
    /// name is not agreement, and two nodes sharing a string must come back as two answers.
    fn with_dependency() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let ont = "class: concept\nedges:\n  - relationship: relates-to\n    target: concept\n\
                   \x20   direction: out\n";
        let write = |rel: &str, body: &str| {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write(".yidam/corpus/concept.ont.yml", ont);
        write(
            ".yidam/corpus/concept/hydrology.yml",
            "class: concept\nlabel: Local hydrology\nlinks:\n  - target: flow.yml\n    \
             relationship: relates-to\n",
        );
        write(
            ".yidam/corpus/concept/flow.yml",
            "class: concept\nlabel: Local flow\n",
        );
        write(
            ".yidam/tonpa/upstream/manifest.yml",
            "commit: \"abc1234\"\n",
        );
        write(".yidam/tonpa/upstream/corpus/concept.ont.yml", ont);
        write(
            ".yidam/tonpa/upstream/corpus/concept/hydrology.yml",
            "class: concept\nlabel: Upstream hydrology\nlinks:\n  - target: routing.yml\n    \
             relationship: relates-to\n",
        );
        write(
            ".yidam/tonpa/upstream/corpus/concept/routing.yml",
            "class: concept\nlabel: Upstream routing\n",
        );
        dir
    }

    fn origins(report: &QueryReport) -> Vec<String> {
        report
            .results
            .iter()
            .map(|r| {
                r.get("origin")
                    .and_then(|v| v.as_str())
                    .unwrap_or("<local>")
                    .to_string()
            })
            .collect()
    }

    /// Without the flag a dependency is not looked at, and the report says `local`.
    #[test]
    fn a_query_that_did_not_ask_cannot_see_a_dependency() {
        let dir = with_dependency();
        let report = run(dir.path(), "concept", &opts());
        assert_eq!(report.scope, "local");
        assert_eq!(report.matched, 2);
    }

    /// Every result is attributed, including the local ones — present and null rather than
    /// absent, so a consumer testing the key never has to distinguish "local" from "a binary
    /// too old to say".
    #[test]
    fn across_attributes_every_result_including_the_local_ones() {
        let dir = with_dependency();
        let report = run_across(dir.path(), "concept", &opts());
        assert_eq!(report.scope, "across");
        assert_eq!(report.matched, 4);
        assert_eq!(
            origins(&report),
            vec!["<local>", "<local>", "upstream", "upstream"]
        );
        assert!(report.results.iter().all(|r| r.contains_key("origin")));
    }

    /// Two corpora hold `concept/hydrology.yml`. A shared class name is not agreement, and an
    /// unqualified id would name a node that is not the one that answered.
    #[test]
    fn a_foreign_id_is_qualified_so_a_collision_is_two_answers() {
        let dir = with_dependency();
        let report = run_across(dir.path(), "concept", &opts());
        let nodes: Vec<&str> = report
            .results
            .iter()
            .filter_map(|r| r.get("node").and_then(|v| v.as_str()))
            .collect();
        assert!(nodes.contains(&"concept/hydrology.yml"));
        assert!(nodes.contains(&"upstream::concept/hydrology.yml"));
    }

    /// **The boundary.** The local node links `routing.yml`, a name only the dependency has.
    /// A merged execution would resolve it; one execution per corpus cannot, and the only
    /// answer is the one the dependency reached through its *own* edge.
    #[test]
    fn no_hop_crosses_the_corpus_boundary() {
        let dir = with_dependency();
        std::fs::write(
            dir.path().join(".yidam/corpus/concept/hydrology.yml"),
            "class: concept\nlabel: Local hydrology\nlinks:\n  - target: routing.yml\n    \
             relationship: relates-to\n",
        )
        .unwrap();
        let report = run_across(dir.path(), "concept -relates-to-> concept", &opts());
        assert_eq!(report.matched, 1, "{:?}", report.results);
        assert_eq!(
            report.results[0]["node"],
            serde_json::json!("upstream::concept/routing.yml"),
            "the only match must be the one the dependency reached itself"
        );
    }

    /// A dependency whose ontology cannot answer the query is excluded with the reason, and
    /// the local answer still comes back. A shared class name is not agreement; the absence
    /// of one is not a defect in the query.
    #[test]
    fn a_dependency_that_cannot_typecheck_the_query_is_excluded_with_the_reason() {
        let dir = with_dependency();
        std::fs::rename(
            dir.path()
                .join(".yidam/tonpa/upstream/corpus/concept.ont.yml"),
            dir.path().join(".yidam/tonpa/upstream/corpus/note.ont.yml"),
        )
        .unwrap();
        let report = run_across(dir.path(), "concept", &opts());
        assert_eq!(report.matched, 2, "the local answer survives");
        let excluded = report
            .diagnostics
            .iter()
            .find(|d| d.code == "corpus-excluded")
            .expect("said which corpus and why");
        assert_eq!(excluded.level, "info");
        assert!(
            excluded.message.contains("upstream"),
            "{}",
            excluded.message
        );
    }

    /// Scope is a field, not something read off the rows. A corpus with nothing installed
    /// answers `across` with exactly the rows a local run gives, and "nothing foreign matched"
    /// must stay distinguishable from "nothing foreign was looked at".
    #[test]
    fn across_over_no_dependencies_still_says_it_looked() {
        let dir = fixture();
        let report = run_across(dir.path(), "reach", &opts());
        // No dependencies are installed, so there is nothing to widen to — and the report is
        // honest that this is a local answer rather than claiming a scope it did not have.
        assert_eq!(report.scope, "local");
    }

    /// The text path must show the boundary too. An agent about to write a claim should not
    /// have to parse an id to notice the answer came from a corpus this repository cites.
    #[test]
    fn the_text_report_marks_a_foreign_row() {
        let dir = with_dependency();
        let rendered = render(&run_across(dir.path(), "concept", &opts()));
        assert!(
            rendered.contains("[upstream] Upstream hydrology"),
            "{rendered}"
        );
        assert!(rendered.contains("across the dependency set"), "{rendered}");
    }

    /// The index covers this repository and not its dependencies, so an anchored span would
    /// enter through local text and answer with foreign rows.
    #[test]
    fn an_anchored_across_is_refused_with_the_reason() {
        let dir = with_dependency();
        let report = run_across(dir.path(), r#"concept~"flow""#, &opts());
        assert_eq!(report.rejected.unwrap().code, "anchor-across");
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
