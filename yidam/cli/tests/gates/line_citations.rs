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
//! asserted empty, and the fifth is counted:
//!
//! - **dead**: every cited range exists and holds text;
//! - **slid**: every citation written in the quoting house style still quotes what the
//!   cited lines say;
//! - **label not cited**: where no quote holds a citation but its label names a symbol,
//!   the cited lines say that symbol (#632);
//! - **stated twice**: where the label names the range as well as the fragment, the two
//!   copies agree.
//!
//! The remaining one — a citation with neither anchor — is Info by design and is still not
//! asserted empty, because the remedy is a judgement about the document. Its **members** are
//! gated instead, listed in `tests/goldens/unanchored-line-citations.txt` (#899, #932). It
//! is by far the largest group: 109 of this repository's 229 line citations, checked for
//! existence and nothing else. 176 carry no quote and 67 of those
//! are held by their label instead, which is the whole of what the documents themselves
//! make decidable; 105 of the remaining 109 label a line number, and a line number that
//! agrees with itself anchors nothing.
//!
//! Six of the 53 that do carry a quote set it off under the citation instead of writing it
//! in quotation marks — three in a fenced block (#758), three in a blockquote (#899) — and
//! every one of them was counted in the paragraph above until somebody measured the
//! population. A blank line ends the paragraph a quote is looked for in, so for as long as
//! the extractor read only inline quotes, the two most explicit ways of writing a quote
//! down were the two forms nothing checked. Both were found the same way and both were
//! found holding rot: three of the four fenced ones had slid (#723, #758, #760), and two of
//! the three blockquoted ones had, one of them by 255 lines onto unrelated code.
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

/// Where the citations nothing can check are listed, one per line — the only file this suite
/// lets a person edit to make it pass (#899, #932).
fn unanchored_baseline_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/unanchored-line-citations.txt")
}

/// A citation's identity in the baseline: the citing file and the anchor as a reader writes
/// it. Not the citing *line* — an edit above a citation would move every key under it and
/// the list would churn on changes that touched no citation. Two identical citations in one
/// file are one key twice, which is why the comparison is of multisets.
fn unanchored_key(c: &yidam::LineCitation) -> String {
    format!("{} → {}", c.file, c.anchor())
}

/// Every non-comment, non-blank line of the baseline, in file order.
fn read_unanchored_baseline() -> Vec<String> {
    let path = unanchored_baseline_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", path.display()))
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// `a` minus `b`, counting repeats.
fn multiset_minus(a: &[String], b: &[String]) -> Vec<String> {
    let mut left: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for k in b {
        *left.entry(k).or_default() += 1;
    }
    a.iter()
        .filter(|k| match left.get_mut(k.as_str()) {
            Some(n) if *n > 0 => {
                *n -= 1;
                false
            }
            _ => true,
        })
        .cloned()
        .collect()
}

/// The residue, listed — and the list held to a file somebody has to edit.
///
/// `unverified-line-citation` is Info and gates on nothing, for a reason its own rationale
/// makes and this test does not disturb: the remedy is a judgement about the document —
/// quote the passage, label the symbol, widen to a stable range, drop the fragment — and a
/// gate cannot make that judgement. What a gate *can* do is refuse to let the population
/// grow while nobody is looking, which is the whole of the finding in #899: six citations
/// of `GRAPH.md` were stale on `main`, one of them by 27 lines for months, and every one of
/// them was in this group and therefore silent from the day it was written.
///
/// **A list, not a count (#932).** This used to be `assert_eq!` against a constant, and the
/// constant had two defects a list does not:
///
/// - *Two green branches merged red.* Two PRs that each retired one citation both passed at
///   `UNANCHORED - 1`, and the merged tree sat at `UNANCHORED - 2`. Each now deletes its own
///   line, and git merges two deletions — or refuses them as a conflict on the PR, before
///   the merge rather than after it.
/// - *A swap passed.* A PR that anchored one citation and added another left the count
///   where it was. Now the addition is a key the file does not hold.
///
/// Loosening the constant to a ceiling would have fixed the first and made the second
/// worse: a ceiling above the truth is room for exactly that many silent additions, which
/// is the drift `.yidam/lint-baseline.yml` refuses for a derived corpus on the same
/// argument. The list stays exact in both directions.
///
/// **Re-pointing is an addition.** A citation whose fragment moves has a new key, so a
/// citation repaired by re-pointing alone fails here until its line is edited too. That is
/// deliberate: a re-pointed, unquoted citation slides again on the next edit to its target,
/// and the count could not see it happen (#954, #1136 — both repaired by quoting instead).
#[test]
fn the_unanchored_population_does_not_grow() {
    let cites = yidam::collect_line_citations(&repo_root());
    let check = yidam::unverified_line_citation(&cites);
    let mut found: Vec<String> = cites
        .iter()
        .filter(|c| c.is_unanchored())
        .map(unanchored_key)
        .collect();
    found.sort();
    assert_eq!(
        found.len(),
        check.violations.len(),
        "the keys listed here and the violations the check reports are two readings of one \
         population and disagree — `is_unanchored` and `unverified_line_citation` have come \
         apart"
    );

    let listed = read_unanchored_baseline();
    let added = multiset_minus(&found, &listed);
    let retired = multiset_minus(&listed, &found);

    if std::env::var("UPDATE_GOLDENS").is_ok() && added.is_empty() && !retired.is_empty() {
        let path = unanchored_baseline_path();
        let text = std::fs::read_to_string(&path).expect("read baseline");
        let mut drop = retired.clone();
        let kept: Vec<&str> = text
            .lines()
            .filter(|l| match drop.iter().position(|r| r == l) {
                Some(i) => {
                    drop.swap_remove(i);
                    false
                }
                None => true,
            })
            .collect();
        std::fs::write(&path, kept.join("\n") + "\n").expect("write baseline");
        return;
    }

    let path = unanchored_baseline_path();
    assert!(
        added.is_empty(),
        "{} citation(s) carry neither a quote nor a symbol label and are not listed in {}:\n\
         {}\n\
         Anchor each — quote the passage beside the link, or label the symbol the lines \
         declare — rather than listing it. A citation you only re-pointed counts: quote it \
         while you are there. Listing one is a hand edit, and the diff is the review.",
        added.len(),
        path.display(),
        added
            .iter()
            .map(|k| format!("  {k}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
    assert!(
        retired.is_empty(),
        "{} listed citation(s) are anchored or gone — a repair landed. Delete their lines \
         from {}, or re-run with UPDATE_GOLDENS=1, which only ever deletes:\n{}",
        retired.len(),
        path.display(),
        retired
            .iter()
            .map(|k| format!("  {k}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The list is kept in the order `found` is sorted into, so two branches' deletions land at
/// predictable lines and a hand-added entry has one right place to go.
#[test]
fn the_unanchored_baseline_is_sorted() {
    let listed = read_unanchored_baseline();
    let mut sorted = listed.clone();
    sorted.sort();
    assert!(
        listed == sorted,
        "{} is out of order; sort its entries (byte order, as Rust's `sort` does)",
        unanchored_baseline_path().display()
    );
}

#[test]
fn multiset_minus_counts_repeats() {
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    assert_eq!(
        multiset_minus(&s(&["a", "a", "b"]), &s(&["a", "c"])),
        s(&["a", "b"])
    );
    assert!(multiset_minus(&s(&["a"]), &s(&["a", "a"])).is_empty());
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
