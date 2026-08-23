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
    assert!(Command::new("git")
        .args(["init", "-q"])
        .current_dir(&consumer)
        .status()
        .unwrap()
        .success());
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
