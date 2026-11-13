//! `.yidam/asks/` end to end (#1467): what `open-questions` emits for an ask, held against
//! the committed report schema.
//!
//! The shared reports fixture holds no dependency, and giving it one would move `doctor`,
//! `lint` and `status` for three SDK runners and the extension that grade against it. So
//! `asks` is exempted from the golden matrix's reachability roster, and this is the one place
//! its declaration is held against a real document. `report_goldens.rs`'s `UNREACHED` names
//! this file.

use std::path::Path;
use std::process::Command;

use crate::common;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// A repository depending on one corpus, with one ask about a node and one about the whole.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        root,
        ".yidam/corpus/concept/datum.yml",
        "class: concept\nlabel: Datum\n",
    );
    write(
        root,
        ".yidam/tonpa.toml",
        "[dependencies.up]\npath = \"../up\"\nrev = \"8d354410\"\n",
    );
    write(
        root,
        ".yidam/asks/up/base-flow.md",
        "---\npin: 8d354410\nnode: concept/base-flow\nstatus: open\n---\n# Does base flow include bank storage?\n",
    );
    write(
        root,
        ".yidam/asks/up/the-pack.md",
        "---\npin: 8d354410\nstatus: open\n---\nA question to the corpus as a whole.\n",
    );
    let git = |args: &[&str]| common::git::git_at(root, args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: asks fixture"]);
    tmp
}

fn json(root: &Path, args: &[&str]) -> serde_json::Value {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .args(["--format", "json"])
        .output()
        .unwrap();
    serde_json::from_str(&String::from_utf8_lossy(&out.stdout))
        .unwrap_or_else(|e| panic!("`yidam {}` did not emit JSON: {e}", args.join(" ")))
}

#[test]
fn every_field_open_questions_emits_for_an_ask_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../sdks/parity/fixtures/reports/report.schema.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let tmp = stage();
    let doc = json(tmp.path(), &["open-questions"]);

    let asks = doc["asks"].as_array().expect("an `asks` array");
    assert_eq!(asks.len(), 2, "{doc}");
    assert!(
        doc["open_questions"].as_array().unwrap().is_empty(),
        "an ask is listed beside the corpus's questions, never among them: {doc}"
    );

    let mut emitted = std::collections::BTreeSet::new();
    common::paths_of(&doc, "", &mut emitted);
    for path in &emitted {
        assert!(
            common::declares(&schema, path),
            "`open-questions` emits `{path}`, which report.schema.json does not declare"
        );
    }
    for required in schema["properties"]["asks"]["items"]["required"]
        .as_array()
        .unwrap()
    {
        let key = required.as_str().unwrap();
        assert!(asks.iter().all(|a| a.get(key).is_some()), "{key}: {doc}");
    }
    // The walk descended into the items, and the optional field was reached by the ask that
    // names a node. Without this the loop above is satisfied by an empty array.
    for witness in [
        "asks[].ask",
        "asks[].package",
        "asks[].node",
        "asks[].pin",
        "asks[].title",
    ] {
        assert!(
            emitted.contains(witness),
            "never emitted `{witness}`: {emitted:?}"
        );
    }
    assert!(
        !common::declares(&schema, "asks[].nonesuch"),
        "`declares` said yes to a field the schema does not carry, so the check above holds nothing"
    );
}
