//! `scripts/spec-bundle.sh` must pack exactly what it claims to (RFC-0068 §5).
//!
//! The release runs the script once and uploads what it writes, so this is the only place
//! the asset's shape is checked before a producer downloads it. It runs the script into a
//! temporary directory, unpacks the tarball, and fails when:
//!
//! - a file in the tarball has no `files` entry in `manifest.json`, or an entry names a file
//!   the tarball does not hold, or one whose bytes differ from its source;
//! - a `versions` value differs from the committed file it is read from;
//! - a committed file under `yidam/sdks/conformance/` is missing from the tarball.
//!
//! Neither the script nor this gate calls the network.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Read;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use crate::common;

const VERSION: &str = "0.0.0-test";
const COMMIT: &str = "0000000000000000000000000000000000000000";

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

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap_or_else(|e| panic!("reading {rel}: {e}"))
}

#[test]
fn the_asset_holds_what_its_manifest_names_and_nothing_else() {
    let root = common::repo_root();
    let out = tempfile::tempdir().unwrap();
    let run = Command::new("bash")
        .arg(root.join("scripts/spec-bundle.sh"))
        .args([VERSION, COMMIT])
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
    assert_eq!(manifest["commit"], COMMIT);

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
        let source = std::fs::read(root.join(src)).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert!(files[rel] == source, "{rel} differs from its source {src}");
    }

    // Each version, from the one committed file it is read from.
    let report: Value = serde_json::from_str(&read(
        &root,
        "yidam/sdks/parity/fixtures/reports/report.schema.json",
    ))
    .unwrap();
    let vocabulary: Value =
        serde_json::from_str(&read(&root, "yidam/sdks/conformance/vocabulary.json")).unwrap();
    let ledger = read(&root, "yidam/sdks/parity/mcp/CONTRACT_SHA");
    let last_record = ledger
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .next_back()
        .expect("a CONTRACT_SHA record");
    let versions = &manifest["versions"];
    assert_eq!(
        versions["report"],
        report["properties"]["format_version"]["const"]
    );
    assert_eq!(
        versions["mcp_contract"],
        read(&root, "yidam/sdks/parity/mcp/VERSION").trim()
    );
    assert_eq!(
        versions["contract_sha"],
        last_record.split_whitespace().nth(1).unwrap()
    );
    assert_eq!(versions["vocabulary"], vocabulary["format_version"]);
    assert_eq!(
        versions["conformance"],
        read(&root, "yidam/sdks/conformance/VERSION").trim()
    );

    // Every committed conformance file ships.
    let shipped: BTreeSet<&str> = listed.values().map(String::as_str).collect();
    let missing: Vec<String> = common::tracked_under(&root, "yidam/sdks/conformance/")
        .into_iter()
        .filter(|f| !shipped.contains(f.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "committed under yidam/sdks/conformance/ and missing from the asset: {missing:?}"
    );
}
