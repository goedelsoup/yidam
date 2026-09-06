//! This repository's own line citations, held to the checks built for them (#563).
//!
//! `docs/` cites the repository by line number well over a hundred times, and until this
//! file existed nothing read a single one: `broken-prose-link` resolves the file and
//! drops the fragment, the docs site build checks pages and not anchors, and `yidam lint`
//! walks a corpus — which the template repository is not. Twelve constitution citations
//! rotted at once inside that gap, and one of them ended up citing a blank line.
//!
//! So the gate lives here, in the suite that already holds this repository's prose to its
//! own claims (`walkthrough_transcripts`, `docs_site`). Four of the five checks are
//! asserted empty:
//!
//! - **dead**: every cited range exists and holds text;
//! - **slid**: every citation written in the quoting house style still quotes what the
//!   cited lines say;
//! - **label not cited**: where no quote holds a citation but its label names a symbol,
//!   the cited lines say that symbol (#632);
//! - **stated twice**: where the label names the range as well as the fragment, the two
//!   copies agree.
//!
//! The remaining one — a citation with neither anchor — is Info by design and not gated
//! on. It is still by far the largest group: 104 of this repository's 150 line citations,
//! checked for existence and nothing else. 119 carry no quote and 15 of those are held by
//! their label instead, which is the whole of what the documents themselves make
//! decidable; the rest label a line number, and a line number that agrees with itself
//! anchors nothing.
//!
//! When this goes red after an innocent edit, the edit moved a cited passage: re-point
//! the citation at the passage's new lines. **The finding names them** when the passage
//! is still in the file and in one place, so the repair is a transcription rather than a
//! search (#622). That friction is the feature — it is the moment the twelve rotted
//! through, made visible.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn render(v: &[yidam::LintViolation]) -> String {
    v.iter()
        .map(|v| format!("  {} — {}", v.node, v.detail))
        .collect::<Vec<_>>()
        .join("\n")
}

/// A scan that sees nothing passes every assertion below vacuously. The population is
/// discovered, not listed, so the floor is existence: this repository demonstrably
/// carries line citations, and some of them demonstrably carry quotes. Both counts
/// going to zero means the scan broke, not that the docs went quiet.
#[test]
fn the_scan_sees_a_population() {
    let cites = yidam::collect_line_citations(&repo_root());
    assert!(
        !cites.is_empty(),
        "no line citations found under docs/ — the scan is looking at nothing"
    );
    assert!(
        cites.iter().any(|c| !c.quotes.is_empty()),
        "no citation carries a quote — quote extraction is looking at nothing \
         ({} citations found)",
        cites.len()
    );
}

#[test]
fn no_line_citation_is_dead() {
    let cites = yidam::collect_line_citations(&repo_root());
    let check = yidam::dead_line_citation(&cites);
    assert!(
        check.passed(),
        "line citations naming lines that are not there:\n{}",
        render(&check.violations)
    );
}

#[test]
fn no_quoted_line_citation_has_slid() {
    let cites = yidam::collect_line_citations(&repo_root());
    let check = yidam::slid_line_citation(&cites);
    assert!(
        check.passed(),
        "citations whose quoted passage is no longer in the cited lines — the target \
         moved; re-point the citation:\n{}",
        render(&check.violations)
    );
}

/// The half of the quoteless population the documents themselves can decide. A citation
/// of code labels the symbol it cites, and until #632 nothing read that label as a claim
/// about the target — which is how seven citations slid onto the wrong lines with every
/// gate green (#627).
#[test]
fn no_labelled_symbol_is_missing_from_the_lines_that_cite_it() {
    let cites = yidam::collect_line_citations(&repo_root());
    let check = yidam::citation_label_not_cited(&cites);
    assert!(
        check.passed(),
        "citations whose label names something the cited lines do not say — the target \
         moved; re-point the citation:\n{}",
        render(&check.violations)
    );
}

/// The floor under the check above, and the one that matters most here: it reports only on
/// citations whose label names a symbol, so an extractor that stopped recognising the house
/// form would leave it passing over nothing at all. The population is discovered, and both
/// halves of the partition it draws must be non-empty — a repository with no symbol-labelled
/// citation and one with no line-numbered citation would each look like this test's success.
#[test]
fn the_two_house_label_forms_are_both_still_read() {
    let cites = yidam::collect_line_citations(&repo_root());
    let quoteless: Vec<_> = cites.iter().filter(|c| c.quotes.is_empty()).collect();
    let anchored = quoteless.iter().filter(|c| !c.symbols.is_empty()).count();
    assert!(
        anchored > 0,
        "no quoteless citation carries a symbol label — label extraction is looking at \
         nothing ({} quoteless of {} citations)",
        quoteless.len(),
        cites.len()
    );
    assert!(
        anchored < quoteless.len(),
        "every quoteless citation reads as symbol-labelled — a label that restates the line \
         number is being taken for a claim about the target"
    );
}

/// The label and the fragment are two statements of one range, and a repair that edits
/// only the resolving copy leaves the other lying. Green on the day it landed, and a
/// latch from then on (#622).
#[test]
fn no_citation_states_two_different_ranges() {
    let cites = yidam::collect_line_citations(&repo_root());
    let check = yidam::citation_range_stated_twice(&cites);
    assert!(
        check.passed(),
        "citations whose label and link name different lines — a half-finished repair:\n{}",
        render(&check.violations)
    );
}

/// The floor under the check above. It reports only on citations whose label states a
/// range, so a label parser that quietly stopped recognising the house form would leave
/// it passing over nothing at all.
#[test]
fn the_labels_this_repository_writes_are_still_read_as_ranges() {
    let cites = yidam::collect_line_citations(&repo_root());
    let labelled = cites
        .iter()
        .filter(|c| yidam::label_range(&c.label).is_some())
        .count();
    assert!(
        labelled * 2 > cites.len(),
        "only {labelled} of {} citations have a label naming a range — the house style \
         states it in both places, so the parser is what changed, not the docs",
        cites.len()
    );
}
