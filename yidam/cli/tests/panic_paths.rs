//! How many ways production code can panic, counted so it cannot grow quietly.
//!
//! #464 proposed escalating `clippy::unwrap_used` to deny "in the modules that must not
//! panic", on the estimate that production carried 78 panic paths. Measured with test modules
//! excluded it carries **23, across 13 files** — even more affordable than the issue thought.
//!
//! It is still not switched on, and the reason is in `Cargo.toml` beside the `[lints]` table:
//! a lint level there applies to the whole package, this repository's tests unwrap freely,
//! and turning it on emits 1,831 warnings under gates that all pass `-D warnings`. Doing it
//! properly means `#![deny(clippy::unwrap_used)]` on chosen modules after their unwraps are
//! removed — and removing them changes behaviour, which RFC-0025 says P0 through P3 do not.
//!
//! So this file holds the line in the meantime. A ratchet, not a gate: the count may fall
//! freely and may not rise. That is the whole of what it can honestly do — it cannot tell a
//! justified `unwrap` on a compile-time constant from a careless one on user input, and it
//! does not try.

use std::path::PathBuf;

mod common;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The count on the day this was written. **May fall, may not rise.**
///
/// Lowering it when you remove a panic path is the intended edit and needs no ceremony.
/// Raising it is a decision: say in the commit message why the new one cannot be a `?`.
const BUDGET: usize = 21;

/// Source with `#[cfg(test)]` items removed, by brace matching.
///
/// Unit tests live inside the files they test and unwrap constantly; counting them would
/// measure the test suite and call it production risk. Whole files that are test modules are
/// excluded by [`test_only_files`] at the call site for the same reason.
fn without_test_items(src: &str) -> String {
    let bytes: Vec<char> = src.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < bytes.len() {
        let rest: String = bytes[i..].iter().take(12).collect();
        if rest.starts_with("#[cfg(test)]") {
            // Skip to the matching close brace of the item this attribute introduces.
            let mut j = i;
            while j < bytes.len() && bytes[j] != '{' && bytes[j] != ';' {
                j += 1;
            }
            if j < bytes.len() && bytes[j] == ';' {
                i = j + 1; // `#[cfg(test)] mod tests;` — nothing inline to skip.
                continue;
            }
            let mut depth = 0;
            while j < bytes.len() {
                match bytes[j] {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            break;
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            i = j + 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

/// Every file that is wholly a test module, found by reading the declarations.
///
/// A `#[cfg(test)] mod <name>;` compiles `<name>.rs` (or `<name>/mod.rs`) only under `cfg(test)`,
/// so nothing in it is production however it is spelled. This used to match the *name* `tests.rs`
/// instead, which is a list that rots into a hole: `git/fixture.rs` arrived in #929 as a
/// `#[cfg(test)] pub(crate) mod fixture;`, is test code by construction, and was counted as
/// production risk purely for not being called `tests`.
fn test_only_files(src: &std::path::Path) -> std::collections::BTreeSet<PathBuf> {
    let mut out = std::collections::BTreeSet::new();
    for entry in common::repo_walk(src) {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension() != Some("rs".as_ref()) {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // `foo/mod.rs` declares `foo/<name>.rs`; `foo.rs` declares `foo/<name>.rs` too.
        let dir = match path.file_stem() {
            Some(s) if s == "mod" || s == "lib" || s == "main" => match path.parent() {
                Some(p) => p.to_path_buf(),
                None => continue,
            },
            _ => path.with_extension(""),
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if line.trim() != "#[cfg(test)]" {
                continue;
            }
            let Some(next) = lines.get(i + 1) else {
                continue;
            };
            // `mod x;`, `pub mod x;`, `pub(crate) mod x;` — the semicolon form only. An
            // inline `mod x { … }` is already skipped by [`without_test_items`].
            let Some(name) = next
                .trim()
                .strip_suffix(';')
                .and_then(|d| d.rsplit_once("mod "))
                .map(|(_, n)| n.trim())
            else {
                continue;
            };
            out.insert(dir.join(format!("{name}.rs")));
            out.insert(dir.join(name).join("mod.rs"));
        }
    }
    out
}

/// Panic paths per file: `.unwrap()` and `.expect(` outside test code and outside comments.
fn census() -> Vec<(String, usize)> {
    let src = repo_root().join("yidam/cli/src");
    let test_only = test_only_files(&src);
    let mut out = Vec::new();
    for entry in common::repo_walk(&src) {
        let path = entry.path();
        if !entry.file_type().is_file() || path.extension() != Some("rs".as_ref()) {
            continue;
        }
        // Files that are wholly test modules, declared `#[cfg(test)] mod …;` elsewhere.
        if test_only.contains(path) {
            continue;
        }
        let text = std::fs::read_to_string(path).expect("source is readable");
        let code = without_test_items(&text);
        let count = code
            .lines()
            .map(|l| l.split("//").next().unwrap_or(""))
            .map(|l| l.matches(".unwrap()").count() + l.matches(".expect(").count())
            .sum::<usize>();
        if count > 0 {
            let rel = path.strip_prefix(repo_root()).unwrap_or(path);
            out.push((rel.display().to_string(), count));
        }
    }
    out.sort();
    out
}

/// The number of ways production code can panic does not grow.
#[test]
fn the_panic_budget_is_not_exceeded() {
    let census = census();
    let total: usize = census.iter().map(|(_, n)| n).sum();

    assert!(
        total <= BUDGET,
        "production code now has {total} panic paths and the budget is {BUDGET}. A new \
         `unwrap`/`expect` in production is a decision: if it cannot be a `?`, raise BUDGET \
         in this file and say in the commit message why.\n{}",
        census
            .iter()
            .map(|(f, n)| format!("  {n:3}  {f}"))
            .collect::<Vec<_>>()
            .join("\n")
    );

    // A budget far above the truth is a budget that has stopped holding anything. This is
    // the half that makes the ratchet real rather than decorative.
    assert!(
        total + 5 >= BUDGET,
        "production has {total} panic paths against a budget of {BUDGET}. Lower BUDGET to \
         {total} — the slack is doing nothing except letting the next four through."
    );
}

/// The census is looking at production code, and at all of it.
///
/// Both halves matter. A stripper that ate the whole file would report zero and pass
/// forever; one that ate nothing would count the test suite and never pass at all.
#[test]
fn the_census_reads_production_code_and_not_the_tests() {
    let stripped = without_test_items(
        "pub fn real() { a.unwrap() }\n\
         #[cfg(test)]\n\
         mod tests {\n    fn t() { b.unwrap(); c.unwrap(); }\n}\n\
         pub fn also_real() { d.expect(\"x\") }\n",
    );
    assert!(stripped.contains("a.unwrap()"), "production code was eaten");
    assert!(
        stripped.contains("d.expect"),
        "code after a test module was eaten"
    );
    assert!(
        !stripped.contains("b.unwrap()"),
        "a test module survived the strip"
    );

    // And `#[cfg(test)] mod tests;` — a declaration, not a block — must not swallow the rest.
    let decl = without_test_items("#[cfg(test)]\nmod tests;\npub fn real() { a.unwrap() }\n");
    assert!(
        decl.contains("a.unwrap()"),
        "a `mod tests;` declaration swallowed the file after it"
    );

    let files = census();
    assert!(
        files.len() >= 5,
        "only {} files have any panic path; the census is reading the wrong tree",
        files.len()
    );
}

/// Test-only module files are found by their declaration, not by being called `tests.rs`.
///
/// This is the half that rots. `src/git/fixture.rs` arrived in #929 declared
/// `#[cfg(test)] pub(crate) mod fixture;` — test code by construction, counted as production
/// risk by a census that matched the *name* `tests`. Both halves are asserted per file:
/// every `tests.rs` under `src/` must still be discovered (so a broken path derivation goes
/// red rather than quietly widening the census), and `fixture.rs` must be discovered too (so
/// the name-matching answer cannot come back).
#[test]
fn every_test_only_module_is_found_by_its_declaration() {
    let src = repo_root().join("yidam/cli/src");
    let found = test_only_files(&src);

    for entry in common::repo_walk(&src) {
        let path = entry.path();
        if entry.file_type().is_file() && path.file_name() == Some("tests.rs".as_ref()) {
            assert!(
                found.contains(path),
                "{} is a test module and the census would count it as production",
                path.display()
            );
        }
    }

    let fixture = src.join("git/fixture.rs");
    assert!(
        fixture.is_file(),
        "{} moved; repoint this assertion at whatever now declares a `#[cfg(test)]` module \
         under a name other than `tests`",
        fixture.display()
    );
    assert!(
        found.contains(&fixture),
        "a `#[cfg(test)]` module not named `tests` was missed — the census is matching names \
         again"
    );
}
