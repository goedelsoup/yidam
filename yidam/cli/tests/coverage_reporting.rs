//! A coverage number says which build it was taken under, and says it truthfully.
//!
//! The measurement is honest by construction — a file the build did not compile has no LCOV
//! record, and `ci-report` classifies the absence rather than reading it as zero. What is
//! *not* honest by construction is the **label**. The feature list printed beside the number
//! is a string in a workflow, and a string can be right on the day it is written and wrong
//! six months later, with nothing going red: the reader would then be told the index path was
//! measured when it was never compiled, which is precisely the claim #464 exists to prevent.
//!
//! So the label is checked against `[features] default` in `Cargo.toml`, discovered from the
//! manifest rather than restated here.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Text with `#` comments removed — prose about the code must not answer for the code.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.split_once('#') {
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => before,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `default` feature list from yidam/cli's manifest.
fn default_features() -> BTreeSet<String> {
    let manifest = read("yidam/cli/Cargo.toml");
    let key = "\ndefault = [";
    let start = manifest
        .find(key)
        .expect("yidam/cli/Cargo.toml declares no `default` feature");
    let rest = &manifest[start + key.len()..];
    let body = &rest[..rest.find(']').expect("unterminated default list")];
    body.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The features named beside a coverage number are the features that build was compiled with.
///
/// The light gate measures the default set. If the two drift, the summary says a file was
/// measured that was never compiled — or, just as bad, calls a compiled file unmeasured and
/// hides a real coverage gap behind a feature gate that does not exist.
#[test]
fn the_reported_feature_set_is_the_one_the_gate_builds() {
    let yml = code_only(&read(".github/workflows/ci.yml"));
    let declared = default_features();

    let labels: Vec<BTreeSet<String>> = yml
        .lines()
        .filter_map(|l| l.split("features:").nth(1))
        .filter(|v| v.contains(','))
        .map(|v| {
            v.split('\'')
                .find(|s| s.contains(','))
                .unwrap_or("")
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect()
        })
        .filter(|s: &BTreeSet<String>| !s.is_empty())
        .collect();

    assert!(
        !labels.is_empty(),
        "no coverage `features:` label found in ci.yml. Either the coverage summary stopped \
         naming its build — which is the thing that makes the number readable — or this \
         parser is looking at the wrong shape."
    );

    for label in &labels {
        assert_eq!(
            label, &declared,
            "a coverage summary is labelled {label:?} and yidam/cli's `default` feature set \
             is {declared:?}. A number under the wrong label is worse than no number: it \
             tells a reader the gated paths were measured when the gate never compiled them."
        );
    }
}

/// The gate that produces an LCOV is the gate that renders one.
///
/// Two halves that fail in opposite directions and both in silence: a job that measures
/// coverage and does not pass it to the summary throws the measurement away, and a job that
/// asks for a coverage section without producing an LCOV fails on a missing file for a
/// reason that has nothing to do with the tests.
#[test]
fn a_job_that_measures_coverage_reports_it_and_the_reverse() {
    let yml = code_only(&read(".github/workflows/ci.yml"));
    let measures = yml.contains("mise run coverage");
    let renders = yml.contains("lcov:");
    assert!(
        measures,
        "no job runs `mise run coverage`; nothing produces the LCOV the summary reads"
    );
    assert!(
        renders,
        "coverage is measured and never handed to the summary — the measurement is discarded"
    );

    // And the reporter must be given everything it needs to tell unmeasured from untested.
    for required in ["src:", "features:", "--diff", "--src", "--features"] {
        assert!(
            yml.contains(required)
                || read(".github/actions/test-summary/action.yml").contains(required),
            "the coverage path never supplies `{required}`, without which the report cannot \
             distinguish a file the build skipped from one no test touched"
        );
    }
}

/// The full-feature run exists, and its output is kept.
///
/// The light number can never cover the gated paths — that is not a defect, it is the trade
/// `ci.yml` makes for pull-request latency. What would be a defect is if the number that
/// *does* cover them were never taken, leaving "unmeasured" as a permanent answer rather
/// than a per-build one.
#[test]
fn the_gated_paths_are_measured_somewhere() {
    let mise = code_only(&read("mise.toml"));
    assert!(
        mise.contains("--all-features") && mise.contains("coverage-full"),
        "no task measures coverage with the gated features compiled in; `unmeasured` would \
         then be the permanent answer for the index, sqlite and rdf paths"
    );
    let yml = code_only(&read(".github/workflows/ci.yml"));
    assert!(
        yml.contains("mise run coverage-full"),
        "`coverage-full` exists as a task and no job runs it — a surface with no consumer, \
         which is the pattern #194's audit named and #461 found in `verify`"
    );
    assert!(
        yml.contains("coverage-full-lcov"),
        "the full-feature LCOV is produced and thrown away; P7's series is a sequence of \
         these and cannot start before its first point is kept"
    );
}
