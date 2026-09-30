//! `yidam query --paths A B` — the typed paths that connect two nodes (#1202).
//!
//! # Why this exists
//!
//! A typed path needs the reader to know the corpus's class names, its relationship names and
//! their direction before writing one. An agent that has just arrived knows two nodes it found
//! by grep. The 2026-09-07 transcript measurement found `query` run **0** times against 10,938
//! greps, and part of that is that the language cannot be learned from the corpus: nothing
//! turned "these two nodes are related" into the path that relates them. This does.
//!
//! # Every printed path has been run
//!
//! A path is found as a chain of nodes, generalised to the class level, and then **executed as
//! a query** before it is printed. It is printed only if that run was accepted and its answer
//! contains the target. So "runnable as-is" is not a property of the spelling routine, which
//! could drift from the parser; it is the observed result of running the string through the
//! same parse, check and executor a caller's `yidam query` will use. A chain whose class path
//! does not survive that — an edge the ontology does not license, a relationship name the
//! grammar cannot spell — is reported as *withheld*, with the reason, rather than dropped.
//!
//! # The edges are the query's own
//!
//! The walk follows [`crate::corpus::Edges::instance_links`], the set `exec::execute` walks —
//! not `neighbors`' wider set, which keeps links to files that do not exist and citations into
//! the catalog. A path over an edge `query` cannot walk would be a path nobody can run. That
//! also answers #1202's question about membership edges: `instance-of` and `source:` links are
//! not traversable here because they are not traversable there.
//!
//! # Bounded, and honest about the bound
//!
//! `--max-hops` bounds the chains considered. Every simple chain within it is enumerated
//! unless a work budget runs out first, and `complete` says whether it did. Separately,
//! `nearest` is the length of the shortest connection **at any depth**, from a plain BFS over
//! the same edges — so an empty answer says whether there is no path at all or no path within
//! the bound, the distinction `absence.rs` draws for an empty query.

use std::collections::{BTreeMap, VecDeque};

use super::{check, exec, lang, Graph};
use crate::cmd::lint::checks::class_of;

/// Hops a path may take, by default.
///
/// Four. Two nodes further apart than that in a typed graph are related through so many
/// intermediate kinds that the class path stops teaching the query and starts listing the
/// ontology.
pub const DEFAULT_MAX_HOPS: usize = 4;

/// Class paths printed, by default.
///
/// Five rather than `query`'s fifty: a caller reading these is choosing one to run, and the
/// count of the rest is reported.
pub const DEFAULT_LIMIT: usize = 5;

/// Node chains kept per class path, as evidence.
const EVIDENCE: usize = 3;

/// Search steps before the enumeration stops and says so.
///
/// Distance pruning keeps the walk to chains that can still reach the target in time, so on
/// any ordinary corpus this is never approached. It exists so a dense corpus at a large
/// `--max-hops` answers `complete: false` instead of not answering.
const BUDGET: usize = 200_000;

/// One class path between the two nodes, and the chains that realised it.
#[derive(Debug, serde::Serialize)]
pub struct Path {
    /// The path in the query language, runnable as-is.
    pub query: String,
    pub hops: usize,
    /// How many node chains within the bound generalise to this path.
    pub chains: usize,
    /// Up to three of them, as node ids from the source to the target.
    pub evidence: Vec<Vec<String>>,
    /// How many nodes `query` returns when this path is run.
    ///
    /// The generalisation #1202 asks about: a path that returns the target and one other node
    /// is close to a fact about these two, and one that returns every node of the class is a
    /// fact about the ontology.
    pub matched: usize,
}

/// A class path that was found and not printed as runnable, and why.
#[derive(Debug, serde::Serialize)]
pub struct Withheld {
    /// The path as it would be written. It does not run as-is — that is why it is here.
    pub query: String,
    pub chains: usize,
    /// The rejection's code when `query` refused the path; null when it ran and did not
    /// return the target, which would be a defect in this command and is reported as one.
    pub code: Option<check::Code>,
    pub reason: String,
}

#[derive(Debug, serde::Serialize)]
pub struct PathsReport {
    /// Which of this command's report shapes this is — always `paths`. See
    /// [`super::QueryReport::kind`].
    pub kind: &'static str,
    /// The source node's id, resolved, or as written when it did not resolve.
    pub from: String,
    pub to: String,
    pub scope: &'static str,
    /// Why no search ran, and null when one did.
    pub rejected: Option<check::Rejection>,
    pub max_hops: usize,
    /// Runnable paths, fewest hops first, capped by `--limit`.
    pub paths: Vec<Path>,
    /// Class paths found within the bound, runnable or not.
    pub found: usize,
    pub returned: usize,
    /// Class paths past `--limit` that were not run, so not known to be runnable.
    pub unchecked: usize,
    pub withheld: Vec<Withheld>,
    /// Whether every simple chain within `max_hops` was enumerated.
    pub complete: bool,
    /// Hops on the shortest connection at any depth, or null when the two are not connected.
    pub nearest: Option<usize>,
}

impl PathsReport {
    fn rejected(from: &str, to: &str, max_hops: usize, rejection: check::Rejection) -> Self {
        Self {
            kind: "paths",
            from: from.to_string(),
            to: to.to_string(),
            scope: "local",
            rejected: Some(rejection),
            max_hops,
            paths: Vec::new(),
            found: 0,
            returned: 0,
            unchecked: 0,
            withheld: Vec::new(),
            complete: true,
            nearest: None,
        }
    }
}

/// Resolve a node as a caller writes it: `reach/tailwater`, with or without `.yml`, with or
/// without the corpus directory in front — the forms `neighbors` accepts.
fn resolve(graph: &Graph, written: &str) -> Option<usize> {
    let bare = |s: &str| s.trim_end_matches(".yml").to_string();
    let want = written.trim().trim_start_matches('/');
    let want = bare(
        want.strip_prefix(&format!("{}/", graph.corpus_dir))
            .unwrap_or(want),
    );
    graph
        .nodes
        .iter()
        .position(|n| bare(&exec::id_of(n, &graph.corpus_dir)) == want)
}

/// One way to leave a node: by which relationship, in which direction, to where.
#[derive(Clone, PartialEq, Eq)]
struct Arc {
    relationship: String,
    direction: lang::Dir,
    to: usize,
}

/// Both directions of every edge `query` can walk, per node, in corpus order.
fn arcs(graph: &Graph) -> Vec<Vec<Arc>> {
    let mut out: Vec<Vec<Arc>> = vec![Vec::new(); graph.nodes.len()];
    for (i, node) in graph.nodes.iter().enumerate() {
        for (edge, to) in graph.edges.instance_links(i) {
            let relationship = edge
                .written_as(node)
                .relationship
                .clone()
                .unwrap_or_default();
            // A node linking to itself cannot sit on a simple chain between two nodes.
            if to == i {
                continue;
            }
            let forward = Arc {
                relationship: relationship.clone(),
                direction: lang::Dir::Out,
                to,
            };
            let backward = Arc {
                relationship,
                direction: lang::Dir::In,
                to: i,
            };
            // The same link written twice is one way to leave, not two.
            if !out[i].contains(&forward) {
                out[i].push(forward);
            }
            if !out[to].contains(&backward) {
                out[to].push(backward);
            }
        }
    }
    out
}

/// Hops from every node to `target`, over edges read in either direction.
fn distances(arcs: &[Vec<Arc>], target: usize) -> Vec<Option<usize>> {
    let mut dist = vec![None; arcs.len()];
    dist[target] = Some(0);
    let mut queue = VecDeque::from([target]);
    while let Some(at) = queue.pop_front() {
        let here = dist[at].unwrap_or(0);
        // Every arc has its reverse on the other node, so the neighbours of `at` are the same
        // set whichever end the search starts from.
        for arc in &arcs[at] {
            if dist[arc.to].is_none() {
                dist[arc.to] = Some(here + 1);
                queue.push_back(arc.to);
            }
        }
    }
    dist
}

/// A chain: the nodes, and the arc taken out of each but the last.
struct Chain {
    nodes: Vec<usize>,
    arcs: Vec<Arc>,
}

/// Every simple chain from `from` to `to` of at most `max_hops`, and whether that is all of
/// them.
fn chains(
    arcs: &[Vec<Arc>],
    dist: &[Option<usize>],
    from: usize,
    to: usize,
    max_hops: usize,
) -> (Vec<Chain>, bool) {
    struct Walk<'a> {
        arcs: &'a [Vec<Arc>],
        dist: &'a [Option<usize>],
        to: usize,
        max_hops: usize,
        on_chain: Vec<bool>,
        nodes: Vec<usize>,
        taken: Vec<Arc>,
        found: Vec<Chain>,
        steps: usize,
    }
    impl Walk<'_> {
        fn go(&mut self, at: usize) -> bool {
            self.steps += 1;
            if self.steps > BUDGET {
                return false;
            }
            if at == self.to {
                self.found.push(Chain {
                    nodes: self.nodes.clone(),
                    arcs: self.taken.clone(),
                });
                return true;
            }
            let depth = self.taken.len();
            for arc in &self.arcs[at] {
                // Pruned on distance: a node that cannot reach the target in the hops left is
                // not worth entering, which is what keeps this from being every walk of
                // length four through the corpus.
                let reachable = self.dist[arc.to].is_some_and(|d| depth + 1 + d <= self.max_hops);
                if self.on_chain[arc.to] || !reachable {
                    continue;
                }
                self.on_chain[arc.to] = true;
                self.nodes.push(arc.to);
                self.taken.push(arc.clone());
                let finished = self.go(arc.to);
                self.taken.pop();
                self.nodes.pop();
                self.on_chain[arc.to] = false;
                if !finished {
                    return false;
                }
            }
            true
        }
    }
    let mut walk = Walk {
        arcs,
        dist,
        to,
        max_hops,
        on_chain: vec![false; arcs.len()],
        nodes: vec![from],
        taken: Vec::new(),
        found: Vec::new(),
        steps: 0,
    };
    walk.on_chain[from] = true;
    let complete = walk.go(from);
    (walk.found, complete)
}

/// A chain at the class level, in the query language.
fn spell(graph: &Graph, chain: &Chain) -> String {
    use std::fmt::Write as _;
    let mut out = class_of(&graph.nodes[chain.nodes[0]]);
    for (arc, &node) in chain.arcs.iter().zip(&chain.nodes[1..]) {
        let (open, close) = match arc.direction {
            lang::Dir::Out => ("-", "->"),
            lang::Dir::In => ("<-", "-"),
        };
        let class = class_of(&graph.nodes[node]);
        let _ = write!(out, " {open}{}{close} {class}", arc.relationship);
    }
    out
}

/// What running a class path as a query did.
enum Ran {
    /// It ran and its answer holds the target. The count is the whole answer's.
    Holds(usize),
    Refused(check::Rejection),
    /// It ran and its answer does not hold the target.
    Misses,
}

/// Run a class path exactly as `yidam query` would, and see whether the target is in it.
fn run(graph: &Graph, text: &str, target: &str) -> Ran {
    let parsed = match lang::parse(text) {
        Ok(parsed) => parsed,
        Err(e) => {
            return Ran::Refused(check::Rejection {
                step: e.token,
                code: check::code::PARSE,
                message: e.message,
            })
        }
    };
    let schema = super::head_schema(graph);
    let checked = match check::check(&parsed, &schema) {
        Ok(checked) => checked,
        Err(rejection) => return Ran::Refused(rejection),
    };
    let outcome = exec::execute(
        &parsed,
        &checked,
        &graph.nodes,
        &graph.edges,
        &graph.corpus_dir,
        None,
    );
    match outcome.matched.iter().any(|m| m == target) {
        true => Ran::Holds(outcome.matched.len()),
        false => Ran::Misses,
    }
}

/// The paths between two nodes of an already-loaded corpus.
pub fn run_on(graph: &Graph, from: &str, to: &str, max_hops: usize, limit: usize) -> PathsReport {
    let unknown = |written: &str, which: usize| check::Rejection {
        step: None,
        code: check::code::UNKNOWN_NODE,
        message: format!(
            "`{written}` names no node in this corpus — write it as `class/name`, the id \
             `yidam query` prints (argument {which})"
        ),
    };
    let Some(source) = resolve(graph, from) else {
        return PathsReport::rejected(from, to, max_hops, unknown(from, 1));
    };
    let Some(target) = resolve(graph, to) else {
        return PathsReport::rejected(from, to, max_hops, unknown(to, 2));
    };
    let id = |i: usize| exec::id_of(&graph.nodes[i], &graph.corpus_dir);

    let arcs = arcs(graph);
    let dist = distances(&arcs, target);
    let (found, complete) = chains(&arcs, &dist, source, target, max_hops);

    // Grouped by spelling, which is what a caller would run. Fewest hops first, then by the
    // text, so two runs print the same list.
    let mut by_path: BTreeMap<(usize, String), Vec<Vec<String>>> = BTreeMap::new();
    for chain in &found {
        by_path
            .entry((chain.arcs.len(), spell(graph, chain)))
            .or_default()
            .push(chain.nodes.iter().map(|&n| id(n)).collect());
    }

    let target_id = id(target);
    let total = by_path.len();
    let mut paths = Vec::new();
    let mut withheld = Vec::new();
    let mut unchecked = 0;
    for ((hops, query), mut evidence) in by_path {
        if paths.len() == limit {
            unchecked += 1;
            continue;
        }
        let chains = evidence.len();
        match run(graph, &query, &target_id) {
            Ran::Holds(matched) => {
                evidence.sort();
                evidence.truncate(EVIDENCE);
                paths.push(Path {
                    query,
                    hops,
                    chains,
                    evidence,
                    matched,
                });
            }
            Ran::Refused(rejection) => withheld.push(Withheld {
                query,
                chains,
                code: Some(rejection.code),
                reason: rejection.message,
            }),
            Ran::Misses => withheld.push(Withheld {
                query,
                chains,
                code: None,
                reason: format!(
                    "ran, and its answer does not contain `{target_id}` — a chain realises this \
                     path, so this is a defect in `--paths`, not a fact about the corpus"
                ),
            }),
        }
    }

    PathsReport {
        kind: "paths",
        from: id(source),
        to: target_id,
        scope: "local",
        rejected: None,
        max_hops,
        returned: paths.len(),
        paths,
        found: total,
        unchecked,
        withheld,
        complete,
        nearest: dist[source],
    }
}

pub fn render(report: &PathsReport) -> String {
    use std::fmt::Write as _;
    if let Some(rejection) = &report.rejected {
        return format!("rejected ({}): {}", rejection.code, rejection.message);
    }
    let mut out = String::new();
    for path in &report.paths {
        let _ = writeln!(out, "{}", path.query);
        let _ = writeln!(
            out,
            "  {} hop(s); returns {} node(s) when run; {} chain(s) realise it, e.g.",
            path.hops, path.matched, path.chains
        );
        for chain in &path.evidence {
            let _ = writeln!(out, "    {}", chain.join(" → "));
        }
    }
    for held in &report.withheld {
        let code = held.code.map(|c| format!(" ({c})")).unwrap_or_default();
        let _ = writeln!(
            out,
            "[withheld] {} — does not run as-is{code}: {}",
            held.query, held.reason
        );
    }
    let bound = format!("within {} hop(s)", report.max_hops);
    let summary = match (report.paths.is_empty(), report.nearest) {
        (false, _) => format!(
            "{} of {} path(s) {bound} shown{}",
            report.returned,
            report.found,
            match report.unchecked {
                0 => String::new(),
                n => format!("; {n} more not run — raise --limit to see them"),
            }
        ),
        (true, None) => format!(
            "no path: `{}` and `{}` are not connected by any edge `query` can walk, at any length",
            report.from, report.to
        ),
        (true, Some(d)) if d > report.max_hops => format!(
            "no path {bound}; the nearest connection is {d} hop(s) — raise --max-hops to {d}"
        ),
        (true, Some(_)) => format!("no runnable path {bound}"),
    };
    let _ = write!(out, "{summary}");
    if !report.complete {
        let _ = write!(
            out,
            "\nthe search stopped at its work budget before enumerating every chain {bound} — \
             lower --max-hops for a complete answer"
        );
    }
    out
}

/// `yidam query --paths FROM TO`.
pub fn paths(
    root: Option<&std::path::Path>,
    from: &str,
    to: &str,
    max_hops: usize,
    limit: usize,
    format: crate::report::Format,
) -> anyhow::Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let report = run_on(&Graph::load(&root), from, to, max_hops, limit);
    // Gates on the refusal only, as `query` does: no path is an answer, not a failure.
    let rejected = report.rejected.is_some();
    crate::report::gate(&root, format, report, !rejected, |r| {
        println!("{}", render(r))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `examples/streamflow`'s shape, small enough to reason about by hand:
    ///
    /// ```text
    /// tailwater -measured-by-> canyon -sources-from-> hydropeaking
    /// tailwater -exhibits-> hydropeaking
    /// canyon -sources-from-> low-flow <-refines- hydropeaking
    /// lower -downstream-of-> tailwater
    /// island (no edges)
    /// ```
    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus");
        for class in ["reach", "gage", "concept"] {
            std::fs::create_dir_all(corpus.join(class)).unwrap();
        }
        let write = |path: &str, text: &str| std::fs::write(corpus.join(path), text).unwrap();
        write(
            "reach.ont.yml",
            "class: reach\nedges:\n  \
             - relationship: measured-by\n    target: gage\n    direction: out\n  \
             - relationship: exhibits\n    target: concept\n    direction: out\n  \
             - relationship: downstream-of\n    target: reach\n    direction: out\n",
        );
        write(
            "gage.ont.yml",
            "class: gage\nedges:\n  \
             - relationship: sources-from\n    target: concept\n    direction: out\n",
        );
        write(
            "concept.ont.yml",
            "class: concept\nedges:\n  \
             - relationship: refines\n    target: concept\n    direction: out\n",
        );
        write(
            "reach/tailwater.yml",
            "class: reach\nlinks:\n  \
             - target: ../gage/canyon.yml\n    relationship: measured-by\n  \
             - target: ../concept/hydropeaking.yml\n    relationship: exhibits\n",
        );
        write(
            "reach/lower.yml",
            "class: reach\nlinks:\n  \
             - target: ./tailwater.yml\n    relationship: downstream-of\n",
        );
        write("reach/island.yml", "class: reach\n");
        write(
            "gage/canyon.yml",
            "class: gage\nlinks:\n  \
             - target: ../concept/hydropeaking.yml\n    relationship: sources-from\n  \
             - target: ../concept/low-flow.yml\n    relationship: sources-from\n",
        );
        write(
            "concept/hydropeaking.yml",
            "class: concept\nlinks:\n  \
             - target: ./low-flow.yml\n    relationship: refines\n",
        );
        write("concept/low-flow.yml", "class: concept\n");
        dir
    }

    fn between(dir: &tempfile::TempDir, from: &str, to: &str, max_hops: usize) -> PathsReport {
        run_on(&Graph::load(dir.path()), from, to, max_hops, DEFAULT_LIMIT)
    }

    fn queries(report: &PathsReport) -> Vec<&str> {
        report.paths.iter().map(|p| p.query.as_str()).collect()
    }

    #[test]
    fn paths_come_back_as_queries_fewest_hops_first() {
        let dir = fixture();
        let report = between(&dir, "reach/tailwater", "concept/hydropeaking", 4);
        assert!(report.rejected.is_none(), "{:?}", report.rejected);
        assert_eq!(
            queries(&report),
            [
                "reach -exhibits-> concept",
                "reach -measured-by-> gage -sources-from-> concept",
                "reach -measured-by-> gage -sources-from-> concept <-refines- concept",
            ]
        );
        assert_eq!(report.from, "reach/tailwater.yml");
        assert_eq!(report.to, "concept/hydropeaking.yml");
        assert!(report.complete);
        assert_eq!(report.nearest, Some(1));
        assert!(report.withheld.is_empty(), "{:?}", report.withheld);
    }

    /// The evidence is the node chain, so a path teaches the query and shows what realised it.
    #[test]
    fn each_path_carries_the_chain_that_realised_it() {
        let dir = fixture();
        let report = between(&dir, "reach/tailwater", "concept/hydropeaking", 4);
        let longest = report.paths.last().unwrap();
        assert_eq!(longest.hops, 3);
        assert_eq!(
            longest.evidence,
            [[
                "reach/tailwater.yml",
                "gage/canyon.yml",
                "concept/low-flow.yml",
                "concept/hydropeaking.yml",
            ]]
        );
    }

    /// #1202's acceptance: every printed path, fed back to `query`, returns the target.
    ///
    /// Through `super::run`, the entry point `yidam query` itself calls — not the helper this
    /// module verifies with, which would be checking the command against itself.
    #[test]
    fn every_printed_path_runs_as_a_query_and_returns_the_target() {
        let dir = fixture();
        for (from, to) in [
            ("reach/tailwater", "concept/hydropeaking"),
            ("reach/lower", "concept/low-flow"),
            ("concept/low-flow", "reach/lower"),
            ("gage/canyon", "reach/lower.yml"),
        ] {
            let report = between(&dir, from, to, 4);
            assert!(!report.paths.is_empty(), "{from} → {to}: no paths");
            for path in &report.paths {
                let ran = super::super::run(
                    dir.path(),
                    &path.query,
                    &super::super::Options {
                        limit: usize::MAX,
                        ..Default::default()
                    },
                );
                assert!(ran.rejected.is_none(), "{}: {:?}", path.query, ran.rejected);
                let nodes: Vec<&serde_json::Value> =
                    ran.results.iter().map(|row| &row["node"]).collect();
                assert!(
                    nodes.contains(&&serde_json::json!(report.to)),
                    "{from} → {to}: `{}` does not return {}: {nodes:?}",
                    path.query,
                    report.to
                );
                assert_eq!(ran.matched, path.matched, "{}", path.query);
            }
        }
    }

    #[test]
    fn a_path_can_run_against_the_authoring_direction() {
        let dir = fixture();
        let report = between(&dir, "concept/low-flow", "reach/lower", 4);
        assert_eq!(
            queries(&report)[..2],
            [
                "concept <-refines- concept <-exhibits- reach <-downstream-of- reach",
                "concept <-sources-from- gage <-measured-by- reach <-downstream-of- reach",
            ]
        );
    }

    /// No path within the bound, and a path beyond it, are different answers.
    #[test]
    fn a_path_past_the_bound_is_named_rather_than_denied() {
        let dir = fixture();
        let report = between(&dir, "reach/lower", "concept/low-flow", 2);
        assert!(report.paths.is_empty());
        assert_eq!(report.found, 0);
        assert_eq!(report.nearest, Some(3));
        assert!(report.complete);
        let text = render(&report);
        assert!(text.contains("nearest connection is 3 hop(s)"), "{text}");
    }

    #[test]
    fn unconnected_nodes_say_so_at_any_length() {
        let dir = fixture();
        let report = between(&dir, "reach/island", "concept/low-flow", 4);
        assert!(report.paths.is_empty());
        assert_eq!(report.nearest, None);
        assert!(
            render(&report).contains("at any length"),
            "{}",
            render(&report)
        );
    }

    #[test]
    fn the_limit_bounds_what_is_run_and_says_how_many_it_left() {
        let dir = fixture();
        let report = run_on(
            &Graph::load(dir.path()),
            "reach/tailwater",
            "concept/hydropeaking",
            4,
            1,
        );
        assert_eq!(queries(&report), ["reach -exhibits-> concept"]);
        assert_eq!(report.found, 3);
        assert_eq!(report.returned, 1);
        assert_eq!(report.unchecked, 2);
        assert!(
            render(&report).contains("2 more not run"),
            "{}",
            render(&report)
        );
    }

    /// A path over an edge the ontology does not license would be refused by `query`, so it
    /// is not printed as runnable — and it is not dropped either.
    #[test]
    fn a_path_query_would_refuse_is_withheld_with_the_reason() {
        let dir = fixture();
        std::fs::write(
            dir.path().join(".yidam/corpus/reach/island.yml"),
            "class: reach\nlinks:\n  - target: ../concept/low-flow.yml\n    relationship: \
             measured-by\n",
        )
        .unwrap();
        let report = between(&dir, "reach/island", "concept/low-flow", 1);
        assert!(report.paths.is_empty(), "{:?}", queries(&report));
        assert_eq!(report.withheld.len(), 1, "{:?}", report.withheld);
        assert_eq!(report.withheld[0].query, "reach -measured-by-> concept");
        assert_eq!(
            report.withheld[0].code.map(|c| c.to_string()).as_deref(),
            Some("unlicensed-hop")
        );
    }

    #[test]
    fn a_node_that_does_not_resolve_is_refused_by_name() {
        let dir = fixture();
        let report = between(&dir, "reach/tailwater", "gage/cannon", 4);
        let rejection = report.rejected.as_ref().expect("must be refused");
        assert_eq!(rejection.code, "unknown-node");
        assert!(
            rejection.message.contains("gage/cannon"),
            "{}",
            rejection.message
        );
        assert!(render(&report).starts_with("rejected ("));
    }

    /// The forms `neighbors` accepts: bare, with `.yml`, and with the corpus directory.
    #[test]
    fn a_node_resolves_in_every_form_neighbors_accepts() {
        let dir = fixture();
        for written in [
            "reach/tailwater",
            "reach/tailwater.yml",
            ".yidam/corpus/reach/tailwater.yml",
        ] {
            let report = between(&dir, written, "gage/canyon", 1);
            assert!(
                report.rejected.is_none(),
                "{written}: {:?}",
                report.rejected
            );
            assert_eq!(report.from, "reach/tailwater.yml");
        }
    }

    /// The budget is what makes `complete` a claim and not a constant: a dense graph at a
    /// deep bound must stop and say so rather than enumerate forever.
    #[test]
    fn an_exhausted_budget_reports_an_incomplete_search() {
        // A complete graph on 12 nodes has ~10^8 simple paths between two nodes within ten
        // hops, far past the budget.
        let n = 12;
        let mut arcs: Vec<Vec<Arc>> = vec![Vec::new(); n];
        for (i, out) in arcs.iter_mut().enumerate() {
            for j in (0..n).filter(|&j| j != i) {
                out.push(Arc {
                    relationship: "near".into(),
                    direction: lang::Dir::Out,
                    to: j,
                });
            }
        }
        let dist = distances(&arcs, n - 1);
        let (_, complete) = chains(&arcs, &dist, 0, n - 1, 10);
        assert!(!complete);
        let (found, complete) = chains(&arcs, &dist, 0, n - 1, 2);
        assert!(complete);
        assert_eq!(found.len(), 1 + (n - 2));
    }
}
