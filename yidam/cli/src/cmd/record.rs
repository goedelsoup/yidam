//! `yidam record` — the reader for what `serve --mcp` was asked (#1019).
//!
//! #719 built the record and nothing read it. The four questions it answers — which queries came
//! back empty, whether the degraded keyword path served real traffic, whether anything acted on a
//! clock, how much of the read surface is used — took `jq` over a gitignored file whose schema
//! lived in `configuration.md` and nowhere the CLI could reach. That is #194's shape one layer up:
//! a mechanism with no path to a user.
//!
//! # A command, and not a lint check
//!
//! *Every retrieve was served degraded* is a claim about corpus health, which is what `lint` is
//! for. But `lint` reads the corpus, and this reads a gitignored file that a fresh clone, a CI
//! runner and every corpus that never declared `[serve] record` do not have. A check that passes
//! when its input is absent is worse than no check, and one that fails on it would redden every
//! build that never served anything. So this reports and never gates: it exits 0 whatever the
//! record says, as `cohort` and `check-diff` do.
//!
//! # The file, and not the fold
//!
//! #1018 folds the file into commits. That fold does not exist yet, and a reader of commits
//! nobody writes would be a second surface with no input. This reads the file; the fold, when it
//! lands, is a second source for the same report rather than a reason to hold this one.
//!
//! # Absent is not empty
//!
//! A corpus with no record file was not recorded. A corpus with an empty one was recorded and
//! asked nothing. The two are different findings — the first says nothing about readers, the
//! second says there were none — and the report keeps them apart with two fields, `declared` and
//! `present`, rather than letting both render as a table of zeros. It is the distinction the
//! writer's every-key-on-every-line rule exists to preserve, applied to the file as a whole.
//!
//! # A line from an older writer
//!
//! Every key is optional here, though the writer puts every key on every line. A line that
//! lacks one reads as not reporting it, never as a crash, because the file outlives the binary
//! that wrote it. A line that is not a JSON object naming a `tool` is counted as unreadable and
//! reported as a number: a torn last line from a killed server is the likeliest one, and a
//! reader that dropped it silently would report N−1 calls as N.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::cmd::serve::record::PATH;
use crate::report::Format;

/// The report, under one key.
///
/// One key because the envelope flattens every report into one namespace, and `calls`,
/// `tools` and `record` are all names another report could want. `consumption` is what this
/// is a record of.
#[derive(Debug, Serialize)]
pub struct Report {
    pub consumption: Consumption,
}

/// What the record says, and whether there was a record to say it.
#[derive(Debug, Default, Serialize)]
pub struct Consumption {
    /// Repo-relative, always [`PATH`]. Stated so a reader who wants the lines knows where.
    pub path: &'static str,
    /// Whether `.yidam/config.toml` declares `[serve] record = true` today.
    pub declared: bool,
    /// Whether the file exists. `false` means nothing was recorded, which is not a claim that
    /// nothing was asked.
    pub present: bool,
    /// Lines read as calls.
    pub calls: usize,
    /// Lines that were not a JSON object naming a `tool`, and are in no other count.
    pub unreadable: usize,
    /// Distinct corpus commits the calls answered from.
    pub commits: usize,
    /// The earliest and latest `at`, in seconds since the epoch. Null with no timestamped line.
    pub first_at: Option<u64>,
    pub last_at: Option<u64>,
    /// Calls that answered, with a row count of zero, and were not rejected.
    pub empty: usize,
    /// Distinct `args_digest`s among [`Self::empty`]: how many *questions* came back empty,
    /// where `empty` counts how many times.
    pub empty_questions: usize,
    /// Calls whose outcome was `error`.
    pub errors: usize,
    /// One row per tool the record names, busiest first.
    pub tools: Vec<ToolRow>,
    /// Tools the MCP contract carries that no line names, in the contract's order.
    pub never_called: Vec<String>,
}

/// One tool's share of the record.
#[derive(Debug, Serialize)]
pub struct ToolRow {
    pub tool: String,
    /// The contract's tier for this tool; null for a name the contract does not carry, which
    /// is what a caller's mistyped tool name records as.
    pub tier: Option<String>,
    pub calls: usize,
    pub errors: usize,
    pub empty: usize,
    /// Calls served degraded. **Null, not zero, for a tool that never reported the key** — a
    /// tool with no degraded path and a tool that never degraded are different facts.
    pub degraded: Option<usize>,
    /// Calls that reported `degraded` at all, true or false. `degraded == reported_degraded`
    /// is *every call was degraded*.
    pub reported_degraded: usize,
    /// The median latency, lower middle on an even count. Null with no line carrying `ms`.
    pub p50_ms: Option<u64>,
}

/// One line, read as far as it can be.
#[derive(Debug, Default)]
struct Line {
    at: Option<u64>,
    commit: Option<String>,
    tool: String,
    digest: Option<String>,
    error: bool,
    results: Option<u64>,
    degraded: Option<bool>,
    rejected: Option<bool>,
    ms: Option<u64>,
}

/// A line as a [`Line`], or `None` for one that is not an object naming a tool.
fn parse(text: &str) -> Option<Line> {
    let v: Value = serde_json::from_str(text).ok()?;
    let o = v.as_object()?;
    let str_of = |k: &str| o.get(k).and_then(Value::as_str).map(str::to_string);
    Some(Line {
        at: o.get("at").and_then(Value::as_u64),
        commit: str_of("commit"),
        tool: str_of("tool")?,
        digest: str_of("args_digest"),
        error: o.get("outcome").and_then(Value::as_str) == Some("error"),
        results: o.get("results").and_then(Value::as_u64),
        degraded: o.get("degraded").and_then(Value::as_bool),
        rejected: o.get("rejected").and_then(Value::as_bool),
        ms: o.get("ms").and_then(Value::as_u64),
    })
}

impl Line {
    /// An answer with no rows that was not a rejection — the record's reason to exist.
    ///
    /// `rejected` absent reads as not rejected: a writer too old to report it answered, and
    /// what it answered with was zero rows.
    fn empty(&self) -> bool {
        !self.error && self.results == Some(0) && self.rejected != Some(true)
    }
}

/// The report over a record's text. `None` text is an absent file.
///
/// `contract` is every tool the MCP contract carries with its tier, taken as an argument so a
/// test can hold the roster still while the contract grows.
pub(crate) fn read(
    text: Option<&str>,
    declared: bool,
    contract: &[(String, String)],
) -> Consumption {
    let mut c = Consumption {
        path: PATH,
        declared,
        present: text.is_some(),
        ..Consumption::default()
    };
    let Some(text) = text else {
        return c;
    };

    let mut lines = Vec::new();
    for raw in text.lines().filter(|l| !l.trim().is_empty()) {
        match parse(raw) {
            Some(line) => lines.push(line),
            None => c.unreadable += 1,
        }
    }

    c.calls = lines.len();
    c.commits = lines
        .iter()
        .filter_map(|l| l.commit.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    c.first_at = lines.iter().filter_map(|l| l.at).min();
    c.last_at = lines.iter().filter_map(|l| l.at).max();
    c.empty = lines.iter().filter(|l| l.empty()).count();
    c.empty_questions = lines
        .iter()
        .filter(|l| l.empty())
        .filter_map(|l| l.digest.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    c.errors = lines.iter().filter(|l| l.error).count();

    let mut by_tool: BTreeMap<&str, Vec<&Line>> = BTreeMap::new();
    for line in &lines {
        by_tool.entry(line.tool.as_str()).or_default().push(line);
    }
    let tier_of = |name: &str| {
        contract
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
    };
    c.tools = by_tool
        .iter()
        .map(|(tool, calls)| {
            let reported: Vec<bool> = calls.iter().filter_map(|l| l.degraded).collect();
            let mut ms: Vec<u64> = calls.iter().filter_map(|l| l.ms).collect();
            ms.sort_unstable();
            ToolRow {
                tool: tool.to_string(),
                tier: tier_of(tool),
                calls: calls.len(),
                errors: calls.iter().filter(|l| l.error).count(),
                empty: calls.iter().filter(|l| l.empty()).count(),
                degraded: (!reported.is_empty()).then(|| reported.iter().filter(|d| **d).count()),
                reported_degraded: reported.len(),
                p50_ms: (!ms.is_empty()).then(|| ms[(ms.len() - 1) / 2]),
            }
        })
        .collect();
    // Busiest first; the map already ordered ties by name, and the sort is stable.
    c.tools.sort_by(|a, b| b.calls.cmp(&a.calls));
    c.never_called = contract
        .iter()
        .filter(|(name, _)| !by_tool.contains_key(name.as_str()))
        .map(|(name, _)| name.clone())
        .collect();
    c
}

/// `YYYY-MM-DD`, UTC, for a timestamp the writer took from the system clock.
fn day(unix_seconds: u64) -> String {
    let (y, m, d) = crate::dates::civil_from_days((unix_seconds / 86_400) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

/// The prose: a sentence on what exists, then the table, then the findings.
///
/// The findings are the point. A table is a report; *every retrieve was served degraded* is a
/// statement about the corpus that nobody could make before the record existed.
pub(crate) fn render(c: &Consumption) -> String {
    let mut out = String::new();
    let config = ".yidam/config.toml";

    if !c.present {
        if c.declared {
            let _ = write!(
                out,
                "Nothing recorded yet: `[serve] record` is declared in {config}, and no server \
                 has opened {} since.\n  This is not a corpus nobody asked — no server was \
                 running here to be asked.\n",
                c.path
            );
        } else {
            let _ = write!(
                out,
                "Nothing recorded: this corpus does not declare `[serve] record`, and {} does \
                 not exist.\n  That says nothing about whether it was read — no server here was \
                 keeping a record.\n  To keep one, add `[serve]` with `record = true` to \
                 {config} and gitignore `.yidam/record/`.\n",
                c.path
            );
        }
        return out;
    }

    if !c.declared {
        let _ = write!(
            out,
            "`[serve] record` is not declared in {config} now; this is what was recorded while \
             it was.\n\n"
        );
    }

    if c.calls == 0 {
        let _ = writeln!(
            out,
            "0 calls recorded. A server kept {} and was asked nothing.",
            c.path
        );
        if c.unreadable > 0 {
            let _ = writeln!(
                out,
                "{} could not be read and {} not counted.",
                plural(c.unreadable, "line", "lines"),
                if c.unreadable == 1 { "is" } else { "are" }
            );
        }
        return out;
    }

    let span = match (c.first_at, c.last_at) {
        (Some(a), Some(b)) if day(a) == day(b) => format!(", {}", day(a)),
        (Some(a), Some(b)) => format!(", {} → {}", day(a), day(b)),
        _ => String::new(),
    };
    let _ = write!(
        out,
        "{} over {}{span}\n\n",
        plural(c.calls, "call", "calls"),
        plural(c.commits, "commit", "commits"),
    );

    let width = c
        .tools
        .iter()
        .map(|t| t.tool.len())
        .max()
        .unwrap_or(0)
        .max("tool".len());
    let _ = writeln!(
        out,
        "{:<width$}  {:>6}  {:>6}  {:>6}  {:>8}  {:>7}",
        "tool", "calls", "errors", "empty", "degraded", "p50"
    );
    for t in &c.tools {
        let degraded = t.degraded.map_or("—".to_string(), |d| d.to_string());
        let p50 = t.p50_ms.map_or("—".to_string(), |ms| format!("{ms}ms"));
        let _ = writeln!(
            out,
            "{:<width$}  {:>6}  {:>6}  {:>6}  {:>8}  {:>7}",
            t.tool, t.calls, t.errors, t.empty, degraded, p50
        );
    }
    out.push('\n');

    for line in findings(c) {
        out.push_str(&line);
        out.push('\n');
    }
    out
}

/// The sentences below the table, one per question the record answers.
fn findings(c: &Consumption) -> Vec<String> {
    let mut out = Vec::new();

    if c.empty == 0 {
        out.push("No call that answered came back empty.".to_string());
    } else {
        out.push(format!(
            "{} returned nothing, across {}.",
            plural(c.empty, "call", "calls"),
            plural(c.empty_questions, "distinct question", "distinct questions")
        ));
    }

    for t in &c.tools {
        let Some(degraded) = t.degraded else { continue };
        if degraded == 0 {
            continue;
        }
        if degraded == t.reported_degraded {
            out.push(format!(
                "Every {} was served degraded — this corpus has never answered it from semantic \
                 search.",
                t.tool
            ));
        } else {
            out.push(format!(
                "{degraded} of {} {} calls were served degraded.",
                t.reported_degraded, t.tool
            ));
        }
    }

    let acted: Vec<String> = c
        .tools
        .iter()
        .filter(|t| t.tier.as_deref() == Some("act"))
        .map(|t| format!("{} {}", t.tool, plural(t.calls, "time", "times")))
        .collect();
    if acted.is_empty() {
        out.push("Nothing acted on a clock: no `act`-tier tool was called.".to_string());
    } else {
        out.push(format!("Acted through this surface: {}.", acted.join(", ")));
    }

    if !c.never_called.is_empty() {
        out.push(format!(
            "{} of the contract's {} never called: {}.",
            c.never_called.len(),
            plural(
                c.never_called.len() + c.tools.iter().filter(|t| t.tier.is_some()).count(),
                "tool was",
                "tools were"
            ),
            c.never_called.join(", ")
        ));
    }

    if c.errors > 0 {
        out.push(format!(
            "{} refused.",
            plural(c.errors, "call was", "calls were")
        ));
    }
    if c.unreadable > 0 {
        out.push(format!(
            "{} could not be read and {} in no count above.",
            plural(c.unreadable, "line", "lines"),
            if c.unreadable == 1 { "is" } else { "are" }
        ));
    }
    out
}

/// `yidam record`: read the consumption record and say what it holds. Never gates.
pub fn record(root: Option<&Path>, format: Format) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let declared = crate::config::load_yidam_config(&root)?.serve.record;
    let path = root.join(PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => Some(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => anyhow::bail!("reading {} ({e})", path.display()),
    };
    let contract = crate::cmd::serve::tools::contract_tools();
    let report = Report {
        consumption: read(text.as_deref(), declared, &contract),
    };
    crate::report::finish(&root, format, report, |r| {
        print!("{}", render(&r.consumption))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> Vec<(String, String)> {
        [
            ("retrieve", "core"),
            ("get_node", "core"),
            ("neighbors", "graph"),
            ("cycle", "act"),
        ]
        .into_iter()
        .map(|(n, t)| (n.to_string(), t.to_string()))
        .collect()
    }

    fn line(tool: &str) -> String {
        format!(
            r#"{{"at":1758758400,"commit":"abc1234","tool":"{tool}","args_digest":"sha256:{tool}","outcome":"ok","results":1,"degraded":null,"rejected":null,"ms":5}}"#
        )
    }

    /// Absent and empty are different findings, and neither is a table of zeros.
    #[test]
    fn an_absent_file_is_not_an_empty_one() {
        let absent = read(None, false, &contract());
        assert!(!absent.present);
        let text = render(&absent);
        assert!(text.starts_with("Nothing recorded:"), "{text}");
        assert!(!text.contains("0 calls"), "{text}");
        assert!(
            !text.contains("degraded"),
            "no table over a file that does not exist: {text}"
        );

        let declared = render(&read(None, true, &contract()));
        assert!(declared.starts_with("Nothing recorded yet:"), "{declared}");

        let empty = read(Some(""), true, &contract());
        assert!(empty.present);
        let text = render(&empty);
        assert!(text.starts_with("0 calls recorded."), "{text}");
    }

    /// N recorded calls are reported as N.
    #[test]
    fn every_line_is_counted_once() {
        let text = [line("retrieve"), line("retrieve"), line("get_node")].join("\n");
        let c = read(Some(&text), true, &contract());
        assert_eq!(c.calls, 3);
        assert_eq!(c.unreadable, 0);
        assert_eq!(c.tools.iter().map(|t| t.calls).sum::<usize>(), 3);
        assert_eq!(c.tools[0].tool, "retrieve", "busiest first");
        assert_eq!(c.never_called, ["neighbors", "cycle"]);
        assert!(render(&c).starts_with("3 calls over 1 commit, 2025-09-25\n"));
    }

    /// A line from a writer with fewer keys is read as far as it goes, and a torn one is
    /// counted rather than dropped.
    #[test]
    fn an_older_or_torn_line_does_not_crash_the_reader() {
        let text = [
            r#"{"tool":"retrieve","outcome":"ok","results":0}"#.to_string(),
            line("get_node"),
            r#"{"at":1758758400,"tool":"retr"#.to_string(),
            r#"["not","an","object"]"#.to_string(),
        ]
        .join("\n");
        let c = read(Some(&text), true, &contract());
        assert_eq!(c.calls, 2);
        assert_eq!(c.unreadable, 2);
        // No `rejected` key: an answer with no rows, from a writer too old to say more.
        assert_eq!(c.empty, 1);
        // No digest: counted as an empty call, and as no question anyone could name.
        assert_eq!(c.empty_questions, 0);
        let retrieve = c.tools.iter().find(|t| t.tool == "retrieve").unwrap();
        assert_eq!(retrieve.p50_ms, None);
        assert_eq!(retrieve.degraded, None);
        assert!(render(&c).contains("2 lines could not be read"));
    }

    /// A rejection is not an absence, and a refusal is not an answer.
    #[test]
    fn empty_counts_only_answers_with_no_rows() {
        let text = [
            line("retrieve").replace(r#""results":1"#, r#""results":0"#),
            line("retrieve")
                .replace(r#""results":1"#, r#""results":0"#)
                .replace(r#""rejected":null"#, r#""rejected":true"#),
            line("retrieve")
                .replace(r#""outcome":"ok""#, r#""outcome":"error""#)
                .replace(r#""results":1"#, r#""results":null"#),
        ]
        .join("\n");
        let c = read(Some(&text), true, &contract());
        assert_eq!(c.empty, 1);
        assert_eq!(c.errors, 1);
        assert_eq!(c.empty_questions, 1);
    }

    /// Every call degraded is a sentence; some is a fraction; a tool with no degraded path is
    /// neither.
    #[test]
    fn degraded_is_null_where_never_reported() {
        let all = [
            line("retrieve").replace(r#""degraded":null"#, r#""degraded":true"#),
            line("retrieve").replace(r#""degraded":null"#, r#""degraded":true"#),
            line("get_node"),
        ]
        .join("\n");
        let c = read(Some(&all), true, &contract());
        let retrieve = c.tools.iter().find(|t| t.tool == "retrieve").unwrap();
        assert_eq!(retrieve.degraded, Some(2));
        assert_eq!(retrieve.reported_degraded, 2);
        let get_node = c.tools.iter().find(|t| t.tool == "get_node").unwrap();
        assert_eq!(get_node.degraded, None);
        assert!(render(&c).contains("Every retrieve was served degraded"));

        let some = all.replacen(r#""degraded":true"#, r#""degraded":false"#, 1);
        let text = render(&read(Some(&some), true, &contract()));
        assert!(
            text.contains("1 of 2 retrieve calls were served degraded."),
            "{text}"
        );
        assert!(!text.contains("Every retrieve"), "{text}");
    }

    /// The `act` tier is read from the contract, not from a list of names here.
    #[test]
    fn a_clock_is_acted_on_only_through_the_act_tier() {
        let none = render(&read(Some(&line("retrieve")), true, &contract()));
        assert!(none.contains("Nothing acted on a clock"), "{none}");
        let acted = render(&read(Some(&line("cycle")), true, &contract()));
        assert!(
            acted.contains("Acted through this surface: cycle 1 time."),
            "{acted}"
        );
    }

    #[test]
    fn a_median_takes_the_lower_middle() {
        let text = [3, 1, 4, 2]
            .iter()
            .map(|ms| line("get_node").replace(r#""ms":5"#, &format!(r#""ms":{ms}"#)))
            .collect::<Vec<_>>()
            .join("\n");
        let c = read(Some(&text), true, &contract());
        assert_eq!(c.tools[0].p50_ms, Some(2));
    }

    /// A record that outlived its declaration is still read, and says so.
    #[test]
    fn an_undeclared_record_is_read_and_flagged() {
        let text = render(&read(Some(&line("retrieve")), false, &contract()));
        assert!(
            text.starts_with("`[serve] record` is not declared"),
            "{text}"
        );
        assert!(text.contains("1 call over 1 commit"), "{text}");
    }
}
