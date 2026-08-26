//! `yidam pack` — a context pack for one goal (#282, E6).
//!
//! # What already existed, and what it could not answer
//!
//! [`crate::cmd::export_llms::LlmsPack`] states the principle this inherits, and delivers it:
//! *"an honest account of what it contains, so a caller can report what it wrote rather than
//! what it was given."* `written`, `elided` and `omitted_by_class` are that account, and they
//! are the fix for a budgeted pack that cut the graph by sort order and called it the corpus.
//!
//! It is **whole-corpus and static**. The moment an agent has an actual question there was no
//! equivalent, and it fell back to top-k with no account of what it did not see. `query`
//! answers the question and reports `matched` beside `returned` — which says *how many* were
//! dropped and never *which kind*, and "I retrieved 12 nodes" and "I retrieved 12 of 40, and
//! the 28 I dropped were all `recording` instances" license completely different next actions.
//!
//! # The budget is the only bound
//!
//! There is no `--limit` here. `query` has one because a person reading a terminal wants the
//! first fifty rows; a pack exists to be *spent*, and a limit underneath the budget would drop
//! nodes that the receipt then reports as neither written nor omitted. The invariant the
//! receipt guarantees is `written + omitted == reachable`, and a second bound breaks it.
//!
//! # It shares the whole-corpus pack's fill, deliberately
//!
//! Coverage before membership — prose first, then whole nodes, and membership round-robin
//! across classes so a budget that admits `n` nodes spreads them over the ontology. That
//! degradation order is a design decision argued once at [`crate::cmd::export_llms`], and a
//! second implementation of it here would mean two things called "the pack" that answer the
//! same corpus differently. `the_two_packs_render_a_node_the_same_way` holds them together.

use std::collections::BTreeMap;

use anyhow::Result;

use crate::cmd::export_llms::{fill, fill_all, order, trailer};
use crate::cmd::lint::checks::{class_of, Node};
use crate::cmd::query::{self, absence, anchor, check, exec};
use crate::model::NodeView;
use crate::paths::repo_root;

/// How a token is estimated, everywhere in this report.
///
/// Frozen and carried in the payload rather than left implicit: a caller using a real
/// tokenizer needs to know the figure it is comparing against is `chars / 4`, and the issue
/// asks for exactly that — *"an honest range beats a precise-looking number computed with the
/// wrong tokenizer."*
const BASIS: &str = "chars/4";

pub struct Options {
    /// Tokens the pack may run to, or `None` for unbounded.
    ///
    /// **Unbounded by default.** A default budget would silently truncate the first pack
    /// anybody runs, which is the failure this whole surface is built to report rather than
    /// commit.
    pub budget: Option<usize>,
    pub anchor_k: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            budget: None,
            anchor_k: query::DEFAULT_ANCHOR_K,
        }
    }
}

/// What the pack cost and what it was allowed to cost.
#[derive(Debug, serde::Serialize)]
pub struct Budget {
    /// Tokens asked for. Present and null when the pack was unbounded.
    pub tokens: Option<usize>,
    /// Tokens the pack actually runs to, by `basis`.
    pub used: usize,
    pub chars: usize,
    /// How both figures were computed — see [`BASIS`].
    pub basis: &'static str,
    /// True when `used` exceeds `tokens`, which happens in exactly one situation: the budget
    /// is too small to hold the receipt itself.
    ///
    /// **The receipt is the floor.** Below it there is nothing left to trade away — dropping
    /// the account to fit would produce a pack that silently holds nothing, which is the
    /// failure this whole surface exists to report rather than commit. Whenever a single node
    /// is written the pack fits, and
    /// `the_pack_fits_its_budget_or_holds_nothing_but_the_account` pins that.
    ///
    /// The whole-corpus pack has the same floor and does not say so; it is named here because
    /// a per-goal pack is asked for small budgets and will meet it.
    pub over_budget: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct PackReport {
    pub query: String,
    /// `local`, always. A pack is this repository's context for its own goal; see the note on
    /// [`run`].
    pub scope: &'static str,
    /// Present and null when the query ran, exactly as [`query::QueryReport`] carries it.
    pub rejected: Option<check::Rejection>,
    pub anchor: Option<anchor::Anchor>,
    /// Why the pack is empty, when it is (#283). Null when it holds something.
    ///
    /// **This is the field that matters most here**, more than on `query`. A pack is what an
    /// agent reads *as* the corpus, and an empty one that says nothing is a context window
    /// asserting the corpus has no view. It is carried into the pack's own text for the same
    /// reason the receipt is: the artefact travels without the envelope.
    pub absence: Option<absence::Absence>,
    pub diagnostics: Vec<check::Diagnostic>,
    /// What the *traversal* cost, unchanged from `query`. Distinct from [`Budget`], which is
    /// what the pack costs the caller who reads it.
    pub cost: exec::Cost,
    /// Nodes the query matched — what the budget had to choose from.
    pub reachable: usize,
    /// Reachable nodes present in `text` as a section, whether full prose or label-only.
    pub written: usize,
    /// Written nodes whose description was dropped to fit.
    pub elided: usize,
    /// Reachable nodes with no section at all. `written + omitted == reachable`.
    pub omitted: usize,
    /// **The field the issue is about.** How many reachable nodes were dropped, by class.
    /// "12 of 40" says how much was lost; this says what kind, which is the half an agent can
    /// act on — spend more budget, or report the gap.
    pub omitted_by_class: BTreeMap<String, usize>,
    pub budget: Budget,
    /// The pack itself, ready to be handed to a model. Empty on a rejection.
    pub text: String,
}

fn rejected(text: &str, report: query::QueryReport) -> PackReport {
    PackReport {
        query: text.to_string(),
        scope: "local",
        rejected: report.rejected,
        anchor: report.anchor,
        absence: report.absence,
        diagnostics: report.diagnostics,
        cost: report.cost,
        reachable: 0,
        written: 0,
        elided: 0,
        omitted: 0,
        omitted_by_class: BTreeMap::new(),
        budget: Budget {
            tokens: None,
            used: 0,
            chars: 0,
            basis: BASIS,
            over_budget: false,
        },
        text: String::new(),
    }
}

/// One matched node, in the shape the pack renderer reads.
///
/// The id loses its `.yml`: `query` reports `reach/tailwater.yml` because that is what
/// `corpus-index` reports, and a pack is read by an agent whose next call is
/// `get_node("reach/tailwater")`. Handing it an id its own next call rejects would make the
/// pack a dead end.
///
/// Link targets are resolved by [`crate::model::resolve_link_target`] — the whole-corpus
/// pack's resolver, not the traversal's narrower one. A `Links:` line is what the node
/// *says*, and a node appearing in both packs must render identically or the two disagree
/// about the corpus in the one place a reader would never check.
fn view(node: &Node, id: &str) -> NodeView {
    NodeView {
        id: id.trim_end_matches(".yml").to_string(),
        class: class_of(node),
        label: node.inst.label.clone().unwrap_or_default(),
        description: node.inst.description.clone().unwrap_or_default(),
        content: node.text.clone(),
        links: node
            .inst
            .links
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .filter_map(|l| {
                l.target.as_ref().map(|t| {
                    (
                        crate::model::resolve_link_target(&class_of(node), t),
                        l.relationship.clone().unwrap_or_else(|| "link".to_string()),
                    )
                })
            })
            .collect(),
        origin: None,
    }
}

/// Build the pack against an already-loaded corpus.
pub fn run_on(ctx: &query::Context, text: &str, opts: &Options) -> PackReport {
    let report = query::run_on(
        ctx,
        text,
        &query::Options {
            // `node` alone. The projection is discarded — the pack renders the nodes
            // themselves — and selecting more would serialize every field of every match to
            // compute a `chars` figure nothing here reads.
            select: vec!["node".to_string()],
            // Every match, for the reason in the module docs.
            limit: usize::MAX,
            anchor_k: opts.anchor_k,
        },
    );
    if report.rejected.is_some() {
        return rejected(text, report);
    }

    let by_id: BTreeMap<String, &Node> = ctx
        .graph
        .nodes
        .iter()
        .map(|n| (exec::id_of(n, &ctx.graph.corpus_dir), n))
        .collect();
    let mut nodes: Vec<NodeView> = report
        .results
        .iter()
        .filter_map(|row| row.get("node")?.as_str())
        .filter_map(|id| by_id.get(id).map(|n| view(n, id)))
        .collect();

    // From the ontology the query already loaded, rather than a second walk of
    // `.yidam/corpus` — and keyed by the stem, which is what `class_of` resolves a node to.
    let fields = crate::claims::ClaimFields::from_declarations(ctx.graph.classes.iter().map(|c| {
        let claim_fields = c
            .properties
            .iter()
            .filter(|p| p.r#type == crate::claims::CLAIM_PROPERTY_TYPE)
            .map(|p| p.name.clone())
            .collect();
        (c.name.clone(), claim_fields)
    }));
    order(&mut nodes, &fields);

    let reachable = nodes.len();
    let filled = match opts.budget {
        None => fill_all(&nodes),
        Some(tokens) => {
            let char_budget = tokens.saturating_mul(4);
            // Held back at their longest so the accounting cannot itself push the pack over
            // the budget it reports — the reserve `render_llms` takes, for the same reason.
            let reserve = header(text, &report, reachable, reachable, opts.budget).len()
                + trailer(&class_totals(&nodes), reachable).len();
            fill(&nodes, char_budget.saturating_sub(reserve))
        }
    };

    let mut body = header(text, &report, filled.written, reachable, opts.budget);
    body.push_str(&filled.body);
    body.push_str(&trailer(&filled.omitted_by_class, filled.elided));

    // Measured after the body is built rather than predicted before it, because the note
    // below is itself part of the pack. Appending it to an already-over pack only makes it
    // more over, so there is nothing to converge on; deciding beforehand whether to include
    // a line whose inclusion changes the measurement would be circular.
    let over_budget = opts.budget.is_some_and(|t| body.len() / 4 > t);
    if over_budget {
        body.push_str(&format!(
            "# Over budget: {} tokens asked for, and the account above costs {}. \
             Nothing was dropped to fit it — a pack that hid its own receipt would read as a \
             corpus with nothing to say.\n",
            opts.budget.unwrap_or(0),
            body.len() / 4,
        ));
    }

    let omitted: usize = filled.omitted_by_class.values().sum();
    PackReport {
        query: text.to_string(),
        scope: "local",
        rejected: None,
        anchor: report.anchor,
        absence: report.absence,
        diagnostics: report.diagnostics,
        cost: report.cost,
        reachable,
        written: filled.written,
        elided: filled.elided,
        omitted,
        omitted_by_class: filled.omitted_by_class,
        budget: Budget {
            tokens: opts.budget,
            used: body.len() / 4,
            chars: body.len(),
            basis: BASIS,
            over_budget,
        },
        text: body,
    }
}

fn class_totals(nodes: &[NodeView]) -> BTreeMap<String, usize> {
    let mut totals: BTreeMap<String, usize> = BTreeMap::new();
    for node in nodes {
        *totals.entry(node.class.clone()).or_default() += 1;
    }
    totals
}

/// The pack's own preamble.
///
/// It carries the receipt in prose because the text is the artefact: a pack pasted into a
/// context window arrives without the JSON envelope, and an agent reading it must be able to
/// see that it anchored on nothing, or that the budget cost it twenty nodes, from the pack
/// alone. The trailer says what was lost; this says what was asked and how it was entered.
fn header(
    query: &str,
    report: &query::QueryReport,
    written: usize,
    reachable: usize,
    budget: Option<usize>,
) -> String {
    let mut s = format!("# Pack: {query}\n");
    // `n of m` unconditionally, where the whole-corpus pack's header drops the `of m` when
    // nothing was lost. Two reasons: a reader must not have to know that a bare number means
    // "all of them", and the reserve below is computed from this string before `written` is
    // known — a conditional that gets *longer* when the budget bites would under-reserve
    // exactly when the reservation matters.
    s.push_str(&format!(
        "# Scope: {} | Nodes: {written} of {reachable}{}\n",
        report.scope,
        match budget {
            Some(b) => format!(" | Token budget: {b} ({BASIS})"),
            None => format!(" | Unbudgeted ({BASIS})"),
        },
    ));
    if let Some(a) = &report.anchor {
        s.push_str(&format!(
            "# Anchored on {} — {}\n",
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
    // The pack's most important line when it is there. A pack with no sections and no
    // explanation is a context window that says the corpus has nothing on the subject, which
    // is the invention #283 is about — arriving in the one artefact an agent reads as though
    // it were the corpus.
    if let Some(a) = &report.absence {
        s.push_str(&format!(
            "# Absent ({}) at step {}: {}\n",
            a.code,
            a.step + 1,
            a.message
        ));
    }
    for d in &report.diagnostics {
        s.push_str(&format!(
            "# [{}] step {}: {}\n",
            d.level,
            d.step + 1,
            d.message
        ));
    }
    s.push('\n');
    s
}

/// Build the pack, loading the corpus — and, only if the query anchors, the index.
///
/// **Local, and there is no `--across`.** A pack is context an agent is about to write from,
/// and #268's boundary is that a foreign node is readable and is never an edge target. A pack
/// mixing the two would put a dependency's prose in a context window under this repository's
/// class names, where `omitted_by_class` would then report `concept: 12` over two corpora
/// that each mean something different by `concept`. The receipt would be arithmetic about a
/// category nobody declared.
pub fn run(root: &std::path::Path, text: &str, opts: &Options) -> PackReport {
    let graph = query::Graph::load(root);
    let anchored =
        query::lang::parse(text).is_ok_and(|q| q.steps.iter().any(|s| s.anchor.is_some()));
    if !anchored {
        return run_on(&query::Context::now(&graph, None), text, opts);
    }
    match query::load_index(root) {
        Ok(retrieval) => run_on(&query::Context::now(&graph, Some(&retrieval)), text, opts),
        Err(e) => rejected(
            text,
            query::rejected_report(
                text,
                check::Rejection {
                    step: None,
                    code: "anchor-unresolvable",
                    message: format!(
                        "the similarity anchor needs the index, and it did not load: {e}"
                    ),
                },
            ),
        ),
    }
}

/// The text form: the pack itself.
///
/// Not a summary of it. `yidam pack '…' > context.md` is the point of the command, and a
/// human-readable digest wrapped around the artefact would make the obvious invocation the
/// wrong one. The receipt is in the pack's own header and trailer.
pub fn render(report: &PackReport) -> String {
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
    report.text.clone()
}

/// Render a context pack for one goal.
pub fn pack(
    text: &str,
    budget: Option<usize>,
    anchor_k: usize,
    format: crate::report::Format,
) -> Result<()> {
    let root = repo_root()?;
    let report = run(
        &root,
        text,
        &Options {
            budget,
            anchor_k: anchor_k.max(1),
        },
    );
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

    /// A corpus with two classes, one typed edge, and prose long enough that a budget has
    /// something to spend. The descriptions are the load-bearing part: a fixture of bare
    /// labels can never make `elided` nonzero, so every degradation test would pass
    /// vacuously.
    const FIXTURE: &[(&str, &str)] = &[
        (
            "concept.ont.yml",
            "class: concept\nproperties:\n  - name: claim_tag\n    type: claim\nedges:\n  \
             - relationship: exhibits\n    target: reach\n    direction: in\n",
        ),
        (
            "reach.ont.yml",
            "class: reach\nedges:\n  - relationship: exhibits\n    target: concept\n    \
             direction: out\n",
        ),
        (
            "concept/hydropeaking.yml",
            "class: concept\nlabel: Hydropeaking\ndescription: Rapid sub-daily variation in \
             discharge driven by generation scheduling rather than by the catchment, and the \
             single largest departure from an unregulated flow regime below a storage dam.\n",
        ),
        (
            "concept/low-flow.yml",
            "class: concept\nlabel: Low flow\ndescription: The lower tail of the discharge \
             distribution, where habitat, temperature and water-right questions all become \
             binding at once and the record is least reliable.\n",
        ),
        (
            "reach/tailwater.yml",
            "class: reach\nlabel: Tailwater\ndescription: The segment immediately below the \
             impoundment, where discharge is set by the outlet works rather than by the \
             catchment upstream of it.\nlinks:\n  - target: ../concept/hydropeaking.yml\n    \
             relationship: exhibits\n",
        ),
        (
            "reach/canyon.yml",
            "class: reach\nlabel: Canyon\ndescription: The confined segment downstream of \
             the tailwater, where the operating signal is still visible but attenuating.\n\
             links:\n  - target: ../concept/low-flow.yml\n    relationship: exhibits\n",
        ),
    ];

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (rel, body) in FIXTURE {
            let path = dir.path().join(".yidam/corpus").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        }
        dir
    }

    fn pack_of(dir: &tempfile::TempDir, query: &str, budget: Option<usize>) -> PackReport {
        run(
            dir.path(),
            query,
            &Options {
                budget,
                ..Options::default()
            },
        )
    }

    /// The invariant the whole receipt rests on. `matched` beside `returned` can only say how
    /// many were lost; this says every reachable node is accounted for as one or the other,
    /// which is what makes "and 28 of them were `recording`" a complete statement rather than
    /// a sample.
    #[test]
    fn every_reachable_node_is_either_written_or_named_as_omitted() {
        let dir = fixture();
        for budget in [None, Some(20), Some(60), Some(200), Some(100_000)] {
            let report = pack_of(&dir, "*", budget);
            assert_eq!(
                report.written + report.omitted,
                report.reachable,
                "budget {budget:?}: {} written + {} omitted != {} reachable",
                report.written,
                report.omitted,
                report.reachable
            );
            assert_eq!(
                report.omitted_by_class.values().sum::<usize>(),
                report.omitted,
                "budget {budget:?}: the per-class breakdown does not sum to the total"
            );
        }
    }

    /// #282's actual ask. "12 of 40" and "12 of 40, and the 28 dropped were all `reach`"
    /// license different next actions, and only the second lets an agent decide whether to
    /// spend more budget or report the gap.
    #[test]
    fn a_budget_that_bites_says_which_classes_it_dropped() {
        let dir = fixture();
        let report = pack_of(&dir, "*", Some(40));
        assert!(report.omitted > 0, "a 40-token budget must not fit 4 nodes");
        assert!(
            !report.omitted_by_class.is_empty(),
            "nodes were dropped and no class was named for them"
        );
        assert!(
            report.text.contains("# Omitted:"),
            "the pack itself must carry the account, not only the JSON envelope:\n{}",
            report.text
        );
    }

    /// The receipt lives in the artefact, not only in the envelope. A pack is pasted into a
    /// context window without the JSON around it, and an agent reading it there must still be
    /// able to see that it is holding a slice.
    #[test]
    fn the_pack_text_states_what_it_holds_of_what_was_reachable() {
        let dir = fixture();
        let report = pack_of(&dir, "*", Some(40));
        assert!(
            report.text.contains(&format!(
                "Nodes: {} of {}",
                report.written, report.reachable
            )),
            "{}",
            report.text
        );
    }

    #[test]
    fn an_unbudgeted_pack_drops_nothing_and_says_so() {
        let dir = fixture();
        let report = pack_of(&dir, "*", None);
        assert_eq!((report.omitted, report.elided), (0, 0));
        assert_eq!(report.written, report.reachable);
        assert!(report.omitted_by_class.is_empty());
        assert_eq!(report.budget.tokens, None);
        assert!(!report.text.contains("# Omitted:"));
    }

    /// A budget is a promise. A pack that reports the budget it was given and quietly exceeds
    /// it is worse than one that refuses, because the caller has already spent the tokens by
    /// the time it can check.
    ///
    /// There is exactly one situation where it cannot be kept, and it is a floor rather than
    /// a leak: the receipt itself costs tokens, and a budget below that cost buys nothing to
    /// trade away. So the promise is conditional and the condition is checkable — **written a
    /// node, fits the budget** — and the overshoot is declared rather than discovered.
    #[test]
    fn the_pack_fits_its_budget_or_holds_nothing_but_the_account() {
        let dir = fixture();
        let mut floors = 0;
        for budget in (10..=400).step_by(10) {
            let report = pack_of(&dir, "*", Some(budget));
            if report.written > 0 {
                assert!(
                    report.budget.used <= budget && !report.budget.over_budget,
                    "budget {budget}: {} node(s) written and the pack ran to {} tokens",
                    report.written,
                    report.budget.used
                );
                continue;
            }
            if report.budget.used <= budget {
                continue;
            }
            floors += 1;
            assert!(
                report.budget.over_budget,
                "budget {budget}: overshot in silence"
            );
            assert!(
                report.text.contains("# Over budget:"),
                "budget {budget}: the pack does not say it overshot:\n{}",
                report.text
            );
        }
        assert!(
            floors > 0,
            "this sweep never reached the floor, so it proved nothing"
        );
    }

    /// The traversal is the query's, so the goal is a *typed* one and not a bag of matches.
    #[test]
    fn a_typed_hop_packs_what_the_hop_reached_and_not_the_class() {
        let dir = fixture();
        let report = pack_of(&dir, "reach -exhibits-> concept", None);
        assert_eq!(report.reachable, 2);
        assert!(report.text.contains("## concept/hydropeaking"));
        assert!(report.text.contains("## concept/low-flow"));
        assert!(
            !report.text.contains("## reach/"),
            "a pack holds the query's answer, not the path it walked:\n{}",
            report.text
        );
    }

    /// A rejection is an answer, and it is not a pack. Returning an empty pack for a
    /// misspelled class would hand an agent a context window that says the corpus knows
    /// nothing — which is the invention #283 is about, arriving through this door.
    #[test]
    fn a_rejected_query_produces_no_pack_at_all() {
        let dir = fixture();
        let report = pack_of(&dir, "concpet", None);
        let rejection = report.rejected.as_ref().expect("must be rejected");
        assert_eq!(rejection.code, "unknown-class");
        assert!(report.text.is_empty());
        assert_eq!(report.reachable, 0);
        assert!(render(&report).starts_with("rejected (unknown-class)"));
    }

    /// A well-formed query that matches nothing is not a rejection, and its pack is empty in
    /// a way that says so: zero reachable, zero omitted, and a header that states both.
    #[test]
    fn an_empty_answer_packs_as_an_empty_answer() {
        let dir = fixture();
        let report = pack_of(&dir, "concept[claim_tag=open]", None);
        assert!(report.rejected.is_none());
        assert_eq!(
            (report.reachable, report.written, report.omitted),
            (0, 0, 0)
        );
        assert!(report.text.contains("Nodes: 0 of 0"), "{}", report.text);
    }

    /// **A pack with nothing in it and nothing to say is a context window asserting the
    /// corpus has no view** (#283). The diagnosis has to be in the *text*, because that is
    /// what travels into a model — an agent handed the JSON envelope reads the envelope, and
    /// an agent handed the pack reads only this.
    #[test]
    fn an_empty_pack_carries_the_reason_in_the_artefact_itself() {
        let dir = fixture();
        let report = pack_of(&dir, "concept[claim_tag=open]", None);
        let absence = report.absence.as_ref().expect("an empty pack is diagnosed");
        assert_eq!(absence.code, "predicate-unsatisfied");
        assert!(
            report.text.contains("# Absent (predicate-unsatisfied)"),
            "{}",
            report.text
        );
        assert!(
            report.text.contains(&absence.message),
            "the pack's text must carry the whole diagnosis, not a code:\n{}",
            report.text
        );
    }

    /// And a pack that holds something is not absent.
    #[test]
    fn a_pack_with_nodes_in_it_carries_no_absence() {
        let dir = fixture();
        assert!(pack_of(&dir, "*", None).absence.is_none());
        assert!(pack_of(&dir, "*", Some(40)).absence.is_none());
    }

    /// The anchor's own account travels into the pack, because an anchor that landed
    /// somewhere unexpected is the commonest reason a pack looks wrong, and a reader holding
    /// only the text would otherwise have no way to see it.
    #[test]
    fn an_anchored_pack_says_where_it_entered_and_how() {
        let dir = fixture();
        let report = pack_of(
            &dir,
            r#"concept~"sub-daily variation" <-exhibits- reach"#,
            None,
        );
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        let anchor = report.anchor.as_ref().expect("the anchor is reported");
        assert!(anchor.degraded, "this fixture ships no index");
        assert!(
            report
                .text
                .contains("# Anchored on concept/hydropeaking.yml"),
            "{}",
            report.text
        );
        assert!(
            report
                .text
                .contains("keyword search, not similarity (no_index)"),
            "{}",
            report.text
        );
    }

    /// **The two packs must not disagree about a node.**
    ///
    /// They share [`fill`] so they degrade alike; this holds the layer underneath, where a
    /// second link resolver or a second id convention would make the same node read
    /// differently depending on which pack a reader was handed. That divergence is invisible
    /// in every other test, because each pack is self-consistent.
    #[test]
    fn the_two_packs_render_a_node_the_same_way() {
        use crate::model::{DomainModel, InstanceFile, Provenance, RenderedViews};

        let dir = fixture();
        let instances: Vec<InstanceFile> = FIXTURE
            .iter()
            .filter(|(rel, _)| rel.contains('/'))
            .map(|(rel, body)| {
                let (class, filename) = rel.split_once('/').unwrap();
                InstanceFile {
                    class: class.to_string(),
                    filename: filename.to_string(),
                    content: body.as_bytes().to_vec(),
                }
            })
            .collect();
        let model = DomainModel {
            classes: vec![],
            instances,
            skills: vec![],
            decisions: vec![],
            index: None,
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "pack-fixture".into(),
                generated_at: 0,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        };

        let whole = crate::cmd::export_llms::render_llms(&model, None).text;
        let goal = pack_of(&dir, "*", None).text;
        for id in ["concept/hydropeaking", "reach/tailwater"] {
            let section = |text: &str| {
                let start = text
                    .find(&format!("## {id}\n"))
                    .unwrap_or_else(|| panic!("{id} missing from:\n{text}"));
                let rest = &text[start..];
                let end = rest.find("\n---\n").expect("a section ends with a rule");
                rest[..end].to_string()
            };
            assert_eq!(
                section(&whole),
                section(&goal),
                "the whole-corpus pack and the goal pack render {id} differently"
            );
        }
    }
}
