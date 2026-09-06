//! The markdown a gate posts to `$GITHUB_STEP_SUMMARY`.
//!
//! What a reader needs from a red check, in the order they need it: what failed, why, and
//! how much of the suite ran at all. The counts come last because they are the part that is
//! legible from the job's colored dot; the failure output is first because it is the part
//! that currently requires expanding a log.

use std::fmt::Write;

use crate::census::{Kind, Skip};
use crate::coverage::{Absence, FileDiff};
use crate::junit::Run;

/// How many of the slowest tests to name. Enough to see a pattern, few enough that the
/// table does not push the failures off the screen.
const SLOWEST: usize = 10;

/// Failure output is a panic message plus a backtrace's worth of context. Past this it stops
/// being a summary; the JUnit artifact carries the whole of it.
const MAX_FAILURE_CHARS: usize = 2_000;

pub fn render(gate: &str, run: &Run, skips: &[Skip]) -> String {
    let mut s = String::new();
    let failed: Vec<_> = run.failed().collect();

    let _ = writeln!(s, "### {gate}");
    let _ = writeln!(s);

    if !failed.is_empty() {
        let _ = writeln!(s, "#### {} failed", failed.len());
        let _ = writeln!(s);
        for case in &failed {
            let _ = writeln!(s, "<details><summary><code>{}</code></summary>", case.id());
            let _ = writeln!(s);
            let _ = writeln!(s, "```");
            let body = case.failure.as_deref().unwrap_or("(no message)").trim();
            let _ = writeln!(s, "{}", truncate(body, MAX_FAILURE_CHARS));
            let captured = case.stdout.trim();
            if !captured.is_empty() {
                let _ = writeln!(s);
                let _ = writeln!(s, "{}", truncate(captured, MAX_FAILURE_CHARS));
            }
            let _ = writeln!(s, "```");
            let _ = writeln!(s);
            let _ = writeln!(s, "</details>");
            let _ = writeln!(s);
        }
    }

    let _ = writeln!(s, "| run | passed | failed | skipped |");
    let _ = writeln!(s, "|---|---|---|---|");
    let _ = writeln!(
        s,
        "| {} | {} | {} | {} |",
        run.cases.len(),
        run.passed(),
        failed.len(),
        skips.len()
    );
    let _ = writeln!(s);

    if skips.is_empty() {
        // Stated rather than omitted. An absent section reads as "nothing to say", and the
        // whole argument of the census is that a skip nobody counted looks like a pass.
        let _ = writeln!(s, "No tests were skipped.");
        let _ = writeln!(s);
    } else {
        let _ = writeln!(s, "#### Skipped ({})", skips.len());
        let _ = writeln!(s);
        let _ = writeln!(s, "| test | kind | why |");
        let _ = writeln!(s, "|---|---|---|");
        for skip in skips {
            let _ = writeln!(
                s,
                "| `{}` | {} | {} |",
                skip.test,
                skip.kind.label(),
                skip.reason
            );
        }
        let _ = writeln!(s);
        if skips.iter().any(|s| s.kind == Kind::Ignored) {
            let _ = writeln!(
                s,
                "> `ignored` tests are absent from the JUnit artifact — nextest does not \
                 write them. They are counted from `nextest list`."
            );
            let _ = writeln!(s);
        }
    }

    let mut timed: Vec<_> = run
        .cases
        .iter()
        .filter_map(|c| c.time.map(|t| (t, c)))
        .collect();
    timed.sort_by(|a, b| b.0.total_cmp(&a.0));
    if !timed.is_empty() {
        let _ = writeln!(s, "<details><summary>Slowest {SLOWEST}</summary>");
        let _ = writeln!(s);
        let _ = writeln!(s, "| test | seconds |");
        let _ = writeln!(s, "|---|---|");
        for (time, case) in timed.iter().take(SLOWEST) {
            let _ = writeln!(s, "| `{}` | {time:.3} |", case.id());
        }
        let _ = writeln!(s);
        let _ = writeln!(s, "</details>");
        let _ = writeln!(s);
    }

    s
}

/// The coverage section: what this change added, and what the run could not see.
///
/// Two lists, never merged. Uncovered lines are lines a test could have executed and did
/// not; unmeasured files were not compiled into the build that produced the LCOV. Adding
/// them together produces the number #464 exists to prevent — one that calls the whole index
/// path untested because a pull request does not build it.
///
/// `features` is what the measurement was taken under, and it is printed whether or not
/// anything was unmeasured. A reader who cannot see which build this was cannot tell a
/// thorough run from a narrow one.
pub fn render_coverage(features: &[String], files: &[FileDiff]) -> String {
    let mut s = String::new();
    let feature_list = if features.is_empty() {
        "unknown".to_string()
    } else {
        features
            .iter()
            .map(|f| format!("`{f}`"))
            .collect::<Vec<_>>()
            .join(", ")
    };

    let _ = writeln!(s, "#### Coverage of this change");
    let _ = writeln!(s);
    let _ = writeln!(s, "Measured under: {feature_list}");
    let _ = writeln!(s);

    let measured: Vec<&FileDiff> = files.iter().filter(|f| f.uncovered.is_some()).collect();
    let added: usize = measured.iter().map(|f| f.added.len()).sum();
    let uncovered: usize = measured
        .iter()
        .map(|f| f.uncovered.as_ref().map_or(0, |u| u.len()))
        .sum();

    if added == 0 {
        let _ = writeln!(s, "No Rust lines added in a file this build compiled.");
        let _ = writeln!(s);
    } else {
        let pct = ((added - uncovered) as f64 / added as f64) * 100.0;
        let _ = writeln!(s, "| added lines | covered | uncovered |");
        let _ = writeln!(s, "|---|---|---|");
        let _ = writeln!(
            s,
            "| {added} | {} ({pct:.0}%) | {uncovered} |",
            added - uncovered
        );
        let _ = writeln!(s);
    }

    let with_gaps: Vec<&&FileDiff> = measured
        .iter()
        .filter(|f| f.uncovered.as_ref().is_some_and(|u| !u.is_empty()))
        .collect();
    if !with_gaps.is_empty() {
        let _ = writeln!(s, "Lines this change added that no test executed:");
        let _ = writeln!(s);
        for f in with_gaps {
            let lines = f.uncovered.as_ref().expect("filtered on Some");
            let _ = writeln!(s, "- `{}` — {}", f.path, join_ranges(lines));
        }
        let _ = writeln!(s);
    }

    // Unmeasured is its own heading on purpose. Folded into the table above it would read as
    // a coverage gap; it is a statement about the build, not about the tests.
    let unmeasured: Vec<&FileDiff> = files.iter().filter(|f| f.uncovered.is_none()).collect();
    if !unmeasured.is_empty() {
        let _ = writeln!(
            s,
            "**Not measured under these features** — these were not compiled into the build this ran against, so nothing here is a claim that they are untested:"
        );
        let _ = writeln!(s);
        for f in &unmeasured {
            let why = match &f.absence {
                Some(Absence::Gated(feature)) => format!("gated behind `{feature}`"),
                Some(Absence::TestOnly) => "test code".to_string(),
                Some(Absence::NoCoverableCode) => "no coverable code".to_string(),
                Some(Absence::Unexplained) | None => {
                    "**not compiled, and nothing says why**".to_string()
                }
            };
            let _ = writeln!(s, "- `{}` ({} lines added) — {why}", f.path, f.added.len());
        }
        let _ = writeln!(s);
    }

    s
}

/// `11, 12, 13, 20` → `11-13, 20`. A list of forty consecutive line numbers is not something
/// anybody reads; a range is.
fn join_ranges(lines: &[u32]) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let start = lines[i];
        let mut end = start;
        while i + 1 < lines.len() && lines[i + 1] == end + 1 {
            i += 1;
            end = lines[i];
        }
        out.push(if start == end {
            start.to_string()
        } else {
            format!("{start}-{end}")
        });
        i += 1;
    }
    out.join(", ")
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let kept: String = text.chars().take(max).collect();
    format!("{kept}\n… truncated; the full output is in this job's JUnit artifact")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::junit;

    fn run_of(xml: &str) -> Run {
        junit::parse(xml).expect("fixture should parse")
    }

    const ONE_FAILURE: &str = r#"<testsuites><testsuite name="s">
        <testcase name="a" classname="yidam::x" time="0.5"><system-out>ok</system-out></testcase>
        <testcase name="b" classname="yidam::x" time="2.5">
          <failure message="left != right">thread 'b' panicked at tests/x.rs:9</failure>
        </testcase>
      </testsuite></testsuites>"#;

    /// The assertion #462 asks for by name: a red check must be legible without opening a log.
    #[test]
    fn a_failure_summary_names_the_failing_test_and_its_output() {
        let md = render("ci (cli)", &run_of(ONE_FAILURE), &[]);
        assert!(
            md.contains("yidam::x b"),
            "the failing test is not named:\n{md}"
        );
        assert!(md.contains("left != right"), "the reason is missing:\n{md}");
        assert!(
            md.contains("panicked at tests/x.rs:9"),
            "the output is missing:\n{md}"
        );
        assert!(
            !md.contains("yidam::x a\n"),
            "a passing test should not be listed as failed"
        );
    }

    #[test]
    fn counts_separate_passed_from_failed() {
        let md = render("ci (cli)", &run_of(ONE_FAILURE), &[]);
        assert!(md.contains("| 2 | 1 | 1 | 0 |"), "counts are wrong:\n{md}");
    }

    /// A fully-skipped suite must not render as a passing one. It is the fixture easiest to
    /// lose, and losing it discards the whole argument of the census.
    #[test]
    fn a_fully_skipped_suite_says_so_rather_than_looking_green() {
        let skips = vec![
            Skip {
                test: "yidam::vault_s3 a".into(),
                reason: "needs a server".into(),
                kind: Kind::Ignored,
            },
            Skip {
                test: "yidam::vault_s3 b".into(),
                reason: "needs a server".into(),
                kind: Kind::Ignored,
            },
        ];
        let md = render("ci (cli)", &run_of(r#"<testsuites></testsuites>"#), &skips);
        assert!(
            md.contains("Skipped (2)"),
            "the census is not in the summary:\n{md}"
        );
        assert!(md.contains("needs a server"));
        assert!(md.contains("nextest does not write them"));
    }

    /// An empty skip list is stated, not omitted — see the comment at the call site.
    #[test]
    fn no_skips_is_said_out_loud() {
        let md = render("ci (cli)", &run_of(ONE_FAILURE), &[]);
        assert!(md.contains("No tests were skipped."), "{md}");
    }

    #[test]
    fn the_slowest_table_orders_by_time() {
        let md = render("ci (cli)", &run_of(ONE_FAILURE), &[]);
        let b = md.find("yidam::x b` | 2.500").expect("slow test missing");
        let a = md.find("yidam::x a` | 0.500").expect("fast test missing");
        assert!(b < a, "slowest should come first:\n{md}");
    }

    // ── coverage ─────────────────────────────────────────────────────────────

    fn gated(path: &str, feature: &str, added: usize) -> FileDiff {
        FileDiff {
            path: path.into(),
            added: (1..=added as u32).collect(),
            uncovered: None,
            absence: Some(Absence::Gated(feature.into())),
        }
    }

    /// The assertion that decides whether the number is honest.
    ///
    /// A gated file contributes **nothing** to the added/uncovered arithmetic. Counted as
    /// uncovered it would drag the percentage down and name the index path untested, which
    /// is the claim #464 exists to stop this repository from making.
    #[test]
    fn unmeasured_files_are_not_counted_as_uncovered() {
        let files = vec![
            FileDiff {
                path: "yidam/cli/src/parse.rs".into(),
                added: vec![10, 11],
                uncovered: Some(vec![11]),
                absence: None,
            },
            gated("yidam/cli/src/embedding.rs", "vector-read", 40),
        ];
        let md = render_coverage(&["reports".into(), "tonpa".into()], &files);
        assert!(
            md.contains("| 2 | 1 (50%) | 1 |"),
            "gated lines entered the sum:\n{md}"
        );
        assert!(md.contains("Not measured under these features"));
        assert!(md.contains("gated behind `vector-read`"));
        assert!(
            !md.contains("embedding.rs` — 1-40"),
            "a gated file must not be listed as uncovered lines:\n{md}"
        );
    }

    /// The build is named even when everything was measured.
    #[test]
    fn the_feature_set_is_always_stated() {
        let md = render_coverage(&["reports".into()], &[]);
        assert!(md.contains("Measured under: `reports`"), "{md}");
    }

    /// A file that is not compiled and has no gate to explain it is the interesting case,
    /// and it must not read like the legitimate ones.
    #[test]
    fn an_unexplained_absence_says_so_loudly() {
        let files = vec![FileDiff {
            path: "yidam/cli/src/orphan.rs".into(),
            added: vec![1],
            uncovered: None,
            absence: Some(Absence::Unexplained),
        }];
        let md = render_coverage(&["reports".into()], &files);
        assert!(md.contains("nothing says why"), "{md}");
    }

    #[test]
    fn consecutive_uncovered_lines_render_as_a_range() {
        let files = vec![FileDiff {
            path: "a.rs".into(),
            added: (10..=14).collect(),
            uncovered: Some(vec![10, 11, 12, 14]),
            absence: None,
        }];
        let md = render_coverage(&[], &files);
        assert!(md.contains("10-12, 14"), "{md}");
    }

    #[test]
    fn a_long_failure_is_truncated_and_says_where_the_rest_is() {
        let long = "x".repeat(MAX_FAILURE_CHARS + 100);
        let xml = format!(
            r#"<testsuites><testsuite name="s"><testcase name="b" classname="c" time="1">
                 <failure message="boom">{long}</failure></testcase></testsuite></testsuites>"#
        );
        let md = render("g", &run_of(&xml), &[]);
        assert!(
            md.contains("truncated"),
            "no truncation notice:\n{}",
            &md[..400.min(md.len())]
        );
        assert!(md.contains("JUnit artifact"));
    }
}
