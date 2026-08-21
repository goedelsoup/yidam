//! Structural checks — the automated half of [`tests/rubric.md`](../../../rubric.md).
//!
//! These run against a captured result directory, not against a live worktree, so a run can
//! be re-checked later without re-running the model.
//!
//! **What these checks look at.** A bootstrapped repository keeps its corpus at
//! `.yidam/corpus/`: class definitions as `<class>.ont.yml` directly in that directory, and
//! instance nodes as `<class>/<instance>.yml` beneath it. The walk rules here mirror
//! `yidam/cli/src/walk.rs` — depth-1 `.ont.yml` is a schema, depth-≥2 `.yml` is an instance —
//! because the CLI's gates and this harness must agree about what a node is.
//!
//! They did not always. Until PROTOCOL_VERSION 0.2.0 these checks looked for `corpus/*.md`
//! at the repository root, a layout the bootstrap skill stopped producing some twenty-two
//! revisions earlier. S1 and S6 failed for the honest reason that the paths were gone. S2,
//! S3 and S7 **passed**, because each iterates the node list and reports the violations it
//! finds, and an empty list yields none. Three checks reported ✓ having examined nothing.
//!
//! So the walk is a precondition, not an input. When it finds no instances, every node-scoped
//! check fails and says why. A red harness is a fixable harness; a green one that read an
//! empty directory is the state this file was in for the whole of 0.1.0.

use anyhow::Result;
use serde::Deserialize;
use std::path::Path;
use walkdir::WalkDir;

/// Where a bootstrapped repository keeps its corpus, relative to the repository root.
pub const CORPUS_DIR: &str = ".yidam/corpus";

/// The `.yidam/` subdirectories the bootstrap skill creates at genesis (step 3).
///
/// `agents/`, `packages/` and `docs/` are deliberately absent: the skill creates those on
/// first use rather than at genesis, so requiring them here would fail every correct run.
const SCAFFOLD_DIRS: [&str; 4] = ["catalog", "corpus", "decisions", "skills"];

/// A node file may not exceed this many lines.
const NODE_LINE_CEILING: usize = 40;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    pub results: Vec<CheckResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub description: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl CheckReport {
    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }
    pub fn total(&self) -> usize {
        self.results.len()
    }
    pub fn any_failed(&self) -> bool {
        self.results.iter().any(|r| !r.passed)
    }
    pub fn print(&self) {
        for r in &self.results {
            let mark = if r.passed { "✓" } else { "✗" };
            println!("[{mark}] {} — {}", r.id, r.description);
            if let Some(d) = &r.detail {
                println!("    {d}");
            }
        }
        println!("{}/{} passed", self.passed(), self.total());
    }
}

// ── the corpus as the checks see it ───────────────────────────────────────────

/// One instance node, read once so seven checks do not each re-read the tree.
#[derive(Debug)]
struct Instance {
    /// Path relative to the corpus directory, e.g. `concept/tailwater.yml`. This is the
    /// identity a link resolves to, which is why it and not the bare filename is the key:
    /// two classes may each hold a `scope.yml`.
    rel: String,
    lines: usize,
    /// Link targets resolved to corpus-relative paths. A target that escapes the corpus, or
    /// that will not normalize, is kept verbatim — it cannot match an instance, which is the
    /// correct outcome for a link pointing outside the graph.
    links: Vec<String>,
    /// A file that would not parse as YAML. It has no links because none could be read, and
    /// that is a different fact from having declared none.
    unparseable: bool,
}

#[derive(Deserialize)]
struct InstanceDoc {
    #[serde(default)]
    links: Vec<InstanceLink>,
}

#[derive(Deserialize)]
struct InstanceLink {
    target: String,
}

#[derive(Debug, Default)]
struct Corpus {
    /// `<class>.ont.yml` files at depth 1 — the ontology the instances are instances of.
    classes: Vec<String>,
    instances: Vec<Instance>,
    /// The corpus directory itself was absent. Distinguished from "present but empty"
    /// because they fail for different reasons and the detail should say which.
    absent: bool,
}

fn read_corpus(result_dir: &Path) -> Corpus {
    let root = result_dir.join(CORPUS_DIR);
    if !root.exists() {
        return Corpus {
            absent: true,
            ..Default::default()
        };
    }

    let mut corpus = Corpus::default();
    for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let Ok(rel) = entry.path().strip_prefix(&root) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().into_owned();
        let depth = rel.components().count();

        // Depth 1 + `.ont.yml` is a class definition; deeper `.yml` is an instance. The two
        // rules together are why a class file is never counted as a node of itself.
        if depth == 1 && name.ends_with(".ont.yml") {
            corpus.classes.push(name);
        } else if depth >= 2 && name.ends_with(".yml") && !name.ends_with(".ont.yml") {
            corpus.instances.push(read_instance(entry.path(), rel));
        }
    }
    corpus.classes.sort();
    corpus.instances.sort_by(|a, b| a.rel.cmp(&b.rel));
    corpus
}

fn read_instance(path: &Path, rel: &Path) -> Instance {
    let rel_str = rel.to_string_lossy().replace('\\', "/");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Instance {
            rel: rel_str,
            lines: 0,
            links: vec![],
            unparseable: true,
        };
    };
    let lines = text.lines().count();
    match serde_yaml::from_str::<InstanceDoc>(&text) {
        Ok(doc) => {
            let parent = rel.parent().unwrap_or(Path::new(""));
            let links = doc
                .links
                .iter()
                .map(|l| resolve(parent, &l.target))
                .collect();
            Instance {
                rel: rel_str,
                lines,
                links,
                unparseable: false,
            }
        }
        Err(_) => Instance {
            rel: rel_str,
            lines,
            links: vec![],
            unparseable: true,
        },
    }
}

/// Resolve a link target against the directory of the node that declares it, and express it
/// the way instances are keyed — corpus-relative, forward slashes, no `..` left in it.
///
/// Instance links are written relative to the file (`../concept/low-flow.yml`), so the
/// naive comparison is between a path and a filename. That was the previous defect: incoming
/// links were tallied under the raw target string and looked up under `file_name()`, so they
/// matched only for a same-directory bare filename — which the layout never produces. Every
/// node read as having zero incoming links, and the orphan check quietly degenerated into a
/// second copy of S2.
fn resolve(from_dir: &Path, target: &str) -> String {
    let mut parts: Vec<String> = from_dir
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    for segment in target.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                // Popping past the corpus root means the link leaves the graph. Keep the
                // target verbatim so it matches no instance and reads legibly in the detail.
                if parts.pop().is_none() {
                    return target.to_string();
                }
            }
            other => parts.push(other.to_string()),
        }
    }
    parts.join("/")
}

// ── the checks ────────────────────────────────────────────────────────────────

/// Run all structural checks against a result directory.
pub fn run_all(result_dir: &Path) -> Result<CheckReport> {
    let corpus = read_corpus(result_dir);

    Ok(CheckReport {
        results: vec![
            check_s1(&corpus),
            check_s2(&corpus),
            check_s3(&corpus),
            check_s4(result_dir),
            check_s5(result_dir),
            check_s6(result_dir),
            check_s7(&corpus),
        ],
    })
}

fn pass(id: &str, description: &str) -> CheckResult {
    CheckResult {
        id: id.into(),
        description: description.into(),
        passed: true,
        detail: None,
    }
}

fn fail(id: &str, description: &str, detail: impl Into<String>) -> CheckResult {
    CheckResult {
        id: id.into(),
        description: description.into(),
        passed: false,
        detail: Some(detail.into()),
    }
}

/// The precondition every node-scoped check shares: there has to be something to check.
///
/// Returns the failure to report when there is not, and `None` when the walk found nodes.
fn no_nodes(corpus: &Corpus, id: &str, description: &str) -> Option<CheckResult> {
    if !corpus.instances.is_empty() {
        return None;
    }
    let why = if corpus.absent {
        format!("{CORPUS_DIR}/ is absent — nothing to check")
    } else {
        format!("{CORPUS_DIR}/ holds no instance nodes — nothing to check")
    };
    Some(fail(id, description, why))
}

fn check_s1(corpus: &Corpus) -> CheckResult {
    const D: &str = "The corpus holds ≥1 class definition and ≥2 instance nodes";
    if corpus.absent {
        return fail("S1", D, format!("{CORPUS_DIR}/ does not exist"));
    }
    let (c, i) = (corpus.classes.len(), corpus.instances.len());
    if c >= 1 && i >= 2 {
        return pass("S1", D);
    }
    fail(
        "S1",
        D,
        format!("found {c} class definition(s), {i} instance(s)"),
    )
}

fn check_s2(corpus: &Corpus) -> CheckResult {
    const D: &str = "Every instance node declares ≥1 link";
    if let Some(vacuous) = no_nodes(corpus, "S2", D) {
        return vacuous;
    }
    let broken: Vec<String> = corpus
        .instances
        .iter()
        .filter(|n| n.unparseable || n.links.is_empty())
        .map(|n| {
            if n.unparseable {
                format!("{} (unparseable)", n.rel)
            } else {
                n.rel.clone()
            }
        })
        .collect();
    if broken.is_empty() {
        pass("S2", D)
    } else {
        fail("S2", D, format!("no links: {}", broken.join(", ")))
    }
}

fn check_s3(corpus: &Corpus) -> CheckResult {
    const D: &str = "No orphan instance nodes (zero in AND zero out links)";
    if let Some(vacuous) = no_nodes(corpus, "S3", D) {
        return vacuous;
    }
    let incoming: std::collections::HashSet<&str> = corpus
        .instances
        .iter()
        .flat_map(|n| n.links.iter().map(String::as_str))
        .collect();
    let orphans: Vec<&str> = corpus
        .instances
        .iter()
        .filter(|n| n.links.is_empty() && !incoming.contains(n.rel.as_str()))
        .map(|n| n.rel.as_str())
        .collect();
    if orphans.is_empty() {
        pass("S3", D)
    } else {
        fail("S3", D, format!("orphans: {}", orphans.join(", ")))
    }
}

/// The verbs a bootstrap run is allowed to write, from step 8 of the skill and the closed
/// vocabulary in [GRAPH.md](../../../../prelude/GRAPH.md).
///
/// `genesis` (or `overlay`, in existing-repo mode) is the root. The other two are the
/// transient-layer commits the protocol requires before step 9: `consume:` for samudaya and
/// again for sadhana, `vendor:` for the prelude move.
const GENESIS_VERBS: [&str; 2] = ["genesis", "overlay"];
const TRANSIENT_VERBS: [&str; 2] = ["consume", "vendor"];

/// One commit of the captured history: `<sha>\t<subject>`, oldest first.
fn commits(root: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(root.join("commits.tsv"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.split_once('\t'))
        .map(|(sha, subject)| (sha.to_string(), subject.to_string()))
        .collect()
}

/// The verb of a subject line — everything before the first `": "`, per `lint::commits`.
fn verb_of(subject: &str) -> &str {
    subject.split_once(": ").map(|(v, _)| v).unwrap_or(subject)
}

/// This asked for exactly one commit until PROTOCOL_VERSION 0.2.0, and a correct run has
/// never produced one. Step 8 writes the genesis commit and then three more — `consume:`
/// samudaya, `consume:` sadhana, `vendor:` the prelude — and step 9 refuses to begin until
/// all four exist. The check could only fail on a correct bootstrap.
///
/// What it was reaching for is that the history is *the bootstrap's* history and nothing
/// else, which a count approximates badly. Ask it directly: the run begins at a genesis
/// commit, and every commit after it is one the protocol prescribes.
fn check_s4(root: &Path) -> CheckResult {
    const D: &str = "The history is the genesis sequence, and holds nothing else";
    let commits = commits(root);
    let Some((_, first)) = commits.first() else {
        return fail("S4", D, "no commits");
    };
    if !GENESIS_VERBS.contains(&verb_of(first)) {
        return fail(
            "S4",
            D,
            format!("the first commit is not a genesis commit: {first:?}"),
        );
    }
    let stray: Vec<&str> = commits[1..]
        .iter()
        .filter(|(_, s)| !TRANSIENT_VERBS.contains(&verb_of(s)))
        .map(|(_, s)| s.as_str())
        .collect();
    if stray.is_empty() {
        pass("S4", D)
    } else {
        fail(
            "S4",
            D,
            format!("commits outside the protocol: {}", stray.join("; ")),
        )
    }
}

/// Read from `genesis.msg` — the raw `%B` body of the **root** commit.
///
/// Two ways to read the wrong thing here, and the capture closes both. `git log`'s verbose
/// format indents the message and separates commits with blank lines, and the previous
/// implementation counted every line after the first blank one, so a two-line message scored
/// three. And the genesis commit is the first commit, not the last: at the end of a correct
/// run HEAD is the `vendor:` commit, whose message is a fixed string from step 8 and would
/// have scored whatever that string scores no matter what the agent wrote.
fn check_s5(root: &Path) -> CheckResult {
    const D: &str = "The genesis commit message is ≥3 lines";
    let msg = std::fs::read_to_string(root.join("genesis.msg")).unwrap_or_default();
    let lines = msg.trim_end().lines().count();
    if lines >= 3 {
        pass("S5", D)
    } else {
        fail("S5", D, format!("message has {lines} line(s)"))
    }
}

fn check_s6(root: &Path) -> CheckResult {
    const D: &str = "The .yidam/ scaffold exists (catalog, corpus, decisions, skills)";
    let missing: Vec<&str> = SCAFFOLD_DIRS
        .iter()
        .filter(|d| !root.join(".yidam").join(d).exists())
        .copied()
        .collect();
    if missing.is_empty() {
        pass("S6", D)
    } else {
        fail(
            "S6",
            D,
            format!("missing: .yidam/{}", missing.join(", .yidam/")),
        )
    }
}

fn check_s7(corpus: &Corpus) -> CheckResult {
    const D: &str = "No instance node exceeds 40 lines";
    if let Some(vacuous) = no_nodes(corpus, "S7", D) {
        return vacuous;
    }
    let oversize: Vec<String> = corpus
        .instances
        .iter()
        .filter(|n| n.lines > NODE_LINE_CEILING)
        .map(|n| format!("{} ({}L)", n.rel, n.lines))
        .collect();
    if oversize.is_empty() {
        pass("S7", D)
    } else {
        fail("S7", D, oversize.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn repo_root() -> PathBuf {
        // tests/harness/yidam-harness/ → repository root
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../..")
            .canonicalize()
            .unwrap()
    }

    /// The corpus the CLI's own report goldens are certified against.
    ///
    /// Staged from one recipe rather than transcribed here: a harness asserting against its
    /// own private corpus is a harness that agrees with nobody. This one holds four
    /// instances across two classes, and one deliberately broken edge.
    fn fixture_repo() -> PathBuf {
        repo_root().join("yidam/prelude/sdks/parity/fixtures/reports/basic/repo")
    }

    fn result_of<'a>(check: &'a CheckReport, id: &str) -> &'a CheckResult {
        check.results.iter().find(|r| r.id == id).unwrap()
    }

    #[test]
    fn reads_the_canonical_corpus_layout() {
        let corpus = read_corpus(&fixture_repo());
        assert_eq!(corpus.classes, vec!["concept.ont.yml", "gauge.ont.yml"]);
        assert_eq!(
            corpus
                .instances
                .iter()
                .map(|n| n.rel.as_str())
                .collect::<Vec<_>>(),
            vec![
                "concept/low-flow.yml",
                "concept/mixing-zone.yml",
                "concept/tailwater.yml",
                "gauge/riffle-station.yml",
            ],
            "a .ont.yml at depth 1 is a class, not a node of itself"
        );
    }

    /// `../concept/low-flow.yml` declared on `concept/tailwater.yml` is an edge INTO
    /// `concept/low-flow.yml`. Keying on the filename instead is what made S3 vacuous.
    #[test]
    fn link_targets_resolve_to_the_node_they_point_at() {
        let corpus = read_corpus(&fixture_repo());
        let tailwater = corpus
            .instances
            .iter()
            .find(|n| n.rel == "concept/tailwater.yml")
            .unwrap();
        assert_eq!(
            tailwater.links,
            vec!["concept/low-flow.yml", "concept/mixing-zone.yml"]
        );
    }

    #[test]
    fn a_link_that_leaves_the_corpus_matches_no_node() {
        assert_eq!(
            resolve(Path::new("concept"), "../../outside.yml"),
            "../../outside.yml"
        );
        assert_eq!(
            resolve(Path::new("concept"), "../gauge/x.yml"),
            "gauge/x.yml"
        );
        assert_eq!(
            resolve(Path::new("concept"), "./sibling.yml"),
            "concept/sibling.yml"
        );
    }

    /// The node-scoped checks pass against a real corpus. Without this, the test below
    /// would also pass if `no_nodes` simply failed everything unconditionally.
    #[test]
    fn the_node_checks_pass_against_the_fixture_corpus() {
        let corpus = read_corpus(&fixture_repo());
        for check in [
            check_s1(&corpus),
            check_s2(&corpus),
            check_s3(&corpus),
            check_s7(&corpus),
        ] {
            assert!(check.passed, "{} failed: {:?}", check.id, check.detail);
        }
    }

    /// The defect this protocol version exists to close: an empty walk used to report ✓.
    #[test]
    fn an_empty_corpus_fails_every_node_scoped_check() {
        let empty = tempfile::TempDir::new().unwrap();
        let report = run_all(empty.path()).unwrap();
        for id in ["S1", "S2", "S3", "S7"] {
            let r = result_of(&report, id);
            assert!(
                !r.passed,
                "{id} passed against a corpus that does not exist"
            );
            assert!(
                r.detail
                    .as_deref()
                    .unwrap_or_default()
                    .contains("nothing to check")
                    || id == "S1",
                "{id} failed without saying the walk found nothing: {:?}",
                r.detail
            );
        }
    }

    /// Present but empty is a different failure from absent, and the detail must say which.
    #[test]
    fn an_empty_corpus_directory_is_distinguished_from_a_missing_one() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(CORPUS_DIR)).unwrap();
        let report = run_all(tmp.path()).unwrap();
        assert!(result_of(&report, "S2")
            .detail
            .as_deref()
            .unwrap()
            .contains("holds no instance nodes"));
    }

    #[test]
    fn an_unparseable_node_is_not_a_node_with_no_links() {
        let tmp = tempfile::TempDir::new().unwrap();
        let class = tmp.path().join(CORPUS_DIR).join("concept");
        std::fs::create_dir_all(&class).unwrap();
        std::fs::write(class.join("broken.yml"), "links: [ this is not yaml\n").unwrap();
        let corpus = read_corpus(tmp.path());
        assert!(corpus.instances[0].unparseable);
        assert!(check_s2(&corpus).detail.unwrap().contains("unparseable"));
    }

    #[test]
    fn an_orphan_is_a_node_nothing_points_at_and_that_points_nowhere() {
        let tmp = tempfile::TempDir::new().unwrap();
        let class = tmp.path().join(CORPUS_DIR).join("concept");
        std::fs::create_dir_all(&class).unwrap();
        std::fs::write(
            class.join("a.yml"),
            "links:\n  - target: ../concept/b.yml\n",
        )
        .unwrap();
        std::fs::write(class.join("b.yml"), "label: B\n").unwrap();
        std::fs::write(class.join("c.yml"), "label: C\n").unwrap();
        let corpus = read_corpus(tmp.path());
        // b has no outgoing links but a points at it; c has neither.
        assert_eq!(check_s3(&corpus).detail.unwrap(), "orphans: concept/c.yml");
    }

    fn with_commits(subjects: &[&str]) -> tempfile::TempDir {
        let tmp = tempfile::TempDir::new().unwrap();
        let tsv: String = subjects
            .iter()
            .enumerate()
            .map(|(i, s)| format!("{i:040}\t{s}\n"))
            .collect();
        std::fs::write(tmp.path().join("commits.tsv"), tsv).unwrap();
        tmp
    }

    /// The sequence step 8 prescribes and step 9 refuses to begin without. The previous S4
    /// asked for exactly one commit, so this — a correct run — failed it.
    #[test]
    fn the_prescribed_genesis_sequence_passes() {
        let tmp = with_commits(&[
            "genesis: causal inference — 3 classes, 13 instances",
            "consume: samudaya — no seeds present; directory removed",
            "consume: sadhana — scaffold template consumed",
            "vendor: yidam prelude into .yidam/.vendor/; template files removed",
        ]);
        assert!(
            check_s4(tmp.path()).passed,
            "{:?}",
            check_s4(tmp.path()).detail
        );
    }

    #[test]
    fn an_existing_repo_bootstrap_begins_at_an_overlay_commit() {
        let tmp = with_commits(&["overlay: the repo — yidam applied to 400-commit repository"]);
        assert!(check_s4(tmp.path()).passed);
    }

    #[test]
    fn a_commit_the_protocol_did_not_prescribe_is_a_failure() {
        let tmp = with_commits(&[
            "genesis: causal inference — 3 classes",
            "fix: typo in a node",
        ]);
        let r = check_s4(tmp.path());
        assert!(!r.passed);
        assert!(r.detail.unwrap().contains("fix: typo"));
    }

    #[test]
    fn a_history_that_does_not_begin_at_genesis_is_a_failure() {
        let tmp = with_commits(&["chore: initial commit", "genesis: too late"]);
        assert!(!check_s4(tmp.path()).passed);
    }

    /// `verb_of` takes everything before the first `": "`, which is what `lint::commits`
    /// does — so a conventional-commits scope makes the verb unrecognizable, deliberately.
    #[test]
    fn a_scoped_verb_is_not_the_verb() {
        assert_eq!(verb_of("vendor(yidam): the prelude"), "vendor(yidam)");
        assert_eq!(verb_of("vendor: the prelude"), "vendor");
    }

    /// `git log`'s blank separator line used to be counted as message content.
    #[test]
    fn the_genesis_message_is_counted_without_gits_padding() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("genesis.msg"), "subject\n\nbody\n").unwrap();
        assert!(
            check_s5(tmp.path()).passed,
            "subject + blank + body is three lines"
        );

        std::fs::write(tmp.path().join("genesis.msg"), "subject\n\n").unwrap();
        assert!(
            !check_s5(tmp.path()).passed,
            "a bare subject is not three lines"
        );
    }

    /// The scaffold check must not require the directories the skill defers to first use.
    #[test]
    fn the_scaffold_check_does_not_demand_the_deferred_directories() {
        let tmp = tempfile::TempDir::new().unwrap();
        for d in SCAFFOLD_DIRS {
            std::fs::create_dir_all(tmp.path().join(".yidam").join(d)).unwrap();
        }
        assert!(
            check_s6(tmp.path()).passed,
            "agents/ and packages/ arrive on first use"
        );
    }
}
