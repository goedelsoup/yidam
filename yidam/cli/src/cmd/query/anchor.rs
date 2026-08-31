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
    let reason = retrieval.degraded_reason();
    let (entries, read) = match retrieval {
        #[cfg(feature = "vector-read")]
        Retrieval::Vector(index) => vector_entries(index, text, classes, k, nodes, corpus_dir)?,
        _ => keyword_entries(text, classes, k, nodes, corpus_dir),
    };
    Ok(Resolved {
        anchor: Anchor {
            step,
            text: text.to_string(),
            k,
            degraded: reason.is_some(),
            degraded_reason: reason,
            repair: retrieval.repair(),
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
#[cfg(feature = "vector-read")]
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

#[cfg(feature = "vector-read")]
fn vector_entries(
    index: &crate::retrieval::vector::IndexState,
    text: &str,
    classes: &[String],
    k: usize,
    nodes: &[Node],
    corpus_dir: &str,
) -> Result<(Vec<Entry>, Vec<String>), String> {
    let candidates = candidates(nodes, classes);
    // The class test is applied here rather than after truncation: `k` must count nodes the
    // step could actually match, or a `k` of 1 against a corpus whose nearest row is of
    // another class resolves to nothing and looks like a miss.
    let hits = crate::retrieval::vector::search(index, text, k, |row| {
        candidates.contains_key(row.path.as_str())
    })?;
    let entries: Vec<Entry> = hits
        .iter()
        .filter_map(|(row, score)| {
            candidates.get(row.path.as_str()).map(|node| Entry {
                node: id_of(node, corpus_dir),
                score: *score,
            })
        })
        .collect();
    let read = entries.iter().map(|e| e.node.clone()).collect();
    Ok((entries, read))
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
