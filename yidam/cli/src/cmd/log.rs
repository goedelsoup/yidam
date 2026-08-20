//! `yidam log` — the testimony view. RFC-0015.
//!
//! The epistemic/operational split is foundational rather than incidental: `GRAPH.md`
//! devotes a section to it, and `SCRIPTURE.md` is explicit that a commit message is
//! *testimony* — "a record of a change in understanding", and "two kinds of events, no
//! others."
//!
//! The classifier that decides which is which is one of the parity-certified functions,
//! implemented and fixtured in all three SDKs. Its only consumer was `yidam backfill`,
//! which classifies commits solely to decide which become decision records. So the moment
//! a corpus does real pipeline work — extraction, connector refreshes, bundle regeneration,
//! all legitimately operational — the testimony is buried under infrastructure churn in
//! `git log`, with a certified classifier sitting one command away from surfacing it.
//!
//! **No new classification logic lives here.** This consumes
//! [`yidam_core::git::classify_commit`], so the CLI view and the SDKs agree on what
//! "epistemic" means by construction rather than by docstring — which is precisely the
//! divergence a downstream re-implementation already demonstrated.

use anyhow::{Context, Result};
use serde::Serialize;

use yidam_core::git::{classify_commit, CommitKind};

use crate::paths::repo_root;
use crate::report::Format;

/// Which events to show.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    /// Both, each line tagged. The default.
    ///
    /// RFC-0015 leans this way and says so twice: testimony is a *discoverable flag*
    /// rather than a hidden default. A `log` that silently omits half a repository's
    /// history would surprise anyone who has used the command it is named after, and the
    /// surprise would land on whoever is least familiar with the split — which is the
    /// reader this view exists for.
    #[default]
    All,
    Epistemic,
    Operational,
}

impl Filter {
    fn admits(self, kind: &CommitKind) -> bool {
        match self {
            Filter::All => true,
            Filter::Epistemic => matches!(kind, CommitKind::Epistemic),
            Filter::Operational => matches!(kind, CommitKind::Operational),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct LogEntry {
    pub hash: String,
    pub short: String,
    /// `epistemic` | `operational`.
    pub kind: &'static str,
    /// The leading verb, or empty when the subject carries none.
    pub verb: String,
    pub subject: String,
    pub author: String,
    /// Author date, ISO-8601.
    pub date: String,
}

#[derive(Debug, Serialize)]
pub struct LogReport {
    /// The revision range walked, as given to git.
    pub range: String,
    /// Which filter produced this: `all` | `epistemic` | `operational`.
    pub filter: &'static str,
    /// Totals over the range *before* filtering, so a filtered view still says what it
    /// left out. A testimony view that cannot tell you how much churn it hid is only
    /// half a view.
    pub total: usize,
    pub epistemic: usize,
    pub operational: usize,
    pub entries: Vec<LogEntry>,
}

/// Read and classify every commit in `range`.
pub fn collect(root: &std::path::Path, range: &str, filter: Filter) -> Result<LogReport> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["log", "--format=%H\x1f%s\x1f%an\x1f%aI\x1e", range])
        .output()
        .context("running git log")?;
    if !out.status.success() {
        anyhow::bail!(
            "git log failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let text = String::from_utf8(out.stdout).context("git log output is not UTF-8")?;
    Ok(build(range, filter, &text))
}

/// Classify a `\x1e`-delimited git log block. Split out so it is testable without a repo.
pub(crate) fn build(range: &str, filter: Filter, log: &str) -> LogReport {
    let mut entries = Vec::new();
    let (mut epistemic, mut operational, mut total) = (0usize, 0usize, 0usize);

    for block in log.split('\x1e').map(str::trim).filter(|b| !b.is_empty()) {
        let mut f = block.splitn(4, '\x1f');
        let (Some(hash), Some(subject), Some(author), Some(date)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };
        let hash = hash.trim();
        let subject = subject.trim();
        if hash.is_empty() {
            continue;
        }

        let event = classify_commit(hash, subject);
        total += 1;
        match event.kind {
            CommitKind::Epistemic => epistemic += 1,
            CommitKind::Operational => operational += 1,
        }
        if !filter.admits(&event.kind) {
            continue;
        }
        entries.push(LogEntry {
            short: hash[..hash.len().min(8)].to_string(),
            hash: hash.to_string(),
            kind: match event.kind {
                CommitKind::Epistemic => "epistemic",
                CommitKind::Operational => "operational",
            },
            verb: event.verb,
            subject: subject.to_string(),
            author: author.trim().to_string(),
            date: date.trim().to_string(),
        });
    }

    LogReport {
        range: range.to_string(),
        filter: match filter {
            Filter::All => "all",
            Filter::Epistemic => "epistemic",
            Filter::Operational => "operational",
        },
        total,
        epistemic,
        operational,
        entries,
    }
}

pub(crate) fn render_text(r: &LogReport) -> String {
    if r.entries.is_empty() {
        return format!("No commits in {} matching the filter.", r.range);
    }
    let mut out = String::new();
    for e in &r.entries {
        let tag = match e.kind {
            "epistemic" => "E",
            _ => "O",
        };
        out.push_str(&format!("{}  [{}]  {}\n", e.short, tag, e.subject));
    }
    // What the view left out, so a filtered log is honest about being filtered.
    out.push_str(&format!(
        "\n{} commit(s): {} epistemic, {} operational",
        r.total, r.epistemic, r.operational
    ));
    if r.filter != "all" {
        out.push_str(&format!(" — showing {}", r.filter));
    }
    out
}

pub fn log(range: Option<String>, filter: Filter, format: Format) -> Result<()> {
    let root = repo_root()?;
    let range = range.unwrap_or_else(|| "HEAD".to_string());
    let report = collect(&root, &range, filter)?;

    if format.is_json() {
        return crate::report::emit(&root, report);
    }
    println!("{}", render_text(&report));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOG: &str = "\
aaaaaaaa1111\x1festablish: confounding as a corpus concept\x1fA\x1f2026-01-01T00:00:00Z\x1e\
bbbbbbbb2222\x1fregen: REGEN blocks refreshed\x1fB\x1f2026-01-02T00:00:00Z\x1e\
cccccccc3333\x1frevise: the figure was wrong\x1fA\x1f2026-01-03T00:00:00Z\x1e";

    #[test]
    fn both_kinds_are_shown_by_default_and_tagged() {
        let r = build("HEAD", Filter::All, LOG);
        assert_eq!(r.entries.len(), 3);
        assert_eq!((r.total, r.epistemic, r.operational), (3, 2, 1));
        let text = render_text(&r);
        assert!(text.contains("aaaaaaaa  [E]  establish:"), "{text}");
        assert!(text.contains("bbbbbbbb  [O]  regen:"), "{text}");
    }

    #[test]
    fn the_testimony_filter_keeps_only_epistemic_events() {
        let r = build("HEAD", Filter::Epistemic, LOG);
        assert_eq!(r.entries.len(), 2);
        assert!(r.entries.iter().all(|e| e.kind == "epistemic"));
        assert_eq!(r.entries[0].verb, "establish");
    }

    #[test]
    fn the_operational_filter_is_the_inverse() {
        let r = build("HEAD", Filter::Operational, LOG);
        assert_eq!(r.entries.len(), 1);
        assert_eq!(r.entries[0].verb, "regen");
    }

    /// A filtered view still reports what it hid. Counting after the filter would make
    /// `--epistemic` claim the repository has no operational history at all.
    #[test]
    fn totals_count_the_range_not_the_filtered_result() {
        let r = build("HEAD", Filter::Epistemic, LOG);
        assert_eq!((r.total, r.epistemic, r.operational), (3, 2, 1));
        assert!(render_text(&r).contains("3 commit(s): 2 epistemic, 1 operational"));
        assert!(render_text(&r).contains("showing epistemic"));
    }

    /// Classification is not re-derived here — an unrecognized verb falls through to
    /// Epistemic, which is the totality the Dafny spec proves and the SDKs share.
    #[test]
    fn an_unknown_verb_follows_the_certified_default() {
        let log =
            "dddddddd4444\x1fviewport: charts that cannot draw\x1fA\x1f2026-01-04T00:00:00Z\x1e";
        let r = build("HEAD", Filter::All, log);
        assert_eq!(r.entries[0].kind, "epistemic");
    }

    #[test]
    fn an_empty_range_says_so_rather_than_printing_nothing() {
        let r = build("v1..v1", Filter::All, "");
        assert_eq!(r.entries.len(), 0);
        assert!(render_text(&r).contains("No commits in v1..v1"));
    }

    #[test]
    fn a_malformed_block_is_skipped_rather_than_panicking() {
        let r = build("HEAD", Filter::All, "not-a-block\x1e\x1e");
        assert_eq!(r.entries.len(), 0);
    }
}
