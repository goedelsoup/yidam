//! What this crate offers a caller who is not `main.rs` (#926).
//!
//! The library used to `std::process::exit(1)` from thirteen commands, and re-exported about
//! fifty names out of a private module with nothing to say that they were the binary's entry
//! points rather than an API. Two rules now hold, and both are checked here rather than
//! written down:
//!
//! 1. **Only the binary exits.** A gate that fails returns [`yidam::report::GateFailed`]; the
//!    exit code is `main.rs`'s to choose. Checked twice — once over the source, once by
//!    running the binary, because a source scan cannot see that the code still *means* exit 1.
//! 2. **Nothing reachable only through a private module is documented as an API.** Every
//!    `pub use` out of a private module carries `#[doc(hidden)]`.
//!
//! Both scanners are functions over text with their own fixtures, because a scanner that
//! looks at nothing passes every file it is given.

mod common;

use common::Example;
use std::path::{Path, PathBuf};

// ── rule 1, over the source ───────────────────────────────────────────────────

/// `src`, with comments and string literals removed.
///
/// Necessary, not fastidious: `report.rs` and `main.rs` both *describe* `std::process::exit`
/// in prose, and a grep over the raw file is answered by that prose — the check would pass
/// on a file whose code had been put back and whose comment had not.
///
/// Char literals are deliberately not handled. Rust spells a lifetime `'a`, so a scanner that
/// treated `'` as an opening quote would swallow code from every generic signature in the
/// crate; and a char literal cannot contain `process::exit` anyway.
fn strip_prose(src: &str) -> String {
    let b: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < b.len() {
        // A raw string: `r"…"`, `r#"…"#`, `r##"…"##`.
        if b[i] == 'r' && i + 1 < b.len() && (b[i + 1] == '"' || b[i + 1] == '#') {
            let mut j = i + 1;
            let mut hashes = 0;
            while j < b.len() && b[j] == '#' {
                hashes += 1;
                j += 1;
            }
            if j < b.len() && b[j] == '"' {
                let close: String = std::iter::once('"')
                    .chain(std::iter::repeat_n('#', hashes))
                    .collect();
                let rest: String = b[j + 1..].iter().collect();
                match rest.find(&close) {
                    Some(k) => {
                        out.push(' ');
                        i = j + 1 + k + close.len();
                        continue;
                    }
                    None => break,
                }
            }
        }
        if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '/' {
            while i < b.len() && b[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
            // Block comments nest in Rust.
            let mut depth = 1;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i] == '/' && i + 1 < b.len() && b[i + 1] == '*' {
                    depth += 1;
                    i += 2;
                } else if b[i] == '*' && i + 1 < b.len() && b[i + 1] == '/' {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            out.push(' ');
            continue;
        }
        if b[i] == '"' {
            i += 1;
            while i < b.len() {
                if b[i] == '\\' {
                    i += 2;
                    continue;
                }
                if b[i] == '"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            out.push(' ');
            continue;
        }
        out.push(b[i]);
        i += 1;
    }
    out
}

#[test]
fn the_prose_stripper_removes_prose_and_keeps_code() {
    // If this test is the one that fails, the scanner below is blind, not the crate.
    assert!(!strip_prose("// std::process::exit(1)\nlet x = 1;").contains("process::exit"));
    assert!(
        !strip_prose("/// a doc comment about std::process::exit\nfn f() {}")
            .contains("process::exit")
    );
    assert!(!strip_prose("/* std::process::exit(1) */").contains("process::exit"));
    assert!(!strip_prose("/* outer /* std::process::exit */ still */").contains("process::exit"));
    assert!(!strip_prose("let s = \"std::process::exit(1)\";").contains("process::exit"));
    assert!(!strip_prose("let s = r#\"std::process::exit(1)\"#;").contains("process::exit"));
    assert!(strip_prose("fn f() { std::process::exit(1); }").contains("process::exit"));
    // A lifetime must not be read as a quote, or everything after one disappears.
    assert!(
        strip_prose("fn f<'a>(x: &'a str) { std::process::exit(1); }").contains("process::exit")
    );
}

fn crate_src() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The binary's own sources: `main.rs` and whatever it declares a module.
///
/// Derived rather than listed. A `mod` added to `main.rs` belongs to the binary the day it is
/// written, and a hand-kept list here would quietly start holding a binary file to the
/// library's rule — or, worse, stop holding a library file to it.
fn binary_sources() -> Vec<PathBuf> {
    let src = crate_src();
    let main = src.join("main.rs");
    let text = std::fs::read_to_string(&main).expect("src/main.rs");
    let mut files = vec![main];
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("mod ") {
            if let Some(name) = rest.strip_suffix(';') {
                let flat = src.join(format!("{name}.rs"));
                let nested = src.join(name).join("mod.rs");
                if flat.is_file() {
                    files.push(flat);
                } else if nested.is_file() {
                    files.push(nested);
                }
            }
        }
    }
    files
}

fn rust_files(dir: &Path, into: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, into);
        } else if path.extension().is_some_and(|e| e == "rs") {
            into.push(path);
        }
    }
}

#[test]
fn only_the_binary_exits_the_process() {
    let binary = binary_sources();
    // The positive control, and it is the whole reason this test can be trusted: `main.rs`
    // *does* call `process::exit`, so a stripper that had eaten the file — or a scanner
    // looking in the wrong place — fails here rather than passing everywhere.
    let main_src = strip_prose(&std::fs::read_to_string(&binary[0]).unwrap());
    assert!(
        main_src.contains("process::exit"),
        "src/main.rs no longer calls process::exit, so this scanner has nothing to prove it \
         can see one. Either the binary stopped exiting — in which case rewrite this control \
         around whatever replaced it — or strip_prose is eating code."
    );

    let mut files = Vec::new();
    rust_files(&crate_src(), &mut files);
    files.sort();
    assert!(
        files.len() > 50,
        "only {} sources found under src/",
        files.len()
    );

    let mut library = 0;
    for path in &files {
        if binary.contains(path) {
            continue;
        }
        library += 1;
        let code = strip_prose(&std::fs::read_to_string(path).unwrap());
        // Per file, not a count over the set: a total is cleared by a scanner blind to the
        // one file that regressed.
        assert!(
            !code.contains("process::exit"),
            "{} calls process::exit. This crate is published as a library, and a library that \
             terminates its caller's process cannot be embedded (#926). Return \
             `crate::report::verdict(..)` — or `Err(GateFailed.into())` — and let main.rs \
             choose the exit code.",
            path.strip_prefix(crate_src()).unwrap_or(path).display()
        );
    }
    assert!(library > 40, "only {library} library sources were checked");
}

// ── rule 1, over the binary ───────────────────────────────────────────────────

#[test]
fn a_failed_gate_exits_one_and_says_nothing_on_stderr() {
    let ex = Example::materialize("streamflow");
    // Two commands, because one is an anecdote. Both reject on a class the corpus does not
    // declare, which is a verdict rather than a fault — the report says so, on stdout, in the
    // format asked for.
    for args in [
        vec!["retrieve", "hydropeaking", "--class", "no-such-class"],
        vec!["pack", "no-such-class"],
    ] {
        let (out, err, code) = ex.run(&args);
        assert_eq!(
            code,
            1,
            "`yidam {}` exited {code}\nstdout: {out}\nstderr: {err}",
            args.join(" ")
        );
        assert!(
            out.contains("rejected"),
            "`yidam {}` exited 1 without printing its rejection:\n{out}",
            args.join(" ")
        );
        // The claim main.rs makes: GateFailed carries no message *and* none is printed. A
        // second answer on stderr is the failure mode this replaced exit(1) could not have.
        assert!(
            !err.contains("Error:") && !err.contains("gate failed"),
            "`yidam {}` narrated its verdict on stderr as well as stdout:\n{err}",
            args.join(" ")
        );
    }
}

/// Run one subcommand somewhere that is not a repository, returning its stderr and code.
fn in_a_bare_directory(subcommand: &str) -> (String, i32) {
    let dir = tempfile::tempdir().unwrap();
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir.path())
        .arg(subcommand)
        .output()
        .unwrap();
    (
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code().unwrap_or(-1),
    )
}

#[test]
fn a_gate_that_reports_its_own_absence_is_still_a_gate() {
    // `doctor` is the third exit site this suite drives, and the one with no fixture: run it
    // outside a repository and every check fails, which is an answer rather than a fault.
    let (err, code) = in_a_bare_directory("doctor");
    assert_eq!(code, 1, "stderr: {err}");
    assert!(
        err.is_empty(),
        "`yidam doctor` wrote to stderr for a verdict it had already printed:\n{err}"
    );
}

#[test]
fn a_real_failure_still_names_itself_on_stderr() {
    // The control for the test above: if `Error:` had stopped being printed for *everything*,
    // that test would pass while the binary had gone silent about genuine faults.
    // `cycle`, not `doctor`: `doctor` reports "not a git repository" as a failed *check* and
    // exits 1 with an empty stderr, which is the gate path above rather than the error path.
    // That distinction is the command's to make and it makes it correctly — but it makes this
    // control prove nothing, which is how it was first written and how it first failed.
    let (err, code) = in_a_bare_directory("cycle");
    assert_eq!(code, 1, "stderr: {err}");
    assert!(
        err.contains("Error:") && err.contains("not a yidam repository"),
        "a genuine failure no longer names itself on stderr:\n{err}"
    );
}

// ── rule 2 ────────────────────────────────────────────────────────────────────

/// Every `pub use` in `lib.rs` that names a private module and is not `#[doc(hidden)]`.
///
/// The set of private modules is read from the same text, so a `mod` added tomorrow is
/// covered without an edit here.
fn undocumented_private_re_exports(lib: &str) -> Vec<String> {
    let private: Vec<&str> = lib
        .lines()
        .filter_map(|l| l.strip_prefix("mod ")?.strip_suffix(';'))
        .collect();
    let lines: Vec<&str> = lib.lines().collect();
    let mut leaked = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("pub use ") else {
            continue;
        };
        let module = rest.split("::").next().unwrap_or("");
        if !private.contains(&module) {
            continue;
        }
        // Walk back over this item's attributes and doc comment looking for the marker.
        let mut hidden = false;
        let mut j = i;
        while j > 0 {
            let prev = lines[j - 1].trim();
            if prev == "#[doc(hidden)]" {
                hidden = true;
                break;
            }
            if prev.starts_with("#[") || prev.starts_with("///") || prev.starts_with("//") {
                j -= 1;
                continue;
            }
            break;
        }
        if !hidden {
            leaked.push((*line).to_string());
        }
    }
    leaked
}

#[test]
fn the_re_export_scanner_can_tell_hidden_from_documented() {
    let hidden = "mod cmd;\npub mod report;\n#[doc(hidden)]\npub use cmd::doctor;\n";
    assert!(undocumented_private_re_exports(hidden).is_empty());

    let leaked = "mod cmd;\npub mod report;\npub use cmd::doctor;\n";
    assert_eq!(undocumented_private_re_exports(leaked).len(), 1);

    // A doc comment and a cfg between the marker and the item are fine.
    let spaced = "mod cmd;\n#[doc(hidden)]\n/// why this is public\n#[cfg(feature = \"x\")]\npub use cmd::doctor;\n";
    assert!(undocumented_private_re_exports(spaced).is_empty());

    // Re-exporting from a *public* module is not a leak and must not be reported.
    let public = "mod cmd;\npub mod report;\npub use report::Format;\n";
    assert!(undocumented_private_re_exports(public).is_empty());
}

#[test]
fn nothing_reachable_only_through_a_private_module_looks_like_an_api() {
    let lib = std::fs::read_to_string(crate_src().join("lib.rs")).unwrap();
    // The scanner must find the surface it is scanning, or it is agreeing with itself.
    let total = lib.lines().filter(|l| l.starts_with("pub use ")).count();
    assert!(
        total > 10,
        "lib.rs has {total} `pub use` items; the scanner is looking at the wrong file"
    );

    let leaked = undocumented_private_re_exports(&lib);
    assert!(
        leaked.is_empty(),
        "these re-export a private module's contents without `#[doc(hidden)]`, so rustdoc \
         publishes them as though they were an API (#926). They are the binary's entry \
         points: they print to stdout and gate. Mark them hidden, or make the module \
         `pub mod` and mean it.\n  {}",
        leaked.join("\n  ")
    );
}
