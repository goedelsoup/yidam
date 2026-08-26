//! `yidam estimate` — quote the cost before charging it (#284, E6).
//!
//! # The asymmetry that makes a quote worth having
//!
//! An agent budgets in tokens and, until this, could only discover what a retrieval cost by
//! paying for it. The only strategy available was to ask for less than might be needed and
//! hope.
//!
//! The obvious objection is that knowing exactly what a query costs means running it — so a
//! quote is the answer with the rows thrown away, and no cheaper. That is true of the
//! *server* and false of the *caller*, which is the same distinction [`super::query::exec`]
//! already draws when it refuses to count the corpus load in `nodes_read`: the executor holds
//! every node either way, and the agent pays for what comes back. A quote runs the traversal
//! and returns none of the prose, so it costs the server what an answer costs and costs the
//! caller a few hundred bytes.
//!
//! # What an agent actually decides
//!
//! Not *whether* to run the query — it has a question either way — but **how much of each
//! node to ask for**. So the quote is a table rather than a number: the same match set priced
//! at each projection, cheapest first, with a `fits` verdict against the budget when one is
//! given. That is what "widen a query it can afford and narrow one it cannot" turns into at
//! the point of decision.
//!
//! # The characters are exact and the tokens are not
//!
//! #284 asks for an honest range over a precise-looking number computed with the wrong
//! tokenizer. The honest thing here is neither: `chars` is **exact** — it is the serialized
//! length of the payload that would come back — and `tokens` is `chars / 4`, labelled. A range
//! would be a second invented number laid over the first, and a caller holding a real
//! tokenizer needs the exact figure rather than a wider guess. So the approximation is named
//! and the exact number is beside it.
//!
//! # Speculative, and the epic says so
//!
//! E6 flags this as the child to drop if agents do not act differently given a quote. Nothing
//! else depends on it: it adds one command and one tool, reads nothing new, and computes
//! entirely from what a query already produces.

use anyhow::Result;

use crate::cmd::pack;
use crate::cmd::query::{self, absence, check, exec};
use crate::paths::repo_root;

/// How a token is estimated. The same basis `pack` reports, and named for the same reason.
const BASIS: &str = "chars/4";

/// The projections quoted on every estimate, cheapest first.
///
/// Three widths and not every combination: these are the three decisions — names only, names
/// with the prose a node summarises itself in, and the whole instance. A caller's own
/// `--select` is priced beside them when it is none of the three, so the answer to "what will
/// *my* call cost" is always in the table.
const WIDTHS: &[&str] = &[
    query::DEFAULT_SELECT,
    "node,class,label,description",
    "node,class,label,body",
];

pub struct Options {
    pub select: Vec<String>,
    /// The `--limit` the quoted call would carry.
    ///
    /// **Priced, not ignored.** `query` bounds its projection at 50 by default, so quoting the
    /// whole match set would tell an agent with four hundred matches to narrow a call that was
    /// already going to return fifty rows. A quote that is not about the call the caller is
    /// about to make is a number, not a quote.
    pub limit: usize,
    pub budget: Option<usize>,
    pub anchor_k: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            select: query::DEFAULT_SELECT.split(',').map(String::from).collect(),
            limit: query::DEFAULT_LIMIT,
            budget: None,
            anchor_k: query::DEFAULT_ANCHOR_K,
        }
    }
}

/// One projection, priced.
#[derive(Debug, serde::Serialize)]
pub struct Projection {
    pub select: String,
    /// Rows that would come back — `matched` capped at the quoted `limit`.
    pub rows: usize,
    /// Exact serialized length of the payload.
    pub chars: usize,
    /// `chars / 4` — see [`BASIS`].
    pub tokens: usize,
    /// Whether it fits the quoted budget. Present and null when no budget was given, so a
    /// caller testing the key never has to distinguish "affordable" from "nothing was asked".
    pub fits: Option<bool>,
}

/// What a context pack over the same match set would run to, unbudgeted.
///
/// **`limit` does not apply here**, and the difference is the point of showing both: `pack`
/// has no limit — the budget is its only bound — so its `nodes` is the whole match set where a
/// projection's `rows` is the capped one. A caller comparing the two rows is comparing fifty
/// nodes' worth of names against four hundred nodes' worth of prose, and needs to see it.
#[derive(Debug, serde::Serialize)]
pub struct PackQuote {
    pub nodes: usize,
    pub chars: usize,
    pub tokens: usize,
    pub fits: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
pub struct Estimate {
    pub query: String,
    pub scope: &'static str,
    /// Present and null when the query ran — `query`'s own discipline. A rejected query has no
    /// cost to quote and the rejection is the answer.
    pub rejected: Option<check::Rejection>,
    /// Why the answer would be empty, when it would be (#283).
    ///
    /// Carried rather than left to the zero, because **a quote of zero reads as cheap**. An
    /// agent told a query costs nothing and not told the class is unpopulated has been handed
    /// the most affordable possible way to learn nothing.
    pub absence: Option<absence::Absence>,
    pub diagnostics: Vec<check::Diagnostic>,
    /// What the walk cost the server, in `query`'s own units. Not what the caller would pay.
    pub cost: exec::Cost,
    /// Nodes the query matches, before any projection or limit.
    pub matched: usize,
    /// The `--limit` every projection below was priced at. The pack row ignores it.
    pub limit: usize,
    /// The budget this quote was priced against, or null.
    pub budget: Option<usize>,
    pub projections: Vec<Projection>,
    pub pack: PackQuote,
    /// How every `tokens` here was computed.
    pub basis: &'static str,
}

fn rejected(text: &str, report: query::QueryReport) -> Estimate {
    Estimate {
        query: text.to_string(),
        scope: "local",
        rejected: report.rejected,
        absence: None,
        diagnostics: report.diagnostics,
        cost: report.cost,
        matched: 0,
        limit: 0,
        budget: None,
        projections: Vec::new(),
        pack: PackQuote {
            nodes: 0,
            chars: 0,
            tokens: 0,
            fits: None,
        },
        basis: BASIS,
    }
}

/// Price a query against an already-loaded corpus.
pub fn run_on(ctx: &query::Context, text: &str, opts: &Options) -> Estimate {
    let pack_opts = pack::Options {
        budget: None,
        anchor_k: opts.anchor_k,
    };
    // **One traversal.** The anchor is resolved here and nowhere else: an estimate that
    // resolved it twice would charge the server double for the one thing it exists to say is
    // affordable.
    let report = query::run_on(ctx, text, &pack::traversal(&pack_opts));
    if report.rejected.is_some() {
        return rejected(text, report);
    }

    let matched = report.matched;
    let ids: Vec<String> = report
        .results
        .iter()
        .filter_map(|row| row.get("node")?.as_str().map(String::from))
        .collect();
    // What the quoted call would actually project. The pack, below, prices `ids` whole.
    let shown: Vec<String> = ids.iter().take(opts.limit).cloned().collect();

    // The caller's own width priced beside the standard three, and deduplicated — a caller
    // who asked for the default must not be shown it twice.
    let asked = opts.select.join(",");
    let mut widths: Vec<String> = WIDTHS.iter().map(|w| w.to_string()).collect();
    if !widths.contains(&asked) {
        widths.push(asked);
    }

    let mut projections: Vec<Projection> = widths
        .into_iter()
        .map(|select| {
            let fields: Vec<String> = select.split(',').map(|f| f.trim().to_string()).collect();
            let (rows, chars) =
                exec::project(&shown, &ctx.graph.nodes, &ctx.graph.corpus_dir, &fields);
            Projection {
                select,
                rows: rows.len(),
                chars,
                tokens: chars / 4,
                fits: opts.budget.map(|b| chars / 4 <= b),
            }
        })
        .collect();
    // Cheapest first, by the exact figure rather than the approximate one: two widths whose
    // token counts round together are still ordered by what they actually cost. A stable sort,
    // so widths that tie — every one of them, on an empty match set — keep the order they were
    // priced in, which is narrowest to widest and the caller's own last.
    projections.sort_by_key(|p| p.chars);

    let packed = pack::from_report(ctx, text, report, &pack_opts);
    Estimate {
        query: text.to_string(),
        scope: packed.scope,
        rejected: None,
        absence: packed.absence,
        diagnostics: packed.diagnostics,
        cost: packed.cost,
        matched,
        limit: opts.limit,
        budget: opts.budget,
        projections,
        pack: PackQuote {
            nodes: packed.reachable,
            chars: packed.budget.chars,
            tokens: packed.budget.used,
            fits: opts.budget.map(|b| packed.budget.used <= b),
        },
        basis: BASIS,
    }
}

/// Price a query, loading the corpus — and, only if it anchors, the index.
pub fn run(root: &std::path::Path, text: &str, opts: &Options) -> Estimate {
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

pub fn render(estimate: &Estimate) -> String {
    if let Some(rejection) = &estimate.rejected {
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
        "{} node(s) match{}{}\n\n",
        estimate.matched,
        // Said only when it bites. A limit that changes nothing is noise on every other quote.
        match estimate.matched > estimate.limit {
            true => format!(", and a projection would return {}", estimate.limit),
            false => String::new(),
        },
        match estimate.budget {
            Some(b) => format!(" — priced against a budget of {b} token(s)"),
            None => String::new(),
        }
    );
    let width = estimate
        .projections
        .iter()
        .map(|p| p.select.len())
        .chain(std::iter::once("a context pack".len()))
        .max()
        .unwrap_or(20);
    out.push_str(&format!(
        "  {:<width$}  {:>7}  {:>9}\n",
        "select", "chars", "~tokens"
    ));
    for p in &estimate.projections {
        out.push_str(&format!(
            "  {:<width$}  {:>7}  {:>9}{}\n",
            p.select,
            p.chars,
            p.tokens,
            verdict(p.fits)
        ));
    }
    out.push_str(&format!(
        "  {:<width$}  {:>7}  {:>9}{}\n",
        "a context pack",
        estimate.pack.chars,
        estimate.pack.tokens,
        verdict(estimate.pack.fits)
    ));
    // Said every time, not only when a budget is in play. A table of round numbers reads as
    // measurement, and half of these are.
    out.push_str(&format!(
        "\n  chars are exact; ~tokens is {} — use chars with a real tokenizer\n",
        estimate.basis
    ));

    if let Some(a) = &estimate.absence {
        // Before the diagnostics and after the table, because it is what makes the table's
        // zeroes mean something. A quote of nothing reads as cheap.
        out.push_str(&format!(
            "  [absent] step {}: {} ({})\n",
            a.step + 1,
            a.message,
            a.code
        ));
    }
    for d in &estimate.diagnostics {
        out.push_str(&format!(
            "  [{}] step {}: {}\n",
            d.level,
            d.step + 1,
            d.message
        ));
    }
    out.trim_end().to_string()
}

fn verdict(fits: Option<bool>) -> &'static str {
    match fits {
        None => "",
        Some(true) => "  fits",
        Some(false) => "  over budget",
    }
}

/// Quote what a query would cost before running it.
#[allow(clippy::too_many_arguments)]
pub fn estimate(
    text: &str,
    select: Option<String>,
    limit: usize,
    budget: Option<usize>,
    anchor_k: usize,
    format: crate::report::Format,
) -> Result<()> {
    let root = repo_root()?;
    let opts = Options {
        select: select
            .unwrap_or_else(|| query::DEFAULT_SELECT.to_string())
            .split(',')
            .map(|f| f.trim().to_string())
            .filter(|f| !f.is_empty())
            .collect(),
        limit,
        budget,
        anchor_k: anchor_k.max(1),
    };
    let estimate = run(&root, text, &opts);
    let rejected = estimate.rejected.is_some();
    if format.is_json() {
        crate::report::emit(&root, estimate)?;
    } else {
        println!("{}", render(&estimate));
    }
    if rejected {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two classes, one typed edge, and prose long enough that the widths differ by more than
    /// rounding — a fixture of bare labels would make every projection cost the same and every
    /// assertion below pass without meaning anything.
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let write = |rel: &str, body: &str| {
            let path = dir.path().join(".yidam/corpus").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write(
            "reach.ont.yml",
            "class: reach\nproperties:\n  - name: regulated\n    type: string\nedges:\n  \
             - relationship: measured-by\n    target: gage\n    direction: out\n",
        );
        write("gage.ont.yml", "class: gage\n");
        write(
            "reach/tailwater.yml",
            "class: reach\nlabel: Tailwater\ndescription: The segment immediately below the \
             impoundment, where discharge is set by the outlet works rather than by the \
             catchment upstream of it.\nproperties:\n  regulated: yes\nlinks:\n  \
             - target: ../gage/canyon.yml\n    relationship: measured-by\n",
        );
        write(
            "gage/canyon.yml",
            "class: gage\nlabel: Canyon Outlet\ndescription: The station at the downstream \
             end of the canyon reach, below the impoundment, with a rating curve maintained \
             against periodic direct measurement.\n",
        );
        dir
    }

    fn quote(dir: &tempfile::TempDir, query: &str, budget: Option<usize>) -> Estimate {
        run(
            dir.path(),
            query,
            &Options {
                budget,
                ..Options::default()
            },
        )
    }

    fn row<'a>(estimate: &'a Estimate, select: &str) -> &'a Projection {
        estimate
            .projections
            .iter()
            .find(|p| p.select == select)
            .unwrap_or_else(|| panic!("no row for `{select}`"))
    }

    /// **The promise.** A quote that is not what the call actually costs is worse than no
    /// quote: an agent that budgeted against it has already spent the difference by the time
    /// it can tell. `chars` is exact by construction — it is the serialized payload — and this
    /// is what holds it to that.
    #[test]
    fn every_quoted_width_is_what_that_query_actually_returns() {
        let dir = fixture();
        let estimate = quote(&dir, "reach -measured-by-> gage", None);
        for projection in &estimate.projections {
            let actual = query::run(
                dir.path(),
                "reach -measured-by-> gage",
                &query::Options {
                    select: projection.select.split(',').map(String::from).collect(),
                    limit: estimate.limit,
                    anchor_k: query::DEFAULT_ANCHOR_K,
                },
            );
            assert_eq!(
                projection.chars, actual.cost.chars,
                "quoted `{}` at {} chars and it came back at {}",
                projection.select, projection.chars, actual.cost.chars
            );
            assert_eq!(projection.tokens, actual.cost.tokens);
        }
    }

    /// The same promise for the pack, which is the row an agent building context reads.
    #[test]
    fn the_pack_row_is_what_pack_actually_produces() {
        let dir = fixture();
        let estimate = quote(&dir, "*", None);
        let packed = crate::cmd::pack::run(dir.path(), "*", &crate::cmd::pack::Options::default());
        assert_eq!(estimate.pack.chars, packed.budget.chars);
        assert_eq!(estimate.pack.tokens, packed.budget.used);
        assert_eq!(estimate.pack.nodes, packed.reachable);
    }

    /// Every width prices the same rows, and those rows are the ones the call would return.
    #[test]
    fn every_width_prices_what_the_call_would_return() {
        let dir = fixture();
        let estimate = quote(&dir, "*", None);
        assert_eq!(estimate.matched, 2);
        for projection in &estimate.projections {
            assert_eq!(projection.rows, estimate.matched, "{}", projection.select);
        }
    }

    /// **A limit is part of the call, so it is part of the quote.** `query` returns fifty rows
    /// by default; quoting four hundred would tell an agent to narrow a call that was already
    /// going to come back small. `pack` has no limit, so its row prices the whole set — and
    /// the two figures sitting side by side is what lets a caller see the difference.
    #[test]
    fn a_limit_bounds_the_projections_it_would_bound_and_not_the_pack() {
        let dir = fixture();
        let estimate = run(
            dir.path(),
            "*",
            &Options {
                limit: 1,
                ..Options::default()
            },
        );
        assert_eq!(estimate.matched, 2, "the walk is not bounded by a limit");
        for projection in &estimate.projections {
            assert_eq!(projection.rows, 1, "{}", projection.select);
        }
        assert_eq!(
            estimate.pack.nodes, 2,
            "a pack has no limit to be bounded by"
        );
        assert!(
            render(&estimate).contains("a projection would return 1"),
            "{}",
            render(&estimate)
        );

        // And it is not mentioned when it changes nothing.
        assert!(!render(&quote(&dir, "*", None)).contains("would return"));
    }

    /// Wider costs more, and the table says so in the order it is read.
    #[test]
    fn the_table_is_ordered_by_what_each_width_costs() {
        let dir = fixture();
        let estimate = quote(&dir, "*", None);
        let costs: Vec<usize> = estimate.projections.iter().map(|p| p.chars).collect();
        let mut sorted = costs.clone();
        sorted.sort();
        assert_eq!(costs, sorted, "cheapest first");
        assert!(
            row(&estimate, "node,class,label").chars
                < row(&estimate, "node,class,label,description").chars,
            "prose has to cost something or this fixture proves nothing"
        );
        assert!(
            row(&estimate, "node,class,label,description").chars
                < row(&estimate, "node,class,label,body").chars
        );
    }

    /// The verdict is what turns a table into a decision. Null without a budget, because a
    /// caller testing the key must not have to distinguish "affordable" from "nothing asked".
    #[test]
    fn a_budget_marks_each_row_and_no_budget_marks_none() {
        let dir = fixture();
        for projection in &quote(&dir, "*", None).projections {
            assert_eq!(projection.fits, None, "{}", projection.select);
        }
        assert_eq!(quote(&dir, "*", None).pack.fits, None);

        let estimate = quote(&dir, "*", Some(60));
        assert_eq!(row(&estimate, "node,class,label").fits, Some(true));
        assert_eq!(row(&estimate, "node,class,label,body").fits, Some(false));
        assert_eq!(estimate.budget, Some(60));
    }

    /// A caller's own `--select` is priced beside the standard three, and never twice.
    #[test]
    fn the_callers_own_projection_is_in_the_table_exactly_once() {
        let dir = fixture();
        let asked = |select: &str| {
            run(
                dir.path(),
                "*",
                &Options {
                    select: select.split(',').map(String::from).collect(),
                    ..Options::default()
                },
            )
        };
        let mine = asked("node,properties.regulated");
        assert_eq!(mine.projections.len(), WIDTHS.len() + 1);
        row(&mine, "node,properties.regulated");

        // The default is already one of the three.
        let standard = asked(query::DEFAULT_SELECT);
        assert_eq!(standard.projections.len(), WIDTHS.len());
    }

    /// **A quote of zero reads as cheap.** An agent told a query costs nothing, and not told
    /// the class is unpopulated, has been handed the most affordable possible way to learn
    /// nothing — which is #283's failure arriving through #284's door.
    #[test]
    fn a_quote_of_nothing_says_why_there_is_nothing() {
        let dir = fixture();
        let estimate = quote(&dir, "reach[regulated=no]", None);
        assert_eq!(estimate.matched, 0);
        let absence = estimate
            .absence
            .as_ref()
            .expect("an empty quote is diagnosed");
        assert_eq!(absence.code, "predicate-unsatisfied");
        assert!(
            render(&estimate).contains("[absent]"),
            "{}",
            render(&estimate)
        );
        // And the pack is not free: it carries that diagnosis, which is the thing worth
        // paying for when the answer is empty.
        assert!(estimate.pack.chars > 0);
    }

    #[test]
    fn a_rejected_query_has_no_quote() {
        let dir = fixture();
        let estimate = quote(&dir, "raech", None);
        assert_eq!(estimate.rejected.as_ref().unwrap().code, "unknown-class");
        assert!(estimate.projections.is_empty());
        assert_eq!(estimate.matched, 0);
        assert!(render(&estimate).starts_with("rejected (unknown-class)"));
    }

    /// The approximation is labelled every time, including when nobody asked about a budget.
    /// A table of round numbers reads as measurement, and half of these are.
    #[test]
    fn the_estimate_says_which_of_its_numbers_are_estimates() {
        let dir = fixture();
        let estimate = quote(&dir, "*", None);
        assert_eq!(estimate.basis, "chars/4");
        assert!(
            render(&estimate).contains("chars are exact; ~tokens is chars/4"),
            "{}",
            render(&estimate)
        );
    }
}
