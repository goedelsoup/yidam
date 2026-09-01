//! How much of the suite actually ran.
//!
//! A suite skips in two ways here, and they are visible in different places. Counting only
//! the half that is easy to reach is how "1,363 tests passed" comes to mean nothing.
//!
//! **`#[ignore]`** — the three S3 tests. nextest does not run them and does not write them to
//! JUnit at all: the document for that binary is `tests="0"` with no children. They are read
//! from `nextest list --message-format json`, where every test carries an `ignored` flag.
//!
//! **A runtime gate** — four sites that check an environment variable or probe for a tool,
//! print a reason, and return. nextest sees these as passes, and they are passes: the test
//! process ran and did not fail. What it did not do is assert anything. They announce
//! themselves with [`MARKER`], and the census reads it out of the captured output that
//! `store-success-output` puts in the JUnit.
//!
//! The second kind is the one worth the machinery. An `#[ignore]` is declared and greppable;
//! a runtime skip is a branch, and a suite that silently stops running is indistinguishable
//! from one that passes.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::Path;

use crate::junit::Run;

/// What a test prints to announce that it did not run.
///
/// Re-exported from the crate root, where [`crate::skipped`] writes it. The census is only as
/// good as its coverage, and a second spelling of the marker is a skip nothing counts — so the
/// writer and this reader are one definition rather than two that agree by transcription.
pub use crate::MARKER;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skip {
    pub test: String,
    pub reason: String,
    pub kind: Kind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Declared `#[ignore]`. Never started.
    Ignored,
    /// Started, announced a reason, and asserted nothing.
    Gated,
}

impl Kind {
    pub fn label(self) -> &'static str {
        match self {
            Kind::Ignored => "ignored",
            Kind::Gated => "gated",
        }
    }
}

/// Runtime skips, read out of the output nextest captured.
///
/// Both streams are searched: the marker is written to stdout by the helper, and reading
/// stderr too costs nothing and means a test that writes its own marker to `eprintln!` is
/// counted rather than missed.
pub fn gated(run: &Run) -> Vec<Skip> {
    let mut out = Vec::new();
    for case in &run.cases {
        for stream in [&case.stdout, &case.stderr] {
            for line in stream.lines() {
                if let Some(reason) = line.trim().strip_prefix(MARKER) {
                    out.push(Skip {
                        test: case.id(),
                        reason: reason.trim().to_string(),
                        kind: Kind::Gated,
                    });
                }
            }
        }
    }
    out
}

// ── nextest list --message-format json ───────────────────────────────────────
//
// Only the two fields the census needs are named. Serde ignores the rest, which is what
// keeps this from breaking on a nextest release that adds one.

#[derive(Deserialize)]
struct Listing {
    #[serde(rename = "rust-suites")]
    rust_suites: BTreeMap<String, Suite>,
}

#[derive(Deserialize)]
struct Suite {
    testcases: BTreeMap<String, TestCase>,
}

#[derive(Deserialize)]
struct TestCase {
    #[serde(default)]
    ignored: bool,
}

/// `#[ignore]`d tests, read from the listing because JUnit does not carry them.
pub fn ignored(json: &str) -> Result<Vec<Skip>> {
    let listing: Listing =
        serde_json::from_str(json).context("parsing `nextest list --message-format json`")?;
    let mut out = Vec::new();
    for (suite, cases) in &listing.rust_suites {
        for (name, case) in &cases.testcases {
            if case.ignored {
                out.push(Skip {
                    test: format!("{suite} {name}"),
                    // The `#[ignore = "..."]` reason is not in the listing. Saying so beats
                    // an empty cell that reads as "no reason given".
                    reason: "#[ignore] — see the test's own note".to_string(),
                    kind: Kind::Ignored,
                });
            }
        }
    }
    out.sort_by(|a, b| a.test.cmp(&b.test));
    Ok(out)
}

pub fn ignored_from_file(path: &Path) -> Result<Vec<Skip>> {
    let json = std::fs::read_to_string(path)
        .with_context(|| format!("reading the nextest listing at {}", path.display()))?;
    ignored(&json)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::junit;

    #[test]
    fn a_runtime_skip_is_counted_with_its_reason() {
        let xml = format!(
            r#"<testsuites><testsuite name="s">
                 <testcase name="signs" classname="yidam::query_history" time="0.1">
                   <system-out>running 1 test
{MARKER} this git cannot sign with SSH
test signs ... ok</system-out>
                 </testcase>
               </testsuite></testsuites>"#
        );
        let skips = gated(&junit::parse(&xml).unwrap());
        assert_eq!(skips.len(), 1);
        assert_eq!(skips[0].reason, "this git cannot sign with SSH");
        assert_eq!(skips[0].kind, Kind::Gated);
    }

    /// A passing test is not a skip. The census must not turn every green case into one by
    /// matching too loosely — the word "skip" appears in plenty of test output.
    #[test]
    fn a_test_that_merely_says_the_word_skip_is_not_counted() {
        let xml = r#"<testsuites><testsuite name="s">
             <testcase name="t" classname="c" time="0.1">
               <system-out>skipping nothing, and the word skipped appears here too</system-out>
             </testcase>
           </testsuite></testsuites>"#;
        assert_eq!(gated(&junit::parse(xml).unwrap()).len(), 0);
    }

    /// Verbatim from `cargo nextest list --message-format json --test vault_s3`, which is
    /// where the three `#[ignore]`d S3 tests are visible and JUnit is silent.
    #[test]
    fn ignored_tests_come_from_the_listing_because_junit_omits_them() {
        let json = r#"{
          "test-count": 3,
          "rust-suites": {
            "yidam::vault_s3": {
              "binary-id": "yidam::vault_s3",
              "testcases": {
                "an_artifact_round_trips_through_a_real_s3_server":
                  {"kind":"test","ignored":true,"filter-match":{"status":"mismatch","reason":"ignored"}},
                "a_bad_secret_is_reported_as_the_server_reported_it":
                  {"kind":"test","ignored":true,"filter-match":{"status":"mismatch","reason":"ignored"}}
              }
            },
            "yidam::formal_specs": {
              "binary-id": "yidam::formal_specs",
              "testcases": {
                "the_lean_toolchain_is_pinned": {"kind":"test","ignored":false,"filter-match":{"status":"matches"}}
              }
            }
          }
        }"#;
        let skips = ignored(json).expect("the committed nextest listing should parse");
        assert_eq!(skips.len(), 2);
        assert!(skips.iter().all(|s| s.kind == Kind::Ignored));
        assert!(skips[0].test.starts_with("yidam::vault_s3 "));
    }

    /// The regression this whole file is built around: the JUnit for a fully-ignored binary
    /// is well formed, carries no cases, and would report a clean bill of health.
    #[test]
    fn junit_alone_reports_no_skips_for_a_suite_that_ran_nothing() {
        let xml = r#"<testsuites name="nextest-run" tests="0" skipped="0" failures="0" errors="0"></testsuites>"#;
        let run = junit::parse(xml).unwrap();
        assert_eq!(gated(&run).len(), 0, "there is nothing in JUnit to find");
        assert_eq!(run.cases.len(), 0);
    }
}
