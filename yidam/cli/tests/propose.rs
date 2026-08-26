//! `yidam propose` through the binary, against a real repository (#269).
//!
//! An integration test rather than unit tests, because the properties that matter are about
//! git objects and about what a person's checkout looks like afterwards — neither of which a
//! unit test can see. `cmd::propose`'s own tests hold the planner; this holds the loop:
//!
//! ```text
//!   finding → propose → review the branch → merge → the report sees the question
//!           → somebody fixes the finding → propose → close → the node is as it was
//! ```
//!
//! # The fixture is shaped like the worked example
//!
//! One class declaring an outbound edge at another is what makes the target's instances
//! citable, and therefore what makes an uncited one a finding. Before #336 that was not
//! enough — the derivation read only a class's own edge list — and a fixture in this shape
//! would have asserted that `propose` proposes nothing, and passed for the wrong reason.

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn commit(dir: &Path, message: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", message]);
}

struct Run {
    stdout: String,
    stderr: String,
    ok: bool,
}

fn propose(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .arg("propose")
        .args(args)
        .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        ok: out.status.success(),
    }
}

/// Write an instance whose description is a block scalar, pointing at `links`.
fn node(dir: &Path, class: &str, name: &str, prose: &str, links: &[&str]) {
    let edges: String = links
        .iter()
        .map(|t| format!("  - target: {t}\n    relationship: sources-from\n"))
        .collect();
    std::fs::write(
        dir.join(format!(".yidam/corpus/{class}/{name}.yml")),
        format!(
            "class: {class}\nlabel: {name}\ndescription: |\n  {prose}\nproperties:\n  \
             claim_tag: verified\nlinks:\n  - target: ../{class}.ont.yml\n    \
             relationship: instance-of\n{edges}"
        ),
    )
    .unwrap();
}

/// A corpus with exactly one orphan, escalating to an error after one commit.
///
/// Shaped like the worked example: `gage` declares `sources-from -> concept`, which is what
/// makes an uncited `concept` a finding. A gage is exempt, because nothing is declared to
/// point at gages.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".yidam/corpus/concept")).unwrap();
    std::fs::create_dir_all(root.join(".yidam/corpus/gage")).unwrap();
    std::fs::write(
        root.join(".yidam/config.toml"),
        "[lint]\nescalate_after = 1\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/corpus/concept.ont.yml"),
        "class: concept\nlabel: Concept\ndescription: |\n  A notion.\nproperties:\n  \
         - name: claim_tag\n    type: claim\n    description: The standing.\nedges:\n  \
         - relationship: refines\n    target: concept\n    direction: out\n    \
         description: A concept this one narrows.\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/corpus/gage.ont.yml"),
        "class: gage\nlabel: Gage\ndescription: |\n  A station.\nproperties:\n  \
         - name: claim_tag\n    type: claim\n    description: The standing.\nedges:\n  \
         - relationship: sources-from\n    target: concept\n    direction: out\n    \
         description: A concept this record computes.\n",
    )
    .unwrap();
    node(
        root,
        "concept",
        "cited",
        "A gage points at this one. [verified]",
        &[],
    );
    node(
        root,
        "concept",
        "lonely",
        "Nothing points at this one. [verified]",
        &[],
    );
    node(
        root,
        "gage",
        "probe",
        "It sources from a concept. [verified]",
        &["../concept/cited.yml"],
    );

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "tester@example.org"]);
    git(root, &["config", "user.name", "Tester"]);
    commit(root, "genesis: corpus");
    dir
}

fn branch(root: &Path) -> String {
    format!("propose/{}", git(root, &["rev-parse", "--short", "HEAD"]))
}

/// The whole loop, in the order a person would run it.
#[test]
fn a_finding_becomes_a_reviewable_commit_and_the_question_it_opens_is_later_closed() {
    let dir = fixture();
    let root = dir.path();
    let before = std::fs::read_to_string(root.join(".yidam/corpus/concept/lonely.yml")).unwrap();

    // ── it drafts, and says what it did not do ──────────────────────────────
    let run = propose(root, &[]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    assert!(
        run.stdout
            .contains("nothing was merged, and no claim was re-tagged"),
        "{}",
        run.stdout
    );
    assert!(run.stdout.contains("open: lonely"), "{}", run.stdout);

    // ── the checkout is untouched ───────────────────────────────────────────
    assert_eq!(
        std::fs::read_to_string(root.join(".yidam/corpus/concept/lonely.yml")).unwrap(),
        before,
        "the working tree was modified"
    );
    assert_eq!(
        git(root, &["status", "--porcelain"]),
        "",
        "the tree is dirty"
    );
    assert_eq!(
        git(root, &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main",
        "the branch was switched"
    );

    // ── the commit is the tool's, in the closed vocabulary ──────────────────
    let b = branch(root);
    let log = git(
        root,
        &["log", "--format=%an <%ae>%n%s", &format!("main..{b}")],
    );
    assert!(log.contains("yidam propose <propose@yidam>"), "{log}");
    assert!(log.contains("open: lonely"), "{log}");
    assert_eq!(
        git(root, &["log", "--format=%cn", &format!("main..{b}")]),
        "Tester",
        "the person who ran it is the committer"
    );

    // ── merged, the question is where the reports look ──────────────────────
    git(root, &["merge", "-q", "--ff-only", &b]);
    let questions = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["open-questions", "--format", "json"])
        .output()
        .unwrap();
    let json: serde_json::Value =
        serde_json::from_slice(&questions.stdout).expect("open-questions emits JSON");
    let nodes: Vec<&str> = json["open_questions"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|q| q["node"].as_str())
        .collect();
    assert!(
        nodes.contains(&".yidam/corpus/concept/lonely.yml"),
        "the proposed question is invisible to `open-questions`: {nodes:?}"
    );

    // ── somebody answers it ─────────────────────────────────────────────────
    node(
        root,
        "concept",
        "cited",
        "A gage points at this one. [verified]",
        &["./lonely.yml"],
    );
    commit(root, "establish: cited refines lonely");

    // ── and the question this command opened is retired, exactly ────────────
    let run = propose(root, &[]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("close: lonely"), "{}", run.stdout);
    let b = branch(root);
    assert_eq!(
        git(
            root,
            &["show", &format!("{b}:.yidam/corpus/concept/lonely.yml")]
        ),
        before.trim_end(),
        "closing the question did not restore the node"
    );
}

/// `--dry-run` drafts and writes nothing — no ref, no objects anybody can reach.
#[test]
fn a_dry_run_writes_no_branch() {
    let dir = fixture();
    let root = dir.path();
    let run = propose(root, &["--dry-run"]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("--dry-run"), "{}", run.stdout);
    assert!(run.stdout.contains("open: lonely"), "{}", run.stdout);
    assert_eq!(
        git(root, &["branch", "--list", "propose/*"]),
        "",
        "a dry run left a branch behind"
    );
}

/// A second run at the same HEAD refuses rather than writing a second copy, and names both
/// ways out.
#[test]
fn a_second_run_at_the_same_head_refuses_and_force_replaces() {
    let dir = fixture();
    let root = dir.path();
    assert!(propose(root, &[]).ok);

    let again = propose(root, &[]);
    assert!(!again.ok, "a second run should refuse: {}", again.stdout);
    assert!(again.stderr.contains("already exists"), "{}", again.stderr);
    assert!(again.stderr.contains("--force"), "{}", again.stderr);

    assert!(propose(root, &["--force"]).ok, "--force should replace it");
}

/// The findings are read from the working tree and the commits are built on HEAD, so a corpus
/// that differs from HEAD would have every proposal drafted from one and applied to the other.
#[test]
fn a_dirty_corpus_is_refused_before_anything_is_drafted() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(
        root.join(".yidam/corpus/concept/lonely.yml"),
        "class: concept\nlabel: edited but not committed\n",
    )
    .unwrap();

    let run = propose(root, &[]);
    assert!(!run.ok, "{}", run.stdout);
    assert!(run.stderr.contains("uncommitted changes"), "{}", run.stderr);
    assert!(
        run.stderr.contains("lonely.yml"),
        "it names the file: {}",
        run.stderr
    );
    assert_eq!(
        git(root, &["branch", "--list", "propose/*"]),
        "",
        "it drafted anyway"
    );
}

/// The JSON report carries the finding each proposal quotes, so a consumer can check the
/// carriage rule rather than trust it.
#[test]
fn the_json_report_carries_each_proposals_finding() {
    let dir = fixture();
    let root = dir.path();
    let run = propose(root, &["--dry-run", "--format", "json"]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    let json: serde_json::Value = serde_json::from_str(&run.stdout).expect("valid JSON");
    assert_eq!(json["format_version"], "1");
    let p = &json["proposals"][0];
    assert_eq!(p["verb"], "open");
    assert_eq!(p["check"], "orphan-in");
    assert_eq!(p["node"], ".yidam/corpus/concept/lonely.yml");
    assert!(
        p["detail"]
            .as_str()
            .unwrap()
            .contains("nothing links to this node"),
        "{p}"
    );
    assert!(
        json["written"].is_null(),
        "--dry-run wrote something: {json}"
    );
}

/// A corpus that never declared the threshold gets no deletion drafted, and the report says
/// so rather than reading as a clean bill of health.
#[test]
fn without_the_declaration_nothing_is_withdrawn() {
    let dir = fixture();
    let root = dir.path();
    let run = propose(root, &["--dry-run"]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    assert!(!run.stdout.contains("withdraw:"), "{}", run.stdout);
}

/// With it, the same finding is drafted as a deletion instead of a question — and the node is
/// still on disk afterwards, because nothing merged.
#[test]
fn a_declared_threshold_drafts_a_withdrawal() {
    let dir = fixture();
    let root = dir.path();
    std::fs::write(
        root.join(".yidam/config.toml"),
        "[lint]\nescalate_after = 1\n\n[propose]\nwithdraw_uncited_after = 1\n",
    )
    .unwrap();
    commit(
        root,
        "decide: an uncited node is over-collection after one commit",
    );

    let run = propose(root, &[]);
    assert!(run.ok, "{}\n{}", run.stdout, run.stderr);
    assert!(run.stdout.contains("withdraw: lonely"), "{}", run.stdout);
    assert!(
        !run.stdout.contains("open: lonely"),
        "asked as well as deleted: {}",
        run.stdout
    );

    assert!(
        root.join(".yidam/corpus/concept/lonely.yml").exists(),
        "the node was deleted from the checkout"
    );
    let listed = git(root, &["ls-tree", "-r", "--name-only", &branch(root)]);
    assert!(
        !listed.contains("lonely.yml"),
        "still on the branch: {listed}"
    );
    assert!(
        listed.contains("cited.yml"),
        "took more than it meant to: {listed}"
    );
}
