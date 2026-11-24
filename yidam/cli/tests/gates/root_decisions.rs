//! Every `repo_root()` outside `paths.rs` is one of two decisions, named.
//!
//! A command finds its corpus one of two ways since RFC-0064 §7. A reader calls
//! `paths::resolve_root`, which honours `--root` and then `YIDAM_ROOT`, and refuses a named
//! directory that holds no corpus. A command that commits calls `paths::working_root`, which
//! reads neither, because #918 left "commit into a corpus named elsewhere" a design question.
//! Both end in `repo_root()`, and a bare call to it is the third answer nobody chose: it
//! ignores the flag and the variable alike, so a producer's CI that exports `YIDAM_ROOT` per
//! site gets a report about whichever checkout the runner stands in.
//!
//! So a call outside `paths.rs` is refused here, and the remedy is to pick one. Comments are
//! skipped, because several doc comments name the function to say why they do not call it.

use std::path::PathBuf;

use crate::common;

fn src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// `file:line` for every call of `repo_root()` outside `paths.rs` and outside comments.
fn bare_calls() -> Vec<String> {
    let src = src();
    let mut out = Vec::new();
    for entry in common::repo_walk(&src) {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension() != Some("rs".as_ref()) {
            continue;
        }
        let rel = path.strip_prefix(&src).unwrap_or(path);
        if rel == std::path::Path::new("paths.rs") {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("source is readable");
        for (i, line) in text.lines().enumerate() {
            let code = line.split("//").next().unwrap_or("");
            if code.contains("repo_root()") {
                out.push(format!("src/{}:{}", rel.display(), i + 1));
            }
        }
    }
    out
}

#[test]
fn no_command_calls_repo_root_without_deciding_which_root_it_means() {
    let found = bare_calls();
    assert!(
        found.is_empty(),
        "these call `repo_root()` directly:\n  {}\n\
         Pick one. A command that reads a corpus calls `paths::resolve_root(root)` with its \
         `--root` (or `None`), so `YIDAM_ROOT` and the refusal of a non-corpus apply. A \
         command that writes a commit calls `paths::working_root()`, which reads neither (#918).",
        found.join("\n  ")
    );
}

/// The scan finds a call when there is one, or the test above passes by reading nothing.
#[test]
fn the_scan_reads_the_source_it_names() {
    let paths = std::fs::read_to_string(src().join("paths.rs")).expect("paths.rs is readable");
    assert!(
        paths.contains("pub fn working_root") && paths.contains("repo_root()"),
        "paths.rs no longer defines `working_root` over `repo_root()` — the gate's premise moved"
    );
    assert!(
        common::repo_walk(&src()).count() > 100,
        "the walk of src/ found too few entries to be reading the crate"
    );
}
