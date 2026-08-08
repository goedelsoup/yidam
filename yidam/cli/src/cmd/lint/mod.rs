//! `yidam lint` — run every check, compare against the ratchet, report.
//!
//! The gate answers one question: *did this commit make the corpus less clean?* It does
//! not answer *is the corpus clean?*, which for any repository with history is usually no.
//! Conflating the two is what produces a gate that is either permanently red or
//! permanently ignored; see [`baseline`].

mod baseline;
mod checks;
mod commits;
mod model;

use std::collections::HashSet;
use std::path::Path;

use anyhow::Result;

pub use model::{Check, Severity};

use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_md_files, walk_ont_files};

/// How `lint` was invoked.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Report everything, gate on nothing. Escape hatch, not a mode of operation.
    pub warn_only: bool,
    /// Print each check's rationale alongside its findings.
    pub explain: bool,
    /// Also check the git log against the commit vocabulary.
    pub commits: bool,
    /// Restrict the commit check to a revision range (e.g. `main..HEAD`).
    pub range: Option<String>,
    /// Rewrite the baseline from this run instead of gating on it.
    pub bless: bool,
}

/// Run every check against the repository at `root`.
pub fn run_checks(root: &Path, opts: &Options) -> Vec<Check> {
    let corpus_dir = yidam_corpus_dir(root);
    let catalog_dir = yidam_catalog_dir(root);

    let instance_paths = walk_corpus_instances(&corpus_dir);
    let nodes = checks::load_nodes(root, &instance_paths);

    let defined: HashSet<String> = walk_ont_files(&corpus_dir)
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(str::to_string)
        })
        .collect();

    let catalog_paths = walk_md_files(&catalog_dir);
    let sources = checks::load_sources(root, &catalog_paths);
    let node_texts: Vec<String> = instance_paths
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .collect();
    let cites = checks::citations(&sources, &nodes, &node_texts);

    // Tables are checked wherever a reader meets one: catalog entries and the READMEs
    // that carry REGEN blocks.
    let mut prose: Vec<(String, String)> = Vec::new();
    for p in catalog_paths.iter().chain(
        [corpus_dir.join("README.md"), catalog_dir.join("README.md")]
            .iter()
            .filter(|p| p.exists()),
    ) {
        prose.push((
            p.strip_prefix(root)
                .unwrap_or(p)
                .to_string_lossy()
                .to_string(),
            std::fs::read_to_string(p).unwrap_or_default(),
        ));
    }

    let mut all = vec![
        checks::missing_class(&nodes),
        checks::unknown_class(&nodes, &defined),
        checks::orphan_out(&nodes),
        checks::dangling_edge(&nodes),
        checks::catalog_unobtained_but_cited(&sources, &cites),
        checks::missing_label(&nodes),
        checks::missing_description(&nodes),
        checks::catalog_used_by_drift(&sources, &cites),
        checks::catalog_location_malformed(&sources),
        checks::malformed_table(&prose),
        checks::orphan_in(&nodes),
        checks::catalog_uncited(&sources, &cites),
    ];

    if opts.commits {
        let subjects = commits::read_subjects(root, opts.range.as_deref());
        all.push(commits::unrecognized_verb(&subjects));
    }

    all
}

pub fn lint(opts: Options) -> Result<()> {
    let root = repo_root()?;
    let all = run_checks(&root, &opts);

    if opts.bless {
        let b = baseline::Baseline::from_checks(&all);
        let count: usize = b.violations.values().map(|v| v.len()).sum();
        b.write(&root)?;
        println!(
            "blessed {count} error-severity violation(s) into {}",
            baseline::path(&root).display()
        );
        println!(
            "this records the corpus's current state as its inherited debt — it does not fix it"
        );
        return Ok(());
    }

    report(&all, &opts);

    let committed = baseline::Baseline::load(&root)?;
    let d = baseline::diff(&all, &committed);

    if opts.warn_only {
        let n: usize = all.iter().map(|c| c.violations.len()).sum();
        eprintln!("lint: {n} finding(s) (reported, not failing)");
        return Ok(());
    }

    if d.is_clean() {
        let n: usize = all.iter().map(|c| c.violations.len()).sum();
        let errs: usize = all
            .iter()
            .filter(|c| c.severity == Severity::Error)
            .map(|c| c.violations.len())
            .sum();
        if errs > 0 {
            println!("lint: {n} finding(s); {errs} error(s), all baselined — no regression");
        } else {
            println!("lint: {n} finding(s), no errors");
        }
        return Ok(());
    }

    if !d.introduced.is_empty() {
        eprintln!("\nnot in the baseline — introduced by this change:");
        for (check, node) in &d.introduced {
            eprintln!("  [{check}] {node}");
        }
    }
    if !d.resolved.is_empty() {
        eprintln!("\nin the baseline but no longer occurring — the baseline is stale:");
        for (check, node) in &d.resolved {
            eprintln!("  [{check}] {node}");
        }
        eprintln!(
            "\nfixing a violation is good; leaving it listed is not. A baseline permitted to be\n\
             wrong drifts, and one that over-lists silently re-permits what it over-lists."
        );
    }
    eprintln!("\nrun `yidam lint --bless` to record the current state as the new baseline.");
    anyhow::bail!(
        "lint: {} introduced, {} stale",
        d.introduced.len(),
        d.resolved.len()
    )
}

fn report(all: &[Check], opts: &Options) {
    for check in all {
        if check.passed() {
            continue;
        }
        println!(
            "\n{} [{}] {} — {} finding(s)",
            check.severity.as_str().to_uppercase(),
            check.id,
            check.title,
            check.violations.len()
        );
        if opts.explain {
            println!("  {}", check.rationale);
        }
        for v in &check.violations {
            println!("  {}: {}", v.node, v.detail);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    /// A minimal well-formed corpus: two nodes pointing at each other.
    fn clean_repo() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let corpus = tmp.path().join(".yidam/corpus");
        let class = corpus.join("reach");
        fs::create_dir_all(&class).unwrap();
        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        fs::write(
            class.join("alpha.yml"),
            "class: reach\nlabel: Alpha\ndescription: A.\nlinks:\n  - target: beta.yml\n    relationship: refines\n",
        )
        .unwrap();
        fs::write(
            class.join("beta.yml"),
            "class: reach\nlabel: Beta\ndescription: B.\nlinks:\n  - target: alpha.yml\n    relationship: refines\n",
        )
        .unwrap();
        tmp
    }

    fn errors(checks: &[Check]) -> usize {
        checks
            .iter()
            .filter(|c| c.severity == Severity::Error)
            .map(|c| c.violations.len())
            .sum()
    }

    #[test]
    fn a_clean_corpus_produces_no_errors() {
        let tmp = clean_repo();
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(errors(&all), 0, "{all:#?}");
    }

    #[test]
    fn every_check_reports_even_when_it_passes() {
        // A check that vanishes when it passes cannot be told from one that did not run.
        let tmp = clean_repo();
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(all.len(), 12);
        let ids: HashSet<&str> = all.iter().map(|c| c.id).collect();
        assert!(ids.contains("dangling-edge"));
        assert!(ids.contains("catalog-used-by-drift"));
    }

    #[test]
    fn a_dangling_edge_is_an_error() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert!(errors(&all) > 0);
    }

    #[test]
    fn the_commit_check_runs_only_when_asked() {
        let tmp = clean_repo();
        let without = run_checks(tmp.path(), &Options::default());
        let with = run_checks(
            tmp.path(),
            &Options {
                commits: true,
                ..Default::default()
            },
        );
        assert_eq!(with.len(), without.len() + 1);
    }

    #[test]
    fn blessing_then_running_again_is_clean() {
        let tmp = clean_repo();
        fs::write(
            tmp.path().join(".yidam/corpus/reach/alpha.yml"),
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        assert!(errors(&all) > 0);

        baseline::Baseline::from_checks(&all)
            .write(tmp.path())
            .unwrap();
        let again = run_checks(tmp.path(), &Options::default());
        let loaded = baseline::Baseline::load(tmp.path()).unwrap();
        assert!(baseline::diff(&again, &loaded).is_clean());
    }

    #[test]
    fn fixing_a_baselined_violation_makes_the_baseline_stale() {
        let tmp = clean_repo();
        let broken = tmp.path().join(".yidam/corpus/reach/alpha.yml");
        fs::write(
            &broken,
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: nowhere.yml\n",
        )
        .unwrap();
        let all = run_checks(tmp.path(), &Options::default());
        baseline::Baseline::from_checks(&all)
            .write(tmp.path())
            .unwrap();

        // Repair it.
        fs::write(
            &broken,
            "class: reach\nlabel: A\ndescription: A.\nlinks:\n  - target: beta.yml\n",
        )
        .unwrap();
        let after = run_checks(tmp.path(), &Options::default());
        let loaded = baseline::Baseline::load(tmp.path()).unwrap();
        let d = baseline::diff(&after, &loaded);
        assert!(!d.resolved.is_empty(), "the fix must show as stale");
        assert!(!d.is_clean());
    }
}
