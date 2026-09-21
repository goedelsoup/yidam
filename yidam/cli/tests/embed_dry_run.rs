//! `yidam embed --dry-run` measures what a push would carry, and writes nothing.
//!
//! It exists because RFC-0033 §8's last open row — whether the 40 KB per-vector metadata
//! ceiling is comfortable for real corpora — could only be answered by running the functions
//! that *compose* the text over corpora that are not ours to write into. `text` is not a
//! field: for a node it is `compose_text` over `prose::text`, which reaches nested keys and
//! flagged properties, and for a catalog source it is the whole markdown body. A grep
//! under-measures both, and the plain `embed` leaves `.yidam/embeddings/` behind.
//!
//! Two properties make the resulting number worth quoting, and each has a test here:
//!
//! 1. **It writes nothing** — not the records, not the directory.
//! 2. **It counts what a real run indexes.** A dry run that measured a different population
//!    than `embed` writes would be a number about nothing. The second test runs both over the
//!    same corpus and holds the counts equal.

mod common;

/// The population a dry run reports is the population a real run writes.
///
/// Compared against the real command's own output rather than against a re-derived list: the
/// walk, the parse-failure skips and the empty-source skips are all in the code under test,
/// and a second implementation of them here would agree with itself instead of with `embed`.
#[test]
fn a_dry_run_counts_what_a_real_run_writes() {
    let examples = common::examples();
    assert!(
        !examples.is_empty(),
        "no example corpora were discovered — this guard would pass having checked nothing"
    );

    let mut checked = 0usize;
    for name in &examples {
        let example = common::Example::materialize(name);
        let (dry, err, code) = example.run(&["embed", "--dry-run"]);
        assert_eq!(code, 0, "`yidam embed --dry-run` failed in {name}:\n{err}");

        let embeddings = example.path().join(".yidam/embeddings");
        assert!(
            !embeddings.exists(),
            "`--dry-run` created {} in {name}",
            embeddings.display()
        );

        let (nodes, sources) = counts(&dry, name);
        assert!(nodes > 0, "no node rows reported in {name}:\n{dry}");

        let (_out, err, code) = example.run(&["embed"]);
        assert_eq!(code, 0, "`yidam embed` failed in {name}:\n{err}");
        let (written_nodes, written_sources) = written(&example.path());
        assert_eq!(
            (nodes, sources),
            (written_nodes, written_sources),
            "the dry run and the real run disagree about {name}"
        );
        checked += 1;
    }
    assert!(checked > 0);
}

/// Both ceilings are named with their own figures, and neither is reported as breached.
///
/// The `> 40960` bucket is zero by construction — `request::metadata` cuts until a row fits —
/// so a non-zero count there would mean the shrink loop returned something it should have
/// refused rather than that a corpus grew.
#[test]
fn the_report_names_both_ceilings_and_breaches_neither() {
    let example = common::Example::materialize(&common::examples()[0]);
    let (dry, err, code) = example.run(&["embed", "--dry-run"]);
    assert_eq!(code, 0, "{err}");

    for expected in [
        "40960-byte ceiling", // the per-vector metadata ceiling
        "largest row:",
        "of 2048", // the filterable ceiling
        "truncated:",
        "Nothing was written.",
    ] {
        assert!(dry.contains(expected), "no {expected:?} in:\n{dry}");
    }

    let over = dry
        .lines()
        .find(|l| l.trim_start().starts_with("40960 < b"))
        .unwrap_or_else(|| panic!("no over-ceiling bucket in:\n{dry}"));
    assert!(
        over.split_whitespace().nth(3) == Some("0"),
        "a row was reported over the ceiling, which the shrink loop rules out: {over}"
    );
}

/// `rows: N (A node, B source…)` → `(A, B)`.
fn counts(report: &str, name: &str) -> (usize, usize) {
    let line = report
        .lines()
        .find(|l| l.trim_start().starts_with("rows:"))
        .unwrap_or_else(|| panic!("no rows line in the {name} report:\n{report}"));
    let field = |unit: &str| -> usize {
        let words: Vec<&str> = line.split_whitespace().collect();
        words
            .iter()
            .position(|w| w.trim_end_matches([',', ';', ')']) == unit)
            .and_then(|i| words[i - 1].trim_start_matches('(').parse().ok())
            .unwrap_or_else(|| panic!("no {unit} count in: {line}"))
    };
    (field("node"), field("source"))
}

/// The records `yidam embed` wrote, split the way the report splits them: `_catalog/` holds
/// the sources and every sibling directory is a class.
fn written(root: &std::path::Path) -> (usize, usize) {
    let dir = root.join(".yidam/embeddings");
    let mut nodes = 0usize;
    let mut sources = 0usize;
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "json") {
                let text = std::fs::read_to_string(&p).unwrap();
                let rec: serde_json::Value = serde_json::from_str(&text).unwrap();
                match rec["kind"].as_str() {
                    Some("source") => sources += 1,
                    _ => nodes += 1,
                }
            }
        }
    }
    (nodes, sources)
}
