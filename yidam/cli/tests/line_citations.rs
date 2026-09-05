//! This repository's own line citations, held to the checks built for them (#563).
//!
//! `docs/` cites the repository by line number well over a hundred times, and until this
//! file existed nothing read a single one: `broken-prose-link` resolves the file and
//! drops the fragment, the docs site build checks pages and not anchors, and `yidam lint`
//! walks a corpus — which the template repository is not. Twelve constitution citations
//! rotted at once inside that gap, and one of them ended up citing a blank line.
//!
//! So the gate lives here, in the suite that already holds this repository's prose to its
//! own claims (`walkthrough_transcripts`, `docs_site`). Two of the three checks are
//! asserted empty:
//!
//! - **dead**: every cited range exists and holds text;
//! - **slid**: every citation written in the quoting house style still quotes what the
//!   cited lines say;
//! - **stated twice**: where the label names the range as well as the fragment, the two
//!   copies agree.
//!
//! The remaining one — a citation carrying no quote — is Info by design and not gated on;
//! most point at code, where the house style quotes nothing. It is by far the largest
//! group: 119 of 149, checked for existence and nothing else. Closing that gap means
//! writing quotes into the prose, and is #622's out-of-scope half.
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
