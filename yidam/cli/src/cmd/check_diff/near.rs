//! The declared name an unmodelled type nearly has.
//!
//! Phase A reports `sponsorship` and stops, leaving the reader to find `sponsored-by`
//! themselves against a vocabulary of 150–189 names. This is the clause that hands it to
//! them.
//!
//! # A string rule, and RFC-0022 is why
//!
//! The obvious implementation is an embedding, and it was measured before it was rejected.
//! Across the 516 unmatched type names in the three instrumented repositories, cosine
//! similarity over `AllMiniLML6V2Q` offers 55 candidates at ≥ 0.70 — and the rule below
//! finds 50 of them, because the near-misses are *morphological* rather than semantic:
//! plurals, tenses, and compounds sharing a root. The five the model adds are `held→holds`,
//! `placement→position`, `edition→version`, `totals→amount`, and `answer→question`, which is
//! wrong.
//!
//! Three genuine hits and one false positive is not what `--features index` costs. So this
//! runs in the light build, with no model, no feature gate and no network — which is the
//! finding, not an accommodation.
//!
//! # It reports a root, never a score
//!
//! A prefix rule has no confidence to report. An overlap fraction dressed as one would be
//! false precision — a number invented by a string comparison — so [`Nearest::shared`]
//! carries the root the two names have in common, which is what a reader would check anyway
//! and cannot be read as a measurement.
//!
//! # A wrong candidate costs a lead, and never a finding
//!
//! The threshold calibration #23 left open is not a question here, because **the candidate
//! annotates a finding that exists either way**. Phase A has already decided to report the
//! type; a near-miss adds a clause to a row rather than creating one. Measured, this offers
//! a candidate for 56 of A's 273 unmatched types, 11 of B's 46 and 79 of C's 203, and adds
//! no row to any report.

use std::collections::BTreeSet;

/// A declared name that shares a root with a type the ontology does not name.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Nearest {
    /// The declared name, kebab-cased as the vocabulary holds it: `sponsored-by`.
    pub name: String,
    /// The root the two names have in common: `sponsor`. **Evidence, not a score.**
    pub shared: String,
}

/// How much of a word two names must agree on before they are the same word.
///
/// Four characters and three trailing is what fits A, B and C. It is a string rule with two
/// magic numbers and no corpus has argued with it yet; RFC-0022 records that as open.
const PREFIX: usize = 4;
const TRAILING: usize = 3;

/// The characters two words share from the start — `sponsorship` and `sponsored` give
/// `sponsor`.
fn shared_root(a: &str, b: &str) -> String {
    a.chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .map(|(x, _)| x)
        .collect()
}

/// The root two words share, when they are near enough to be the same word.
///
/// Near enough is a [`PREFIX`]-character prefix and a length within [`TRAILING`] — the
/// reading that admits `sponsorship`/`sponsored`, whose tails (`ship`, `ed`) are longer than
/// the tolerance even though the words are one root. Measuring the tails instead loses a
/// fifth of the candidates and the flagship case with them.
///
/// The prefix floor is also what keeps short words out. `by`, `to`, `on` and `id` appear in
/// declared names constantly, and a rule that let them match would offer `sponsored-by` for
/// every type whose name ends in `-by`.
fn same_word(a: &str, b: &str) -> Option<String> {
    let (la, lb) = (a.chars().count(), b.chars().count());
    if la < PREFIX || lb < PREFIX || la.abs_diff(lb) > TRAILING {
        return None;
    }
    let root = shared_root(a, b);
    (root.chars().count() >= PREFIX).then_some(root)
}

/// The declared name nearest `name`, if any word of it is a word of one.
///
/// Split on `-` and nothing else, which is the rule RFC-0022 calibrated. A handful of
/// declared relationships are written with spaces — `shared a split with` — and splitting
/// on those too finds two more candidates in one repository and none in the other two, at
/// the cost of no longer being the rule that was measured.
///
/// **Ranking is by agreement, then by root length, then by name.** More words in common
/// beats a longer root, and the vocabulary is a `BTreeSet`, so a tie is broken
/// lexicographically and the same corpus always reports the same candidate.
pub fn nearest(name: &str, vocabulary: &BTreeSet<String>) -> Option<Nearest> {
    let mine: Vec<&str> = name.split('-').filter(|w| !w.is_empty()).collect();
    let mut best: Option<(usize, Nearest)> = None;

    for cand in vocabulary {
        let theirs: Vec<&str> = cand.split('-').filter(|w| !w.is_empty()).collect();
        let mut agreed = 0usize;
        let mut longest = String::new();
        for w in &mine {
            let root = theirs
                .iter()
                .filter_map(|t| same_word(w, t))
                .max_by_key(|r| r.chars().count());
            if let Some(root) = root {
                agreed += 1;
                if root.chars().count() > longest.chars().count() {
                    longest = root;
                }
            }
        }
        if agreed == 0 {
            continue;
        }
        let better = best.as_ref().is_none_or(|(a, n)| {
            (agreed, longest.chars().count()) > (*a, n.shared.chars().count())
        });
        if better {
            best = Some((
                agreed,
                Nearest {
                    name: cand.clone(),
                    shared: longest,
                },
            ));
        }
    }

    best.map(|(_, n)| n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vocabulary(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| n.to_string()).collect()
    }

    fn near(name: &str, names: &[&str]) -> Option<(String, String)> {
        nearest(name, &vocabulary(names)).map(|n| (n.name, n.shared))
    }

    /// RFC-0022's worked example, and the one the issue is written about.
    #[test]
    fn a_type_the_ontology_models_under_another_word_form_is_found() {
        assert_eq!(
            near("sponsorship", &["chamber", "sponsored-by"]),
            Some(("sponsored-by".into(), "sponsor".into()))
        );
    }

    /// The character of the whole population: plurals, tenses and spellings, which is why
    /// the embedding RFC-0022 measured bought so little over a string rule.
    #[test]
    fn plurals_and_tenses_are_the_same_word() {
        assert_eq!(
            near("tenures", &["tenure"]),
            Some(("tenure".into(), "tenure".into()))
        );
        assert_eq!(
            near("precinct", &["precincts"]),
            Some(("precincts".into(), "precinct".into()))
        );
        assert_eq!(
            near("release", &["released"]),
            Some(("released".into(), "release".into()))
        );
    }

    /// One word of a compound is enough, from either side.
    #[test]
    fn a_compound_matches_on_the_word_it_shares() {
        assert_eq!(
            near("district-enrollment", &["enrollment"]),
            Some(("enrollment".into(), "enrollment".into()))
        );
        assert_eq!(
            near("computed", &["computed-for"]),
            Some(("computed-for".into(), "computed".into()))
        );
    }

    /// The tolerance is a length difference and not a tail length, which is the reading that
    /// admits `sponsorship`. Four characters apart is where it stops.
    #[test]
    fn a_word_four_characters_longer_is_a_different_word() {
        assert_eq!(near("statement", &["state"]), None);
        assert_eq!(
            near("statements", &["statement"]),
            Some(("statement".into(), "statement".into()))
        );
    }

    /// Three characters of agreement is not a root. `held`/`holds` is one of the five the
    /// embedding found and this cannot, and RFC-0022 records the trade.
    #[test]
    fn three_shared_characters_are_not_enough() {
        assert_eq!(near("held", &["holds"]), None);
        assert_eq!(near("answer", &["question"]), None);
        assert_eq!(near("edition", &["version"]), None);
    }

    /// `by`, `to` and `on` are everywhere in a vocabulary, and a rule that let them match
    /// would offer a candidate for every name that ends in one.
    #[test]
    fn a_word_shorter_than_the_prefix_never_matches() {
        assert_eq!(near("amended-by", &["sponsored-by"]), None);
        assert_eq!(near("by", &["by"]), None);
    }

    /// Most unmatched types have no near-miss at all, which is the property that keeps this
    /// from annotating every row.
    #[test]
    fn a_name_sharing_no_root_gets_no_candidate() {
        assert_eq!(
            near("agenda-item", &["bill", "chamber", "sponsored-by"]),
            None
        );
    }

    /// Agreement first: two words in common beats one word with a longer root.
    #[test]
    fn more_words_in_common_beats_a_longer_root() {
        assert_eq!(
            near(
                "district-enrollment",
                &["district-enrollments", "enrollment-projection"]
            ),
            Some(("district-enrollments".into(), "enrollment".into()))
        );
    }

    /// Then root length, because a longer shared root is stronger evidence than a shorter
    /// one — `adopti` over `plan`.
    #[test]
    fn a_longer_shared_root_wins_among_single_word_matches() {
        assert_eq!(
            near("plan-adoption", &["adopting-vote", "districting-plan"]),
            Some(("adopting-vote".into(), "adopti".into()))
        );
    }

    /// And then the name, so a corpus reports the same candidate on every run.
    #[test]
    fn an_otherwise_equal_tie_is_broken_lexicographically() {
        assert_eq!(
            near("sponsor", &["sponsors", "sponsored"]),
            Some(("sponsored".into(), "sponsor".into()))
        );
    }
}
