//! The cluster image is built wherever it can break, and published where the CLI is (#1227).
//!
//! `docs/cluster/Dockerfile` went weeks with no workflow building it. `cluster-image.yml` now
//! builds it on pull requests and publishes it from `release.yml`. What this file holds is
//! that the pull-request half keeps reaching the image's inputs. A path filter is a roster,
//! and a roster stops covering the next path dependency without ever going red.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

mod common;

fn read(rel: &str) -> String {
    let p = common::repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

fn workflow() -> serde_yaml::Value {
    serde_yaml::from_str(&read(".github/workflows/cluster-image.yml"))
        .expect("cluster-image.yml is not parseable YAML")
}

/// Every `path =` dependency reachable from `yidam/cli`, as repository-relative directories.
///
/// Every dependency table, dev-dependencies included: cargo loads a path dev-dependency's
/// manifest to build the package at all, so the image build breaks when one moves.
/// Transitive, because a path dependency of a path dependency is copied and compiled the same
/// way.
fn path_dependencies() -> BTreeSet<String> {
    let root = common::repo_root().canonicalize().unwrap();
    let mut seen = BTreeSet::new();
    let mut queue: Vec<PathBuf> = vec![root.join("yidam/cli")];
    while let Some(dir) = queue.pop() {
        let manifest: toml::Table = std::fs::read_to_string(dir.join("Cargo.toml"))
            .unwrap_or_else(|e| panic!("{}/Cargo.toml unreadable: {e}", dir.display()))
            .parse()
            .expect("Cargo.toml parses");
        let mut tables: Vec<&toml::Table> = Vec::new();
        for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(t) = manifest.get(key).and_then(|v| v.as_table()) {
                tables.push(t);
            }
        }
        if let Some(targets) = manifest.get("target").and_then(|v| v.as_table()) {
            for cfg in targets.values().filter_map(|v| v.as_table()) {
                for key in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(t) = cfg.get(key).and_then(|v| v.as_table()) {
                        tables.push(t);
                    }
                }
            }
        }
        for dep in tables.iter().flat_map(|t| t.values()) {
            let Some(path) = dep.get("path").and_then(|p| p.as_str()) else {
                continue;
            };
            let resolved = dir.join(path).canonicalize().unwrap_or_else(|e| {
                panic!("{} names path `{path}`, unresolvable: {e}", dir.display())
            });
            let rel = resolved
                .strip_prefix(&root)
                .expect("a path dependency outside the repository")
                .display()
                .to_string();
            if seen.insert(rel) {
                queue.push(resolved);
            }
        }
    }
    seen
}

/// Whether a `paths:` filter entry covers every file under `dir`.
fn covers(filter: &str, dir: &str) -> bool {
    filter == format!("{dir}/**")
        || filter
            .strip_suffix("/**")
            .is_some_and(|parent| Path::new(dir).starts_with(parent))
}

/// Both non-publishing triggers list the image's inputs: the Dockerfile, its ignore file, the
/// CLI crate, and every path dependency the CLI reaches, discovered from the manifests.
#[test]
fn the_pull_request_build_reaches_every_input_of_the_image() {
    let deps = path_dependencies();
    // The two the issue names, so a manifest reader that finds nothing cannot pass.
    for known in ["yidam/prelude/sdks/rust", "yidam/tests/harness/ci-report"] {
        assert!(
            deps.contains(known),
            "the manifest walk did not find `{known}`, which yidam/cli/Cargo.toml depends on \
             by path; found {deps:?}"
        );
    }

    let required: Vec<String> = vec![
        "docs/cluster/Dockerfile".into(),
        "docs/cluster/Dockerfile.dockerignore".into(),
    ];
    let dirs: Vec<String> = std::iter::once("yidam/cli".to_string())
        .chain(deps)
        .collect();

    let w = workflow();
    for trigger in ["pull_request", "push"] {
        let filters: Vec<&str> = w["on"][trigger]["paths"]
            .as_sequence()
            .unwrap_or_else(|| panic!("cluster-image.yml's `{trigger}` trigger has no `paths`"))
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        let mut missing: Vec<&str> = required
            .iter()
            .filter(|f| !filters.contains(&f.as_str()))
            .map(String::as_str)
            .collect();
        missing.extend(
            dirs.iter()
                .filter(|d| !filters.iter().any(|f| covers(f, d)))
                .map(String::as_str),
        );
        assert!(
            missing.is_empty(),
            "cluster-image.yml's `{trigger}` paths miss {missing:?}. The image compiles them, \
             so a change there can break the Dockerfile build on a pull request that never \
             runs it, and it is found at the tag instead (#871)."
        );
    }
}

/// A tag publishes the image, from the one set of steps pull requests run.
#[test]
fn a_cli_tag_publishes_the_image_through_the_same_workflow() {
    let release: serde_yaml::Value =
        serde_yaml::from_str(&read(".github/workflows/release.yml")).expect("release.yml parses");
    let jobs = release["jobs"].as_mapping().expect("release.yml has jobs");
    let callers: Vec<&serde_yaml::Value> = jobs
        .values()
        .filter(|j| j["uses"].as_str() == Some("./.github/workflows/cluster-image.yml"))
        .collect();
    assert_eq!(
        callers.len(),
        1,
        "release.yml should call cluster-image.yml exactly once, and calls it {} times",
        callers.len()
    );
    let job = callers[0];
    assert_eq!(
        job["with"]["tag"].as_str(),
        Some("${{ github.ref_name }}"),
        "the image job must hand cluster-image.yml the tag, or it builds and pushes nothing"
    );
    for scope in ["packages", "id-token", "attestations"] {
        assert_eq!(
            job["permissions"][scope].as_str(),
            Some("write"),
            "the image job does not grant `{scope}: write`. A caller's permissions are the \
             called workflow's ceiling, so the push or the attestation would be refused."
        );
    }

    // What the called workflow publishes on: two platforms, an SBOM, an attestation.
    let text = read(".github/workflows/cluster-image.yml");
    for needle in [
        "linux/amd64",
        "linux/arm64",
        "sbom:",
        "actions/attest-build-provenance@",
        "gh attestation verify",
    ] {
        assert!(
            text.lines()
                .map(|l| l.split('#').next().unwrap_or(""))
                .any(|l| l.contains(needle)),
            "cluster-image.yml no longer carries `{needle}` outside a comment"
        );
    }

    // release.sh refuses a tag whose workflows are absent at HEAD, and a local `uses:` resolves
    // at the caller's ref, so the called file is one of the CLI layer's.
    let cli_line = read("release.sh")
        .lines()
        .find(|l| l.contains("TAG=\"cli/v"))
        .expect("release.sh has no cli layer")
        .to_string();
    assert!(
        cli_line.contains(".github/workflows/cluster-image.yml"),
        "release.sh's cli layer does not name cluster-image.yml: {cli_line}"
    );
}

#[test]
fn covers_reads_a_directory_glob_and_nothing_looser() {
    assert!(covers("yidam/cli/**", "yidam/cli"));
    assert!(covers("yidam/**", "yidam/prelude/sdks/rust"));
    assert!(!covers("yidam/cli/**", "yidam/cli-other"));
    assert!(!covers("yidam/cli/src/**", "yidam/cli"));
    assert!(!covers("docs/cluster/Dockerfile", "docs/cluster"));
}
