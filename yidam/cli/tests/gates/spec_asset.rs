//! The spec asset's committed sources must be what the binary emits (RFC-0068 §2, §4).
//!
//! `yidam/sdks/conformance/` is what a release packs into `yidam-spec-<version>.tar.gz`, and
//! a producer that never builds yidam diffs its own copies against it. So a committed schema
//! that drifted from `yidam schema`, or a vocabulary that drifted from the certified lists,
//! would hand every such producer the wrong contract under a release's name.
//!
//! The two comparisons are byte for byte, and `UPDATE_GOLDENS=1` rewrites both, as with the
//! other goldens. So a change to a node's shape, or a verb added to the vocabulary, is a diff
//! under `conformance/` in the same PR, where a reviewer sees it.

use std::collections::BTreeSet;
use std::path::PathBuf;

use crate::common::repo_root;

fn conformance() -> PathBuf {
    repo_root().join("yidam/sdks/conformance")
}

fn updating() -> bool {
    std::env::var("UPDATE_GOLDENS").is_ok()
}

/// The five repository-independent schemas, exactly as `yidam schema` writes them.
///
/// Also fails on a file under `schemas/` that `schema::all()` does not name: a schema the
/// binary stopped emitting would otherwise ship in every later asset as though it held.
#[test]
fn committed_schemas_are_what_yidam_schema_writes() {
    let dir = conformance().join("schemas");
    let rendered = yidam::core_schemas_rendered().expect("schemas render");
    let named: BTreeSet<String> = rendered.iter().map(|(f, _)| f.to_string()).collect();

    if updating() {
        std::fs::create_dir_all(&dir).unwrap();
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            if !named.contains(&name) {
                std::fs::remove_file(entry.path()).unwrap();
            }
        }
        for (file, text) in &rendered {
            std::fs::write(dir.join(file), text).unwrap();
        }
        return;
    }

    let on_disk: BTreeSet<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    let extra: Vec<_> = on_disk.difference(&named).collect();
    assert!(
        extra.is_empty(),
        "{} holds {extra:?}, which `schema::all()` does not emit. Re-run with \
         UPDATE_GOLDENS=1 to drop them.",
        dir.display()
    );
    for (file, text) in &rendered {
        let path = dir.join(file);
        let committed = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "missing {}. Re-run with UPDATE_GOLDENS=1 to write it.",
                path.display()
            )
        });
        assert_eq!(
            &committed,
            text,
            "\n{} drifted from what `yidam schema` writes. If the change is intended, re-run \
             with UPDATE_GOLDENS=1 and review the diff.\n",
            path.display()
        );
    }
}

/// The certified verbs and their kinds, sorted by kind and then by verb.
fn vocabulary() -> String {
    let mut verbs: Vec<(&str, &str)> = yidam_core::git::EPISTEMIC_VERBS
        .iter()
        .map(|v| ("epistemic", *v))
        .chain(
            yidam_core::git::OPERATIONAL_VERBS
                .iter()
                .map(|v| ("operational", *v)),
        )
        .collect();
    verbs.sort();
    let body = serde_json::json!({
        "format_version": "1",
        "verbs": verbs
            .iter()
            .map(|(kind, verb)| serde_json::json!({ "verb": verb, "kind": kind }))
            .collect::<Vec<_>>(),
    });
    format!("{}\n", serde_json::to_string_pretty(&body).unwrap())
}

#[test]
fn committed_vocabulary_is_the_certified_lists() {
    let path = conformance().join("vocabulary.json");
    let expected = vocabulary();
    if updating() {
        std::fs::write(&path, &expected).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(&path).unwrap_or_else(|_| {
        panic!(
            "missing {}. Re-run with UPDATE_GOLDENS=1 to write it.",
            path.display()
        )
    });
    assert_eq!(
        committed,
        expected,
        "\n{} drifted from OPERATIONAL_VERBS and EPISTEMIC_VERBS. If the change is intended, \
         re-run with UPDATE_GOLDENS=1 and review the diff.\n",
        path.display()
    );
}
