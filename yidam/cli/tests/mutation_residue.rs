//! No source file is carrying a mutant.
//!
//! `mise run mutants` uses `cargo-mutants --in-place`, which edits the working tree rather
//! than a copy — it has to, because `yidam/cli` dev-depends on a crate outside its own package
//! directory and the copy cannot resolve it. The cost is that an interrupted run leaves a
//! mutated source file behind.
//!
//! That happened twice while #468 was being written, and the second time it reached a full
//! gate run. `cargo fmt --check` caught it, and only because that particular mutant happened
//! to rewrap a line:
//!
//! ```text
//! -        if  /* ~ changed by cargo-mutants ~ */holds_content(&root.join(p)) {
//! ```
//!
//! (That example is why the scan skips this file: written out in full, the marker in a
//! comment about the marker is indistinguishable from the marker in a mutant. `census.rs`
//! solved the same problem by choosing a token that is not an English word; here the token is
//! not mine to choose, so the file that documents it is the one exception.)
//!
//! A mutant that formats cleanly — `replace || with &&`, `replace -> bool with true` — leaves
//! no formatting to notice and would have been committed. The tests would have caught it only
//! if a test covers that line, which is exactly the thing a *surviving* mutant tells you is
//! not true. So the residue is checked for by name.
//!
//! It is cheap and it is not clever: `cargo-mutants` writes a marker comment into every
//! mutation it makes, and this looks for it.

use std::path::PathBuf;
use walkdir::WalkDir;

/// The comment `cargo-mutants` injects beside every change it makes.
///
/// Split so that this file does not match itself: the scan reads every `.rs` in the
/// repository, and a literal here would be found in it. The same reason `census.rs` chose a
/// marker that is not an English word.
const MARKER: &str = concat!("changed by ", "cargo-mutants");

/// This file, relative to the crate. `file!()` so a rename cannot leave the skip behind.
const SELF: &str = file!();

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn no_source_file_is_holding_a_mutant() {
    let mut residue = Vec::new();
    let mut scanned = 0usize;

    for entry in WalkDir::new(repo_root())
        .into_iter()
        .filter_entry(|e| {
            let n = e.file_name().to_string_lossy();
            n != "target" && n != "node_modules" && n != ".git" && n != ".claude"
        })
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        // This file, which quotes the marker in its own header. Skipped by path rather
        // than by rewriting the prose, so a later edit to the example cannot resurrect the
        // self-match.
        if path.ends_with(SELF) {
            continue;
        }
        scanned += 1;
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line.contains(MARKER) {
                let rel = path.strip_prefix(repo_root()).unwrap_or(path);
                residue.push(format!("  {}:{}", rel.display(), i + 1));
            }
        }
    }

    assert!(
        scanned > 100,
        "only {scanned} Rust files scanned; the walk is looking at the wrong tree and this \
         assertion is vacuous"
    );
    assert!(
        residue.is_empty(),
        "these lines are holding a mutant left behind by an interrupted `mise run mutants`:\n\
         {}\n\nRestore them with `git checkout -- <path>` — and commit your own work first, \
         because that command takes uncommitted edits with it.",
        residue.join("\n")
    );
}
