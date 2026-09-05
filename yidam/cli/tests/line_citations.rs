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
//!   cited lines say.
//!
//! The third — a citation carrying no quote — is Info by design and not gated on; most
//! point at code, where the house style quotes nothing.
//!
//! When this goes red after an innocent edit, the edit moved a cited passage: re-point
//! the citation at the passage's new lines. That friction is the feature — it is the
//! moment the twelve rotted through, made visible.

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
