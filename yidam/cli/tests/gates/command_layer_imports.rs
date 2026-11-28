//! The core modules do not reach up into the command layer (#1635).
//!
//! `src/cmd/` holds the commands, and the commands are built on the modules beside it:
//! `model`, `sources`, `table`, `kuten`, `derive`. When #1635 was filed those modules named
//! `crate::cmd::` in 42 places, so the dependency ran both ways. It compiles, because it is
//! one crate; what it costs is that none of them can be tested, moved or split out without
//! the commands they reach into. `model.rs` imports three renderers from three commands that
//! are themselves built on `model`.
//!
//! The fix is to move what a core module imports down a layer, beside its callers, and that
//! is done a group at a time. This file holds the line while it is: no file outside
//! `src/cmd/` and `src/main.rs` may name `crate::cmd` in code, except the ones on
//! [`REACHES_UP`], and each of those exactly as often as it is listed. Exactly, both ways:
//! a file that gains a reference fails, and so does one that loses one, so the change that
//! moves something down also lowers the number. That is what makes the list one that can
//! only shrink, rather than a ceiling that drifts back up to wherever it was.
//!
//! ## What is counted
//!
//! Every occurrence of the path `crate::cmd` in code, as a whole word. One path is one
//! reference: `use crate::cmd::lint::{a, b}` counts once, because it is one edge from this
//! file to one module, and counting the braces would make a tidy import look worse than two
//! scattered ones. A `#[cfg(test)]` module is code and is counted, because a test that
//! reaches into a command couples the module to it just as surely: three of the 42 are in
//! test modules. Comments are not counted, doc comments included, and neither is the inside
//! of a string or char literal. An intra-doc link to a command is a pointer a reader
//! follows, not a dependency the compiler resolves, and the core modules hold nearly sixty
//! lines of them.
//!
//! `src/lib.rs` is in scope, and its `pub use cmd::…` re-exports are not counted, because
//! they are written relative to the crate root rather than through `crate::`. That is the
//! binary's door into the command layer, and it is meant to be there.
//!
//! ## What it does not hold
//!
//! - A path through `super::cmd::` from a file at the crate root, or `crate::{cmd::…}` in
//!   a grouped import. Neither form occurs in code today; both would walk past this scan.
//! - A reference that arrives through a re-export: a core module naming `crate::sources::x`
//!   where `x` is itself `pub use crate::cmd::…` is not seen in the importing file.
//! - Whitespace inside the path, `crate :: cmd`, which `rustfmt` never writes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::common;

/// The files still reaching into `crate::cmd`, each with its exact count of references.
///
/// **Lower an entry in the change that moves a reference down, and delete it at zero.** The
/// remaining groups are #1635's: `cmd::lint` is reached most often, then `cmd::run` and
/// `cmd::catalog`. A new entry is a new upward edge, and the place to argue for one is the
/// pull request that adds it, not this list.
const REACHES_UP: &[(&str, usize)] = &[
    ("src/computed.rs", 2),
    ("src/coupling.rs", 2),
    ("src/derive.rs", 3),
    ("src/derive/records.rs", 3),
    ("src/git/mod.rs", 1),
    ("src/kuten.rs", 9),
    ("src/model.rs", 3),
    ("src/sources/draft.rs", 1),
    ("src/sources/manifest.rs", 1),
    ("src/sources/mod.rs", 2),
    ("src/table.rs", 2),
];

/// The path this file looks for.
const PATH: &str = "crate::cmd";

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Whether `rel`, a path relative to `yidam/cli/`, is a core module this rule applies to.
///
/// The commands may name each other, and `main.rs` is the binary that dispatches to them.
fn in_scope(rel: &str) -> bool {
    rel.starts_with("src/")
        && rel.ends_with(".rs")
        && !rel.starts_with("src/cmd/")
        && rel != "src/main.rs"
}

/// Every `.rs` under `src/`, as a path relative to `yidam/cli/`, with its text.
///
/// Walked rather than listed from git, so a core module that is not yet committed is read
/// too: the commit that adds a file is exactly the one that should go red here.
fn sources() -> Vec<(String, String)> {
    let dir = crate_dir();
    let mut out: Vec<(String, String)> = common::repo_walk(&dir.join("src"))
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|x| x == "rs"))
        .map(|e| {
            let rel = e
                .path()
                .strip_prefix(&dir)
                .unwrap_or(e.path())
                .to_string_lossy()
                .replace('\\', "/");
            (
                rel,
                std::fs::read_to_string(e.path()).expect("source is readable"),
            )
        })
        .collect();
    out.sort();
    out
}

fn is_ident(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// `text` with every comment and every string and char literal blanked out.
///
/// Blanked to spaces rather than removed, and newlines kept, so a line number read off the
/// result is the line number in the file. Block comments nest, as Rust's do. A raw string
/// runs to its own closing quote and hash count. A `'` begins a char literal only when it
/// closes one or two characters on (or opens an escape); otherwise it is a lifetime and
/// stays.
fn code_of(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let at = |i: usize| chars.get(i).copied();
    let blank = |c: char| if c == '\n' { '\n' } else { ' ' };
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < len {
        let c = chars[i];
        let next = at(i + 1);
        if c == '/' && next == Some('/') {
            while i < len && chars[i] != '\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if c == '/' && next == Some('*') {
            let mut depth = 0usize;
            while i < len {
                if chars[i] == '/' && at(i + 1) == Some('*') {
                    depth += 1;
                    out.push_str("  ");
                    i += 2;
                } else if chars[i] == '*' && at(i + 1) == Some('/') {
                    depth -= 1;
                    out.push_str("  ");
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            continue;
        }
        // `r"…"`, `r#"…"#`, `br#"…"#`, and not the `r` that ends an identifier.
        let raw_prefix = match i.checked_sub(1).map(|p| chars[p]) {
            None => true,
            Some('b') => i.checked_sub(2).is_none_or(|p| !is_ident(chars[p])),
            Some(p) => !is_ident(p),
        };
        if c == 'r' && raw_prefix {
            let mut j = i + 1;
            while at(j) == Some('#') {
                j += 1;
            }
            let hashes = j - i - 1;
            if at(j) == Some('"') {
                let mut k = j + 1;
                let end = loop {
                    if k >= len {
                        break len;
                    }
                    if chars[k] == '"' && (1..=hashes).all(|h| at(k + h) == Some('#')) {
                        break k + 1 + hashes;
                    }
                    k += 1;
                };
                out.extend(chars[i..end].iter().map(|&c| blank(c)));
                i = end;
                continue;
            }
        }
        if c == '"' {
            out.push('"');
            i += 1;
            while i < len {
                if chars[i] == '\\' {
                    out.push(' ');
                    if let Some(escaped) = at(i + 1) {
                        out.push(blank(escaped));
                    }
                    i += 2;
                } else if chars[i] == '"' {
                    out.push('"');
                    i += 1;
                    break;
                } else {
                    out.push(blank(chars[i]));
                    i += 1;
                }
            }
            continue;
        }
        if c == '\'' {
            if next == Some('\\') {
                let close = (i + 3..len).find(|&k| chars[k] == '\'').unwrap_or(len - 1);
                out.extend(std::iter::repeat_n(' ', close + 1 - i));
                i = close + 1;
                continue;
            }
            if at(i + 2) == Some('\'') {
                out.push_str("   ");
                i += 3;
                continue;
            }
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The line of each reference to `crate::cmd` in `text`, read from code only.
///
/// Once per occurrence of the path, so two on one line are two, and a grouped `use` is one.
/// A whole word: `crate::cmdline` is not the command layer, and `$crate::cmd` in a macro is.
fn references(text: &str) -> Vec<usize> {
    let mut lines = Vec::new();
    for (n, line) in code_of(text).lines().enumerate() {
        let mut from = 0;
        while let Some(found) = line[from..].find(PATH) {
            let start = from + found;
            let end = start + PATH.len();
            let before = line[..start].chars().next_back();
            let after = line[end..].chars().next();
            if !before.is_some_and(is_ident) && !after.is_some_and(is_ident) {
                lines.push(n + 1);
            }
            from = end;
        }
    }
    lines
}

/// The counted references in each in-scope file that has any.
fn reaching_up() -> BTreeMap<String, Vec<usize>> {
    sources()
        .into_iter()
        .filter(|(rel, _)| in_scope(rel))
        .map(|(rel, text)| (rel, references(&text)))
        .filter(|(_, lines)| !lines.is_empty())
        .collect()
}

fn listed() -> BTreeMap<&'static str, usize> {
    REACHES_UP.iter().copied().collect()
}

fn at_lines(lines: &[usize]) -> String {
    lines
        .iter()
        .map(usize::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The counter, on text whose answer is known.
///
/// A counter that read every comment would find the nearly sixty lines of intra-doc links and fail
/// for them; one that read nothing would pass every assertion below having looked at
/// nothing. Each case pins one of the decisions the module doc states.
#[test]
fn the_counter_reads_code_and_not_prose() {
    let cases: &[(&str, &str, &[usize])] = &[
        ("a plain use", "use crate::cmd::lint::Severity;\n", &[1]),
        (
            "a grouped use is one path",
            "use crate::cmd::lint::{citations, local_citations};\n",
            &[1],
        ),
        (
            "an expression path",
            "fn f() {\n    crate::cmd::tracked::list(root);\n}\n",
            &[2],
        ),
        (
            "two on one line are two",
            "let a = (crate::cmd::x::A, crate::cmd::y::B);\n",
            &[1, 1],
        ),
        ("the module itself", "use crate::cmd;\n", &[1]),
        ("inside a macro", "$crate::cmd::x!();\n", &[1]),
        (
            "a test module is code",
            "#[cfg(test)]\nmod tests {\n    use crate::cmd::lint::commits::Subject;\n}\n",
            &[3],
        ),
        ("a line comment", "// see crate::cmd::lint\n", &[]),
        ("an intra-doc link", "/// [`crate::cmd::lint::ttl`]\n", &[]),
        ("an inner doc", "//! [`crate::cmd::embed`] reads it\n", &[]),
        ("a trailing comment", "let x = 1; // crate::cmd::x\n", &[]),
        (
            "a nested block comment",
            "/* a /* crate::cmd::x */ crate::cmd::y */ use crate::cmd::z;\n",
            &[1],
        ),
        ("a string", "let s = \"crate::cmd::x\";\n", &[]),
        ("an escaped quote", "let s = \"\\\" crate::cmd::x\";\n", &[]),
        (
            "a raw string",
            "let s = r#\"a \"quote\" crate::cmd::x\"#;\n",
            &[],
        ),
        ("a byte raw string", "let s = br\"crate::cmd::x\";\n", &[]),
        (
            "a multi-line string keeps line numbers",
            "let s = \"one\ncrate::cmd::x\";\nuse crate::cmd::y;\n",
            &[3],
        ),
        (
            "a quote char does not open a string",
            "let q = '\"'; use crate::cmd::x;\n",
            &[1],
        ),
        (
            "a lifetime does not open a char",
            "fn f<'a>(x: &'a str) { crate::cmd::g(x) }\n",
            &[1],
        ),
        ("a longer word", "use crate::cmdline::Args;\n", &[]),
        ("a prefixed word", "use subcrate::cmd::Args;\n", &[]),
        (
            "another module",
            "use crate::sources::template::bind;\n",
            &[],
        ),
    ];
    for (what, text, want) in cases {
        assert_eq!(
            references(text),
            want.to_vec(),
            "{what}: counted wrong in {text:?}"
        );
    }
}

/// The scan reads the crate, and sees references where they are known to be.
#[test]
fn the_scan_sees_what_it_is_looking_for() {
    let sources = sources();
    assert!(
        sources.len() > 150,
        "only {} sources under src/; this crate has many more, so every assertion below is \
         scanning the wrong directory",
        sources.len()
    );
    // The commands name each other freely, so a scan that did not exclude them would find
    // hundreds. Seeing them here is the evidence that the exclusion is what hides them.
    let in_cmd: usize = sources
        .iter()
        .filter(|(rel, _)| rel.starts_with("src/cmd/"))
        .map(|(_, text)| references(text).len())
        .sum();
    assert!(
        in_cmd > 0,
        "no reference to {PATH} under src/cmd/, where the commands name each other; the \
         counter is reading the wrong text"
    );
    for rel in ["src/cmd/mod.rs", "src/main.rs"] {
        assert!(
            !in_scope(rel),
            "{rel} is the command layer and is not graded"
        );
    }
    for rel in ["src/lib.rs", "src/model.rs", "src/sources/mod.rs"] {
        assert!(in_scope(rel), "{rel} is a core module and must be graded");
    }
}

/// No file outside the command layer names it, beyond the exemptions.
#[test]
fn no_core_module_reaches_into_the_command_layer() {
    let listed = listed();
    let offenders: Vec<String> = reaching_up()
        .into_iter()
        .filter(|(rel, _)| !listed.contains_key(rel.as_str()))
        .map(|(rel, lines)| format!("{rel}: {PATH} at line {}", at_lines(&lines)))
        .collect();
    assert!(
        offenders.is_empty(),
        "these core modules import from the command layer:\n  {}\n\nThe commands are built on \
         these modules, so an import the other way makes the dependency run both ways \
         (#1635). Move what the module needs down a layer, out of src/cmd/ and beside its \
         callers, and have the command import it from there. A comment or an intra-doc link \
         is not counted; only a path in code is.",
        offenders.join("\n  ")
    );
}

/// Each exemption is exact: a file holds as many references as its entry says.
///
/// Fewer is a failure too, and that is the half that makes the list shrink. Without it a
/// move lands, the entry stays at its old number, and the next upward import fits in the
/// room the move made without anything going red.
#[test]
fn every_exemption_is_exact() {
    let found = reaching_up();
    let mut wrong = Vec::new();
    for (rel, count) in REACHES_UP {
        let lines = found.get(*rel).cloned().unwrap_or_default();
        let have = lines.len();
        if !crate_dir().join(rel).is_file() {
            wrong.push(format!(
                "{rel} is listed with {count} and does not exist. Delete its entry."
            ));
        } else if *count == 0 {
            wrong.push(format!(
                "{rel} is listed with 0, which is no exemption. Delete its entry."
            ));
        } else if !in_scope(rel) {
            wrong.push(format!(
                "{rel} is the command layer, which this rule does not grade. Delete its entry."
            ));
        } else if have > *count {
            wrong.push(format!(
                "{rel} has {have} references to {PATH} (line {}) and is listed with {count}. \
                 The list only shrinks: move the new import down a layer instead of raising \
                 the number.",
                at_lines(&lines)
            ));
        } else if have < *count {
            let fix = if have == 0 {
                "delete its entry".to_string()
            } else {
                format!("lower its entry to {have}")
            };
            wrong.push(format!(
                "{rel} has {have} references to {PATH} and is listed with {count}. Something \
                 moved down a layer, which is the point: {fix} in REACHES_UP, in this change, \
                 so the room it made cannot be refilled."
            ));
        }
    }
    let mut seen = std::collections::BTreeSet::new();
    for (rel, _) in REACHES_UP {
        if !seen.insert(*rel) {
            wrong.push(format!("{rel} is listed twice. Keep one entry."));
        }
    }
    assert!(
        wrong.is_empty(),
        "REACHES_UP in tests/gates/command_layer_imports.rs disagrees with the source:\n  {}",
        wrong.join("\n  ")
    );
}
