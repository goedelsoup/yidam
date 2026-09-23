//! `yidam regen --check` — REGEN freshness as a verdict.
//!
//! Its own file rather than a row in the golden matrix, and the reason is the defect this
//! command exists to close. The matrix stages one fixture and runs every command over it in
//! order; `yidam status` **writes** the REGEN block it renders, so by the time a shared
//! `regen --check` ran it would find the block current — and report the passing arm no
//! matter what the fixture held. A golden that flips when somebody reorders a list is worse
//! than no golden.
//!
//! So: stage per case, assert both arms.

use std::path::{Path, PathBuf};
use std::process::Command;

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/fixtures/reports/basic")
}

fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    for entry in walkdir::WalkDir::new(fixture_dir().join("repo"))
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry
            .path()
            .strip_prefix(fixture_dir().join("repo"))
            .unwrap()
            .to_path_buf();
        let dest = tmp.path().join(&rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(tmp.path())
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: reports fixture"]);
    tmp
}

struct Run {
    stdout: String,
    code: i32,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn readme(root: &Path) -> String {
    std::fs::read_to_string(root.join("README.md")).unwrap()
}

/// The fixture ships a placeholder in its `yidam status` block, so a fresh copy is stale.
#[test]
fn a_stale_block_is_named_and_gates() {
    let tmp = stage();
    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 1, "a stale block has to fail, or it is not a gate");
    assert!(r.stdout.contains("REGEN block(s) stale"), "{}", r.stdout);
    assert!(r.stdout.contains("README.md"), "{}", r.stdout);
    assert!(
        r.stdout.contains("(status)"),
        "names the generator: {}",
        r.stdout
    );
}

/// The whole promise: it reports without writing.
///
/// This is what lets it run against a tree with work in flight, which the run-and-diff step
/// it replaces cannot do — that one needs a clean tree to have anything to diff against.
#[test]
fn checking_writes_nothing() {
    let tmp = stage();
    let before = readme(tmp.path());
    run(tmp.path(), &["regen", "--check"]);
    assert_eq!(
        readme(tmp.path()),
        before,
        "--check rewrote the file it checked"
    );

    // And the generators' own output is suppressed, or a JSON report would have thirty
    // lines of corpus index in front of it.
    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    assert!(
        json.stdout.trim_start().starts_with('{'),
        "not a JSON document: {}",
        &json.stdout[..json.stdout.len().min(200)]
    );
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["stale"][0]["generator"], "status");
}

/// Run the generators for real, then ask again. The gate has to be satisfiable.
#[test]
fn regen_then_check_passes() {
    let tmp = stage();
    let wrote = run(tmp.path(), &["regen"]);
    assert_eq!(wrote.code, 0, "{}", wrote.stdout);
    assert_ne!(readme(tmp.path()), "", "regen wrote something");

    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    assert!(
        r.stdout.contains("Every REGEN block is current"),
        "{}",
        r.stdout
    );

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["passed"], true);
    assert_eq!(doc["stale"].as_array().unwrap().len(), 0);
}

/// A `vault-status` block is one this gate reports, which it was not until #831.
///
/// The block sat outside the generator list, so `yidam regen` did not populate it and
/// `--check` did not call it stale. In the corpus that reported it, the block held its "run
/// this to populate" placeholder from genesis through sixty-two phases and every CI run —
/// empty at first, and simply *wrong* from the day a vault was declared.
///
/// Staged onto the fixture rather than added to it: the fixture is the parity corpus the SDKs
/// are graded against, and a block here is not a fact about `update_regen`.
#[test]
fn a_vault_status_block_is_reported_stale_and_can_be_satisfied() {
    let tmp = stage();
    let staged = format!(
        "{}\n## Artifacts\n\n<!-- REGEN: yidam vault-status\n-->\n\
         _Run `yidam regen` to populate._\n<!-- /REGEN -->\n",
        readme(tmp.path())
    );
    std::fs::write(tmp.path().join("README.md"), &staged).unwrap();

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    let stale: Vec<&str> = doc["stale"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["generator"].as_str().unwrap())
        .collect();
    assert_eq!(json.code, 1, "{}", json.stdout);
    assert!(stale.contains(&"vault-status"), "stale: {stale:?}");

    // Satisfiable, and by the command the gate's own message prescribes.
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert_eq!(run(tmp.path(), &["regen", "--check"]).code, 0);
    assert!(
        readme(tmp.path()).contains("No vault configured"),
        "the block holds what the generator renders: {}",
        readme(tmp.path())
    );
}

/// Both formats agree on the verdict, as every other gate in this contract does.
#[test]
fn exit_codes_are_identical_across_formats() {
    let tmp = stage();
    for expected in [1, 0] {
        let text = run(tmp.path(), &["regen", "--check"]);
        let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
        assert_eq!(text.code, expected);
        assert_eq!(text.code, json.code);
        if expected == 1 {
            run(tmp.path(), &["regen"]);
        }
    }
}

/// Every field the report emits is declared in the committed schema.
///
/// The golden matrix's own version of this check only sees fields that appear in a golden,
/// and this report has none — see the module comment for why.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let tmp = stage();
    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();

    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().parent().unwrap().join("report.schema.json"))
            .unwrap(),
    )
    .unwrap();
    let declared = schema["properties"].as_object().unwrap();

    for key in doc.as_object().unwrap().keys() {
        assert!(
            declared.contains_key(key),
            "`regen --check` emits `{key}`, which report.schema.json does not declare"
        );
    }
    for required in schema["required"].as_array().unwrap() {
        assert!(doc.get(required.as_str().unwrap()).is_some(), "{required}");
    }
}

/// The gate's verdict does not move when a build artifact appears in the tree.
///
/// **This is the whole of #895.** Three generators decided what to write by stat-ing a path
/// nothing commits: `status` rendered `index present` / `index not initialized`,
/// `index-status` rendered a build date and a stale-node count, and `bundle-status` rendered
/// a byte size. All three blocks are committed and all three are gated here, so
/// `regen --check` answered about the machine rather than about the commit. Measured on the
/// reporting corpus, same tree and same commit: `mv .yidam/index /tmp` took it from exit 1 to
/// exit 0. CI never has either artifact and so never saw the failure — it landed on the
/// contributors who had adopted `--features index`, which the repository's own decision
/// record had invited them to do.
///
/// Two artifacts and not one: `bundle-status` was found by asking #895's question of the
/// other generators, and a test covering only the index would have gone green over it.
///
/// The arms are asserted in this order on purpose. Green-then-green proves nothing on its
/// own — a check that had stopped looking at these blocks would pass both. So the first arm
/// stages the blocks stale and requires all three to be *named*, which is what establishes
/// that the gate can see them at all; only then is the artifact introduced.
#[test]
fn a_built_index_or_bundle_does_not_move_the_gate() {
    let tmp = stage();
    let root = tmp.path();

    // Both blocks, staged onto the fixture rather than added to it: the fixture is the parity
    // corpus the SDKs are graded against, and a block here is not a fact about `update_regen`.
    let placeholder = |marker: &str| {
        format!("\n<!-- REGEN: yidam {marker}\n-->\n_placeholder_\n<!-- /REGEN -->\n")
    };
    let corpus_readme = root.join(".yidam/corpus/README.md");
    let staged = std::fs::read_to_string(&corpus_readme).unwrap() + &placeholder("index-status");
    std::fs::write(&corpus_readme, staged).unwrap();
    std::fs::create_dir_all(root.join("web")).unwrap();
    std::fs::write(
        root.join("web/README.md"),
        format!("# web\n{}", placeholder("bundle-status")),
    )
    .unwrap();

    // The gate sees all three, or the rest of this test is vacuous.
    let stale_now = |root: &Path| -> Vec<String> {
        let json = run(root, &["regen", "--check", "--format", "json"]);
        let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
        doc["stale"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["generator"].as_str().unwrap().to_string())
            .collect()
    };
    let named = stale_now(root);
    for generator in ["status", "index-status", "bundle-status"] {
        assert!(
            named.iter().any(|g| g == generator),
            "`{generator}`'s block is staged stale and the gate did not name it, so this test \
             is not watching it: {named:?}"
        );
    }

    assert_eq!(run(root, &["regen"]).code, 0);
    assert_eq!(
        run(root, &["regen", "--check"]).code,
        0,
        "the gate has to be satisfiable before it can be shown to be stable"
    );

    // Now the artifacts. Neither is in any commit; both are what `--features index` and
    // `yidam export --format bundle` leave in a working tree.
    std::fs::create_dir_all(root.join(".yidam/index")).unwrap();
    std::fs::write(
        root.join(".yidam/index/meta.json"),
        r#"{"generated_at":1780000000,"model_name":"bge-small-en-v1.5",
            "embedding_dim":384,"node_count":4,"indexed_commit":"deadbeef"}"#,
    )
    .unwrap();
    std::fs::write(root.join(".yidam/bundle.yiz"), b"not really a bundle").unwrap();

    let after = run(root, &["regen", "--check"]);
    assert_eq!(
        after.code, 0,
        "building an index and a bundle moved the gate, so its verdict is a fact about the \
         machine and not about the commit:\n{}",
        after.stdout
    );

    // And the committed `status` line no longer carries the cell that said so.
    let line = readme(root);
    assert!(
        !line.contains("index present") && !line.contains("index not initialized"),
        "the status block still reports index state:\n{line}"
    );
}

/// The measurement is not lost — it moved to stdout, where being current is the point.
///
/// The other half of the repair above, and the one that keeps it from being a deletion. A
/// block that stopped depending on the tree would be just as green if the commands had
/// stopped looking at the tree altogether, and then `yidam doctor`'s index check and the
/// editor's status line would be answering from nothing.
#[test]
fn the_live_commands_still_report_what_the_block_no_longer_does() {
    let tmp = stage();
    let root = tmp.path();

    let absent = run(root, &["index-status"]);
    assert!(
        absent.stdout.contains("not initialized"),
        "index-status with no index: {}",
        absent.stdout
    );

    std::fs::create_dir_all(root.join(".yidam/index")).unwrap();
    std::fs::write(
        root.join(".yidam/index/meta.json"),
        r#"{"generated_at":1780000000,"model_name":"bge-small-en-v1.5",
            "embedding_dim":384,"node_count":4}"#,
    )
    .unwrap();

    // Built before the corpus files were staged, so this is the stale arm — and it is the
    // arm that proves the command still walks the tree rather than reading meta.json alone.
    let stale = run(root, &["index-status"]);
    assert!(
        stale.stdout.contains("2026-05-28") && stale.stdout.contains("Stale nodes"),
        "index-status over an out-of-date index has to say so: {}",
        stale.stdout
    );

    // The fresh arm, which is the one that names the model. Far enough ahead that no staged
    // file can be newer.
    std::fs::write(
        root.join(".yidam/index/meta.json"),
        r#"{"generated_at":4102444800,"model_name":"bge-small-en-v1.5",
            "embedding_dim":384,"node_count":4}"#,
    )
    .unwrap();
    let fresh = run(root, &["index-status"]);
    assert!(
        fresh.stdout.contains("bge-small-en-v1.5")
            && fresh.stdout.contains("384")
            && fresh.stdout.contains("up-to-date"),
        "index-status over a current index has to describe it: {}",
        fresh.stdout
    );

    let json = run(root, &["index-status", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["index_present"], true);
    assert_eq!(doc["model"], "bge-small-en-v1.5");

    // `status --format json` keeps the field its block gave up.
    let status: serde_json::Value =
        serde_json::from_str(&run(root, &["status", "--format", "json"]).stdout).unwrap();
    assert_eq!(status["index_present"], true);
}
