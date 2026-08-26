//! Why `retrieve` found nothing, derived rather than guessed (#334, E6).
//!
//! # The tool an agent reaches for first is the one that could not say
//!
//! [`super::super::query::absence`] answered this for `query`, `pack` and `estimate` — the
//! surfaces where the ontology derives the reason. It did not touch `retrieve`, which is the
//! tool an agent reaches for before it knows enough to write a query. There, `results: []`
//! still meant all of: the corpus has nothing on this subject, the query's words are not the
//! corpus's words, the class filter names a class holding no instances, and the class filter
//! names a class that does not exist.
//!
//! The last was the sharpest, because `retrieve` took a `class` argument and never checked
//! it. `retrieve("hydropeaking", class: "gauge")` against a corpus declaring `gage` returned
//! zero results and reported nothing wrong — the exact failure `query`'s `unknown-class`
//! rejection exists to prevent, one tool over, on the more-used tool.
//!
//! # A rejection is not an absence, here either
//!
//! An unknown class is the *caller* being wrong, and it is reported as [`Rejection`] on a new
//! `rejected` key rather than folded into the diagnosis. The contract's rule is not softened
//! because this tool is used more: a server answering a typo with `class-unpopulated` tells a
//! caller its mistake was a true negative.
//!
//! # The semantic path can say more than the issue supposed
//!
//! #334 scoped the vector arm out — *there is no threshold, and inventing one would be a
//! claim about a model*. That is right about thresholds and wrong about what follows from
//! their absence. [`crate::retrieval::vector::search`] scores every row the filter admits and
//! truncates to `k`; it never drops a row for scoring badly. So an empty vector answer is not
//! a weak answer — it is proof the filter admitted **no rows at all**, which is derivable
//! without asserting anything about similarity.
//!
//! That splits a case the keyword path cannot see: a class the corpus populates and the index
//! does not hold is an index built before those nodes were written. `class-unindexed` says so
//! and names the repair. A weak-but-non-empty answer remains undiagnosable, and is left
//! undiagnosed rather than thresholded.
//!
//! # `retrieve` is `core`, so the ontology may be absent
//!
//! Unlike `query`'s family this cannot hide behind the `ontology` tier. A server with no
//! `.ont.yml` can derive neither of the two class rows: it cannot reject an unknown class,
//! because no class is declared, and it cannot call one unpopulated, because nothing declares
//! it to be a class at all. That state is reported as its own code — `class-undeclared` —
//! which says plainly that this tool cannot tell a misspelling from an empty class here, and
//! why. Silence would leave the caller reading a typo as coverage, which is the whole point.

use serde_json::{json, Value};

use super::ServerState;

/// The empty answer, explained. Serialised onto `retrieve`'s `absence` key.
///
/// Deliberately a smaller shape than `query`'s. There is no `step`, because `retrieve` has no
/// steps. There is no `elsewhere`, because `retrieve` already searches every installed
/// dependency — the field exists on `query` to point at corpora a local walk did not read,
/// and here it would be empty on every response by construction, which is a field that
/// teaches a client nothing and costs it a branch.
pub(crate) struct Absence {
    /// From a closed set, so a client branches without matching prose.
    pub code: &'static str,
    pub message: String,
    /// The denominator the message is about: how many nodes the class filter admitted to the
    /// search. It is what makes `no-term-match` a statement rather than a shrug — *none of
    /// four* and *none of nine hundred* are different facts about a corpus.
    pub instances: usize,
}

impl Absence {
    pub(crate) fn to_json(&self) -> Value {
        json!({
            "code": self.code,
            "message": self.message,
            "instances": self.instances,
        })
    }
}

/// The caller's class filter is wrong. Serialised onto `retrieve`'s `rejected` key.
pub(crate) struct Rejection {
    pub code: &'static str,
    pub message: String,
}

impl Rejection {
    pub(crate) fn to_json(&self) -> Value {
        json!({ "code": self.code, "message": self.message })
    }
}

/// Does the corpus declare any classes at all?
///
/// The carve-out `query::check` already makes: a corpus with no `.ont.yml` has no schema
/// layer, which is a different problem from a misspelling and must not be reported as one.
fn unschematised(state: &ServerState) -> bool {
    state.classes.is_empty()
}

/// Validate the `class` filter before searching.
///
/// Returns `None` on an unschematised corpus — not because the name is right, but because
/// nothing here can tell. That case reappears as `class-undeclared` if the search comes back
/// empty, so it is reported rather than lost.
pub(crate) fn reject_unknown_class(state: &ServerState, class: Option<&str>) -> Option<Rejection> {
    let class = class?;
    if unschematised(state) {
        return None;
    }
    if state.classes.iter().any(|(stem, _)| stem == class) {
        return None;
    }
    let declared: Vec<&str> = state.classes.iter().map(|(s, _)| s.as_str()).collect();
    Some(Rejection {
        code: "unknown-class",
        message: format!(
            "`{class}` is not a class this corpus declares{}",
            crate::cmd::query::check::nearest(class, &declared)
        ),
    })
}

/// How many nodes the class filter admitted — local and dependency alike, which is the set
/// `retrieve` actually searches.
fn candidates(state: &ServerState, class: Option<&str>) -> usize {
    state
        .nodes
        .iter()
        .chain(state.dep_nodes.iter())
        .filter(|n| class.is_none_or(|c| n.class == c))
        .count()
}

/// Diagnose an empty `retrieve`.
///
/// Called only when `results` is empty and the call was not rejected. Every branch reads off
/// something the corpus or the index states; none infers from the query's shape.
pub(crate) fn diagnose(
    state: &ServerState,
    query: &str,
    class: Option<&str>,
    vector: bool,
) -> Absence {
    let instances = candidates(state, class);

    // Nothing to search. Which of the two class answers is honest depends on whether anything
    // declares classes at all, and saying the wrong one is worse than saying neither.
    if let Some(class) = class {
        if instances == 0 {
            return match unschematised(state) {
                false => Absence {
                    code: "class-unpopulated",
                    message: format!(
                        "the ontology declares `{class}` and no node in this corpus or its \
                         installed dependencies belongs to it. The class exists; nothing has \
                         been written into it yet."
                    ),
                    instances,
                },
                true => Absence {
                    code: "class-undeclared",
                    message: format!(
                        "no node carries `class: {class}`, and this corpus declares no \
                         ontology — so this tool cannot tell a class nobody has written into \
                         from a name that was misspelled. Add `.yidam/corpus/{class}.ont.yml` \
                         to make that distinction answerable."
                    ),
                    instances,
                },
            };
        }
    }

    // ── the semantic path ────────────────────────────────────────────────────
    //
    // Reached only when the search admitted no rows, since nothing is dropped for scoring
    // badly. With candidates in the corpus, that is a statement about the index and not about
    // coverage — and it is the one diagnosis the keyword path is blind to.
    if vector {
        if instances > 0 {
            return Absence {
                code: "class-unindexed",
                message: match class {
                    Some(class) => format!(
                        "the vector index holds no rows for `{class}`, though this corpus \
                         holds {instances}. The index was built before those nodes were \
                         written: run `yidam embed && yidam index-build`."
                    ),
                    None => format!(
                        "the vector index holds no rows, though this corpus holds \
                         {instances} node(s). Run `yidam embed && yidam index-build`."
                    ),
                },
                instances,
            };
        }
        return Absence {
            code: "index-empty",
            message: "the vector index holds no rows and there is nothing to search.".to_string(),
            instances,
        };
    }

    // ── the keyword path ─────────────────────────────────────────────────────
    if crate::retrieval::terms(query).is_empty() {
        return Absence {
            code: "query-no-terms",
            message: "the query contains no searchable terms, so nothing was compared against \
                      the corpus."
                .to_string(),
            instances,
        };
    }

    Absence {
        code: "no-term-match",
        message: format!(
            "none of the {instances} node(s) searched contains any word of this query. This \
             is a statement about the words, not about the corpus's coverage — keyword search \
             matches terms rather than meanings, so a corpus that discusses the subject in \
             other words answers exactly this way."
        ),
        instances,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::serve::tests::test_state;

    /// The vector arm, exercised from the light build.
    ///
    /// [`diagnose`] takes `vector` as an argument rather than reading `state.retrieval`, and
    /// that is what makes these two codes testable at all here: the arm that produces them is
    /// behind `--features index`, which no pull request compiles. A diagnosis reachable only
    /// in a build CI does not run is a diagnosis nobody has ever seen.
    #[test]
    fn a_populated_class_the_index_does_not_hold_is_an_index_problem() {
        let state = test_state();
        let a = diagnose(&state, "graph", Some("concept"), true);

        assert_eq!(a.code, "class-unindexed");
        assert_eq!(a.instances, 4);
        assert!(
            a.message.contains("index-build"),
            "the repair is the point of the code: {}",
            a.message
        );
    }

    /// An index over a corpus with nothing in it is not a statement about coverage.
    #[test]
    fn an_index_with_no_rows_over_an_empty_class_is_its_own_answer() {
        let mut state = test_state();
        state.nodes.clear();
        state.dep_nodes.clear();
        let a = diagnose(&state, "graph", None, true);

        assert_eq!(a.code, "index-empty");
        assert_eq!(a.instances, 0);
    }

    /// The two class codes are chosen by evidence, not by convenience.
    ///
    /// The same call, against the same corpus, differing only in whether anything declares
    /// classes. Reporting `class-unpopulated` in the second case would assert an ontology this
    /// corpus has not written.
    #[test]
    fn the_class_answer_depends_on_whether_an_ontology_exists() {
        let mut state = test_state();
        assert_eq!(
            diagnose(&state, "graph", Some("silent"), false).code,
            "class-unpopulated"
        );

        state.classes.clear();
        assert_eq!(
            diagnose(&state, "graph", Some("silent"), false).code,
            "class-undeclared"
        );
    }

    /// An unknown name is only knowable as unknown where something declares the known ones.
    #[test]
    fn nothing_is_rejected_against_a_corpus_that_declares_no_classes() {
        let mut state = test_state();
        assert!(reject_unknown_class(&state, Some("concpt")).is_some());
        assert!(reject_unknown_class(&state, Some("concept")).is_none());
        assert!(reject_unknown_class(&state, None).is_none());

        state.classes.clear();
        assert!(
            reject_unknown_class(&state, Some("concpt")).is_none(),
            "with nothing declared there is no name to be near, and no ground to refuse on"
        );
    }
}
