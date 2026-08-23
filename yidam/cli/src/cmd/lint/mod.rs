//! `yidam lint` — run every check, compare against the ratchet, report.
//!
//! The gate answers one question: *did this commit make the corpus less clean?* It does
//! not answer *is the corpus clean?*, which for any repository with history is usually no.
//! Conflating the two is what produces a gate that is either permanently red or
//! permanently ignored; see [`baseline`].

mod baseline;
pub(crate) mod checks;
mod commits;
pub(crate) mod history;
pub mod json;
mod model;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::Result;

pub use model::{Check, Severity};

/// The severity `unrecognized-verb` reports at.
///
/// Exposed so that a surface rendering the same rule somewhere else — `yidam vocabulary
/// --check`, and through it the SCM input box — reads its severity from the check rather
/// than restating it. A squiggle stricter than the gate is an editor asserting a verdict
/// nobody agreed to.
/// The committed baseline, or an empty one.
///
/// Exposed for `serve --lsp`, which needs the same debt accounting the gate uses: a finding
/// the baseline already records is inherited and must not be rendered as a regression.
pub(crate) fn load_baseline(root: &Path) -> baseline::Baseline {
    baseline::Baseline::load(root).unwrap_or_default()
}

/// The wire report, built exactly as `--format json` builds it.
pub(crate) fn build_report(
    root: &Path,
    checks: &[Check],
    base: &baseline::Baseline,
) -> json::LintReport {
    json::build(root, checks, base, &baseline::diff(checks, base))
}

pub(crate) fn commit_verb_severity() -> Severity {
    commits::unrecognized_verb(&[]).severity
}

use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_linkable_files, walk_md_files, walk_ont_files};

/// Unsaved editor buffers, keyed by absolute path.
///
/// Every check in this module reads the working tree, which is exactly right for a gate and
/// exactly wrong for an editor: the file you are typing into is the one whose findings you
/// want, and it is the one on disk that is stale. An overlay lets `serve --lsp` answer about
/// the buffer without any check knowing that is what it is doing.
///
/// Empty for every other caller, and `Overlay::read` is then a plain `read_to_string`.
#[derive(Debug, Default, Clone)]
pub struct Overlay(HashMap<PathBuf, String>);

impl Overlay {
    pub fn set(&mut self, path: PathBuf, text: String) {
        self.0.insert(path, text);
    }

    pub fn clear(&mut self, path: &Path) {
        self.0.remove(path);
    }

    /// The buffer if one is open, otherwise the file.
    pub fn read(&self, path: &Path) -> String {
        match self.0.get(path) {
            Some(text) => text.clone(),
            None => std::fs::read_to_string(path).unwrap_or_default(),
        }
    }
}

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
    /// Output format. `text` is what this command has always printed.
    pub format: crate::report::Format,
}

/// Run every check against the repository at `root`.
pub fn run_checks(root: &Path, opts: &Options) -> Vec<Check> {
    run_checks_with(root, opts, &Overlay::default())
}

/// Every check, reading through `overlay` rather than straight from disk.
pub fn run_checks_with(root: &Path, opts: &Options, overlay: &Overlay) -> Vec<Check> {
    let corpus_dir = yidam_corpus_dir(root);
    let catalog_dir = yidam_catalog_dir(root);

    let instance_paths = walk_corpus_instances(&corpus_dir);
    let nodes = checks::load_nodes(root, &instance_paths, overlay);

    let ont_paths = walk_ont_files(&corpus_dir);
    let classes = checks::load_classes(root, &ont_paths, overlay);
    let defined: HashSet<String> = ont_paths
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(str::to_string)
        })
        .collect();

    let catalog_paths = walk_md_files(&catalog_dir);
    let sources = checks::load_sources(root, &catalog_paths, overlay);
    let node_texts: Vec<String> = instance_paths.iter().map(|p| overlay.read(p)).collect();
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
            overlay.read(p),
        ));
    }

    // Resolution records, when this repository runs a sangha at all. Collective mode is
    // opt-in, so an absent directory is the common case and walks to nothing.
    let resolutions_dir = crate::paths::yidam_sangha_dir(root).join("resolutions");
    let mut annotations: Vec<checks::Annotation> = Vec::new();
    for p in walk_md_files(&resolutions_dir) {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(&p)
            .to_string_lossy()
            .to_string();
        annotations.extend(checks::annotations_in(&rel, &overlay.read(&p)));
    }

    // ── Prose links ─────────────────────────────────────────────────────────────
    //
    // Authored markdown, and what counts as authored is declared rather than hard-coded:
    // see [`crate::authorship`]. `.yidam/.vendor/` used to be named here as the single
    // exception, on a rationale that generalizes — a defect in the prelude is fixed
    // upstream and adopted by re-vendoring, so reporting one to a derived repo hands it a
    // finding it cannot act on. It is now the built-in instance of the general mechanism,
    // and a repository that is not a vendoring repository can say the same about a
    // generated directory or a frozen import of its own.
    //
    // `docs/` is included — documentation about the repository is authored, and its links
    // rot the same way. Not `crates/` or `web/`, whose READMEs carry illustrative targets
    // rather than references to files that are supposed to exist.
    let authorship = crate::authorship::Authorship::load_or_default(root);
    let mut prose_link_paths: Vec<std::path::PathBuf> = walk_linkable_files(&root.join(".yidam"));
    prose_link_paths.extend(walk_linkable_files(&root.join("docs")));

    let mut prose_links: Vec<checks::ProseLink> = Vec::new();
    let mut unauthored: Vec<checks::UnauthoredLink> = Vec::new();
    for p in &prose_link_paths {
        let rel = p
            .strip_prefix(root)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string();
        let region = authorship.covering(&rel);
        // `excluded` is the one kind that means *do not look*; the file is not even read.
        if region.is_some_and(|r| !r.kind.reportable()) {
            continue;
        }
        let dir = p.parent().unwrap_or(root);
        let links = checks::prose_links(&rel, dir, &overlay.read(p));
        match region {
            Some(region) => unauthored.extend(
                links
                    .into_iter()
                    .map(|link| checks::UnauthoredLink { region, link }),
            ),
            None => prose_links.extend(links),
        }
    }
    let stale_regions = crate::authorship::stale(root, &authorship);

    let mut all = vec![
        checks::missing_class(&nodes),
        checks::unknown_class(&nodes, &defined),
        checks::orphan_out(&nodes),
        checks::dangling_edge(&nodes),
        checks::undeclared_property(&nodes, &classes),
        checks::missing_property(&nodes, &classes),
        checks::property_type(&nodes, &classes),
        checks::unlicensed_edge(&nodes, &classes),
        checks::edge_target_class(&nodes, &classes),
        checks::catalog_unobtained_but_cited(&sources, &cites),
        checks::missing_label(&nodes),
        checks::missing_description(&nodes),
        checks::claim_tag_malformed(&nodes, &node_texts),
        checks::catalog_used_by_drift(&sources, &cites),
        checks::catalog_location_malformed(&sources),
        checks::malformed_table(&prose),
        orphan_in_dated(root, &nodes, &classes),
        checks::catalog_uncited(&sources, &cites),
        checks::class_asserts_purpose(&classes),
        checks::resolution_annotation_malformed(&annotations),
        checks::resolution_annotation_decides(&annotations),
        checks::broken_prose_link(&prose_links),
        checks::unauthored_prose_link(&unauthored),
        checks::authorship_region_stale(&stale_regions),
    ];

    if opts.commits {
        let subjects = commits::read_subjects(root, opts.range.as_deref());
        all.push(commits::unrecognized_verb(&subjects));
    }

    all
}

/// [`checks::orphan_in`], with each finding dated by when it stopped being cited.
///
/// The check is pure and stays that way; the history is read here, where there is a
/// repository to read it from. Two properties are deliberate:
///
/// **The replay runs only when there is something to date.** A corpus with no orphans has
/// nothing to explain, and the common case should not pay for the uncommon one.
///
/// **A date, not an age.** An age is a function of when you ask, so the same corpus would
/// render differently every day and no golden could pin it — the same reason `index-status`
/// reports `built_at` and lets its client do the arithmetic.
fn orphan_in_dated(root: &Path, nodes: &[checks::Node], classes: &[checks::Class]) -> Check {
    let mut check = checks::orphan_in(nodes, classes);
    if check.violations.is_empty() {
        return check;
    }
    let since = history::uncited_since(root);
    for v in &mut check.violations {
        if let Some(ts) = since.get(&v.node).filter(|t| **t > 0) {
            // Reuses the exporters' civil-date conversion rather than adding a second one;
            // the calendar arithmetic is the kind that is wrong in one copy and right in
            // the other. The clock half is dropped — a day is the resolution anyone reads
            // an orphan's age at.
            let iso = crate::cmd::export::unix_to_iso(*ts as u64);
            let day = iso.split('T').next().unwrap_or(&iso);
            v.detail = format!("{} — uncited since {day}", v.detail);
        }
    }
    check
}

pub fn lint(opts: Options) -> Result<()> {
    let root = repo_root()?;
    // Same reason as `graph_check`: `lint` reported "0 finding(s), no errors" from an empty
    // directory that was not a repository at all.
    crate::paths::require_yidam_repo(&root)?;
    // Read the manifest here, where there is an error channel. `run_checks` degrades to the
    // built-ins so the editor keeps answering mid-edit; a gate that did the same would
    // re-scan every region the file declares and report the flood as a corpus that got
    // worse, rather than as a file with a typo in it.
    crate::authorship::Authorship::load(&root)?;
    let all = run_checks(&root, &opts);

    // JSON short-circuits the prose path entirely rather than interleaving with it: the
    // text output is a contract of its own (byte-identical to what it has always been),
    // and a report that half-prints is worse than either.
    //
    // The EXIT CODE is deliberately shared. A gate that gates differently depending on how
    // you asked for the answer is not a gate, so the verdict below is computed the same way
    // and returned the same way in both modes — only the rendering differs.
    if opts.format.is_json() {
        return lint_json(&root, &all, &opts);
    }

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

/// The JSON path: same checks, same baseline, same verdict, same exit code.
fn lint_json(root: &Path, all: &[Check], opts: &Options) -> Result<()> {
    if opts.bless {
        // Blessing writes a file and reports what it recorded; there is no gate to
        // report on, and pretending otherwise would put `passed: true` on a run that
        // checked nothing.
        let b = baseline::Baseline::from_checks(all);
        let recorded: usize = b.violations.values().map(|v| v.len()).sum();
        b.write(root)?;
        return crate::report::emit(
            root,
            serde_json::json!({
                "blessed": { "recorded_violations": recorded,
                             "path": baseline::path(root).display().to_string() }
            }),
        );
    }

    let committed = baseline::Baseline::load(root)?;
    let d = baseline::diff(all, &committed);
    crate::report::emit(root, json::build(root, all, &committed, &d))?;

    // Same verdict as the text path, and the same silence about it on success.
    if opts.warn_only || d.is_clean() {
        return Ok(());
    }
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
        assert_eq!(all.len(), 24);
        let ids: HashSet<&str> = all.iter().map(|c| c.id).collect();
        assert!(ids.contains("dangling-edge"));
        assert!(ids.contains("catalog-used-by-drift"));
        assert!(ids.contains("class-asserts-purpose"));
        // Reported in a repository with no sangha at all, which is the common case: a
        // check that disappears when there is nothing to check cannot be told from one
        // that was never wired in.
        assert!(ids.contains("resolution-annotation-malformed"));
        assert!(ids.contains("resolution-annotation-decides"));
        assert!(ids.contains("broken-prose-link"));
        assert!(ids.contains("unauthored-prose-link"));
        assert!(ids.contains("claim-tag-malformed"));
        assert!(ids.contains("authorship-region-stale"));
        // The class contract. `clean_repo`'s ontology declares neither properties nor
        // edges, so all five pass here — which is the case worth pinning: silence is not a
        // contract, and a corpus whose ontology is not filled in must not be flooded.
        assert!(ids.contains("undeclared-property"));
        assert!(ids.contains("missing-property"));
        assert!(ids.contains("property-type"));
        assert!(ids.contains("unlicensed-edge"));
        assert!(ids.contains("edge-target-class"));
    }

    fn check<'a>(all: &'a [Check], id: &str) -> &'a Check {
        all.iter().find(|c| c.id == id).expect(id)
    }

    /// A file with one link that goes nowhere, at `rel` under the repo.
    fn broken_link_at(root: &Path, rel: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "See [the thing](./nowhere.md).\n").unwrap();
    }

    fn declare(root: &Path, body: &str) {
        fs::create_dir_all(root.join(".yidam")).unwrap();
        fs::write(root.join(crate::authorship::MANIFEST), body).unwrap();
    }

    /// The measured case: a directory whose own README says it is a frozen copy of an
    /// upstream project. The link is broken, it is real, and it is not this repo's to fix.
    #[test]
    fn a_broken_link_in_an_imported_region_is_reported_but_does_not_gate() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/reference/upstream/notes.md");
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/upstream/\n    from: acme/gis at the fork point\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());

        let scoped = check(&all, "unauthored-prose-link");
        assert_eq!(scoped.violations.len(), 1);
        assert_eq!(scoped.severity, Severity::Info);
        let detail = &scoped.violations[0].detail;
        assert!(detail.contains("acme/gis at the fork point"), "{detail}");
        assert!(detail.contains("falsify"), "{detail}");
        assert_eq!(errors(&all), 0);
    }

    /// A declared region is not a blanket finding: only links that actually fail to
    /// resolve are reported, exactly as in authored material.
    #[test]
    fn a_resolving_link_in_a_declared_region_is_not_a_finding() {
        let tmp = clean_repo();
        let dir = tmp.path().join("docs/reference/upstream");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("target.md"), "# There\n").unwrap();
        fs::write(dir.join("notes.md"), "See [there](./target.md).\n").unwrap();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/upstream/\n    from: acme/gis\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "unauthored-prose-link").passed());
    }

    /// The mutation. Delete the declaration and the same link is an error that gates —
    /// which is what makes the declaration, rather than the path, the thing doing the work.
    #[test]
    fn without_the_declaration_the_same_link_gates() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/reference/upstream/notes.md");
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(check(&all, "broken-prose-link").violations.len(), 1);
        assert!(check(&all, "unauthored-prose-link").passed());
        assert_eq!(errors(&all), 1);
    }

    /// `generated` names the generator, so the finding arrives addressed to it.
    #[test]
    fn a_generated_region_reports_against_the_generator() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), ".yidam/reports/coverage.md");
        declare(
            tmp.path(),
            "generated:\n  - path: .yidam/reports/\n    by: yidam report\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        let detail = &check(&all, "unauthored-prose-link").violations[0].detail;
        assert!(detail.contains("yidam report"), "{detail}");
        assert!(detail.contains("the generator's"), "{detail}");
    }

    /// The escape hatch, and the only kind that produces silence.
    #[test]
    fn an_excluded_region_is_not_read_at_all() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), "docs/scratch/notes.md");
        declare(
            tmp.path(),
            "excluded:\n  - path: docs/scratch/\n    why: working notes\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        assert!(check(&all, "unauthored-prose-link").passed());
    }

    /// The special case became an instance: no manifest, same treatment.
    #[test]
    fn the_vendored_prelude_needs_no_declaration() {
        let tmp = clean_repo();
        broken_link_at(tmp.path(), ".yidam/.vendor/prelude/GRAPH.md");
        let all = run_checks(tmp.path(), &Options::default());
        assert!(check(&all, "broken-prose-link").passed());
        assert_eq!(check(&all, "unauthored-prose-link").violations.len(), 1);
    }

    #[test]
    fn a_declaration_that_matches_nothing_is_reported_and_does_not_gate() {
        let tmp = clean_repo();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/reference/gone/\n    from: acme/gis\n",
        );
        let all = run_checks(tmp.path(), &Options::default());
        let stale = check(&all, "authorship-region-stale");
        assert_eq!(stale.violations.len(), 1);
        assert_eq!(stale.severity, Severity::Warn);
        // The manifest is the offending file, so an editor squiggles the entry itself.
        assert_eq!(stale.violations[0].node, crate::authorship::MANIFEST);
        assert_eq!(errors(&all), 0);
    }

    /// A manifest that exists and cannot be read must be reported as that, not absorbed
    /// into the flood of findings it was written to scope.
    #[test]
    fn an_unreadable_manifest_fails_the_command() {
        let tmp = clean_repo();
        declare(
            tmp.path(),
            "imported:\n  - path: docs/x\n    from: a\n    why: b\n",
        );
        assert!(crate::authorship::Authorship::load(tmp.path()).is_err());
        // …while the checks themselves keep answering, for the editor's sake.
        let all = run_checks(tmp.path(), &Options::default());
        assert_eq!(all.len(), 24);
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
