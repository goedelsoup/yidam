//! How old a catalog record is, and whether the corpus asked to be told.
//!
//! `docs/domain-computer.md` specifies connector behaviour that nothing enforced:
//!
//! > May fail; results cached locally and refreshed on TTL or on demand
//!
//! There was no TTL. A catalog entry recorded where something came from and never recorded
//! that the record had aged, so a corpus resting on a source fetched a year ago read exactly
//! like one fetched today — the same shape as everything else this roadmap found: the state
//! exists, and nothing asks about it.
//!
//! # Days, and this is the one place that counts them
//!
//! Every other clock here counts commits, and `GRAPH.md` argues why: a day count is a
//! function of when you ran the report, so the same corpus answers differently tomorrow and
//! nothing can pin or cache it. That argument is about **corpus state** — how long a node
//! has gone uncited is a fact about the repository, and the repository's clock is `HEAD`.
//!
//! A source's staleness is not a fact about the repository. A statute does not become stale
//! because you committed, and a gauge record does not stay fresh because you did not. So this
//! counts days, it does answer differently tomorrow, and that is the entire point of a TTL
//! rather than a defect in it.
//!
//! # It does not check upstream, and must not
//!
//! `doctor` writes nothing and does no network. Everything here is computed from what is
//! recorded — the entry's own `retrieved:`, or failing that the commit that last touched its
//! file. Whether the source *actually* changed is not knowable from in here, and an expiry
//! does not claim it did. It claims nobody has looked.

use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

/// Where an entry's age came from.
///
/// Reported, never inferred away. The two are different evidence and a reader is owed the
/// difference: one is the author saying when they fetched it, the other is git saying when
/// the file last changed — which counts a typo fix as a refresh and therefore errs in the
/// flattering direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Dated {
    /// The entry's own `retrieved:` field.
    Declared,
    /// The commit that last touched the entry's file.
    Committed,
}

impl Dated {
    pub fn as_str(self) -> &'static str {
        match self {
            Dated::Declared => "declared",
            Dated::Committed => "from git",
        }
    }
}

/// One catalog entry's age and what the corpus asked about it.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Age {
    /// Repo-relative path of the entry.
    pub entry: String,
    /// `YYYY-MM-DD`, or `None` when neither the entry nor git could say.
    pub retrieved: Option<String>,
    pub dated: Option<Dated>,
    /// Days since [`Self::retrieved`], or `None` when there is no date to count from.
    pub age_days: Option<i64>,
    /// The TTL that applies: the entry's own, else the corpus default.
    pub ttl_days: Option<u32>,
}

impl Age {
    /// Days past expiry — positive when expired, `None` when nothing can be said.
    ///
    /// `None` covers the two honest silences: no TTL applies, or no date could be found.
    /// They are different states and the report distinguishes them; both mean this entry is
    /// not a finding.
    pub fn overdue_days(&self) -> Option<i64> {
        let ttl = self.ttl_days? as i64;
        let over = self.age_days? - ttl;
        (over > 0).then_some(over)
    }

    /// Whether a TTL applies and no date could be found to measure it against.
    ///
    /// Its own state rather than an expiry. A corpus that asked to be told when a record
    /// aged, and cannot be told, has a gap in its bookkeeping and not a stale source — and
    /// reporting the second would be asserting something nobody knows.
    pub fn undatable(&self) -> bool {
        self.ttl_days.is_some() && self.age_days.is_none()
    }
}

/// Days between two `YYYY-MM-DD` dates, or `None` if either is unparseable.
fn days_between(from: &str, to: &str) -> Option<i64> {
    Some(crate::dates::days_from_civil_str(to)? - crate::dates::days_from_civil_str(from)?)
}

/// The last commit date of every path under `dir`, as `YYYY-MM-DD`.
///
/// One `git log` over the whole directory rather than one per entry: a catalog of thirty
/// sources would otherwise be thirty subprocesses on a path `doctor` runs every time.
pub fn committed_dates(root: &Path, dir: &Path) -> HashMap<String, String> {
    let rel = dir.strip_prefix(root).unwrap_or(dir);
    let Ok(out) = Command::new("git")
        .current_dir(root)
        .args([
            "log",
            "--date=short",
            "--format=%x00%ad",
            "--name-only",
            "--",
            &rel.to_string_lossy(),
        ])
        .output()
    else {
        return HashMap::new();
    };
    if !out.status.success() {
        return HashMap::new();
    }
    let mut dates: HashMap<String, String> = HashMap::new();
    let mut current = String::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        if let Some(date) = line.strip_prefix('\0') {
            current = date.to_string();
            continue;
        }
        if line.trim().is_empty() || current.is_empty() {
            continue;
        }
        // Newest first, so the first sighting of a path is its last commit.
        dates
            .entry(line.to_string())
            .or_insert_with(|| current.clone());
    }
    dates
}

/// Resolve every entry's age against `today`.
///
/// `today` is passed in rather than read here so that a report is testable and a golden is
/// stable — the one thing a wall-clock feature must not do is make its own tests depend on
/// the day they run.
pub fn ages(
    sources: &[super::checks::Source],
    committed: &HashMap<String, String>,
    default_ttl: Option<u32>,
    today: &str,
) -> Vec<Age> {
    sources
        .iter()
        .map(|s| {
            let (retrieved, dated) = match s.retrieved.as_deref().filter(|d| !d.trim().is_empty()) {
                Some(d) => (Some(d.to_string()), Some(Dated::Declared)),
                None => match committed.get(&s.rel) {
                    Some(d) => (Some(d.clone()), Some(Dated::Committed)),
                    None => (None, None),
                },
            };
            let age_days = retrieved
                .as_deref()
                .and_then(|from| days_between(from, today))
                // A date in the future is not an age. It is a typo or a clock, and
                // reporting a negative age as freshness would silence the entry forever.
                .filter(|d| *d >= 0);
            Age {
                entry: s.rel.clone(),
                retrieved,
                dated,
                age_days,
                ttl_days: s.ttl_days.or(default_ttl),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(
        rel: &str,
        retrieved: Option<&str>,
        ttl: Option<u32>,
    ) -> super::super::checks::Source {
        super::super::checks::Source {
            rel: rel.to_string(),
            path: std::path::PathBuf::from(rel),
            obtained: true,
            used_by: vec![],
            locations: vec![],
            retrieved: retrieved.map(str::to_string),
            ttl_days: ttl,
            artifacts: Vec::new(),
        }
    }

    #[test]
    fn days_are_counted_across_a_leap_year() {
        assert_eq!(days_between("2024-02-28", "2024-03-01"), Some(2));
        assert_eq!(days_between("2023-02-28", "2023-03-01"), Some(1));
        assert_eq!(days_between("2025-01-01", "2026-01-01"), Some(365));
        assert_eq!(days_between("2026-08-26", "2026-08-26"), Some(0));
    }

    #[test]
    fn an_unparseable_date_counts_nothing_rather_than_zero() {
        assert_eq!(days_between("last spring", "2026-08-26"), None);
        assert_eq!(days_between("2026-13-01", "2026-08-26"), None);
        assert_eq!(days_between("2026-08", "2026-08-26"), None);
    }

    /// The declared date wins, and the report says so — the two are different evidence.
    #[test]
    fn a_declared_date_is_preferred_over_the_commit_date() {
        let committed = HashMap::from([("a.md".to_string(), "2020-01-01".to_string())]);
        let a = ages(
            &[source("a.md", Some("2026-01-01"), Some(30))],
            &committed,
            None,
            "2026-08-26",
        );
        assert_eq!(a[0].dated, Some(Dated::Declared));
        assert_eq!(a[0].retrieved.as_deref(), Some("2026-01-01"));
    }

    #[test]
    fn without_a_declared_date_git_answers_and_is_labelled() {
        let committed = HashMap::from([("a.md".to_string(), "2026-01-01".to_string())]);
        let a = ages(
            &[source("a.md", None, Some(30))],
            &committed,
            None,
            "2026-08-26",
        );
        assert_eq!(a[0].dated, Some(Dated::Committed));
        assert!(a[0].overdue_days().unwrap() > 0);
    }

    /// A corpus that declared nothing is told nothing. Absent both TTLs, no entry expires.
    #[test]
    fn with_no_ttl_declared_anywhere_nothing_expires() {
        let committed = HashMap::from([("a.md".to_string(), "1999-01-01".to_string())]);
        let a = ages(
            &[source("a.md", None, None)],
            &committed,
            None,
            "2026-08-26",
        );
        assert_eq!(a[0].overdue_days(), None);
        assert!(!a[0].undatable());
    }

    /// The entry's own TTL beats the corpus default: a statute and a gauge record do not age
    /// at the same rate, which is the whole reason the per-entry form is the primary one.
    #[test]
    fn an_entrys_own_ttl_overrides_the_corpus_default() {
        let committed = HashMap::from([("a.md".to_string(), "2026-01-01".to_string())]);
        let a = ages(
            &[source("a.md", None, Some(10_000))],
            &committed,
            Some(30),
            "2026-08-26",
        );
        assert_eq!(a[0].ttl_days, Some(10_000));
        assert_eq!(a[0].overdue_days(), None, "the entry said it ages slowly");
    }

    #[test]
    fn the_corpus_default_applies_where_an_entry_is_silent() {
        let committed = HashMap::from([("a.md".to_string(), "2026-01-01".to_string())]);
        let a = ages(
            &[source("a.md", None, None)],
            &committed,
            Some(30),
            "2026-08-26",
        );
        assert_eq!(a[0].ttl_days, Some(30));
        assert!(a[0].overdue_days().is_some());
    }

    /// A TTL with nothing to measure it against is its own state, not an expiry. Claiming a
    /// source is stale when nobody knows its age asserts something nobody knows.
    #[test]
    fn a_ttl_with_no_date_is_undatable_rather_than_expired() {
        let a = ages(
            &[source("a.md", None, Some(30))],
            &HashMap::new(),
            None,
            "2026-08-26",
        );
        assert!(a[0].undatable());
        assert_eq!(a[0].overdue_days(), None);
    }

    /// A date in the future is a typo or a clock, and reading it as freshness would silence
    /// the entry permanently — the one direction a staleness check must not fail in.
    #[test]
    fn a_future_date_is_not_treated_as_fresh() {
        let a = ages(
            &[source("a.md", Some("2099-01-01"), Some(30))],
            &HashMap::new(),
            None,
            "2026-08-26",
        );
        assert!(
            a[0].undatable(),
            "a future date leaves the entry unmeasured, not fresh"
        );
    }

    #[test]
    fn an_entry_inside_its_ttl_is_not_overdue() {
        let a = ages(
            &[source("a.md", Some("2026-08-01"), Some(180))],
            &HashMap::new(),
            None,
            "2026-08-26",
        );
        assert_eq!(a[0].overdue_days(), None);
        assert_eq!(a[0].age_days, Some(25));
    }
}
