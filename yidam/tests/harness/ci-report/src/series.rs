//! One record per push to main — the history the gates never kept.
//!
//! Every run of every gate computes a number, displays it, and throws it away. There is no
//! way to see that a suite has been shrinking, or that coverage moved three weeks ago and
//! nobody noticed. `quality-report.json` describes one commit; this is the sequence.
//!
//! # Where it lives, and why that was a decision
//!
//! On the `quality-series` orphan branch, one JSONL file, appended by CI on each push to
//! main. RFC-0025 left the choice open and asked for it to be settled here, "because the
//! answer is hard to change once there is a year of them".
//!
//! Git, because git is the one store this repository already trusts and `.yidam/`-shaped
//! history is its whole thesis; a Pages-side artifact has no history older than the last
//! deploy. An **orphan branch** rather than a file on `main`, because a bot commit per push
//! would land in `git log main` beside real work, would race a human push, and — `ci.yml`
//! being `on: push: branches: [main]` — would re-trigger the workflow that wrote it. The
//! orphan branch is fetched by the docs build and read by nothing else.
//!
//! # It is not a second source of truth
//!
//! A record states what a gate measured and computes nothing of its own. Nothing reads it to
//! decide whether CI passes. If a number here disagrees with a gate, the gate is right and
//! this has a bug.
//!
//! # JSONL, and why a bad line is skipped rather than fatal
//!
//! The file is append-only and read by a static site build. One truncated write — a cancelled
//! job, a runner that died mid-push — must not blank a year of history, so [`read`] drops the
//! lines it cannot parse and says how many. A parser that refused the file would turn a
//! single bad append into a page with nothing on it.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::quality;

/// One push to main.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Short commit. The key: a re-run of the same workflow replaces rather than duplicates.
    pub commit: String,
    /// Unix seconds. Not a formatted date — a formatter is a second opinion about a timezone,
    /// and `ci-runs-UTC` is a lesson this repository has already paid for once.
    pub recorded_at: u64,
    pub gates: usize,
    pub totals: quality::Totals,
    /// Seconds of test execution the run reported, summed across gates.
    ///
    /// Not CI wall-clock. Wall-clock is dominated by toolchain provisioning and cache state
    /// and moves for reasons that have nothing to do with this repository; the sum of what
    /// the tests themselves took is the number that goes up when a suite gets slower.
    pub test_seconds: f64,
    /// `None` when no gate in that run measured coverage.
    pub coverage: Option<CoverageRecord>,
    /// `None` when the bench baseline could not be read.
    pub bench: Option<BenchRecord>,
    /// The jobs of that run which did not succeed, by name (#516).
    ///
    /// `totals.failed` counts test cases, and a run can fail without one of them failing.
    /// A series of `failed: 0` across a month of red builds is a chart that says the
    /// repository was healthy, drawn from numbers that were each individually true.
    ///
    /// `None` means the record was written without a job list — every record before #516,
    /// and any run whose merge could not reach the API. An empty vector means it asked and
    /// nothing had failed, which is a different claim and is stored as one.
    #[serde(default)]
    pub unsuccessful_jobs: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CoverageRecord {
    pub added: usize,
    pub uncovered: usize,
    /// The features the measurement was taken under. Carried per record because a series
    /// whose points were measured under different builds is not one line — and #464's whole
    /// argument is that a coverage number without its feature set cannot be read.
    pub features: Vec<String>,
}

/// The headline cost the scaling benchmark reports, at its largest corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BenchRecord {
    pub nodes: u64,
    /// Tokens the focused-scan arm charges an agent at that size — the number the whole
    /// experiment exists to drive down.
    pub focused_tokens: u64,
    pub full_scan_tokens: u64,
    pub focused_precision: f64,
}

/// Build a record from a run's report and the committed bench baseline.
///
/// The bench numbers are read from the baseline rather than measured again. The baseline *is*
/// the measurement — `bench_baseline.rs` fails the build if a fresh run disagrees with it — so
/// running the benchmark a second time here would spend two seconds to re-derive a number the
/// gate has already certified, and would let the series and the ratchet disagree.
pub fn record(report: &quality::Envelope, bench_baseline: Option<&str>, now: u64) -> Record {
    let totals = report
        .quality
        .gates
        .iter()
        .map(|g| g.totals)
        .fold(quality::Totals::zero(), quality::Totals::plus);

    let test_seconds = report
        .quality
        .gates
        .iter()
        .flat_map(|g| &g.suites)
        .flat_map(|s| &s.tests)
        .filter_map(|t| t.seconds)
        .sum();

    let coverage = report
        .quality
        .gates
        .iter()
        .find_map(|g| g.coverage.as_ref())
        .map(|c| CoverageRecord {
            added: c.added,
            uncovered: c.uncovered,
            features: c.features.clone(),
        });

    Record {
        commit: report.yidam.commit.clone(),
        recorded_at: now,
        gates: report.quality.gates.len(),
        totals,
        test_seconds,
        coverage,
        bench: bench_baseline.and_then(bench_from_baseline),
        // Read off the report rather than recomputed: the report is what the run concluded,
        // and a second opinion here is how the chart and the page come to disagree.
        unsuccessful_jobs: report
            .quality
            .run
            .as_ref()
            .map(|r| r.unsuccessful().iter().map(|j| j.name.clone()).collect()),
    }
}

/// The largest row of a `bench --scaling` report.
fn bench_from_baseline(text: &str) -> Option<BenchRecord> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    let rows = value["rows"].as_array()?;
    let row = rows
        .iter()
        .max_by_key(|r| r["corpus"]["nodes"].as_u64().unwrap_or(0))?;
    Some(BenchRecord {
        nodes: row["corpus"]["nodes"].as_u64()?,
        focused_tokens: row["focused_scan"]["tokens"].as_u64()?,
        full_scan_tokens: row["full_scan"]["tokens"].as_u64()?,
        focused_precision: row["focused_scan"]["precision"].as_f64()?,
    })
}

/// What a read of the series found, and what it could not read.
#[derive(Debug, Default)]
pub struct Series {
    pub records: Vec<Record>,
    /// Line numbers that did not parse. Reported rather than swallowed: a file quietly
    /// dropping half its lines looks exactly like a file with half as much history.
    pub unreadable: Vec<usize>,
}

/// Parse a JSONL series, skipping lines that do not parse.
pub fn parse(text: &str) -> Series {
    let mut out = Series::default();
    for (i, line) in text.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        match serde_json::from_str::<Record>(line) {
            Ok(record) => out.records.push(record),
            Err(_) => out.unreadable.push(i + 1),
        }
    }
    out
}

pub fn read(path: &Path) -> Result<Series> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(parse(&text)),
        // An absent file is an empty series, not an error: the first push to write one finds
        // nothing there, and that is the normal case exactly once.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Series::default()),
        Err(e) => Err(e).with_context(|| format!("reading the series at {}", path.display())),
    }
}

/// Append a record, replacing any earlier one for the same commit.
///
/// Re-running a workflow must not produce two points for one commit. The series would show a
/// step that never happened, and the sparkline would draw it.
pub fn append(series: &Series, record: Record) -> Vec<Record> {
    let mut out: Vec<Record> = series
        .records
        .iter()
        .filter(|r| r.commit != record.commit)
        .cloned()
        .collect();
    out.push(record);
    out
}

/// JSONL: one record per line, and every line valid on its own.
pub fn render(records: &[Record]) -> String {
    let mut out = String::new();
    for record in records {
        out.push_str(&serde_json::to_string(record).expect("a record serializes"));
        out.push('\n');
    }
    out
}

/// The series as columns a page can draw, newest last.
///
/// Returned as `(label, points)` rather than as a struct per metric, so a metric added to the
/// record shows up on the page without a second edit here.
pub fn columns(records: &[Record]) -> BTreeMap<&'static str, Vec<f64>> {
    let mut out: BTreeMap<&'static str, Vec<f64>> = BTreeMap::new();
    for r in records {
        out.entry("asserted")
            .or_default()
            .push(r.totals.asserted as f64);
        out.entry("failed")
            .or_default()
            .push(r.totals.failed as f64);
        out.entry("skipped")
            .or_default()
            .push(r.totals.skipped as f64);
        out.entry("test_seconds").or_default().push(r.test_seconds);
        if let Some(b) = &r.bench {
            out.entry("bench_focused_tokens")
                .or_default()
                .push(b.focused_tokens as f64);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::junit;

    const RUN: &str = r#"<testsuites><testsuite name="s">
        <testcase name="a" classname="yidam::x" time="0.5"/>
        <testcase name="b" classname="yidam::y" time="2.5">
          <system-out>YIDAM-SKIP: needs a server</system-out>
        </testcase>
      </testsuite></testsuites>"#;

    fn envelope() -> quality::Envelope {
        let run = junit::parse(RUN).expect("fixture parses");
        let skips = crate::census::gated(&run);
        let gate = quality::gate("ci (cli)", None, &["reports".into()], &run, &skips, None);
        quality::fragment(
            &quality::Provenance {
                version: "0.7.0".into(),
                commit: "abc1234".into(),
                root: "/repo".into(),
            },
            gate,
        )
    }

    #[test]
    fn a_record_carries_what_the_run_measured_and_invents_nothing() {
        let r = record(&envelope(), None, 1_756_000_000);
        assert_eq!(r.commit, "abc1234");
        assert_eq!(r.gates, 1);
        assert_eq!(r.totals.cases, 2);
        assert_eq!(r.totals.asserted, 1, "the skipped test is not an assertion");
        assert_eq!(r.totals.skipped, 1);
        assert_eq!(r.test_seconds, 3.0);
        assert!(r.coverage.is_none(), "no gate measured coverage");
        assert!(r.bench.is_none());
    }

    #[test]
    fn the_bench_headline_is_read_from_the_largest_row() {
        let baseline = r#"{"rows":[
            {"corpus":{"nodes":8},"focused_scan":{"tokens":100,"precision":0.5},"full_scan":{"tokens":900}},
            {"corpus":{"nodes":4096},"focused_scan":{"tokens":300,"precision":0.1},"full_scan":{"tokens":9000}}
        ]}"#;
        let b = record(&envelope(), Some(baseline), 0)
            .bench
            .expect("a bench record");
        assert_eq!(b.nodes, 4096, "the headline must be the largest corpus");
        assert_eq!(b.focused_tokens, 300);
        assert_eq!(b.full_scan_tokens, 9000);
    }

    /// A baseline this cannot read leaves the field absent rather than zero, for the reason
    /// the whole epic is about: an absent measurement and a measurement of zero are different
    /// facts and a chart cannot tell them apart.
    #[test]
    fn an_unreadable_baseline_leaves_the_bench_field_absent() {
        assert!(record(&envelope(), Some("not json"), 0).bench.is_none());
        assert!(record(&envelope(), Some(r#"{"rows":[]}"#), 0)
            .bench
            .is_none());
    }

    /// The assertion #468 asks for by name.
    #[test]
    fn a_malformed_line_is_skipped_and_the_rest_survives() {
        let good = serde_json::to_string(&record(&envelope(), None, 1)).unwrap();
        let text = format!("{good}\n{{ truncated\n\n{good}\n");
        let series = parse(&text);
        assert_eq!(
            series.records.len(),
            2,
            "a bad line took the good ones with it"
        );
        assert_eq!(series.unreadable, vec![2], "the bad line was not reported");
    }

    #[test]
    fn re_running_a_workflow_replaces_the_commits_record_rather_than_adding_one() {
        let first = record(&envelope(), None, 100);
        let series = parse(&render(&[first.clone()]));
        let again = Record {
            recorded_at: 200,
            ..first.clone()
        };
        let out = append(&series, again.clone());
        assert_eq!(out.len(), 1, "one commit produced two points");
        assert_eq!(out[0].recorded_at, 200, "the later run did not win");
    }

    #[test]
    fn appending_a_different_commit_keeps_both_in_order() {
        let first = record(&envelope(), None, 100);
        let second = Record {
            commit: "def5678".into(),
            recorded_at: 200,
            ..first.clone()
        };
        let out = append(&parse(&render(&[first])), second);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].commit, "abc1234");
        assert_eq!(out[1].commit, "def5678", "the newest record must come last");
    }

    /// A record says which jobs failed, not merely that no test did (#516).
    ///
    /// `totals.failed` is a count of test cases. A run whose `ci (cli · full features)` died
    /// on a coverage flag has `failed: 0` and is red, and a year of those is a chart that
    /// says the repository was healthy — every point individually true.
    #[test]
    fn a_record_carries_the_jobs_that_did_not_succeed() {
        let mut report = envelope();
        report.quality.run = Some(quality::RunJobs {
            jobs: vec![
                quality::Job {
                    name: "ci (cli)".into(),
                    conclusion: "success".into(),
                },
                quality::Job {
                    name: "ci (cli · full features)".into(),
                    conclusion: "failure".into(),
                },
            ],
            pending: Vec::new(),
        });
        let r = record(&report, None, 1788000000);
        assert_eq!(
            r.unsuccessful_jobs,
            Some(vec!["ci (cli · full features)".to_string()]),
            "the failing job is what a reader of this record most needs"
        );
        assert_eq!(
            r.totals.failed, 0,
            "and no test failed, which is the whole reason the field has to exist"
        );
    }

    /// Asking and finding nothing is not the same claim as never asking.
    #[test]
    fn nothing_failed_and_nobody_asked_are_stored_differently() {
        let mut report = envelope();
        report.quality.run = Some(quality::RunJobs {
            jobs: vec![quality::Job {
                name: "ci (cli)".into(),
                conclusion: "success".into(),
            }],
            pending: Vec::new(),
        });
        assert_eq!(
            record(&report, None, 0).unsuccessful_jobs,
            Some(Vec::new()),
            "a run that was checked and was clean records an empty list"
        );

        let unasked = envelope();
        assert_eq!(
            record(&unasked, None, 0).unsuccessful_jobs,
            None,
            "a run nobody could ask about must not record a clean bill of health"
        );
    }

    /// The records written before the field existed still parse.
    ///
    /// The series is append-only and years long. A consumer that could not read its own
    /// history would blank every point before the change that added a field — which is the
    /// same failure as a parser that refuses a truncated line, one release later.
    #[test]
    fn a_record_from_before_the_job_list_still_parses() {
        let line = r#"{"commit":"aaa1111","recorded_at":1788000000,"gates":4,"totals":{"cases":10,"failed":0,"passed":10,"skipped":0,"gated":0,"ignored":0,"asserted":10},"test_seconds":1.5,"coverage":null,"bench":null}"#;
        let series = parse(line);
        assert!(series.unreadable.is_empty(), "{:?}", series.unreadable);
        assert_eq!(series.records.len(), 1);
        assert_eq!(series.records[0].unsuccessful_jobs, None);
    }

    #[test]
    fn a_record_round_trips_through_jsonl() {
        let r = record(&envelope(), None, 42);
        let series = parse(&render(&[r.clone()]));
        assert_eq!(series.records, vec![r]);
        assert!(series.unreadable.is_empty());
    }

    #[test]
    fn columns_are_drawn_in_record_order() {
        let a = record(&envelope(), None, 1);
        let b = Record {
            commit: "def5678".into(),
            test_seconds: 9.0,
            ..a.clone()
        };
        let cols = columns(&[a, b]);
        assert_eq!(cols["test_seconds"], vec![3.0, 9.0]);
        assert_eq!(cols["asserted"].len(), 2);
    }
}
