//! An installed dependency is readable, and is not an edge target.
//!
//! `yidam tonpa` resolved a source, fetched a bundle, hashed it, wrote a lock, verified
//! against it — and unpacked a whole corpus into `.yidam/tonpa/<pkg>/` that nothing in the
//! CLI ever read. A repository could declare a dependency, verify it, commit the lock, and
//! observe no difference in anything the tool could tell it.
//!
//! These tests pin the read layer, and the boundary around it. The boundary is the part
//! worth stating twice: an edge in this model is a claim, and the constitution governs who
//! may assert one. A citation into a corpus with a different ontology, its own electors, and
//! its own revision history is a different object from a local edge, and it wants its own
//! argument rather than arriving as a side effect of a package manager.

use std::path::Path;

/// Write an installed dependency the way `tonpa install` leaves one: a manifest beside a
/// corpus laid out exactly like this repository's own.
fn install_dep(root: &Path, pkg: &str, class: &str, name: &str, label: &str) {
    let base = root.join(".yidam").join("tonpa").join(pkg);
    std::fs::create_dir_all(base.join("corpus").join(class)).unwrap();
    std::fs::write(
        base.join("manifest.yml"),
        "bundle_version: \"1\"\ncommit: \"abc1234\"\ngenesis: \"2026-01-01\"\n",
    )
    .unwrap();
    std::fs::write(
        base.join("corpus").join(class).join(format!("{name}.yml")),
        format!("class: {class}\nlabel: {label}\ndescription: about {label}\n"),
    )
    .unwrap();
}

#[test]
fn an_installed_dependency_is_discovered_and_parsed() {
    let tmp = tempfile::tempdir().unwrap();
    install_dep(
        tmp.path(),
        "watermarks",
        "concept",
        "tailwater",
        "Tailwater",
    );

    assert_eq!(
        yidam::model::dependency_names(tmp.path()),
        vec!["watermarks"]
    );

    let nodes = yidam::model::dependency_nodes(tmp.path(), "watermarks");
    assert_eq!(nodes.len(), 1, "expected one node, got {nodes:?}");
    assert_eq!(nodes[0].id, "concept/tailwater");
    assert_eq!(nodes[0].label, "Tailwater");
    assert_eq!(nodes[0].origin.as_deref(), Some("watermarks"));
    assert!(!nodes[0].is_local());
}

/// A directory without a manifest is not an installed package.
///
/// `.yidam/tonpa/` also holds `tonpa.lock`, and a half-extracted or hand-made directory
/// should not be reported as a dependency — "what is on disk" has to mean something
/// stricter than "what is in this folder".
#[test]
fn a_directory_without_a_manifest_is_not_a_package() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(tmp.path().join(".yidam/tonpa/junk/corpus/concept")).unwrap();
    std::fs::write(tmp.path().join(".yidam/tonpa/tonpa.lock"), "packages = []").unwrap();
    assert!(yidam::model::dependency_names(tmp.path()).is_empty());
}

/// Two corpora may hold the same `class/name`, and a bare id cannot tell them apart.
#[test]
fn a_foreign_node_is_qualified_by_its_package() {
    let tmp = tempfile::tempdir().unwrap();
    install_dep(tmp.path(), "alpha", "concept", "shared", "Alpha's");
    install_dep(tmp.path(), "beta", "concept", "shared", "Beta's");

    let nodes = yidam::model::all_dependency_nodes(tmp.path());
    let ids: Vec<String> = nodes.iter().map(|n| n.qualified_id()).collect();
    assert_eq!(ids, vec!["alpha::concept/shared", "beta::concept/shared"]);

    // The bare ids collide, which is exactly why the qualified form has to exist.
    let bare: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(bare, vec!["concept/shared", "concept/shared"]);
}

/// A repository with no dependencies reads as none, not as an error.
#[test]
fn no_dependencies_is_an_empty_answer() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(yidam::model::dependency_names(tmp.path()).is_empty());
    assert!(yidam::model::all_dependency_nodes(tmp.path()).is_empty());
    assert!(yidam::model::dependency_nodes(tmp.path(), "absent").is_empty());
}

/// Class schema files sit beside the class directories and are not instances.
#[test]
fn ont_schema_files_are_not_read_as_nodes() {
    let tmp = tempfile::tempdir().unwrap();
    install_dep(tmp.path(), "pkg", "concept", "real", "Real");
    std::fs::write(
        tmp.path().join(".yidam/tonpa/pkg/corpus/concept.ont.yml"),
        "class: concept\n",
    )
    .unwrap();

    let nodes = yidam::model::dependency_nodes(tmp.path(), "pkg");
    assert_eq!(
        nodes.len(),
        1,
        "a .ont.yml schema is not an instance: {:?}",
        nodes.iter().map(|n| &n.id).collect::<Vec<_>>()
    );
}

// ── path dependencies ─────────────────────────────────────────────────────────
//
// A sibling repository read where it sits. Not fetched, not hashed, not locked — hashing a
// working tree that changes under you records nothing. It is the only form that supports a
// development loop, which is also the common case for one person with several derivations
// on one machine.

/// Write a yidam repository at `dir` holding one corpus node.
fn sibling_repo(dir: &Path, class: &str, name: &str, label: &str) {
    let corpus = dir.join(".yidam").join("corpus").join(class);
    std::fs::create_dir_all(&corpus).unwrap();
    std::fs::write(
        corpus.join(format!("{name}.yml")),
        format!("class: {class}\nlabel: {label}\ndescription: about {label}\n"),
    )
    .unwrap();
}

fn declare(root: &Path, body: &str) {
    std::fs::create_dir_all(root.join(".yidam")).unwrap();
    std::fs::write(root.join(".yidam").join("tonpa.toml"), body).unwrap();
}

#[test]
fn a_path_dependency_is_read_from_where_it_sits() {
    let tmp = tempfile::tempdir().unwrap();
    let consumer = tmp.path().join("consumer");
    std::fs::create_dir_all(&consumer).unwrap();
    sibling_repo(&tmp.path().join("producer"), "concept", "weir", "Weir");
    declare(
        &consumer,
        "[dependencies.producer]\npath = \"../producer\"\n",
    );

    assert_eq!(yidam::model::dependency_names(&consumer), vec!["producer"]);
    let nodes = yidam::model::dependency_nodes(&consumer, "producer");
    assert_eq!(nodes.len(), 1, "{nodes:?}");
    assert_eq!(nodes[0].qualified_id(), "producer::concept/weir");
    assert_eq!(nodes[0].origin.as_deref(), Some("producer"));
}

/// An edit in the producer is visible in the consumer with no fetch, no lock, no reinstall.
///
/// This is the entire reason path dependencies exist. If it needed a release cycle the
/// mechanism would not support the loop it is for, and the fastest way to iterate on
/// cross-corpus work would remain "do not use the mechanism".
#[test]
fn an_edit_in_the_producer_is_visible_immediately() {
    let tmp = tempfile::tempdir().unwrap();
    let consumer = tmp.path().join("consumer");
    let producer = tmp.path().join("producer");
    std::fs::create_dir_all(&consumer).unwrap();
    sibling_repo(&producer, "concept", "weir", "Weir");
    declare(
        &consumer,
        "[dependencies.producer]\npath = \"../producer\"\n",
    );

    assert_eq!(
        yidam::model::dependency_nodes(&consumer, "producer")[0].label,
        "Weir"
    );

    // Edit the producer in place — no republish, no reinstall.
    sibling_repo(&producer, "concept", "weir", "Weir (revised)");
    assert_eq!(
        yidam::model::dependency_nodes(&consumer, "producer")[0].label,
        "Weir (revised)",
        "a path dependency that caches is not a path dependency"
    );
}

/// A path declaration wins over an unpacked directory of the same name.
///
/// Silently preferring a stale fetched copy would make an edit appear to have no effect,
/// which is the one failure a development loop must not have.
#[test]
fn a_path_declaration_shadows_a_fetched_copy_of_the_same_name() {
    let tmp = tempfile::tempdir().unwrap();
    let consumer = tmp.path().join("consumer");
    std::fs::create_dir_all(&consumer).unwrap();
    sibling_repo(
        &tmp.path().join("producer"),
        "concept",
        "weir",
        "from the path",
    );
    install_dep(&consumer, "producer", "concept", "weir", "from the bundle");
    declare(
        &consumer,
        "[dependencies.producer]\npath = \"../producer\"\n",
    );

    // The name must appear ONCE. Asserting only through `dependency_nodes` would not catch
    // a missing dedup: that function takes the first match and path declarations are pushed
    // first, so ordering alone would keep it looking right while the dependency was listed
    // twice everywhere else.
    assert_eq!(
        yidam::model::dependency_names(&consumer),
        vec!["producer"],
        "a name declared as a path and also unpacked must resolve to one dependency"
    );
    assert_eq!(
        yidam::model::all_dependency_nodes(&consumer).len(),
        1,
        "the shadowed copy must not contribute nodes of its own"
    );

    let nodes = yidam::model::dependency_nodes(&consumer, "producer");
    assert_eq!(
        nodes.len(),
        1,
        "the name must resolve once, not twice: {nodes:?}"
    );
    assert_eq!(nodes[0].label, "from the path");
}

/// A sibling that is not on this machine is a normal state, not an error.
///
/// Someone else clones the consumer without the producer beside it. "What can be read" is a
/// different question from "what is declared", and only the second is `tonpa status`'s.
#[test]
fn a_path_that_does_not_exist_is_skipped_not_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    declare(tmp.path(), "[dependencies.absent]\npath = \"../nowhere\"\n");
    assert!(yidam::model::dependency_names(tmp.path()).is_empty());
    assert!(yidam::model::all_dependency_nodes(tmp.path()).is_empty());
}

// ── path dependencies, through the commands rather than the read layer ────────
//
// `deps::resolved` read a path dependency correctly from the day #202 landed. The three
// `tonpa` query commands did not, and they disagreed with it and with each other:
//
//   tonpa status   →  [missing lock] producer — run `yidam tonpa install`
//   tonpa install  →  Error: local path dependencies not yet supported: ../producer
//
// So the command that diagnosed the problem prescribed the command that refused to fix it,
// about a dependency that was working. Worse, `install`'s failure was a `?` on the first
// path dependency in name order, which aborted the run — one path dependency stopped every
// *fetched* dependency after it from installing.
//
// These go through the binary on purpose. The bug was never in the read layer, and a test
// that called `deps::resolved` would have passed throughout.

use std::process::Command;

use crate::common;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A consumer repository declaring `producer` as a path dependency, and the producer beside
/// it. Returns the tempdir (kept alive by the caller) and the consumer's path.
fn producer_and_consumer(extra_toml: &str) -> (tempfile::TempDir, std::path::PathBuf) {
    let tmp = tempfile::tempdir().unwrap();
    let producer = tmp.path().join("producer");
    std::fs::create_dir_all(producer.join(".yidam/corpus/concept")).unwrap();
    std::fs::write(
        producer.join(".yidam/corpus/concept/alpha.yml"),
        "class: concept\nlabel: Alpha\ndescription: about Alpha\n",
    )
    .unwrap();

    let consumer = tmp.path().join("consumer");
    std::fs::create_dir_all(consumer.join(".yidam")).unwrap();
    // A real git repository: `repo_root` resolves the path relative to the toplevel, and a
    // fallback to the working directory would make this test pass for the wrong reason.
    common::git::git(&consumer, &["init", "-q"]);
    std::fs::write(
        consumer.join(".yidam/tonpa.toml"),
        format!("[dependencies.producer]\npath = \"../producer\"\n{extra_toml}"),
    )
    .unwrap();
    (tmp, consumer)
}

#[test]
fn status_reports_a_path_dependency_as_linked_rather_than_missing() {
    let (_tmp, consumer) = producer_and_consumer("");
    let r = run(&consumer, &["tonpa", "status"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains("[linked]") && r.stdout.contains("../producer"),
        "a readable path dependency must report as linked: {}",
        r.stdout
    );
    assert!(
        !r.stdout.contains("[missing lock]"),
        "a path dependency has nothing to lock, so it must not be reported as unlocked: {}",
        r.stdout
    );
}

/// `lint` refuses a `tonpa.toml` that does not parse, naming the file and the line (#1616).
/// Its checks read the declarations leniently, so the typo used to lint as a corpus that
/// depends on nothing. The control is the same repository before the typo, in both formats.
#[test]
fn lint_refuses_a_tonpa_toml_that_does_not_parse() {
    let (_tmp, consumer) = producer_and_consumer("");
    for args in [&["lint"][..], &["lint", "--format", "json"][..]] {
        let r = run(&consumer, args);
        assert!(
            !r.stderr.contains("tonpa.toml"),
            "control, {args:?}: stdout: {}\nstderr: {}",
            r.stdout,
            r.stderr
        );
    }

    let path = consumer.join(".yidam/tonpa.toml");
    let declared = std::fs::read_to_string(&path).unwrap();
    std::fs::write(&path, format!("{declared}\n[broken\n")).unwrap();
    for args in [&["lint"][..], &["lint", "--format", "json"][..]] {
        let r = run(&consumer, args);
        assert_ne!(r.code, 0, "{args:?}: stdout: {}", r.stdout);
        assert!(
            r.stderr.contains("parsing") && r.stderr.contains("tonpa.toml: line 4, column "),
            "{args:?}: stderr: {}",
            r.stderr
        );
    }
}

#[test]
fn status_reports_a_path_dependency_that_does_not_resolve() {
    let (_tmp, consumer) = producer_and_consumer("");
    std::fs::write(
        consumer.join(".yidam/tonpa.toml"),
        "[dependencies.gone]\npath = \"../nope\"\n",
    )
    .unwrap();
    let r = run(&consumer, &["tonpa", "status"]);
    assert!(
        r.stdout.contains("[path missing]"),
        "a path that resolves to no corpus is the one path-dependency state worth flagging: {}",
        r.stdout
    );
}

#[test]
fn install_skips_a_path_dependency_instead_of_failing_on_it() {
    let (_tmp, consumer) = producer_and_consumer("");
    let r = run(&consumer, &["tonpa", "install"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    assert!(
        !r.stderr.contains("not yet supported"),
        "install must not claim path dependencies are unsupported: {}",
        r.stderr
    );
}

#[test]
fn a_path_dependency_does_not_abort_the_install_of_a_fetched_one() {
    // `aaa` sorts before `producer` in the BTreeMap this iterates, so before the fix the
    // path dependency was reached second — and the run still had to survive it to report on
    // the fetched one. Naming the fetched dependency `zzz` puts it *after* the path one,
    // which is the ordering that actually aborted.
    let (_tmp, consumer) = producer_and_consumer(
        "\n[dependencies.zzz]\nurl = \"https://example.invalid/bundle.yiz\"\n",
    );
    let r = run(&consumer, &["tonpa", "install"]);
    // The fetch of `zzz` fails: the host does not resolve, deliberately — this test must not
    // touch the network. What matters is that the run got *to* it, so the failure names the
    // fetched dependency rather than the path one.
    assert!(
        !r.stderr.contains("not yet supported"),
        "the path dependency must not be what stops the run: {}",
        r.stderr
    );
}

#[test]
fn update_declines_a_named_path_dependency_with_a_reason() {
    let (_tmp, consumer) = producer_and_consumer("");
    let r = run(&consumer, &["tonpa", "update", "producer"]);
    assert_ne!(
        r.code, 0,
        "naming a path dependency is an error, not a no-op"
    );
    assert!(
        r.stderr.contains("path dependency") && r.stderr.contains("editing it"),
        "the error must say why nothing happened and what to do instead: {}",
        r.stderr
    );
}

#[test]
fn update_with_only_path_dependencies_says_so_rather_than_exiting_silently() {
    let (_tmp, consumer) = producer_and_consumer("");
    let r = run(&consumer, &["tonpa", "update"]);
    assert_eq!(r.code, 0, "stderr: {}", r.stderr);
    assert!(
        r.stdout.contains("Nothing to update"),
        "silence here reads as 'updated, no changes': {}",
        r.stdout
    );
}

// ── a path dependency pinned to a commit (#1466) ──────────────────────────────
//
// city-engine read six sibling corpora at commits and had neither form: fetched needs a
// release, path is unpinned. It built a `corpora.toml` of pins, copied packs, and a binary
// that mirrored 65 sibling class files into its own corpus so `check-diff` would know the
// names. These go through the binary, for the reason the section above gives.

/// A git repository producing a corpus with one class and one instance, committed. Returns
/// the commit.
fn committed_producer(dir: &Path, class: &str, node: &str) -> String {
    let corpus = dir.join(".yidam/corpus");
    std::fs::create_dir_all(corpus.join(class)).unwrap();
    std::fs::write(
        corpus.join(format!("{class}.ont.yml")),
        format!("class: {class}\nlabel: {class}\n"),
    )
    .unwrap();
    std::fs::write(
        corpus.join(class).join(format!("{node}.yml")),
        format!("class: {class}\nlabel: {node}\n"),
    )
    .unwrap();
    common::git::git(dir, &["init", "-q"]);
    common::git::git(dir, &["config", "user.name", "t"]);
    common::git::git(dir, &["config", "user.email", "t@example.invalid"]);
    common::git::git(dir, &["add", "-A"]);
    common::git::git_at(
        dir,
        &["commit", "-q", "--no-gpg-sign", "-m", "genesis: producer"],
        common::git::FIXTURE_DATE,
    );
    common::git::out(dir, &["rev-parse", "HEAD"])
}

/// A consumer beside a committed producer, with nothing declared yet.
fn pinned_fixture() -> (
    tempfile::TempDir,
    std::path::PathBuf,
    std::path::PathBuf,
    String,
) {
    let tmp = tempfile::tempdir().unwrap();
    let producer = tmp.path().join("producer");
    let sha = committed_producer(&producer, "gauge-station", "alpha");
    let consumer = tmp.path().join("consumer");
    std::fs::create_dir_all(consumer.join(".yidam/corpus")).unwrap();
    common::git::git(&consumer, &["init", "-q"]);
    (tmp, consumer, producer, sha)
}

fn node_names(root: &Path) -> Vec<String> {
    yidam::model::all_dependency_nodes(root)
        .into_iter()
        .map(|n| n.label)
        .collect()
}

#[test]
fn add_with_a_commit_pins_locks_and_reads_the_corpus_at_that_commit() {
    let (_tmp, consumer, producer, sha) = pinned_fixture();

    let r = run(
        &consumer,
        &["tonpa", "add", &format!("../producer@{}", &sha[..8])],
    );
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);

    // Declared at the *full* commit, whatever abbreviation was typed.
    let toml = std::fs::read_to_string(consumer.join(".yidam/tonpa.toml")).unwrap();
    assert!(toml.contains(&format!("rev = \"{sha}\"")), "{toml}");
    let lock = std::fs::read_to_string(consumer.join(".yidam/tonpa/tonpa.lock")).unwrap();
    assert!(lock.contains(&format!("rev = \"{sha}\"")), "{lock}");

    // The producer moves on — an uncommitted edit and a new commit. Neither is read.
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/beta.yml"),
        "class: gauge-station\nlabel: beta\n",
    )
    .unwrap();
    common::git::git(&producer, &["add", "-A"]);
    common::git::git_at(
        &producer,
        &["commit", "-q", "--no-gpg-sign", "-m", "add beta"],
        common::git::FIXTURE_DATE,
    );
    assert_eq!(node_names(&consumer), vec!["alpha".to_string()]);

    let r = run(&consumer, &["tonpa", "status"]);
    assert!(
        r.stdout.contains("[ok]") && r.stdout.contains("pinned"),
        "{}",
        r.stdout
    );
    let r = run(&consumer, &["tonpa", "verify"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
}

/// The installed copy is a build product: deleted, `install` writes it back from the
/// producer's objects at the locked commit — even with the producer's HEAD elsewhere.
#[test]
fn install_restores_a_pinned_path_at_its_commit() {
    let (_tmp, consumer, producer, sha) = pinned_fixture();
    assert_eq!(
        run(&consumer, &["tonpa", "add", &format!("../producer@{sha}")]).code,
        0
    );
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/beta.yml"),
        "label: b\n",
    )
    .unwrap();
    common::git::git(&producer, &["add", "-A"]);
    common::git::git(&producer, &["commit", "-q", "--no-gpg-sign", "-m", "beta"]);

    std::fs::remove_dir_all(consumer.join(".yidam/tonpa/producer")).unwrap();
    let r = run(&consumer, &["tonpa", "status"]);
    assert!(r.stdout.contains("[not installed]"), "{}", r.stdout);

    let r = run(&consumer, &["tonpa", "install"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    assert_eq!(node_names(&consumer), vec!["alpha".to_string()]);
    assert_eq!(run(&consumer, &["tonpa", "verify"]).code, 0);
}

/// `update` moves the pin to the producer's HEAD, in the declaration and the lock at once —
/// the refresh commit city-engine made by hand across 58 files.
#[test]
fn update_moves_a_pinned_path_to_the_producers_head() {
    let (_tmp, consumer, producer, sha) = pinned_fixture();
    assert_eq!(
        run(&consumer, &["tonpa", "add", &format!("../producer@{sha}")]).code,
        0
    );
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/beta.yml"),
        "class: gauge-station\nlabel: beta\n",
    )
    .unwrap();
    common::git::git(&producer, &["add", "-A"]);
    common::git::git(&producer, &["commit", "-q", "--no-gpg-sign", "-m", "beta"]);
    let head = common::git::out(&producer, &["rev-parse", "HEAD"]);

    let r = run(&consumer, &["tonpa", "update", "producer"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    let toml = std::fs::read_to_string(consumer.join(".yidam/tonpa.toml")).unwrap();
    assert!(toml.contains(&format!("rev = \"{head}\"")), "{toml}");
    let mut names = node_names(&consumer);
    names.sort();
    assert_eq!(names, vec!["alpha".to_string(), "beta".to_string()]);
    assert_eq!(run(&consumer, &["tonpa", "verify"]).code, 0);

    let r = run(&consumer, &["tonpa", "update", "producer"]);
    assert!(r.stdout.contains("already at"), "{}", r.stdout);
}

/// A `rev` that does not name a commit is refused before anything is written.
#[test]
fn add_refuses_a_rev_that_is_not_a_commit() {
    let (_tmp, consumer, _producer, _) = pinned_fixture();
    let r = run(&consumer, &["tonpa", "add", "../producer@no-such-ref"]);
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("is not a commit"), "{}", r.stderr);
    assert!(!consumer.join(".yidam/tonpa.toml").exists());
}

/// `check-diff` asks about a type named for nothing the ontology declares. A class the
/// pinned dependency declares is declared, and the question goes away — the control is the
/// same commit asked about before the dependency was added.
#[test]
fn check_diff_knows_a_class_a_dependency_declares() {
    let (_tmp, consumer, _producer, sha) = pinned_fixture();
    common::git::git(&consumer, &["config", "user.name", "t"]);
    common::git::git(&consumer, &["config", "user.email", "t@example.invalid"]);
    std::fs::write(consumer.join(".yidam/corpus/.keep"), "").unwrap();
    common::git::git(&consumer, &["add", "-A"]);
    common::git::git(
        &consumer,
        &["commit", "-q", "--no-gpg-sign", "-m", "genesis: consumer"],
    );
    std::fs::create_dir_all(consumer.join("crates/a/src")).unwrap();
    std::fs::write(
        consumer.join("crates/a/src/lib.rs"),
        "pub struct GaugeStation;\n",
    )
    .unwrap();
    common::git::git(&consumer, &["add", "-A"]);
    common::git::git(&consumer, &["commit", "-q", "--no-gpg-sign", "-m", "code"]);

    let asked = |consumer: &Path| {
        let r = run(
            consumer,
            &["check-diff", "HEAD~1..HEAD", "--format", "json"],
        );
        assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
        r.stdout.contains("\"name\":\"gauge-station\"")
            || r.stdout.contains("\"name\": \"gauge-station\"")
    };
    assert!(
        asked(&consumer),
        "the control: unknown before the dependency"
    );

    assert_eq!(
        run(&consumer, &["tonpa", "add", &format!("../producer@{sha}")]).code,
        0
    );
    assert!(!asked(&consumer), "known once the dependency declares it");
}

// ── a pinned path's export, and `query --dependency` (RFC-0063) ───────────────

const PACK: &str = ".yidam/computed/pack.json";

/// Commit `body` as the producer's pack, touching nothing in its corpus — a `compute:`
/// commit, as RFC-0063 §0.2 found in all six siblings. Returns the commit.
fn compute(producer: &Path, body: &str) -> String {
    std::fs::create_dir_all(producer.join(".yidam/computed")).unwrap();
    std::fs::write(producer.join(PACK), body).unwrap();
    common::git::git(producer, &["add", "-A"]);
    common::git::git(
        producer,
        &["commit", "-q", "--no-gpg-sign", "-m", "compute: pack"],
    );
    common::git::out(producer, &["rev-parse", "HEAD"])
}

fn commit_beta(producer: &Path) {
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/beta.yml"),
        "class: gauge-station\nlabel: beta\n",
    )
    .unwrap();
    common::git::git(producer, &["add", "-A"]);
    common::git::git(producer, &["commit", "-q", "--no-gpg-sign", "-m", "beta"]);
}

fn add_with_export(consumer: &Path, sha: &str) {
    let r = run(
        consumer,
        &[
            "tonpa",
            "add",
            &format!("../producer@{sha}"),
            "--export",
            PACK,
        ],
    );
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
}

/// Declared, read at the pin, unpacked beside the corpus, locked and verified.
#[test]
fn add_with_an_export_unpacks_and_locks_it_at_the_pin() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    let sha = compute(&producer, "{\"v\":1}\n");
    std::fs::write(producer.join(PACK), "{\"v\":\"working tree\"}\n").unwrap();
    add_with_export(&consumer, &sha);

    let toml = std::fs::read_to_string(consumer.join(".yidam/tonpa.toml")).unwrap();
    assert!(toml.contains(&format!("export = \"{PACK}\"")), "{toml}");
    let lock = std::fs::read_to_string(consumer.join(".yidam/tonpa/tonpa.lock")).unwrap();
    assert!(lock.contains("export_sha256 = "), "{lock}");
    let unpacked = consumer.join(".yidam/tonpa/producer/export/pack.json");
    assert_eq!(std::fs::read_to_string(&unpacked).unwrap(), "{\"v\":1}\n");
    assert_eq!(run(&consumer, &["tonpa", "verify"]).code, 0);

    std::fs::write(&unpacked, "{}\n").unwrap();
    assert_ne!(run(&consumer, &["tonpa", "verify"]).code, 0);
    assert_eq!(run(&consumer, &["tonpa", "install"]).code, 0);
    assert_eq!(std::fs::read_to_string(&unpacked).unwrap(), "{\"v\":1}\n");
}

/// What an older CLI's `install` leaves: the declaration, and a lock entry with no export.
#[test]
fn verify_fails_an_export_the_lock_does_not_hash() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    let sha = compute(&producer, "{}\n");
    add_with_export(&consumer, &sha);
    let lock_path = consumer.join(".yidam/tonpa/tonpa.lock");
    let lock: String = std::fs::read_to_string(&lock_path)
        .unwrap()
        .lines()
        .filter(|l| !l.starts_with("export"))
        .map(|l| format!("{l}\n"))
        .collect();
    std::fs::write(&lock_path, lock).unwrap();

    let r = run(&consumer, &["tonpa", "verify"]);
    assert_ne!(r.code, 0);
    assert!(
        r.stdout.contains("no hash for it") && r.stderr.contains("tonpa install"),
        "stdout: {}\nstderr: {}",
        r.stdout,
        r.stderr
    );
}

#[test]
fn an_export_older_than_the_corpus_at_the_pin_is_refused() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    let written = compute(&producer, "{}\n");
    commit_beta(&producer);
    let head = common::git::out(&producer, &["rev-parse", "HEAD"]);
    let r = run(
        &consumer,
        &[
            "tonpa",
            "add",
            &format!("../producer@{head}"),
            "--export",
            PACK,
        ],
    );
    assert_ne!(r.code, 0);
    assert!(
        r.stderr.contains(&written) && r.stderr.contains(&head),
        "{}",
        r.stderr
    );
    assert!(!consumer.join(".yidam/tonpa.toml").exists());
}

#[test]
fn an_export_on_an_unpinned_path_is_refused() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    compute(&producer, "{}\n");
    let r = run(
        &consumer,
        &["tonpa", "add", "../producer", "--export", PACK],
    );
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("pinned path dependency"), "{}", r.stderr);
}

/// With an export, `update` moves to the commit that last wrote it, not to HEAD, and refuses
/// once the corpus has moved past it.
#[test]
fn update_with_an_export_moves_to_the_last_compute_commit() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    let first = compute(&producer, "{\"v\":1}\n");
    add_with_export(&consumer, &first);

    let second = compute(&producer, "{\"v\":2}\n");
    std::fs::write(producer.join("README.md"), "not the corpus\n").unwrap();
    common::git::git(&producer, &["add", "-A"]);
    common::git::git(&producer, &["commit", "-q", "--no-gpg-sign", "-m", "docs"]);

    let r = run(&consumer, &["tonpa", "update", "producer"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    let toml = std::fs::read_to_string(consumer.join(".yidam/tonpa.toml")).unwrap();
    assert!(toml.contains(&format!("rev = \"{second}\"")), "{toml}");
    assert_eq!(
        std::fs::read_to_string(consumer.join(".yidam/tonpa/producer/export/pack.json")).unwrap(),
        "{\"v\":2}\n"
    );

    commit_beta(&producer);
    let r = run(&consumer, &["tonpa", "update", "producer"]);
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("recompute the export"), "{}", r.stderr);
}

/// Updating everything, one stale export refuses that dependency alone: the others still move,
/// and the lock and declaration record what is on disk.
#[test]
fn update_all_records_what_moved_when_one_export_is_refused() {
    let (_tmp, consumer, producer, sha) = pinned_fixture();
    let first = compute(&producer, "{\"v\":1}\n");
    add_with_export(&consumer, &first);
    let r = run(
        &consumer,
        &[
            "tonpa",
            "add",
            &format!("../producer@{sha}"),
            "--name",
            "plain",
        ],
    );
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);

    commit_beta(&producer);
    let head = common::git::out(&producer, &["rev-parse", "HEAD"]);
    let r = run(&consumer, &["tonpa", "update"]);
    assert_ne!(r.code, 0, "{}", r.stdout);
    assert!(r.stderr.contains("recompute the export"), "{}", r.stderr);
    let toml = std::fs::read_to_string(consumer.join(".yidam/tonpa.toml")).unwrap();
    assert!(
        toml.contains(&format!("rev = \"{head}\"")),
        "plain moved: {toml}"
    );
    assert!(
        toml.contains(&format!("rev = \"{first}\"")),
        "producer stayed: {toml}"
    );
    let r = run(&consumer, &["tonpa", "verify"]);
    assert_eq!(r.code, 0, "the lock matches the disk: {}", r.stdout);
}

/// `--root <path> --at <locked rev>`, by name: the producer's later commit is not read.
#[test]
fn query_dependency_reads_the_pin_at_its_locked_commit() {
    let (_tmp, consumer, producer, sha) = pinned_fixture();
    assert_eq!(
        run(&consumer, &["tonpa", "add", &format!("../producer@{sha}")]).code,
        0
    );
    commit_beta(&producer);

    let r = run(
        &consumer,
        &["query", "gauge-station", "--dependency", "producer"],
    );
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    assert!(
        r.stdout.contains("alpha") && !r.stdout.contains("beta"),
        "{}",
        r.stdout
    );

    let r = run(
        &consumer,
        &["query", "gauge-station", "--dependency", "nobody"],
    );
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("no dependency named"), "{}", r.stderr);

    // Declared at one commit, locked at another.
    let toml_path = consumer.join(".yidam/tonpa.toml");
    let head = common::git::out(&producer, &["rev-parse", "HEAD"]);
    let toml = std::fs::read_to_string(&toml_path)
        .unwrap()
        .replace(&sha, &head);
    std::fs::write(&toml_path, toml).unwrap();
    let r = run(
        &consumer,
        &["query", "gauge-station", "--dependency", "producer"],
    );
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("tonpa install"), "{}", r.stderr);
}

#[test]
fn query_dependency_refuses_a_live_path_and_names_the_root() {
    let (_tmp, consumer) = producer_and_consumer("");
    let r = run(&consumer, &["query", "concept", "--dependency", "producer"]);
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("--root"), "{}", r.stderr);

    let r = run(
        &consumer,
        &[
            "query",
            "concept",
            "--dependency",
            "producer",
            "--root",
            ".",
        ],
    );
    assert_ne!(r.code, 0, "clap refuses the pair");
}

/// `[[derive.records]]` over an export cites the dependency's corpus as installed at the pin,
/// and is refused until `tonpa install` has written it (RFC-0063 §3.3).
#[test]
fn derive_records_read_an_export_against_the_corpus_at_the_pin() {
    let (_tmp, consumer, producer, _) = pinned_fixture();
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/alpha.yml"),
        "class: gauge-station\nlabel: alpha\nnote: Read weekly by the district [verified].\n",
    )
    .unwrap();
    common::git::git(&producer, &["add", "-A"]);
    common::git::git(
        &producer,
        &["commit", "-q", "--no-gpg-sign", "-m", "tag alpha"],
    );
    let sha = compute(
        &producer,
        r#"{"r": [{"node": "gauge-station/alpha", "span": "Read weekly", "tier": "verified"}]}"#,
    );
    add_with_export(&consumer, &sha);
    std::fs::write(
        consumer.join(".yidam/config.toml"),
        "[[derive.records]]\nexport = \"producer\"\nrecords = [\"/r/*\"]\ntier = \"0/tier\"\n",
    )
    .unwrap();

    let r = run(&consumer, &["derive", "check", "--format", "json"]);
    assert_eq!(r.code, 0, "stdout: {}\nstderr: {}", r.stdout, r.stderr);
    let doc: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(doc["records"], 1, "{doc}");
    let d = &doc["derivations"][0];
    assert_eq!(d["path"], ".yidam/tonpa/producer/export/pack.json");
    assert_eq!(d["record"], "/r/0");
    assert_eq!(d["tier"], "verified");

    // The producer's working tree is never read: an edit there changes nothing.
    std::fs::write(
        producer.join(".yidam/corpus/gauge-station/alpha.yml"),
        "class: gauge-station\nlabel: alpha\n",
    )
    .unwrap();
    assert_eq!(run(&consumer, &["derive", "check"]).code, 0);

    std::fs::remove_dir_all(consumer.join(".yidam/tonpa/producer/export")).unwrap();
    // Reported, not propagated: the refusal is a finding on stdout, in either format.
    let r = run(&consumer, &["derive", "check"]);
    assert_ne!(r.code, 0);
    assert!(r.stdout.contains("tonpa install"), "stdout: {}", r.stdout);
    let r = run(&consumer, &["derive", "check", "--format", "json"]);
    assert_ne!(r.code, 0);
    let doc: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(
        doc["derivations"][0]["findings"][0]["check"],
        "derive-record-unmatched"
    );
}
