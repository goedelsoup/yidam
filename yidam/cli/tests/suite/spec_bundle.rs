//! `scripts/spec-bundle.sh` must pack exactly what it claims to (RFC-0068 §5).
//!
//! The release runs the script once and uploads what it writes, so this is the only place
//! the asset's shape is checked before a producer downloads it. It runs the script into a
//! temporary directory, unpacks the tarball, and fails when:
//!
//! - a file in the tarball has no `files` entry in `manifest.json`, or an entry names a file
//!   the tarball does not hold, or one whose bytes differ from its source;
//! - a `versions` value differs from the committed file it is read from;
//! - a committed file under `yidam/sdks/conformance/` is missing from the tarball;
//! - `manifest.json`'s `commit` is not the commit the script was handed.
//!
//! Every comparison is against `HEAD`'s tree, because that is what the script packs: an
//! uncommitted edit is not in the asset, so the working tree is not what it is checked against.
//!
//! Neither the script nor this gate calls the network.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::common;

const VERSION: &str = "0.0.0-test";

/// Every regular file in the tarball, keyed by its path under `yidam-spec-<version>/`.
fn unpack(tarball: &Path) -> BTreeMap<String, Vec<u8>> {
    let file = std::fs::File::open(tarball).expect("the tarball");
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(file));
    let prefix = format!("yidam-spec-{VERSION}/");
    let mut out = BTreeMap::new();
    for entry in archive.entries().expect("tar entries") {
        let mut entry = entry.expect("a tar entry");
        let path = entry.path().unwrap().to_string_lossy().to_string();
        assert!(
            path.starts_with(&prefix) || path == prefix.trim_end_matches('/'),
            "{path} is outside {prefix}"
        );
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        out.insert(path[prefix.len()..].to_string(), bytes);
    }
    out
}

/// The bytes of `rel` in `commit`'s tree.
fn blob(root: &Path, commit: &str, rel: &str) -> Vec<u8> {
    let out = common::git::raw(root, &["cat-file", "blob", &format!("{commit}:{rel}")]);
    assert!(out.status.success(), "{rel} is not in {commit}");
    out.stdout
}

fn read(root: &Path, commit: &str, rel: &str) -> String {
    String::from_utf8(blob(root, commit, rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
}

#[test]
fn the_asset_holds_what_its_manifest_names_and_nothing_else() {
    let root = common::repo_root();
    let commit = common::git::out(&root, &["rev-parse", "HEAD"]);
    let out = tempfile::tempdir().unwrap();
    let run = Command::new("bash")
        .arg(root.join("scripts/spec-bundle.sh"))
        .args([VERSION, commit.as_str()])
        .arg(out.path())
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "spec-bundle.sh failed:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let name = format!("yidam-spec-{VERSION}.tar.gz");
    assert!(
        out.path().join(format!("{name}.sha256")).is_file(),
        "no checksum beside the tarball"
    );

    let mut files = unpack(&out.path().join(&name));
    let manifest: Value =
        serde_json::from_slice(&files.remove("manifest.json").expect("manifest.json"))
            .expect("manifest.json parses");
    assert_eq!(manifest["format_version"], "1");
    assert_eq!(manifest["cli"], VERSION);
    assert_eq!(manifest["commit"], commit.as_str());

    // Complete in both directions, and each entry is a copy of its source.
    let listed: BTreeMap<String, String> = manifest["files"]
        .as_object()
        .expect("files")
        .iter()
        .map(|(k, v)| (k.clone(), v.as_str().expect("a source path").to_string()))
        .collect();
    let in_tarball: BTreeSet<&String> = files.keys().collect();
    let in_manifest: BTreeSet<&String> = listed.keys().collect();
    let unlisted: Vec<_> = in_tarball.difference(&in_manifest).collect();
    let absent: Vec<_> = in_manifest.difference(&in_tarball).collect();
    assert!(
        unlisted.is_empty(),
        "in the tarball with no `files` entry: {unlisted:?}"
    );
    assert!(
        absent.is_empty(),
        "named by `files` and not in the tarball: {absent:?}"
    );
    for (rel, src) in &listed {
        let source = blob(&root, &commit, src);
        assert!(files[rel] == source, "{rel} differs from {src} at {commit}");
    }

    // Each version, from the one committed file it is read from.
    let read = |rel: &str| read(&root, &commit, rel);
    let report: Value = serde_json::from_str(&read(
        "yidam/sdks/parity/fixtures/reports/report.schema.json",
    ))
    .unwrap();
    let vocabulary: Value =
        serde_json::from_str(&read("yidam/sdks/conformance/vocabulary.json")).unwrap();
    let ledger = read("yidam/sdks/parity/mcp/CONTRACT_SHA");
    let last_record = ledger
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.trim_start().starts_with('#'))
        .next_back()
        .expect("a CONTRACT_SHA record");
    let versions = &manifest["versions"];
    let contract = read("yidam/sdks/parity/mcp/VERSION");
    let mut record = last_record.split_whitespace();
    assert_eq!(
        record.next(),
        Some(contract.trim()),
        "CONTRACT_SHA's last record must be for parity/mcp/VERSION"
    );
    assert_eq!(
        versions["report"],
        report["properties"]["format_version"]["const"]
    );
    assert_eq!(versions["mcp_contract"], contract.trim());
    assert_eq!(versions["contract_sha"], record.next().unwrap());
    assert_eq!(versions["vocabulary"], vocabulary["format_version"]);
    assert_eq!(
        versions["conformance"],
        read("yidam/sdks/conformance/VERSION").trim()
    );

    // Every committed conformance file ships.
    let shipped: BTreeSet<&str> = listed.values().map(String::as_str).collect();
    let committed = common::git::out(
        &root,
        &[
            "ls-tree",
            "-r",
            "--name-only",
            &commit,
            "--",
            "yidam/sdks/conformance/",
        ],
    );
    let missing: Vec<&str> = committed.lines().filter(|f| !shipped.contains(f)).collect();
    assert!(
        missing.is_empty(),
        "committed under yidam/sdks/conformance/ and missing from the asset: {missing:?}"
    );
}
