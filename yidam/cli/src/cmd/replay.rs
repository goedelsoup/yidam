//! `yidam replay` — corpus health, reconstructed across the repository's whole history.
//!
//! Every gate in this repository answers about *now*. The harness scores the genesis commit
//! and `lint` scores the working tree, so the question a maintainer actually asks — *is this
//! corpus getting better or worse?* — had no answer that was not assembled by hand.
//!
//! It was assembled by hand once, and the assembling is what found the thing worth finding:
//! one derived repository's orphan rate climbing 22% → 36% across its life while another
//! held zero, and then, on decomposition, that almost all of the gap was ontology shape and
//! the real signal was twelve `recording` nodes arriving in a single sweep. None of that is
//! visible in a snapshot. All of it is visible in a series.
//!
//! The series is a fold over [`super::lint::history::replay`] — the same walk that dates
//! `orphan-in` findings — so a row here and a finding there cannot disagree about what the
//! graph looked like on a given day.

use anyhow::Result;
use std::collections::BTreeMap;

use crate::paths::repo_root;

/// One class's standing at one commit.
#[derive(serde::Serialize, Clone, Default)]
pub struct ClassRow {
    pub nodes: usize,
    pub orphans: usize,
}

/// The corpus at one commit that touched it.
#[derive(serde::Serialize)]
pub struct ReplayRow {
    pub commit: String,
    pub date: String,
    pub nodes: usize,
    /// Nodes nothing points at, source classes excluded — the same population `orphan-in`
    /// reports, counted as of this commit.
    pub orphans: usize,
    /// Per class. The corpus-wide number is the one that misleads: it sums classes nothing
    /// is meant to point at with classes whose other instances are cited, and only the
    /// second is a finding.
    pub by_class: BTreeMap<String, ClassRow>,
}

#[derive(serde::Serialize)]
struct ReplayReport<'a> {
    replay: &'a [ReplayRow],
}

/// Reconstruct the series. One row per commit that touched the corpus.
pub(crate) fn collect(root: &std::path::Path) -> Vec<ReplayRow> {
    let mut rows = Vec::new();
    super::lint::history::replay(root, |f| {
        let mut by_class: BTreeMap<String, ClassRow> = BTreeMap::new();
        for node in f.out.keys() {
            by_class
                .entry(super::lint::history::class_of(node).to_string())
                .or_default()
                .nodes += 1;
        }
        let mut orphans = 0;
        for node in f.orphans() {
            orphans += 1;
            by_class
                .entry(super::lint::history::class_of(node).to_string())
                .or_default()
                .orphans += 1;
        }
        let iso = crate::cmd::export::unix_to_iso(f.ts.max(0) as u64);
        rows.push(ReplayRow {
            commit: f.sha.chars().take(7).collect(),
            date: iso.split('T').next().unwrap_or(&iso).to_string(),
            nodes: f.out.len(),
            orphans,
            by_class,
        });
    });
    rows
}

/// Keep `n` rows, evenly spaced, always including the first and last.
///
/// A repository with 200 corpus commits prints 200 rows, which is a series nobody reads. The
/// sample is of the *rendering* and not of the walk: every commit is still replayed, so a
/// sampled row carries the true state at that commit rather than an interpolation.
fn sample(rows: &[ReplayRow], n: usize) -> Vec<&ReplayRow> {
    if n == 0 || rows.len() <= n {
        return rows.iter().collect();
    }
    let last = rows.len() - 1;
    (0..n)
        .map(|i| &rows[i * last / (n - 1)])
        .collect::<Vec<_>>()
}

pub(crate) fn render(rows: &[ReplayRow], every: usize) -> String {
    if rows.is_empty() {
        return "No corpus history (no commits touching .yidam/corpus/).".to_string();
    }
    let shown = sample(rows, every);
    let mut out = String::from("Date         Commit    Nodes   Orphans   Share\n");
    out.push_str("──────────   ───────   ─────   ───────   ─────\n");
    for r in &shown {
        let pct = if r.nodes == 0 {
            0
        } else {
            r.orphans * 100 / r.nodes
        };
        out.push_str(&format!(
            "{:<10}   {:<7}   {:>5}   {:>7}   {:>4}%\n",
            r.date, r.commit, r.nodes, r.orphans, pct
        ));
    }
    if shown.len() < rows.len() {
        out.push_str(&format!(
            "\n{} of {} commits shown. --every 0 for all; --format json carries every row and \
             the per-class breakdown.\n",
            shown.len(),
            rows.len()
        ));
    }

    // The corpus-wide share is the number that misled, so the breakdown that corrects it is
    // printed beside it rather than left to the JSON.
    if let Some(head) = rows.last() {
        let mut named: Vec<(&String, &ClassRow)> = head
            .by_class
            .iter()
            .filter(|(_, c)| c.orphans > 0)
            .collect();
        if !named.is_empty() {
            named.sort_by(|a, b| b.1.orphans.cmp(&a.1.orphans).then(a.0.cmp(b.0)));
            out.push_str("\nUncited at HEAD, by class\n");
            for (name, c) in named {
                out.push_str(&format!("  {:<24} {} of {}\n", name, c.orphans, c.nodes));
            }
        }
    }
    out
}

pub fn replay(format: crate::report::Format, every: usize) -> Result<()> {
    let root = repo_root()?;
    let rows = collect(&root);
    if format.is_json() {
        return crate::report::emit(&root, ReplayReport { replay: &rows });
    }
    print!("{}", render(&rows, every));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(date: &str, nodes: usize, orphans: usize) -> ReplayRow {
        ReplayRow {
            commit: "abc1234".into(),
            date: date.into(),
            nodes,
            orphans,
            by_class: BTreeMap::new(),
        }
    }

    #[test]
    fn an_empty_history_says_so() {
        assert!(render(&[], 8).contains("No corpus history"));
    }

    /// The ends are what a trend is read from, so neither is ever the row dropped.
    #[test]
    fn sampling_keeps_the_first_and_last() {
        let rows: Vec<ReplayRow> = (0..50)
            .map(|i| row(&format!("2026-01-{i:02}"), i, 0))
            .collect();
        let s = sample(&rows, 5);
        assert_eq!(s.len(), 5);
        assert_eq!(s[0].date, rows[0].date);
        assert_eq!(s[4].date, rows[49].date);
    }

    #[test]
    fn a_short_series_is_not_sampled() {
        let rows: Vec<ReplayRow> = (0..3).map(|i| row("2026-01-01", i, 0)).collect();
        assert_eq!(sample(&rows, 8).len(), 3);
        assert_eq!(sample(&rows, 0).len(), 3, "0 means every row");
    }

    /// Dropping rows silently would make a truncated series read as a complete one.
    #[test]
    fn a_sampled_render_says_what_it_dropped() {
        let rows: Vec<ReplayRow> = (0..40).map(|i| row("2026-01-01", i, 0)).collect();
        let text = render(&rows, 5);
        assert!(text.contains("5 of 40 commits shown"), "{text}");
        assert!(
            !render(&rows, 0).contains("commits shown"),
            "nothing dropped"
        );
    }

    #[test]
    fn the_share_is_orphans_over_nodes() {
        let text = render(&[row("2026-01-01", 96, 18)], 0);
        assert!(text.contains("18"), "{text}");
        assert!(text.contains("18%"), "18 of 96 is 18%: {text}");
    }
}
