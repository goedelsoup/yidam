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

use crate::cmd::lint::history::Expectation;
use crate::paths::repo_root;

/// One class's standing at one commit, against what the class declares.
#[derive(serde::Serialize, Clone, Default)]
pub struct ClassRow {
    pub nodes: usize,
    /// Nodes nothing points at *and* that `orphan-in` reports — zero for a source class,
    /// whose instances are exempt.
    pub orphans: usize,
    /// Nodes nothing points at, counted for every class including the exempt ones.
    ///
    /// The number `orphans` cannot carry: a source class reads `0 of 12` there, when what
    /// is true is that all twelve are uncited and that is what the class is *for*. Saying
    /// so is the difference between a report that hides the exemption and one that shows
    /// the model holding.
    pub uncited: usize,
    /// `uncited`, `cited`, or `unstated` — what the class declares about being pointed at.
    pub expectation: &'static str,
    /// Whether the class is behaving as it declared.
    ///
    /// `None` when it declared nothing: there is no expectation to meet, and scoring a
    /// class against one it never stated is how a corpus with an unfilled ontology gets
    /// reported as failing.
    pub meets_expectation: Option<bool>,
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
        let cited = &f.cited;
        for node in f.out.keys() {
            let row = by_class
                .entry(super::lint::history::class_of(node).to_string())
                .or_default();
            row.nodes += 1;
            if !cited.contains(node) {
                row.uncited += 1;
            }
        }
        let mut orphans = 0;
        for node in f.orphans() {
            orphans += 1;
            by_class
                .entry(super::lint::history::class_of(node).to_string())
                .or_default()
                .orphans += 1;
        }
        for (name, row) in by_class.iter_mut() {
            let declared = f.expectations.get(name).copied();
            row.expectation = declared
                .map(|e| e.as_str())
                .unwrap_or(Expectation::Unstated.as_str());
            // Met, per what was declared: a source class expects every instance uncited, a
            // class declaring an inbound edge expects none. A class that declared nothing
            // is scored against nothing.
            row.meets_expectation = declared.map(|e| match e {
                Expectation::Uncited => row.uncited == row.nodes,
                Expectation::Cited => row.uncited == 0,
                Expectation::Unstated => true,
            });
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
    // `Uncited%` and not `Share`: a share of what was the whole ambiguity. This column is
    // the *corrected* series — source classes are excluded from `orphans`, so it is the
    // 7% → 18% figure the measurement document ended up quoting rather than the 22% → 36%
    // it opens with. Naming it is what keeps a reader from re-deriving the reading that
    // document spends a section undoing.
    let mut out = String::from("Date         Commit    Nodes   Uncited   Uncited%\n");
    out.push_str("──────────   ───────   ─────   ───────   ────────\n");
    for r in &shown {
        let pct = if r.nodes == 0 {
            0
        } else {
            r.orphans * 100 / r.nodes
        };
        out.push_str(&format!(
            "{:<10}   {:<7}   {:>5}   {:>7}   {:>7}%\n",
            r.date, r.commit, r.nodes, r.orphans, pct
        ));
    }
    // The two counts here are different populations and both are called "uncited", so the
    // difference is stated rather than left to be inferred. The series counts what
    // `orphan-in` would report; the breakdown counts every uncited node, because a source
    // class reading `0` there would hide the exemption instead of showing it working.
    out.push_str(
        "\nSeries excludes source classes — the classes whose instances nothing is meant to\n\
         point at. The breakdown below counts every uncited node, those included.\n",
    );
    if shown.len() < rows.len() {
        out.push_str(&format!(
            "\n{} of {} commits shown. --every 0 for all; --format json carries every row and \
             the per-class breakdown.\n",
            shown.len(),
            rows.len()
        ));
    }

    // A corpus-wide number cannot say whether a class is behaving as its ontology declared,
    // and that — not the level — is the question. So the breakdown is printed beside the
    // series rather than left to the JSON, and each class is read against what it declared
    // rather than against a corpus average that sums classes with different expectations.
    if let Some(head) = rows.last() {
        let mut named: Vec<(&String, &ClassRow)> = head
            .by_class
            .iter()
            .filter(|(_, c)| c.uncited > 0)
            .collect();
        if !named.is_empty() {
            named.sort_by(|a, b| b.1.uncited.cmp(&a.1.uncited).then(a.0.cmp(b.0)));
            out.push_str("\nUncited at HEAD, by class, against what the class declares\n");
            for (name, c) in named {
                out.push_str(&format!(
                    "  {:<24} {:>3} of {:<3}  {}\n",
                    name,
                    c.uncited,
                    c.nodes,
                    verdict(c)
                ));
            }
        }
    }
    out
}

/// How one class reads against its own declaration.
///
/// The three phrasings are deliberately not gradations of the same thing. A source class at
/// 12 of 12 is the model *working*; a class expecting citation with 13 of 20 is the only
/// one of the three that is a finding; and a class that declared nothing is not being
/// scored at all — saying "no expectation declared" rather than nothing keeps the reader
/// from reading silence as a pass.
fn verdict(c: &ClassRow) -> String {
    match (c.expectation, c.meets_expectation) {
        ("uncited", Some(true)) => "uncited by design — the ontology holding".to_string(),
        ("uncited", _) => format!(
            "declared a source class, and {} of its instances are cited",
            c.nodes - c.uncited
        ),
        ("cited", _) => "declared cited — this is the asymmetry worth reading".to_string(),
        _ => "the class declares no edges, so no expectation to read against".to_string(),
    }
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

    fn class_row(nodes: usize, uncited: usize, expectation: &'static str) -> ClassRow {
        ClassRow {
            nodes,
            orphans: if expectation == "uncited" { 0 } else { uncited },
            uncited,
            expectation,
            meets_expectation: match expectation {
                "uncited" => Some(uncited == nodes),
                "cited" => Some(uncited == 0),
                _ => None,
            },
        }
    }

    fn head_with(classes: &[(&str, ClassRow)]) -> ReplayRow {
        let mut r = row("2026-01-01", 0, 0);
        for (name, c) in classes {
            r.nodes += c.nodes;
            r.orphans += c.orphans;
            r.by_class.insert(name.to_string(), c.clone());
        }
        r
    }

    /// The whole point of the breakdown. Three classes, three readings, and a corpus-wide
    /// number would have summed them into one that means none of them.
    #[test]
    fn each_class_is_read_against_what_it_declared() {
        let head = head_with(&[
            ("person", class_row(3, 3, "uncited")),
            ("band", class_row(2, 1, "cited")),
            ("note", class_row(2, 1, "unstated")),
        ]);
        let out = render(&[head], 0);

        assert!(out.contains("person"), "{out}");
        assert!(
            out.contains("uncited by design — the ontology holding"),
            "a source class at 100% is the model working, not a finding:\n{out}"
        );
        assert!(
            out.contains("this is the asymmetry worth reading"),
            "a class declaring citation with an uncited instance is the finding:\n{out}"
        );
        assert!(
            out.contains("no expectation to read against"),
            "silence must read as silence, not as a pass:\n{out}"
        );
    }

    /// A source class whose instances ARE cited is the ontology being wrong about itself —
    /// the opposite finding, and one no corpus-wide rate can express.
    #[test]
    fn a_source_class_that_is_cited_is_reported_as_such() {
        let head = head_with(&[("person", class_row(4, 3, "uncited"))]);
        let out = render(&[head], 0);
        assert!(
            out.contains("declared a source class, and 1 of its instances are cited"),
            "{out}"
        );
    }

    /// The column reads `Uncited%` because a share of *what* was the ambiguity, and the two
    /// populations on this report are different: the series excludes source classes, the
    /// breakdown does not. A reader must not have to infer that.
    #[test]
    fn the_report_states_which_population_each_count_is_over() {
        let head = head_with(&[("person", class_row(3, 3, "uncited"))]);
        let out = render(&[head], 0);
        assert!(out.contains("Uncited%"), "{out}");
        assert!(out.contains("Series excludes source classes"), "{out}");
        assert!(
            out.contains("counts every uncited node, those included"),
            "{out}"
        );
    }

    /// The series column is the corrected figure: source classes are excluded from
    /// `orphans`, so a corpus that is *entirely* source classes reads 0% rather than 100%.
    /// That is the 22% → 7% correction the measurement document ended up making to itself.
    #[test]
    fn the_series_excludes_source_classes_from_its_percentage() {
        let head = head_with(&[("person", class_row(10, 10, "uncited"))]);
        let out = render(&[head], 0);
        let series = out.lines().nth(2).unwrap();
        assert!(
            series.contains("0%"),
            "every node is uncited by design; the series must not read 100%: {series}"
        );
        // …and the breakdown still says all ten are uncited, so the exemption is visible
        // rather than hidden.
        assert!(out.contains("10 of 10"), "{out}");
    }

    /// A class with nothing uncited is not listed: the breakdown is the exceptions, and a
    /// healthy class in it is a line nobody reads past.
    #[test]
    fn a_class_with_nothing_uncited_is_not_listed() {
        let head = head_with(&[
            ("band", class_row(3, 0, "cited")),
            ("note", class_row(2, 1, "unstated")),
        ]);
        let out = render(&[head], 0);
        assert!(!out.contains("band"), "{out}");
        assert!(out.contains("note"), "{out}");
    }
}
