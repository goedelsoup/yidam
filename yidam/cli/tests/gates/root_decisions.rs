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

/// `file:line` for every use of `repo_root` outside `paths.rs` and outside comments.
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
            if names_repo_root(code_of(line)) {
                out.push(format!("src/{}:{}", rel.display(), i + 1));
            }
        }
    }
    out
}

/// A line up to its `//` comment, if it has one. A `//` inside a string literal is code — a
/// URL or a path — and cutting there would hide whatever follows it on the line.
fn code_of(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_str = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' if in_str => i += 1,
            b'"' => in_str = !in_str,
            // A `'"'` char literal is not the start of a string.
            b'\''
                if !in_str
                    && bytes.get(i + 1) == Some(&b'"')
                    && bytes.get(i + 2) == Some(&b'\'') =>
            {
                i += 2
            }
            b'/' if !in_str && bytes.get(i + 1) == Some(&b'/') => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

/// Whether code names `repo_root` as an identifier: a call, or a function passed by name
/// (`.or_else(crate::paths::repo_root)`), which reaches the same answer without parentheses.
fn names_repo_root(code: &str) -> bool {
    let ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
    code.match_indices("repo_root").any(|(at, word)| {
        let before = code.as_bytes()[..at].last().copied();
        let after = code.as_bytes().get(at + word.len()).copied();
        !before.is_some_and(ident) && !after.is_some_and(ident)
    })
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

/// The scan's two holes, closed: a `//` in a string does not end the line, and a function
/// pointer counts as a call.
#[test]
fn the_scan_sees_past_a_url_and_through_a_function_pointer() {
    for (line, found) in [
        (
            r#"let u = Url::parse("https://x").map(|_| crate::paths::repo_root());"#,
            true,
        ),
        (
            "let r = r.or_else(|_| fallback()).or_else(crate::paths::repo_root);",
            true,
        ),
        ("let c = '\"'; let r = repo_root();", true),
        (
            "let r = paths::resolve_root(None)?; // not repo_root()",
            false,
        ),
        ("let r = paths::working_root()?;", false),
        ("let s = \"a \\\" // b\"; let r = repo_root();", true),
        ("let my_repo_root_dir = here;", false),
    ] {
        assert_eq!(names_repo_root(code_of(line)), found, "{line}");
    }
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
