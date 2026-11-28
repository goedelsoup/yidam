//! A path becomes a forward-slash string in one place under `src/`: `paths::slash_str` (#1637).
//!
//! A node's id, a report's `file`, a baseline key and a link target are all a path turned into
//! a string with `\` rewritten to `/`. That fix-up was written inline forty-three times in
//! thirty-five files. Each copy was the same, but nothing held them the same, and a copy that
//! drifted — a `"\\"` pattern, a `MAIN_SEPARATOR`, a missing call on one side of a comparison —
//! would give one file two spellings on Windows: an edge that resolves in one command and
//! dangles in another. `paths::slash(&Path)` and `paths::slash_str(&str)` are the one copy now.
//!
//! **What is counted:** every line of every `.rs` under `src/` that contains the inline form,
//! with a `'\\'` char pattern or a `"\\"` string pattern, comments included — a doc comment
//! that wants to show the form can describe it instead. The count is held at exactly one
//! line, in `paths.rs`.
//!
//! **What this does not hold:** a separator fix-up spelled some other way (`MAIN_SEPARATOR`,
//! `.chars().map(…)`, `components().join("/")`), and a string that never needed one. Nor does
//! it prove that every path string is slashed — only that the ones that are, are slashed by
//! the helper. Tests under `tests/` are not scanned; they spell out expected paths on purpose.

use std::path::{Path, PathBuf};

use crate::common;

/// The file that holds the one copy, relative to `src/`.
const HOME: &str = "paths.rs";

fn src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Whether a line writes the separator fix-up inline.
fn is_inline_fixup(line: &str) -> bool {
    [r#".replace('\\', "/")"#, r#".replace("\\", "/")"#]
        .iter()
        .any(|needle| line.contains(needle))
}

/// `(src-relative path, 1-based line)` for every inline fix-up under `src/`, and how many
/// `.rs` files the walk read.
fn inline_fixups() -> (Vec<(PathBuf, usize)>, usize) {
    let src = src();
    let mut found = Vec::new();
    let mut scanned = 0usize;
    for entry in common::repo_walk(&src) {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension() != Some("rs".as_ref()) {
            continue;
        }
        scanned += 1;
        let text = std::fs::read_to_string(path).expect("source is readable");
        let rel = path.strip_prefix(&src).unwrap_or(path).to_path_buf();
        for (i, line) in text.lines().enumerate() {
            if is_inline_fixup(line) {
                found.push((rel.clone(), i + 1));
            }
        }
    }
    (found, scanned)
}

#[test]
fn a_path_is_slashed_only_by_the_paths_helper() {
    let (found, _) = inline_fixups();
    let elsewhere: Vec<String> = found
        .iter()
        .filter(|(rel, _)| rel != Path::new(HOME))
        .map(|(rel, line)| format!("src/{}:{line}", rel.display()))
        .collect();
    assert!(
        elsewhere.is_empty(),
        "these lines rewrite `\\` to `/` inline:\n  {}\n\
         Call `crate::paths::slash(&path)` for a `Path`, or `crate::paths::slash_str(&s)` for \
         a string you already hold, instead. A second copy is a second place the separator \
         rule can drift, and a node with two spellings resolves in one command and dangles in \
         another (#1637).",
        elsewhere.join("\n  ")
    );

    let home = found
        .iter()
        .filter(|(rel, _)| rel == Path::new(HOME))
        .count();
    assert_eq!(
        home, 1,
        "src/{HOME} writes the inline fix-up on {home} lines; the gate expects exactly one, the \
         body of `slash_str`. If the helper was rewritten another way, this gate's needle no \
         longer finds the one copy — update `is_inline_fixup` and this count together, so the \
         gate still sees what it holds."
    );
}

/// The needle matches both spellings and does not match its neighbours.
#[test]
fn the_scan_sees_both_patterns_and_not_their_neighbours() {
    for (line, found) in [
        (r#"    s.replace('\\', "/")"#, true),
        (r#"        .replace('\\', "/");"#, true),
        (
            r#"let rel = rel.to_string_lossy().replace("\\", "/");"#,
            true,
        ),
        (r#"v.replace('\\', "\\\\").replace('"', "\\\"")"#, false),
        (r#"add(f.replace('\\', r"\\"), t)"#, false),
        (r#"let rel = crate::paths::slash(rel);"#, false),
    ] {
        assert_eq!(is_inline_fixup(line), found, "{line}");
    }
}

/// The walk reads the crate and finds the one copy, or the gate above passes by reading
/// nothing.
#[test]
fn the_scan_reads_the_source_it_names() {
    let (found, scanned) = inline_fixups();
    assert!(
        scanned > 100,
        "only {scanned} `.rs` files under src/ were read; the walk is looking at the wrong tree"
    );
    assert!(
        found.iter().any(|(rel, _)| rel == Path::new(HOME)),
        "the scan found no inline fix-up in src/{HOME}, where `slash_str` holds the one copy — \
         either the helper moved or the needle no longer matches it"
    );
}
