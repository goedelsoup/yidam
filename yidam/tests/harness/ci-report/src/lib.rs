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
//!
//! [`summary`] renders what a person reads on a red check; [`quality`] renders the same run
//! as `quality-report.json`, which is what #467's pages read. Both are built from one parse
//! of one document, so the page and the job summary cannot come to disagree about a number.
//!
//! [`series`] is the sequence of those reports — one record per push to main, on an orphan
//! branch, because every run before #468 computed its numbers and threw them away.

pub mod census;
pub mod coverage;
pub mod junit;
pub mod quality;
pub mod series;
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

/// [`skipped`], for a precondition CI is expected to provide: a skip on a workstation, and a
/// failure under CI (#1621).
///
/// A plain [`skipped`] is right where CI legitimately lacks the thing too — a MinIO, a deep
/// clone. Where the CI job does provide it, a skip there means the job changed under the test,
/// and the census counting one more gated line is not something anybody reads on a green
/// check. So under CI this panics, after writing the same marker, so the reason is in the
/// captured output either way.
///
/// "Under CI" is the `CI` variable GitHub Actions sets to `true`; empty, `false` and `0` read
/// as unset, so a local `CI=0` stays a workstation.
pub fn skipped_unless_ci(reason: &str) {
    skip_or_fail(reason, std::env::var("CI").ok().as_deref());
}

fn skip_or_fail(reason: &str, ci: Option<&str>) {
    skipped(reason);
    if under_ci(ci) {
        panic!(
            "{reason} — and CI is set, where this precondition is expected to hold, so its \
             absence fails the test rather than skipping it"
        );
    }
}

fn under_ci(value: Option<&str>) -> bool {
    value.is_some_and(|v| !matches!(v.trim(), "" | "false" | "0"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_skip_outside_ci_returns_and_under_ci_fails() {
        for unset in [None, Some(""), Some("false"), Some("0")] {
            super::skip_or_fail("no tool here", unset);
        }
        for set in [Some("true"), Some("1")] {
            let failed = std::panic::catch_unwind(|| super::skip_or_fail("no tool here", set));
            let message = failed.expect_err("CI is set and the skip returned");
            let message = message
                .downcast_ref::<String>()
                .expect("the panic carries the reason");
            assert!(message.starts_with("no tool here"), "{message}");
        }
    }

    #[test]
    fn the_marker_is_not_a_word_that_turns_up_in_ordinary_output() {
        // If this ever becomes a plain English word, the census starts counting passing tests
        // that merely mention it — the failure it exists to avoid, inverted.
        assert!(super::MARKER.contains('-') && super::MARKER.ends_with(':'));
        assert!(!super::MARKER.to_lowercase().contains("skipping"));
    }
}
