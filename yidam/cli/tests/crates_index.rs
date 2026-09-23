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
//!
//! # The fixture declares one capability and not two
//!
//! #472's finding is that a crate implementing a connector and a crate implementing nothing
//! produced the same row. A fixture where every crate is declared could not show that, and one
//! where none is could not either — the distinction needs both rows present at once, which is
//! why `retrieval` is declared and `calculator` is not.

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
    // One of the two crates is declared. `retrieval` is named the way RFC-0026 §4's own
    // example names a crate — `cargo run -p <name>` — and `calculator` is a directory nothing
    // runs, which is the row the index could not distinguish before #472.
    write(
        ".yidam/capabilities.toml",
        "[capability.gather-gages]\nkind   = \"connector\"\n\
         run    = [\"cargo\", \"run\", \"-p\", \"retrieval\", \"--\"]\n\
         reads  = [\".yidam/corpus/**\"]\nwrites = [\".yidam/corpus/**\"]\n\
         verb   = \"refresh\"\n",
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
        table.contains("Retrieval against the corpus index"),
        "the aligned description did not survive:\n{table}"
    );
}

#[test]
fn an_inherited_description_resolves() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert!(
        table.contains("This corpus's domain computer"),
        "the inherited description did not resolve:\n{table}"
    );
}

/// The row of the crate that implements something names what it implements.
#[test]
fn a_declared_crate_names_its_capability_and_its_kind() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert!(
        table.contains(
            "| [retrieval](retrieval/) | connector `gather-gages` | Retrieval against the \
             corpus index |"
        ),
        "the declared crate's row does not name the capability that runs it:\n{table}"
    );
}

/// #472, end to end: the two rows are different rows.
///
/// The assertion is over the rows themselves rather than over a literal, so a future column
/// that happened to render the same constant in both would still fail it. `calculator` is a
/// directory with a Cargo manifest and nothing declaring it, which is exactly the shape the
/// index used to report as though it were a working connector.
#[test]
fn a_crate_nothing_declares_does_not_read_as_one_that_implements_something() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());

    let row = |name: &str| {
        table
            .lines()
            .find(|l| l.contains(&format!("[{name}]")))
            .unwrap_or_else(|| panic!("no row for {name}:\n{table}"))
            .to_string()
    };
    let declared = row("retrieval");
    let scaffold = row("calculator");

    assert!(
        declared.contains("gather-gages"),
        "the declared crate does not name its capability: {declared}"
    );
    assert!(
        !scaffold.contains("gather-gages") && !scaffold.contains("connector"),
        "a crate nothing declares is reported as implementing a capability: {scaffold}"
    );
    assert!(
        scaffold.contains("| — |"),
        "a crate nothing declares says nothing about it: {scaffold}"
    );
}

/// The header says what the column is, because two em dashes in a row mean two absences.
#[test]
fn the_table_names_the_capability_column() {
    let tmp = stage();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert!(
        table.starts_with("| Crate | Capability | Description |"),
        "the header does not distinguish the columns:\n{table}"
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

/// A repository declaring no capabilities still gets an index, with every row saying so.
///
/// The near-miss: a generator that read the manifest unconditionally and failed where there is
/// none would break `crates-index` in every repository that has not declared a capability yet,
/// which is most of them on the day they are created.
#[test]
fn a_repository_with_no_manifest_still_renders_the_index() {
    let tmp = stage();
    std::fs::remove_file(tmp.path().join(".yidam/capabilities.toml")).unwrap();
    run(tmp.path(), &["crates-index"]);
    let table = index(tmp.path());
    assert_eq!(
        table.lines().filter(|l| l.starts_with("| [")).count(),
        2,
        "{table}"
    );
    assert!(!table.contains("gather-gages"), "{table}");
}
