//! The markdown a gate posts to `$GITHUB_STEP_SUMMARY`.
//!
//! What a reader needs from a red check, in the order they need it: what failed, why, and
//! how much of the suite ran at all. The counts come last because they are the part that is
//! legible from the job's colored dot; the failure output is first because it is the part
//! that currently requires expanding a log.

use std::fmt::Write;

use crate::census::{Kind, Skip};
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
