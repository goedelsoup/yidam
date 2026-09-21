//! Resolving a similarity anchor to entry nodes — #263's hybrid anchoring.
//!
//! `class~"…"` enters the graph by meaning and leaves it by typed edge. That pairing is the
//! mechanism `docs/research/system` argues for and the one `serve --mcp` did not have: an
//! agent asking about a corpus got top-*k* retrieval, which is a scan, from a system whose
//! whole argument is that a scan is the wrong shape.
//!
//! # Three rules, and what each one is protecting
//!
//! **The anchor is local.** `retrieve` chains `state.dep_nodes` after `state.nodes`, which is
//! right for retrieval — an agent asking what is known about X should be told when the answer
//! lives in a corpus this repository merely cites. A query reports `"scope": "local"` and must
//! not enter through a dependency's node, so both paths here restrict to nodes this
//! repository owns. The vector path gets it for free (`embed` walks the local corpus only) and
//! asserts it anyway, because "for free" is a property of another file.
//!
//! **The anchor is class-qualified.** Enforced by [`super::lang`], for the reason RFC-0018
//! gives: a hop's verdict depends on the source class's `edge_policy`, so a bare anchor could
//! not be typechecked before it ran. Here that shows up as `classes` — the step's narrowed
//! set — being a filter on candidates rather than a check applied afterwards, so `k` counts
//! nodes of the right class.
//!
//! **The anchor degrades and says so.** In a build without the index, or against a corpus
//! with none, this falls through to the same keyword scorer `retrieve` falls through to, and
//! the report carries `degraded`, `degraded_reason` and the repair. `bench` refuses instead —
//! a measurement of anchored traversal against a keyword baseline measures nothing — and that
//! difference is deliberate: a query answering worse is useful, a benchmark answering worse is
//! a false number.

use super::exec::id_of;
use crate::cmd::lint::checks::{class_of, Node};
use crate::retrieval::Retrieval;

/// One entry node, with the score that chose it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Entry {
    pub node: String,
    pub score: f32,
}

/// What the anchor did, as the report carries it.
///
/// `degraded` and `degraded_reason` are the MCP `retrieve` keys by name and by discipline —
/// present always, `degraded_reason` null exactly when `degraded` is false — and the reason
/// strings come from [`Retrieval::degraded_reason`], not from a second list here. RFC-0018
/// calls this borrowing rather than reuse: these are fields on an RFC-0016 payload, not on a
/// `tools/call` result, and the only thing genuinely shared is the vocabulary.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Anchor {
    /// Which step anchored. Always 0 today — an anchor is an entry — and carried anyway so a
    /// consumer never has to assume it.
    pub step: usize,
    pub text: String,
    pub k: usize,
    pub degraded: bool,
    pub degraded_reason: Option<&'static str>,
    /// What to do about it, or null. Null exactly when `degraded` is false.
    pub repair: Option<&'static str>,
    /// The entry nodes, best first. Present so what the query anchored on is always visible;
    /// an anchor that landed somewhere surprising is the first thing to check when the answer
    /// is surprising.
    pub entries: Vec<Entry>,
}

/// The entry set, plus what producing it cost.
pub struct Resolved {
    pub anchor: Anchor,
    /// Entry ids in score order — the order the walk starts in.
    pub entries: Vec<String>,
    /// Nodes whose content the resolution had to read, for [`super::exec::Cost::nodes_read`].
    ///
    /// **The two paths cost differently, and that is the point.** A vector anchor reads the
    /// `k` nodes it returns: the embeddings were computed at index time and the walk opens
    /// nothing else. The keyword fallback has to read every candidate node's text to score
    /// it, which is a scan of the class — so a degraded anchor costs what a scan costs, and
    /// the cost block says so without anyone having to notice the flag.
    pub read: Vec<String>,
}

/// Resolve `text` to at most `k` entry nodes of one of `classes`.
///
/// Never fails into an empty result: an anchor that resolves to nothing comes back with no
/// entries and a report that still says which path ran and why, which is the same rule the
/// rest of this surface follows — an empty answer must be distinguishable from a broken one.
pub fn resolve(
    retrieval: &Retrieval,
    step: usize,
    text: &str,
    classes: &[String],
    k: usize,
    nodes: &[Node],
    corpus_dir: &str,
) -> Result<Resolved, String> {
    // The vector arm can find, on its first search, that the index was built in a space this
    // binary does not embed into — which `Retrieval` cannot know at load time without paying
    // to load the model. So the reason starts as what the retrieval state declares and is
    // overridden by what the search discovered.
    //
    // `unused_mut` in the light build is correct there and not worth a second code path: that
    // build never embeds, so it can never reach the override.
    #[cfg_attr(not(feature = "vector-read"), allow(unused_mut))]
    let mut reason = retrieval.degraded_reason();
    #[cfg_attr(not(feature = "vector-read"), allow(unused_mut))]
    let mut repair = retrieval.repair();
    let (entries, read) = match retrieval {
        #[cfg(feature = "vector-read")]
        Retrieval::Vector(index) => {
            let searched = search_index(
                |filter, residual| {
                    crate::retrieval::vector::search(index, text, k, filter, residual)
                },
                classes,
                k,
                nodes,
                corpus_dir,
            )?;
            degrade_or(searched, &mut reason, &mut repair, || {
                keyword_entries(text, classes, k, nodes, corpus_dir)
            })
        }
        #[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
        Retrieval::Remote(remote) => {
            let searched = search_index(
                |filter, residual| {
                    crate::retrieval::remote::search(remote, text, k, filter, residual)
                },
                classes,
                k,
                nodes,
                corpus_dir,
            )?;
            degrade_or(searched, &mut reason, &mut repair, || {
                keyword_entries(text, classes, k, nodes, corpus_dir)
            })
        }
        _ => keyword_entries(text, classes, k, nodes, corpus_dir),
    };
    Ok(Resolved {
        anchor: Anchor {
            step,
            text: text.to_string(),
            k,
            degraded: reason.is_some(),
            degraded_reason: reason,
            repair,
            entries: entries.clone(),
        },
        entries: entries.into_iter().map(|e| e.node).collect(),
        read,
    })
}

/// Nodes of a candidate class, keyed by the repo-relative path the index records.
///
/// Keyed by `Node::rel` and matched against `VectorRow::path` — both are
/// `strip_prefix(root)` of the same walk, so they agree on separator and case on any one
/// platform. A row that resolves to no node here is a catalog source or an index built before
/// a file moved, and either way it is not a node this query may enter through.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn candidates<'a>(
    nodes: &'a [Node],
    classes: &[String],
) -> std::collections::BTreeMap<&'a str, &'a Node> {
    nodes
        .iter()
        .filter(|n| classes.contains(&class_of(n)))
        .map(|n| (n.rel.as_str(), n))
        .collect()
}

/// What a step resolved to: its entries, and the nodes reading them charged.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
type Entries = (Vec<Entry>, Vec<String>);

/// A vector step's outcome: what it found, or the frozen reason it could not.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
enum Anchored {
    Found(Entries),
    /// `(degraded_reason, repair)` — both frozen strings from [`crate::retrieval`], never
    /// composed here. Two copies of "why is retrieval degraded" is two answers to one
    /// question, which is the whole argument `retrieval/mod.rs` opens with.
    Degraded(&'static str, &'static str),
}

/// Fold an [`Anchored`] into entries, recording the reason when it degraded.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn degrade_or(
    searched: Anchored,
    reason: &mut Option<&'static str>,
    repair: &mut Option<&'static str>,
    fallback: impl FnOnce() -> Entries,
) -> Entries {
    match searched {
        Anchored::Found(found) => found,
        Anchored::Degraded(why, how) => {
            *reason = Some(why);
            *repair = Some(how);
            fallback()
        }
    }
}

/// Run one backend's search and resolve its hits to nodes this step may enter through.
///
/// `search` is passed rather than the backend, so the local and remote arms differ in one
/// expression rather than in a copy of everything after it.
///
/// **The class set is pushed, as a claim about nodes.** RFC-0033 §4.5 passed
/// [`crate::retrieval::Filter::any()`] here and did the whole narrowing locally, on a premise
/// it stated and could not hold: a row's `class` was written by whatever `yidam embed` wrote
/// when the index was built, a node's class is computed now, the two agree today and nothing
/// held them to it. [`crate::retrieval::Filter::nodes_of`] is that premise made into a
/// question the index answers — see [`crate::retrieval::Filter::as_applied`] for how, and for
/// why an index that answers "no" leaves this function doing exactly what it did before.
///
/// The residual is unchanged and stays authoritative whichever way the index answered: a row
/// may be of the right class and still belong to a dependency, a catalog source, or a file
/// that has since moved.
#[cfg_attr(not(feature = "vector-read"), allow(dead_code))]
fn search_index(
    search: impl FnOnce(
        &crate::retrieval::Filter,
        &dyn Fn(&crate::retrieval::Hit) -> bool,
    ) -> Result<crate::retrieval::Searched, String>,
    classes: &[String],
    k: usize,
    nodes: &[Node],
    corpus_dir: &str,
) -> Result<Anchored, String> {
    let candidates = candidates(nodes, classes);
    // The ownership test is applied during the search rather than after truncation: `k` must
    // count nodes the step could actually match, or a `k` of 1 against a corpus whose nearest
    // row is of another class resolves to nothing and looks like a miss.
    let residual = |hit: &crate::retrieval::Hit| candidates.contains_key(hit.path.as_str());
    let searched = search(&crate::retrieval::Filter::nodes_of(classes), &residual)?;

    let hits = match searched {
        crate::retrieval::Searched::Hits(hits) => hits,
        crate::retrieval::Searched::SpaceMismatch => {
            return Ok(Anchored::Degraded(
                crate::retrieval::STALE_CONTRACT,
                crate::retrieval::STALE_CONTRACT_REPAIR,
            ))
        }
        #[cfg(feature = "s3-vectors")]
        crate::retrieval::Searched::Unavailable(_why) => {
            return Ok(Anchored::Degraded(
                crate::retrieval::REMOTE_UNAVAILABLE,
                crate::retrieval::REMOTE_UNAVAILABLE_REPAIR,
            ))
        }
    };

    let entries: Vec<Entry> = hits
        .iter()
        .filter_map(|hit| {
            candidates.get(hit.path.as_str()).map(|node| Entry {
                node: id_of(node, corpus_dir),
                score: hit.score,
            })
        })
        .collect();
    let read = entries.iter().map(|e| e.node.clone()).collect();
    let _ = k;
    Ok(Anchored::Found((entries, read)))
}

/// The fallback: the same scorer `retrieve` degrades to, over the step's candidate classes.
fn keyword_entries(
    text: &str,
    classes: &[String],
    k: usize,
    nodes: &[Node],
    corpus_dir: &str,
) -> (Vec<Entry>, Vec<String>) {
    let terms = crate::retrieval::terms(text);
    let mut read = Vec::new();
    let mut scored: Vec<Entry> = Vec::new();
    for node in nodes.iter().filter(|n| classes.contains(&class_of(n))) {
        let id = id_of(node, corpus_dir);
        // Charged whether it scores or not. Rejecting a node still means having read it, and
        // a fallback that only charged for its hits would look cheaper the worse it did.
        read.push(id.clone());
        // `node.text` and not a re-read of `node.path`: at a past commit the path names a
        // file whose current contents are a different revision's, and scoring against those
        // would anchor the query in the wrong year.
        let haystack = format!(
            "{} {} {}",
            node.inst.label.as_deref().unwrap_or_default(),
            node.inst.description.as_deref().unwrap_or_default(),
            node.text
        )
        .to_lowercase();
        if let Some(score) = crate::retrieval::keyword_score(&terms, &haystack) {
            scored.push(Entry { node: id, score });
        }
    }
    // Ties break on the id: corpus order would do here, but an anchor's entries are the one
    // ordering in this surface that is *not* corpus order, and two orderings that agree only
    // by accident is how a golden starts pinning the filesystem.
    scored.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.node.cmp(&b.node))
    });
    scored.truncate(k);
    (scored, read)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::Overlay;
    use crate::walk::walk_corpus_instances;

    /// Degrading records the frozen reason AND its repair, together.
    ///
    /// The two are written into separate `&mut` bindings by a function whose other arm writes
    /// neither, which is the shape where one of them gets forgotten. A reason with no repair is
    /// a diagnosis with no treatment, and the states exist precisely because their treatments
    /// differ.
    #[test]
    fn degrading_records_the_reason_and_the_repair_and_falls_back() {
        let mut reason = None;
        let mut repair = None;
        let entries = degrade_or(
            Anchored::Degraded(
                crate::retrieval::STALE_CONTRACT,
                crate::retrieval::STALE_CONTRACT_REPAIR,
            ),
            &mut reason,
            &mut repair,
            || (vec![], vec!["fell back".to_string()]),
        );
        assert_eq!(reason, Some("stale_contract"));
        assert_eq!(repair, Some(crate::retrieval::STALE_CONTRACT_REPAIR));
        assert_eq!(entries.1, vec!["fell back".to_string()]);
    }

    /// A step that found something reports no reason and does not run the fallback.
    #[test]
    fn finding_something_records_nothing_and_does_not_fall_back() {
        let mut reason = Some("stale-from-an-earlier-step");
        let mut repair = None;
        let entries = degrade_or(
            Anchored::Found((vec![], vec!["from the index".to_string()])),
            &mut reason,
            &mut repair,
            || panic!("the fallback ran for a step that found its entries"),
        );
        assert_eq!(entries.1, vec!["from the index".to_string()]);
        // Untouched — a successful step does not clear a reason the caller already had, and
        // does not invent one.
        assert_eq!(reason, Some("stale-from-an-earlier-step"));
        assert_eq!(repair, None);
    }

    /// A remote index that did not answer reports its own reason, not the local one.
    ///
    /// The mapping is two lines and both arms look alike, which is exactly where a copied arm
    /// keeps the string it was copied from — and `stale_contract` would send a reader to
    /// rebuild an index that is not the problem.
    #[cfg(all(feature = "vector-read", feature = "s3-vectors"))]
    #[test]
    fn an_unreachable_remote_does_not_report_a_stale_contract() {
        let mut reason = None;
        let mut repair = None;
        degrade_or(
            Anchored::Degraded(
                crate::retrieval::REMOTE_UNAVAILABLE,
                crate::retrieval::REMOTE_UNAVAILABLE_REPAIR,
            ),
            &mut reason,
            &mut repair,
            || (vec![], vec![]),
        );
        assert_eq!(reason, Some("remote_unavailable"));
        assert_ne!(reason, Some(crate::retrieval::STALE_CONTRACT));
        assert!(repair.is_some_and(|r| r.contains("index.remote")));
    }

    /// A backend, faithful in the three things [`search_index`] depends on: it asks the
    /// filter what it may apply, applies it, and applies the residual before returning —
    /// which is what both real backends do.
    ///
    /// `path_derived` is the answer the index gives about its own class metadata. It is a
    /// constructor argument and not a constant because both answers are behaviour under test.
    struct Backend {
        rows: Vec<crate::retrieval::Hit>,
        path_derived: bool,
        seen: std::cell::RefCell<Vec<crate::retrieval::Filter>>,
    }

    impl Backend {
        fn new(path_derived: bool, rows: Vec<crate::retrieval::Hit>) -> Self {
            Self {
                rows,
                path_derived,
                seen: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn search(
            &self,
            filter: &crate::retrieval::Filter,
            residual: &dyn Fn(&crate::retrieval::Hit) -> bool,
        ) -> Result<crate::retrieval::Searched, String> {
            let applied = filter.as_applied(self.path_derived);
            self.seen.borrow_mut().push(applied.clone());
            let mut hits: Vec<crate::retrieval::Hit> = self
                .rows
                .iter()
                .filter(|r| applied.admits(&r.class))
                .filter(|r| residual(r))
                .cloned()
                .collect();
            // `s3vectors::response::finish`'s ordering, which both real backends return in:
            // score descending, ties broken on the path.
            hits.sort_by(|a, b| {
                b.score
                    .total_cmp(&a.score)
                    .then_with(|| a.path.cmp(&b.path))
            });
            Ok(crate::retrieval::Searched::Hits(hits))
        }

        fn applied(&self) -> Vec<crate::retrieval::Filter> {
            self.seen.borrow().clone()
        }
    }

    /// One indexed row: the path the index recorded, and the class it recorded for it.
    ///
    /// The two are separate arguments precisely because the premise under test is that they
    /// agree. A helper deriving one from the other could not express a stale index.
    fn row(path: &str, class: &str, score: f32) -> crate::retrieval::Hit {
        crate::retrieval::Hit {
            path: path.to_string(),
            class: class.to_string(),
            label: String::new(),
            text: String::new(),
            score,
            truncated: false,
        }
    }

    fn entries_of(backend: &Backend, classes: &[&str], nodes: &[Node]) -> Vec<String> {
        let classes: Vec<String> = classes.iter().map(|c| c.to_string()).collect();
        let anchored = search_index(
            |filter, residual| backend.search(filter, residual),
            &classes,
            5,
            nodes,
            ".yidam/corpus",
        )
        .unwrap();
        match anchored {
            Anchored::Found((entries, _)) => entries.into_iter().map(|e| e.node).collect(),
            Anchored::Degraded(why, _) => panic!("degraded: {why}"),
        }
    }

    /// An index that vouches for its class metadata is handed the step's class set.
    ///
    /// This is the change RFC-0033 §4.5 deferred: the narrowing used to happen after the
    /// fetch, and `Filter::any()` was all a service ever saw.
    #[test]
    fn an_anchored_step_pushes_its_class_set_to_an_index_that_vouches_for_it() {
        let dir = fixture();
        let nodes = nodes(dir.path());
        let backend = Backend::new(
            true,
            vec![
                row(".yidam/corpus/gage/outlet.yml", "gage", 0.9),
                row(".yidam/corpus/reach/tailwater.yml", "reach", 0.8),
            ],
        );

        assert_eq!(
            entries_of(&backend, &["reach"], &nodes),
            ["reach/tailwater.yml"]
        );
        assert_eq!(
            backend.applied(),
            vec![crate::retrieval::Filter::nodes_of(&["reach".to_string()])],
            "the class set was not pushed"
        );
    }

    /// An index that makes no claim about its class metadata is searched wide, and the
    /// narrowing happens where it always did.
    ///
    /// The rows here are what an older binary's derivation could have written — the right
    /// nodes under a name this corpus does not use. Pushed, they would be excluded and the
    /// step would resolve to nothing; unpushed, the residual finds them by path.
    #[test]
    fn an_index_that_cannot_vouch_is_searched_wide_and_still_answers() {
        let dir = fixture();
        let nodes = nodes(dir.path());
        let backend = Backend::new(
            false,
            vec![
                row(".yidam/corpus/reach/tailwater.yml", "Reach", 0.9),
                row(".yidam/corpus/reach/canyon.yml", "Reach", 0.8),
                row(".yidam/corpus/gage/outlet.yml", "Gage", 0.7),
            ],
        );

        assert_eq!(
            entries_of(&backend, &["reach"], &nodes),
            ["reach/tailwater.yml", "reach/canyon.yml"],
            "a class derivation this binary does not share turned into an empty result"
        );
        assert_eq!(
            backend.applied(),
            vec![crate::retrieval::Filter::any()],
            "a filter naming nodes was pushed to an index that vouches for nothing"
        );
    }

    /// A step that narrowed to no class pushes nothing: `$in: []` is a validation error at the
    /// service, and there is no candidate node for it to have matched either way.
    #[test]
    fn an_empty_class_set_is_not_pushed() {
        let dir = fixture();
        let nodes = nodes(dir.path());
        let backend = Backend::new(
            true,
            vec![row(".yidam/corpus/reach/tailwater.yml", "reach", 0.9)],
        );

        assert!(entries_of(&backend, &[], &nodes).is_empty());
        assert_eq!(backend.applied(), vec![crate::retrieval::Filter::any()]);
    }

    fn fixture() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("reach")).unwrap();
        std::fs::create_dir_all(corpus.join("gage")).unwrap();
        std::fs::write(
            corpus.join("reach/tailwater.yml"),
            "class: reach\nlabel: Tailwater\ndescription: Flow below the outlet works.\n",
        )
        .unwrap();
        std::fs::write(
            corpus.join("reach/canyon.yml"),
            "class: reach\nlabel: Canyon\ndescription: An unregulated mountain reach.\n",
        )
        .unwrap();
        std::fs::write(
            corpus.join("gage/outlet.yml"),
            "class: gage\nlabel: Outlet works gage\ndescription: Below the outlet works.\n",
        )
        .unwrap();
        dir
    }

    fn nodes(dir: &std::path::Path) -> Vec<Node> {
        let corpus = dir.join(".yidam/corpus");
        crate::cmd::lint::checks::load_nodes(
            dir,
            &walk_corpus_instances(&corpus),
            &Overlay::default(),
        )
    }

    fn resolved(query: &str, classes: &[&str], k: usize) -> Resolved {
        let dir = fixture();
        let nodes = nodes(dir.path());
        let classes: Vec<String> = classes.iter().map(|c| c.to_string()).collect();
        resolve(
            &Retrieval::NoIndex,
            0,
            query,
            &classes,
            k,
            &nodes,
            ".yidam/corpus",
        )
        .unwrap()
    }

    /// The class is a filter on candidates, not a check after the fact. `gage/outlet` is the
    /// better keyword match for "outlet works" and must not appear at all.
    #[test]
    fn the_anchor_never_leaves_the_class_it_names() {
        let r = resolved("outlet works", &["reach"], 5);
        assert!(
            r.entries.iter().all(|e| e.starts_with("reach/")),
            "{:?}",
            r.entries
        );
    }

    /// `--anchor-k` is a width, and an anchor is a starting point rather than an answer.
    #[test]
    fn k_bounds_the_entry_set() {
        let r = resolved("outlet works below", &["reach"], 1);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0], "reach/tailwater.yml");
    }

    /// The degraded arm reports why and what to do, from `Retrieval` and not from a second
    /// list here — the whole reason that type moved out of `cmd/serve`.
    #[test]
    fn a_degraded_anchor_says_why_and_what_to_do() {
        let r = resolved("outlet works", &["reach"], 5);
        assert!(r.anchor.degraded);
        assert_eq!(r.anchor.degraded_reason, Some("no_index"));
        assert!(r.anchor.repair.unwrap().contains("index-build"));
    }

    /// A scan costs what a scan costs. The fallback reads every candidate to score it, and
    /// the cost block has to show that or `bench` would read a degraded anchor as a cheap one.
    ///
    /// The query is nonsense syllables on purpose: the shared scorer matches **substrings**,
    /// not words, so an ordinary-looking miss like "nothing matches this at all" scores every
    /// node in this fixture — `at` is inside `tailwater`. That is `retrieve`'s own behaviour
    /// and not a defect here, but it makes a "matches nothing" fixture harder to write than
    /// it looks, and the first draft of this test asserted the opposite of what it ran.
    #[test]
    fn the_fallback_charges_for_every_candidate_it_scored() {
        let r = resolved("zzzz qqqq", &["reach"], 5);
        assert!(r.entries.is_empty());
        assert_eq!(r.read.len(), 2, "both reaches were read to reject them");
    }

    /// An anchor that resolves to nothing is not an error and not a rejection — it is an
    /// empty answer that still says which path produced it.
    #[test]
    fn an_anchor_that_matches_nothing_still_reports_its_path() {
        let r = resolved("zzzz qqqq", &["reach"], 5);
        assert!(r.anchor.entries.is_empty());
        assert_eq!(r.anchor.degraded_reason, Some("no_index"));
    }
}
