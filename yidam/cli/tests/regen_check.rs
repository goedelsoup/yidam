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

mod common;

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
    let git = |args: &[&str]| common::git::git_at(tmp.path(), args, common::git::FIXTURE_DATE);
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

/// Every path the report emits is declared in the committed schema.
///
/// The golden matrix's own version of this check only sees fields that appear in a golden,
/// and this report has none — see the module comment for why.
///
/// **Two arms, and item fields as well as top-level ones.** `unclaimed[]` is emitted only by a
/// repository that holds a block naming no generator, so a version of this test that staged
/// nothing would hold the array's declaration and neither of its members. It is exempted from
/// the matrix's reachability roster for that reason — `report_goldens.rs`'s `UNREACHED` says
/// why the shared fixture may not carry a broken marker — which makes this the only place the
/// declaration is held against a real document (#1062).
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir().parent().unwrap().join("report.schema.json"))
            .unwrap(),
    )
    .unwrap();

    let mut seen: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    for typo in [None, Some("statsu")] {
        let tmp = stage();
        if let Some(name) = typo {
            let staged = format!(
                "{}\n<!-- REGEN: yidam {name}\n-->\n_placeholder_\n<!-- /REGEN -->\n",
                readme(tmp.path())
            );
            std::fs::write(tmp.path().join("README.md"), &staged).unwrap();
            common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);
        }
        let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
        let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();

        let mut emitted = std::collections::BTreeSet::new();
        common::paths_of(&doc, "", &mut emitted);
        for path in &emitted {
            assert!(
                common::declares(&schema, path),
                "`regen --check` emits `{path}`, which report.schema.json does not declare"
            );
        }
        for required in schema["required"].as_array().unwrap() {
            assert!(doc.get(required.as_str().unwrap()).is_some(), "{required}");
        }
        seen.extend(emitted);
    }

    // `declares` can say yes to everything and the loop above still passes — the failure
    // this repository keeps finding in a scanner that looks at nothing. So it is asked about
    // a top-level name and an item field the schema does not carry, at the same two depths.
    for absent in ["nonesuch", "unclaimed[].nonesuch"] {
        assert!(
            !common::declares(&schema, absent),
            "`{absent}` is not in the schema and the walk said it was, so the check above \
             holds nothing"
        );
    }

    // The walk descended, and the second arm produced a member rather than an empty array.
    // Without this the assertion above is satisfied by a document with nothing in it, and by
    // a `paths_of` that stops at the envelope.
    for witness in ["stale", "unclaimed[].file", "unclaimed[].generator"] {
        assert!(
            seen.contains(witness),
            "the runs never emitted `{witness}`, so this test describes the depth it stopped \
             at rather than the contract: {seen:?}"
        );
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

/// A block whose name no generator carries is reported, and the gate says so (#1062).
///
/// **The defect was silence, not a wrong answer.** Every other part of this contract is
/// pushed: a generator names a file and a command, and the block is recorded when the two
/// disagree. A command no generator carries is never anybody's tag, so nothing reaches the
/// block and nothing records it — measured on this fixture at `3111beb`, a
/// `<!-- REGEN: yidam statsu -->` block survived `yidam regen` unchanged and
/// `yidam regen --check` printed *"Every REGEN block is current."* A typo in a marker name
/// turned a generated block into a hand-maintained one with nothing saying so.
///
/// Three arms, and the second and third are the ones that make the first mean anything:
/// the block must not be *written* by anything either, and the gate must be satisfiable.
#[test]
fn a_block_naming_no_generator_is_reported_and_gates() {
    let tmp = stage();
    let staged = format!(
        "{}\n## Typo\n\n<!-- REGEN: yidam statsu\n-->\n\
         _Run `yidam regen` to populate._\n<!-- /REGEN -->\n",
        readme(tmp.path())
    );
    std::fs::write(tmp.path().join("README.md"), &staged).unwrap();
    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);

    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert!(
        readme(tmp.path()).contains("<!-- REGEN: yidam statsu"),
        "the block was rewritten out of existence, which is not what happens: {}",
        readme(tmp.path())
    );
    assert!(
        readme(tmp.path()).contains("_Run `yidam regen` to populate._"),
        "some generator wrote into a block that is not its own — `update_regen` matches its \
         open tag by prefix, so this is the failure that looks like success:\n{}",
        readme(tmp.path())
    );

    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    // Reworded when `count` was registered (#1071): the list this heading introduces now
    // contains a generator that *does* exist and still does not write this block, so the old
    // wording contradicted the remedy printed two lines under it.
    assert!(
        r.stdout.contains("name a command no generator writes"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("README.md  (yidam statsu)"),
        "{}",
        r.stdout
    );
    // The remedy has to be one that clears the gate. `yidam regen` is not it, and saying so
    // is half the finding.
    assert!(
        r.stdout.contains("Correct the command or delete the block"),
        "{}",
        r.stdout
    );
    assert!(
        r.stdout.contains("The generators are: status,"),
        "the remedy names no generators, so a reader cannot tell what the name should be: {}",
        r.stdout
    );

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["passed"], false);
    assert_eq!(doc["unclaimed"][0]["file"], "README.md");
    assert_eq!(doc["unclaimed"][0]["generator"], "statsu");

    // Satisfiable, by the remedy the gate prescribes.
    std::fs::write(
        tmp.path().join("README.md"),
        readme(tmp.path()).replace("yidam statsu", "yidam vault-status"),
    )
    .unwrap();
    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    let after = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(after.code, 0, "{}", after.stdout);
}

/// The marker namespace is not yidam's, and a block another program writes is not a finding.
///
/// `ohio-education-funding` carries fourteen blocks under thirteen `edfund-connect <name>`
/// commands — `repository-overview`, `claim-totals`, `connector-registry` and ten more —
/// each refreshed by its own domain binary. Judging a command yidam does not own would
/// report a working repository as broken, with no remedy yidam could print that would be
/// true.
///
/// The `yidam`-prefixed block beside it is the control: without it, a check that had stopped
/// scanning altogether would pass this test.
#[test]
fn a_block_belonging_to_another_program_is_left_alone() {
    let tmp = stage();
    let staged = format!(
        "{}\n<!-- REGEN: edfund-connect claim-totals\n-->\n_theirs_\n<!-- /REGEN -->\n\
         <!-- REGEN: yidam statsu\n-->\n_ours, misspelled_\n<!-- /REGEN -->\n",
        readme(tmp.path())
    );
    std::fs::write(tmp.path().join("README.md"), &staged).unwrap();
    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    let named: Vec<&str> = doc["unclaimed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["generator"].as_str().unwrap())
        .collect();
    assert_eq!(
        named,
        ["statsu"],
        "the scan either missed the misspelling or claimed another program's block: {named:?}"
    );
}

/// A block a document *shows* is not a block the repository *has*.
///
/// `malformed-regen-block` warns rather than errors for exactly this reason — it does not
/// mask, so it cannot tell a documented example from a damaged file. This is a gate, so it
/// masks, and the fenced example below is what that buys.
#[test]
fn a_documented_example_is_not_a_block() {
    let tmp = stage();
    let staged = format!(
        "{}\n## How markers work\n\nWrite one like this:\n\n```markdown\n\
         <!-- REGEN: yidam demo-index\n-->\n_placeholder_\n<!-- /REGEN -->\n```\n\n\
         The inline form is `<!-- REGEN: yidam other-index -->`.\n",
        readme(tmp.path())
    );
    std::fs::write(tmp.path().join("README.md"), &staged).unwrap();
    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(
        doc["unclaimed"].as_array().unwrap().len(),
        0,
        "a shown block was read as a said one: {}",
        json.stdout
    );
}

/// The scan reaches the files the generators reach, which the prose walk does not.
///
/// `lint`'s `regen_files()` reads `.yidam/`, `docs/` and the root `README.md`. Five
/// generators write outside all three — `crates-index`, `packages-index`, `bundle-status`,
/// `kuten` and `practice` — so a scan built on that walk would be blind to a misnamed block
/// in any of them. `crates/README.md` and `AGENTS.md` are one of each.
#[test]
fn the_scan_reaches_beyond_the_prose_walk() {
    let tmp = stage();
    let root = tmp.path();
    std::fs::create_dir_all(root.join("crates")).unwrap();
    std::fs::write(
        root.join("crates/README.md"),
        "# crates\n\n<!-- REGEN: yidam crates-idx\n-->\n_x_\n<!-- /REGEN -->\n",
    )
    .unwrap();
    std::fs::write(
        root.join("AGENTS.md"),
        "# agents\n\n<!-- REGEN: yidam kuten-block\n-->\n_x_\n<!-- /REGEN -->\n",
    )
    .unwrap();
    common::git::git_at(root, &["add", "-A"], common::git::FIXTURE_DATE);

    let json = run(root, &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    let files: Vec<&str> = doc["unclaimed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["file"].as_str().unwrap())
        .collect();
    assert!(
        files.contains(&"crates/README.md") && files.contains(&"AGENTS.md"),
        "the scan did not reach both files: {files:?}"
    );
}

/// An untracked document is not yet a block this repository has.
///
/// The gate answers about the commit — the same argument
/// [`a_built_index_or_bundle_does_not_move_the_gate`] makes about a build artifact. Stated
/// as a test because it is a real boundary a reader will hit: a misnamed block goes red on
/// `git add`, not on save.
#[test]
fn an_untracked_document_is_not_scanned() {
    let tmp = stage();
    std::fs::write(
        tmp.path().join("DRAFT.md"),
        "# draft\n\n<!-- REGEN: yidam statsu\n-->\n_x_\n<!-- /REGEN -->\n",
    )
    .unwrap();

    let json = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&json.stdout).unwrap();
    assert_eq!(doc["unclaimed"].as_array().unwrap().len(), 0);

    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);
    let after = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&after.stdout).unwrap();
    assert_eq!(doc["unclaimed"][0]["file"], "DRAFT.md");
}

/// `doctor` and the gate agree.
///
/// A report that calls a repository sound while the gate fails it is the disagreement
/// `stale_blocks` was extracted to prevent, and a second half added to the gate and not to
/// the report reopens it.
#[test]
fn doctor_reports_what_the_gate_gates_on() {
    let tmp = stage();
    let staged = format!(
        "{}\n<!-- REGEN: yidam statsu\n-->\n_x_\n<!-- /REGEN -->\n",
        readme(tmp.path())
    );
    std::fs::write(tmp.path().join("README.md"), &staged).unwrap();
    common::git::git_at(tmp.path(), &["add", "-A"], common::git::FIXTURE_DATE);

    let d = run(tmp.path(), &["doctor", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&d.stdout).unwrap();
    let regen = doc["checks"]
        .as_array()
        .expect("doctor reports checks")
        .iter()
        .find(|c| c["id"] == "regen")
        .expect("doctor has a regen check");
    assert_eq!(regen["verdict"], "fail", "{}", d.stdout);
    assert!(
        regen["detail"]
            .as_str()
            .unwrap_or_default()
            .contains("yidam statsu"),
        "{regen}"
    );
}

/// A repository that tracks nothing is read, not refused.
///
/// The scan borrows the tracked set, and the accessor it first borrowed was the one
/// `clone` and `overlay` copy from — which refuses an empty set, because a copy of nothing
/// is not a template. A *read* of nothing is an answer. Measured against an initialised
/// derived repository before its first commit, the gate printed *"cannot read the template
/// … Run this from a git checkout of yidam"*, which is untrue of everywhere it was run.
///
/// It still reports the stale blocks, which is the half that says the command got past the
/// scan rather than bailing before it.
#[test]
fn a_repository_with_nothing_committed_is_still_reported_on() {
    let tmp = stage();
    // Back to before the genesis commit, keeping the working tree.
    std::fs::remove_dir_all(tmp.path().join(".git")).unwrap();
    let git = |args: &[&str]| common::git::git_at(tmp.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);

    let r = run(tmp.path(), &["regen", "--check", "--format", "json"]);
    let doc: serde_json::Value = serde_json::from_str(&r.stdout)
        .unwrap_or_else(|e| panic!("the gate did not report at all ({e}): {}", r.stdout));
    assert_eq!(doc["unclaimed"].as_array().unwrap().len(), 0);
    assert!(
        !doc["stale"].as_array().unwrap().is_empty(),
        "the fixture's placeholder block went unreported, so the scan bailed early: {}",
        r.stdout
    );
}
