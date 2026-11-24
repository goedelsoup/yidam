//! `[open — unanswerable: <reason>]` end to end (#1506, RFC-0061): what `open-questions` and
//! `status` emit for a marked claim, held against the committed report schema.
//!
//! The shared reports fixture writes no marked claim, and giving it one would move `due`,
//! `status` and `open-questions` for three SDK runners and the extension that grade against
//! it. So `open_questions[].unanswerable` and `open_questions[].reasons` are exempted from
//! the golden matrix's reachability roster, and this is the one place their declaration is
//! held against a real document. `report_goldens.rs`'s `UNREACHED` names this file.

use std::path::Path;
use std::process::Command;

use crate::common;

fn write(root: &Path, rel: &str, text: &str) {
    let p = root.join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

/// Three nodes: one wholly terminal, one part terminal and part owed, one plainly open.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    write(
        root,
        ".yidam/corpus/concept/sealed.yml",
        "class: concept\nlabel: Sealed\ndescription: |\n  Whether the file survives is [open — unanswerable: not in the public record].\n",
    );
    write(
        root,
        ".yidam/corpus/concept/mixed.yml",
        "class: concept\nlabel: Mixed\ndescription: |\n  The first date is [open — unanswerable: superseded].\n  The second date is [open].\n",
    );
    write(
        root,
        ".yidam/corpus/concept/plain.yml",
        "class: concept\nlabel: Plain\ndescription: |\n  The cause is [open].\n",
    );
    let git = |args: &[&str]| common::git::git_at(root, args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: unanswerable fixture"]);
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

fn schema() -> serde_json::Value {
    serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../sdks/parity/fixtures/reports/report.schema.json"),
        )
        .unwrap(),
    )
    .unwrap()
}

fn entry<'a>(doc: &'a serde_json::Value, label: &str) -> &'a serde_json::Value {
    doc["open_questions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["label"] == label)
        .unwrap_or_else(|| panic!("no entry labelled {label}: {doc}"))
}

#[test]
fn every_field_a_marked_claim_emits_is_declared_in_the_schema() {
    let schema = schema();
    let tmp = stage();
    let doc = json(tmp.path(), &["open-questions"]);

    // A marked claim is still an open question: all three are listed.
    assert_eq!(doc["open_questions"].as_array().unwrap().len(), 3, "{doc}");
    let sealed = entry(&doc, "Sealed");
    assert_eq!(sealed["unanswerable"], "not in the public record", "{doc}");
    assert_eq!(
        sealed["reasons"],
        serde_json::json!(["not in the public record"]),
        "{doc}"
    );
    let mixed = entry(&doc, "Mixed");
    assert!(mixed.get("unanswerable").is_none(), "{doc}");
    assert_eq!(mixed["reasons"], serde_json::json!(["superseded"]), "{doc}");
    let plain = entry(&doc, "Plain");
    assert!(plain.get("unanswerable").is_none() && plain.get("reasons").is_none());

    let mut emitted = std::collections::BTreeSet::new();
    common::paths_of(&doc, "", &mut emitted);
    for path in &emitted {
        assert!(
            common::declares(&schema, path),
            "`open-questions` emits `{path}`, which report.schema.json does not declare"
        );
    }
    for witness in ["open_questions[].unanswerable", "open_questions[].reasons"] {
        assert!(
            emitted.contains(witness),
            "never emitted `{witness}`: {emitted:?}"
        );
    }
    assert!(
        !common::declares(&schema, "open_questions[].nonesuch"),
        "`declares` said yes to a field the schema does not carry, so the check above holds nothing"
    );
}

#[test]
fn status_counts_the_marked_question_among_the_open_ones() {
    let tmp = stage();
    let doc = json(tmp.path(), &["status"]);
    assert_eq!(doc["open_questions"], 3, "{doc}");
    assert_eq!(doc["unanswerable"], 1, "{doc}");
}
