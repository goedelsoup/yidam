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
