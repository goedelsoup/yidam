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
