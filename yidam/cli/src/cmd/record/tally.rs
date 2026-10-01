//! What a run of record lines adds up to, in a shape that adds up again.
//!
//! The report and the fold (#1018) read the same lines and must agree on what they say, so
//! both go through this one type: `yidam record` tallies the file, `--fold` tallies it and
//! commits the tally, and a later `yidam record` merges the committed tally with whatever the
//! file has gained since. Every field is therefore one that **merges exactly** — a sum, a set,
//! a min or a max, a histogram — and nothing is kept that a merge would have to approximate.
//! That is why latency is a histogram of milliseconds rather than a median: a median of two
//! medians is not the median, and a report that changed its p50 because a fold happened would
//! be reporting the fold rather than the calls.
//!
//! # What it keeps from a line, and what it does not
//!
//! Every count the report draws, and the set of nodes `retrieve` returned with how often. Not
//! the lines. The per-call `node_ids` set is what makes two calls look related (the module
//! doc in `serve/record.rs` states that cost); once tallied, an id has a count and no
//! neighbours, so the history carries *how often this node came back* and nothing that groups
//! one caller's answers together. `args_digest` survives only for the calls that came back
//! empty, because *how many distinct questions came back empty* is a set and cannot be summed.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// One line, read as far as it can be.
#[derive(Debug, Default)]
pub(crate) struct Line {
    pub at: Option<u64>,
    pub commit: Option<String>,
    pub tool: String,
    pub digest: Option<String>,
    pub error: bool,
    pub results: Option<u64>,
    pub degraded: Option<bool>,
    pub rejected: Option<bool>,
    pub ms: Option<u64>,
    pub node_ids: Option<Vec<String>>,
}

/// A line as a [`Line`], or `None` for one that is not an object naming a tool.
pub(crate) fn parse(text: &str) -> Option<Line> {
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
        node_ids: o.get("node_ids").and_then(Value::as_array).map(|ids| {
            ids.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        }),
    })
}

impl Line {
    /// An answer with no rows that was not a rejection — the record's reason to exist.
    ///
    /// `rejected` absent reads as not rejected: a writer too old to report it answered, and
    /// what it answered with was zero rows.
    pub fn empty(&self) -> bool {
        !self.error && self.results == Some(0) && self.rejected != Some(true)
    }
}

/// Every count the report draws, over some run of lines.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Tally {
    /// Lines read as calls.
    pub calls: usize,
    /// Lines that were not a JSON object naming a `tool`.
    pub unreadable: usize,
    /// Every corpus commit a call answered from.
    pub commits: BTreeSet<String>,
    pub first_at: Option<u64>,
    pub last_at: Option<u64>,
    pub empty: usize,
    /// The `args_digest` of every call that came back empty.
    pub empty_questions: BTreeSet<String>,
    pub errors: usize,
    pub tools: BTreeMap<String, ToolTally>,
    pub retrieved: Retrieved,
}

/// One tool's share.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct ToolTally {
    pub calls: usize,
    pub errors: usize,
    pub empty: usize,
    /// Calls that reported `degraded: true`.
    pub degraded: usize,
    /// Calls that reported `degraded` at all.
    pub reported_degraded: usize,
    /// How many calls took each number of milliseconds.
    pub ms: BTreeMap<u64, usize>,
}

/// What answered `retrieve` calls returned, for the complement #1020 makes computable.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub(crate) struct Retrieved {
    /// How many times each id came back.
    pub nodes: BTreeMap<String, usize>,
    /// Answered calls whose line named its nodes.
    pub named: usize,
    /// Answered calls whose line did not — an older writer's.
    pub unnamed: usize,
}

impl Tally {
    /// The tally of a text, one line per call. A line that does not parse is counted, never
    /// dropped: a torn last line from a killed server is the likeliest one.
    pub fn of_text(text: &str) -> Self {
        let mut t = Self::default();
        for raw in text.lines().filter(|l| !l.trim().is_empty()) {
            match parse(raw) {
                Some(line) => t.add(&line),
                None => t.unreadable += 1,
            }
        }
        t
    }

    pub fn add(&mut self, line: &Line) {
        self.calls += 1;
        if let Some(c) = &line.commit {
            self.commits.insert(c.clone());
        }
        if let Some(at) = line.at {
            self.first_at = Some(self.first_at.map_or(at, |f| f.min(at)));
            self.last_at = Some(self.last_at.map_or(at, |l| l.max(at)));
        }
        if line.empty() {
            self.empty += 1;
            if let Some(d) = &line.digest {
                self.empty_questions.insert(d.clone());
            }
        }
        if line.error {
            self.errors += 1;
        }

        let tool = self.tools.entry(line.tool.clone()).or_default();
        tool.calls += 1;
        tool.errors += usize::from(line.error);
        tool.empty += usize::from(line.empty());
        if let Some(d) = line.degraded {
            tool.reported_degraded += 1;
            tool.degraded += usize::from(d);
        }
        if let Some(ms) = line.ms {
            *tool.ms.entry(ms).or_default() += 1;
        }

        // Only answered calls, from any writer: a refusal returned nothing to name.
        if line.tool == "retrieve" && !line.error {
            match &line.node_ids {
                Some(ids) => {
                    self.retrieved.named += 1;
                    for id in ids {
                        *self.retrieved.nodes.entry(id.clone()).or_default() += 1;
                    }
                }
                None => self.retrieved.unnamed += 1,
            }
        }
    }

    /// Fold `other` into this, as if its lines had followed these.
    pub fn merge(&mut self, other: &Self) {
        self.calls += other.calls;
        self.unreadable += other.unreadable;
        self.commits.extend(other.commits.iter().cloned());
        self.first_at = min_of(self.first_at, other.first_at);
        self.last_at = max_of(self.last_at, other.last_at);
        self.empty += other.empty;
        self.empty_questions
            .extend(other.empty_questions.iter().cloned());
        self.errors += other.errors;
        for (name, theirs) in &other.tools {
            let ours = self.tools.entry(name.clone()).or_default();
            ours.calls += theirs.calls;
            ours.errors += theirs.errors;
            ours.empty += theirs.empty;
            ours.degraded += theirs.degraded;
            ours.reported_degraded += theirs.reported_degraded;
            for (ms, n) in &theirs.ms {
                *ours.ms.entry(*ms).or_default() += n;
            }
        }
        for (id, n) in &other.retrieved.nodes {
            *self.retrieved.nodes.entry(id.clone()).or_default() += n;
        }
        self.retrieved.named += other.retrieved.named;
        self.retrieved.unnamed += other.retrieved.unnamed;
    }
}

impl ToolTally {
    /// The median latency, lower middle on an even count — what sorting every call's `ms`
    /// and taking index `(n - 1) / 2` gives, read off the histogram instead.
    pub fn p50(&self) -> Option<u64> {
        let n: usize = self.ms.values().sum();
        if n == 0 {
            return None;
        }
        let target = (n - 1) / 2;
        let mut seen = 0;
        for (ms, count) in &self.ms {
            seen += count;
            if seen > target {
                return Some(*ms);
            }
        }
        None
    }
}

fn min_of(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

fn max_of(a: Option<u64>, b: Option<u64>) -> Option<u64> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (a, b) => a.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(tool: &str, ms: u64, extra: &str) -> String {
        format!(
            r#"{{"at":{at},"commit":"c{ms}","tool":"{tool}","args_digest":"sha256:{ms}","outcome":"ok","results":1,"degraded":true,"rejected":null,"ms":{ms}{extra}}}"#,
            at = 1_758_758_400 + ms,
        )
    }

    /// The property the fold rests on: tallying two halves and merging them says exactly what
    /// tallying the whole does. A field that broke this would make a report change because a
    /// fold happened.
    #[test]
    fn a_merge_of_two_halves_is_the_tally_of_the_whole() {
        let lines = [
            line("retrieve", 3, r#","node_ids":["concept/a","concept/b"]"#),
            line("retrieve", 1, r#","node_ids":["concept/a"]"#),
            line("get_node", 4, ""),
            r#"{"torn"#.to_string(),
            line("retrieve", 2, "").replace(r#""results":1"#, r#""results":0"#),
            line("retrieve", 9, "").replace(r#""outcome":"ok""#, r#""outcome":"error""#),
        ];
        let whole = Tally::of_text(&lines.join("\n"));
        for cut in 0..=lines.len() {
            let mut merged = Tally::of_text(&lines[..cut].join("\n"));
            merged.merge(&Tally::of_text(&lines[cut..].join("\n")));
            assert_eq!(merged, whole, "cut at {cut}");
        }
        assert_eq!(whole.calls, 5);
        assert_eq!(whole.unreadable, 1);
        assert_eq!(whole.retrieved.nodes["concept/a"], 2);
        assert_eq!((whole.retrieved.named, whole.retrieved.unnamed), (2, 1));
    }

    /// The histogram's median is the sorted list's, lower middle on an even count.
    #[test]
    fn a_histogram_median_is_the_lower_middle() {
        let t = |ms: &[u64]| ToolTally {
            ms: ms.iter().fold(BTreeMap::new(), |mut m, ms| {
                *m.entry(*ms).or_default() += 1;
                m
            }),
            ..ToolTally::default()
        };
        assert_eq!(t(&[3, 1, 4, 2]).p50(), Some(2));
        assert_eq!(t(&[5, 5, 1]).p50(), Some(5));
        assert_eq!(t(&[7]).p50(), Some(7));
        assert_eq!(t(&[]).p50(), None);
    }

    /// What reaches the history: counts, and no per-call grouping of ids.
    #[test]
    fn a_tally_keeps_no_line() {
        let t = Tally::of_text(&line("retrieve", 3, r#","node_ids":["concept/a"]"#));
        let json = serde_json::to_string(&t).unwrap();
        assert!(
            !json.contains("sha256:3"),
            "a non-empty call's digest: {json}"
        );
        assert!(json.contains(r#""concept/a":1"#), "{json}");
    }
}
