//! Reading a test run, and saying what did not run.
//!
//! `$GITHUB_STEP_SUMMARY` appeared zero times in 1,527 lines of workflow (#462). 1,399 tests
//! reported as a colored dot, `cargo test` emitted nothing a consumer could parse, and the
//! four suites that skip on an absent environment printed an honest reason into scrollback
//! nobody opens. This crate is the consumer that was missing.
//!
//! It is both halves of the skip convention: [`skipped`] writes the marker and
//! [`census`] counts it. One crate owning both is what keeps them from being two spellings
//! that agree by luck — the tests depend on it to write, and the binary uses it to read.

pub mod census;
pub mod coverage;
pub mod junit;
pub mod summary;

/// The prefix a skipped test writes, on its own line.
///
/// Chosen not to be a word that appears in ordinary test output: the census matches this
/// prefix and nothing looser, so a test that merely discusses skipping is not counted as
/// having skipped. A line prefix rather than a substring, for the same reason.
pub const MARKER: &str = "YIDAM-SKIP:";

/// Say that this test is not going to exercise its subject, and why.
///
/// A runtime skip is a branch, not a declaration, so a harness records it as a pass — the
/// process ran and did not fail. What it did not do is assert anything, and a suite that
/// quietly stops asserting is indistinguishable from one that passes. This is how it says so
/// where something can count it.
///
/// `reason` completes the sentence "this test did not run because …", and is read by a person
/// looking at a job summary: name the missing thing and how to supply it.
///
/// ```
/// let endpoint = "http://127.0.0.1:9000";
/// ci_report::skipped(&format!("set YIDAM_S3_TEST=1 and run a MinIO on {endpoint}"));
/// ```
///
/// Written to stdout, which nextest captures into the JUnit report under the `ci` profile.
pub fn skipped(reason: &str) {
    println!("{MARKER} {reason}");
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_marker_is_not_a_word_that_turns_up_in_ordinary_output() {
        // If this ever becomes a plain English word, the census starts counting passing tests
        // that merely mention it — the failure it exists to avoid, inverted.
        assert!(super::MARKER.contains('-') && super::MARKER.ends_with(':'));
        assert!(!super::MARKER.to_lowercase().contains("skipping"));
    }
}
