//! `yidam crates-index` against the layout a derived repository actually writes.
//!
//! Reported from a derived repository: a `crates/` directory holding a Cargo workspace —
//! the arrangement `prelude/guidelines/directories.md` describes as normal — rendered an
//! index with a row for the virtual workspace manifest, and rendered the one real crate's
//! description as an em dash because its `[package]` table was column-aligned.
//!
//! Both survived every gate. `yidam regen --check` passed, because the block matched what
//! the generator produced; the generator was simply wrong. So the test is end to end: the
//! unit tests pin the parsers, and this pins what a person reads in `crates/README.md`.

use std::path::Path;
use std::process::Command;

/// A `crates/` workspace: a virtual manifest, one aligned member, one member that inherits
/// its description. Nothing else — the command needs a repository root, not a corpus.
fn stage() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let write = |rel: &str, text: &str| {
        let path = root.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };

    write(
        "crates/README.md",
        "# crates\n\n<!-- REGEN: yidam crates-index -->\n_Run `yidam crates-index` to populate._\n<!-- /REGEN -->\n",
    );
    write(
        "crates/Cargo.toml",
        "[workspace]\nmembers = [\"retrieval\", \"calculator\"]\nresolver = \"2\"\n\n[workspace.package]\nedition = \"2021\"\ndescription = \"This corpus's domain computer\"\n",
    );
    // Column-aligned, which is what made the description vanish.
    write(
        "crates/retrieval/Cargo.toml",
        "[package]\nname         = \"retrieval\"\nedition      = \"2021\"\ndescription  = \"Retrieval against the corpus index\"\n",
    );
    write(
        "crates/calculator/Cargo.toml",
        "[package]\nname = \"calculator\"\ndescription.workspace = true\nedition.workspace = true\n",
    );

    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(root)
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
    git(&["commit", "-q", "-m", "genesis: crates fixture"]);
    tmp
}

fn run(root: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).to_string()
}

fn index(root: &Path) -> String {
    let readme = std::fs::read_to_string(root.join("crates/README.md")).unwrap();
    let start = readme.find("-->").expect("a REGEN opener") + 3;
    let end = readme.find("<!-- /REGEN").expect("a REGEN closer");
    readme[start..end].trim().to_string()
}

#[test]
fn a_virtual_workspace_manifest_gets_no_row() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());

    // The defect's exact shape: a row whose link text and target were both empty of
    // meaning, pointing at `crates/` from inside `crates/README.md`.
    assert!(
        !table.contains("](crates/)"),
        "the workspace manifest is still listed:\n{table}"
    );
    assert_eq!(
        table.lines().filter(|l| l.starts_with("| [")).count(),
        2,
        "two crates, and the virtual manifest is not one of them:\n{table}"
    );
}

#[test]
fn an_aligned_manifest_keeps_its_description() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert!(
        table.contains("| [retrieval](retrieval/) | Retrieval against the corpus index |"),
        "the aligned description did not survive:\n{table}"
    );
}

#[test]
fn an_inherited_description_resolves() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert!(
        table.contains("| [calculator](calculator/) | This corpus's domain computer |"),
        "the inherited description did not resolve:\n{table}"
    );
}

/// The gate has to agree with the generator, or the fix trades a wrong table for a red CI.
#[test]
fn the_written_index_is_not_stale() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(tmp.path())
        .args(["regen", "--check", "--format", "json"])
        .output()
        .unwrap();
    let doc: serde_json::Value =
        serde_json::from_str(&String::from_utf8_lossy(&out.stdout)).expect("a JSON report");
    let stale: Vec<&serde_json::Value> = doc["stale"]
        .as_array()
        .map(|s| {
            s.iter()
                .filter(|s| s["generator"] == "crates-index")
                .collect()
        })
        .unwrap_or_default();
    assert!(stale.is_empty(), "crates-index reports itself stale: {doc}");
}

/// An empty `crates/` is not a table of nothing.
#[test]
fn no_crates_says_so() {
    let tmp = stage();
    std::fs::remove_file(tmp.path().join("crates/Cargo.toml")).unwrap();
    std::fs::remove_dir_all(tmp.path().join("crates/retrieval")).unwrap();
    std::fs::remove_dir_all(tmp.path().join("crates/calculator")).unwrap();
    run(tmp.path(), &["crates-index"]);
    assert_eq!(index(tmp.path()), "_No crates yet._");
}
