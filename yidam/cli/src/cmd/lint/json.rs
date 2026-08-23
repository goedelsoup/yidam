//! `yidam lint --format json` — the report contract's first and largest consumer.
//!
//! The domain types in [`super::model`] are deliberately not `Serialize`. A check carries
//! `&'static str` ids and knows nothing about the baseline; the wire types here carry what
//! a *consumer* needs, which is a superset: whether each violation is inherited debt, and
//! where in the file it sits.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use super::baseline::{Baseline, Diff};
use super::model::Check;
use crate::report::Span;

#[derive(Debug, Serialize)]
pub struct LintReport {
    pub gate: Gate,
    pub checks: Vec<CheckOut>,
}

/// The verdict. This is the part a consumer must not recompute.
#[derive(Debug, Serialize)]
pub struct Gate {
    /// Whether the run agrees with the committed baseline.
    ///
    /// Reflects the gate, not the process exit code: `--warn-only` exits 0 on a failing
    /// gate by design, and a consumer asking "is this corpus in the state it claims" wants
    /// the verdict rather than the invocation's mood.
    pub passed: bool,
    /// Error-severity violations absent from the baseline. These fail CI.
    pub new_violations: usize,
    /// Error-severity violations the baseline already records. These do not fail CI —
    /// they are the inherited debt the ratchet exists to hold still.
    pub baselined_violations: usize,
    /// Baseline entries with no corresponding violation. These fail CI too: a baseline
    /// permitted to be wrong drifts, and one that over-lists silently re-permits what it
    /// over-lists.
    pub stale_baseline_entries: Vec<StaleEntry>,
    /// Entries that have outlived the expiry the baseline declares, and so no longer
    /// forgive the violation they list. These fail CI.
    ///
    /// Separate from `new_violations` although both fail: an introduced violation is
    /// something this change did, an expired one is something the repository agreed to
    /// deal with and has not, and a consumer telling somebody they introduced a finding
    /// that has sat in their baseline for two hundred commits would be wrong.
    pub expired_baseline_entries: Vec<ExpiredEntry>,
}

#[derive(Debug, Serialize)]
pub struct ExpiredEntry {
    pub check: String,
    pub node: String,
    /// Corpus-touching commits it has stood for.
    pub commits: usize,
}

#[derive(Debug, Serialize)]
pub struct StaleEntry {
    pub check: String,
    pub node: String,
}

#[derive(Debug, Serialize)]
pub struct CheckOut {
    pub id: &'static str,
    pub title: &'static str,
    /// `error` | `warn` | `info` — the severity the check is *declared* at.
    ///
    /// **Not necessarily the severity of its findings.** A dated finding can escalate past
    /// this on residence time, so a consumer deciding how loudly to render one must read
    /// [`ViolationOut::severity`]. This field says what the check is for; that one says
    /// what happened.
    pub severity: &'static str,
    pub rationale: &'static str,
    pub violations: Vec<ViolationOut>,
}

#[derive(Debug, Serialize)]
pub struct ViolationOut {
    /// The violation's identity, exactly as the baseline compares it. Unchanged by the
    /// presence of `span`.
    pub node: String,
    pub detail: String,
    /// This finding's own severity — the check's, unless residence time escalated it.
    ///
    /// Only `error` gates and only `error` is ever baselined, and after ageing that is a
    /// question about the finding rather than about the check it belongs to.
    pub severity: &'static str,
    /// How long the condition has held. Present only for corpus-state findings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub age: Option<AgeOut>,
    /// Whether the committed baseline already records this violation.
    ///
    /// **Only meaningful when `severity` is `error`.** The baseline records error-severity
    /// violations and nothing else, so a `warn` or `info` violation is always `false` here
    /// — it is not a regression, it simply never gates. A consumer deciding how loudly to
    /// render a finding must read `severity` and this field together.
    pub in_baseline: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
}

/// How long a corpus-state finding has held.
///
/// Commits rather than days, deliberately: a day count is a function of when the report was
/// run, so the same corpus answers differently tomorrow and no consumer can cache or pin
/// it. A commit count is a function of HEAD. `first_commit` is the full sha of the commit
/// at which the condition began, so a consumer can `git show` it.
#[derive(Debug, Serialize)]
pub struct AgeOut {
    pub first_commit: String,
    /// The `YYYY-MM-DD` of that commit — the same day the text report prints.
    pub first_day: String,
    /// Corpus-touching commits it has held for, counting the one at HEAD.
    pub commits: usize,
}

/// Split a `path:line` node into its parts.
///
/// Three checks encode a line in the identity itself — the prose-link and annotation
/// checks — because a file can trip them many times and the path alone would collapse
/// those into one. That line is exact, so it is the best span available and costs nothing
/// to recover.
fn node_line(node: &str) -> Option<(&str, usize)> {
    let (path, line) = node.rsplit_once(':')?;
    Some((path, line.parse().ok()?))
}

/// Best-effort location for a violation. Never part of its identity.
fn span_for(root: &Path, node: &str, detail: &str) -> Option<Span> {
    if let Some((_, line)) = node_line(node) {
        return Some(Span { line });
    }
    // Otherwise: find the first line naming the thing the detail complains about.
    let needle = detail_needle(detail)?;
    let text = std::fs::read_to_string(root.join(node)).ok()?;
    text.lines()
        .position(|l| l.contains(needle))
        .map(|i| Span { line: i + 1 })
}

/// The token in a violation's detail that identifies where in the file to look.
///
/// Two conventions across the checks, both honoured: most quote the offending value in
/// backticks, and the older ones append it after a colon (`target does not exist:
/// ../gone.yml`). The colon form matters more than it looks — `dangling-edge` uses it, and
/// a broken edge is the finding an editor most wants to put a squiggle under.
fn detail_needle(detail: &str) -> Option<&str> {
    if let Some(quoted) = detail.split('`').nth(1) {
        if !quoted.is_empty() {
            return Some(quoted);
        }
    }
    let tail = detail.rsplit_once(": ").map(|(_, t)| t.trim())?;
    // A trailing sentence is not a locator; a path-shaped token is.
    if tail.is_empty() || tail.contains(' ') {
        return None;
    }
    Some(tail)
}

/// Which of a check's violations the baseline already records.
///
/// Multiset, matching [`super::baseline::diff`] exactly: a node tripping the same check
/// twice against one baseline entry is one debt and one regression, not two of either. Any
/// other accounting here would disagree with the gate it is reporting on.
fn baselined_flags(check: &Check, baseline: &Baseline) -> Vec<bool> {
    let mut pool: Vec<&str> = baseline
        .violations
        .get(check.id)
        .map(|entries| entries.iter().map(|e| e.node.as_str()).collect())
        .unwrap_or_default();
    check
        .violations
        .iter()
        .map(|v| {
            // Only a finding that gates can be in the baseline, and which findings gate is
            // now a per-violation question — see `Check::severity_of`.
            if !check.gates(v) {
                return false;
            }
            match pool.iter().position(|n| *n == v.node) {
                Some(i) => {
                    pool.remove(i);
                    true
                }
                None => false,
            }
        })
        .collect()
}

pub fn build(root: &Path, all: &[Check], baseline: &Baseline, d: &Diff) -> LintReport {
    let mut flags: BTreeMap<&str, Vec<bool>> = BTreeMap::new();
    for check in all {
        flags.insert(check.id, baselined_flags(check, baseline));
    }

    let baselined_violations: usize = flags.values().flatten().filter(|b| **b).count();

    let checks = all
        .iter()
        .map(|check| {
            let f = &flags[check.id];
            CheckOut {
                id: check.id,
                title: check.title,
                severity: check.severity.as_str(),
                rationale: check.rationale,
                violations: check
                    .violations
                    .iter()
                    .enumerate()
                    .map(|(i, v)| ViolationOut {
                        node: v.node.clone(),
                        detail: v.detail.clone(),
                        severity: check.severity_of(v).as_str(),
                        age: v.age.as_ref().map(|a| AgeOut {
                            first_commit: a.sha.clone(),
                            first_day: crate::cmd::export::unix_to_iso(a.ts as u64)
                                .split('T')
                                .next()
                                .unwrap_or_default()
                                .to_string(),
                            commits: a.commits,
                        }),
                        in_baseline: f[i],
                        span: span_for(root, &v.node, &v.detail),
                    })
                    .collect(),
            }
        })
        .collect();

    LintReport {
        gate: Gate {
            passed: d.is_clean(),
            new_violations: d.introduced.len(),
            baselined_violations,
            stale_baseline_entries: d
                .resolved
                .iter()
                .map(|(check, node)| StaleEntry {
                    check: check.clone(),
                    node: node.clone(),
                })
                .collect(),
            expired_baseline_entries: d
                .expired
                .iter()
                .map(|e| ExpiredEntry {
                    check: e.check.clone(),
                    node: e.node.clone(),
                    commits: e.commits,
                })
                .collect(),
        },
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::super::baseline::Entry;
    use super::super::model::{Severity, Violation};
    use super::*;

    fn err_check(id: &'static str, nodes: &[&str]) -> Check {
        Check::new(
            id,
            "T",
            Severity::Error,
            "R",
            nodes
                .iter()
                .map(|n| Violation::new(*n, "`x` broke"))
                .collect(),
        )
    }

    fn baseline_with(id: &str, nodes: &[&str]) -> Baseline {
        let mut b = Baseline::default();
        b.violations.insert(
            id.to_string(),
            nodes
                .iter()
                .map(|n| Entry {
                    node: n.to_string(),
                    detail: String::new(),
                    since: None,
                })
                .collect(),
        );
        b
    }

    #[test]
    fn in_baseline_is_per_violation_not_per_check() {
        let c = err_check("dangling-edge", &["a.yml", "b.yml"]);
        let b = baseline_with("dangling-edge", &["a.yml"]);
        assert_eq!(baselined_flags(&c, &b), vec![true, false]);
    }

    /// The same accounting the gate uses: two occurrences against one entry is one debt
    /// and one regression.
    #[test]
    fn duplicates_are_matched_as_a_multiset() {
        let c = err_check("orphan-in", &["a.yml", "a.yml"]);
        let b = baseline_with("orphan-in", &["a.yml"]);
        assert_eq!(baselined_flags(&c, &b), vec![true, false]);
    }

    /// The baseline records error severity and nothing else, so a warn violation can
    /// never be in it — and must not be reported as a regression on that account.
    #[test]
    fn warn_violations_are_never_baselined() {
        let c = Check::new(
            "class-asserts-purpose",
            "T",
            Severity::Warn,
            "R",
            vec![Violation::new("a.yml", "x")],
        );
        let b = baseline_with("class-asserts-purpose", &["a.yml"]);
        assert_eq!(baselined_flags(&c, &b), vec![false]);
    }

    #[test]
    fn a_line_bearing_node_yields_an_exact_span() {
        assert_eq!(node_line("docs/x.md:14"), Some(("docs/x.md", 14)));
        assert_eq!(node_line("a.yml"), None);
        // A hash is not a path:line.
        assert_eq!(node_line("deadbeef"), None);
    }

    #[test]
    fn span_falls_back_to_searching_for_the_quoted_token() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(
            tmp.path().join("n.yml"),
            "class: c\nlabel: L\nlinks:\n  - target: ../gone.yml\n",
        )
        .unwrap();
        let s = span_for(tmp.path(), "n.yml", "`../gone.yml` does not resolve");
        assert_eq!(s, Some(Span { line: 4 }));
    }

    /// `dangling-edge` names its target after a colon rather than in backticks.
    #[test]
    fn the_colon_convention_locates_a_broken_edge() {
        assert_eq!(
            detail_needle("target does not exist: ../gone.yml"),
            Some("../gone.yml")
        );
        // Backticks still win when both are present.
        assert_eq!(detail_needle("`a.yml` missing: whatever"), Some("a.yml"));
        // A prose tail is not a locator.
        assert_eq!(detail_needle("this is: a whole sentence"), None);
        assert_eq!(detail_needle("no locator here"), None);
    }

    #[test]
    fn span_is_absent_rather_than_guessed() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(span_for(tmp.path(), "missing.yml", "`x` broke"), None);
        std::fs::write(tmp.path().join("n.yml"), "nothing relevant\n").unwrap();
        assert_eq!(span_for(tmp.path(), "n.yml", "`absent` broke"), None);
    }
}
