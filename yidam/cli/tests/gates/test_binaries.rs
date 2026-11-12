//! How many test binaries this crate links, and that every test file is in one of them (#1488).
//!
//! Every file directly under `tests/` is a crate of its own to cargo, so a separate link of
//! the whole library and everything it reaches. #1010 moved the 47 gates that only read files
//! into `tests/gates/`; #1488 moved the 78 that start the binary into `tests/suite/`. Measured
//! on macOS after one edit to a core source file, `cargo test --no-run` went from 18–21s wall
//! and 114–133s of CPU across 83 units to 7.8s and 22s across 7. That is per edit, and
//! `ci-cli-full` pays it once per feature set.
//!
//! Two things undo it without anything going red, and this gate is for both:
//!
//! - **A new file at `tests/<name>.rs`.** It compiles, it runs, it passes, and it is one more
//!   link. The habit it follows is cargo's default. A test that genuinely needs a binary of
//!   its own — `harness = false`, or process-global state that nextest's per-test process
//!   does not already isolate — goes in [`OWN_BINARY`] with the reason.
//! - **A file in a suite directory that its `main.rs` does not declare.** That one is worse:
//!   cargo never compiles it, so its tests do not fail, they do not exist. Nothing else
//!   would notice, because a test that is not built reports nothing.
//!
//! Suites are discovered — any `tests/<dir>/main.rs` — rather than listed, so a third suite
//! is covered the day it appears.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// Top-level `tests/*.rs` files allowed to be a binary of their own, with why. Empty: no
/// test here has needed one. nextest runs each test in its own process whatever binary it
/// is in, so isolation is not a reason by itself.
const OWN_BINARY: &[(&str, &str)] = &[];

fn tests_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests")
}

fn rs_files(dir: &Path) -> BTreeSet<String> {
    std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", dir.display()))
        .map(|e| e.expect("a readable entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == "rs"))
        .map(|p| p.file_stem().unwrap().to_string_lossy().into_owned())
        .collect()
}

/// Every `tests/<dir>/` holding a `main.rs`: the directories cargo builds as one binary.
fn suites() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = std::fs::read_dir(tests_dir())
        .expect("tests/ is readable")
        .map(|e| e.expect("a readable entry").path())
        .filter(|p| p.join("main.rs").is_file())
        .collect();
    out.sort();
    out
}

/// The module names a `main.rs` declares with `mod <name>;`, whatever attributes precede it.
fn declared(main_rs: &str) -> BTreeSet<String> {
    main_rs
        .lines()
        .map(str::trim)
        .filter(|l| !l.starts_with("//"))
        .filter_map(|l| l.strip_prefix("pub ").unwrap_or(l).strip_prefix("mod "))
        .filter_map(|rest| rest.strip_suffix(';'))
        .map(|name| name.trim().to_owned())
        .collect()
}

#[test]
fn no_test_file_is_a_binary_of_its_own() {
    let allowed: BTreeSet<&str> = OWN_BINARY.iter().map(|(f, _)| *f).collect();
    let extra: Vec<String> = rs_files(&tests_dir())
        .into_iter()
        .filter(|stem| !allowed.contains(format!("{stem}.rs").as_str()))
        .map(|stem| format!("tests/{stem}.rs"))
        .collect();
    assert!(
        extra.is_empty(),
        "these link as a test binary of their own:\n  {}\n\nEach is a separate link of the \
         whole library, per edit and per feature set (#1488). Move it to tests/suite/ (it \
         starts the binary) or tests/gates/ (it only reads files) and declare it as a `mod` \
         in that directory's main.rs. If it truly needs its own process image, name it in \
         OWN_BINARY with the reason.",
        extra.join("\n  ")
    );

    let manifest = std::fs::read_to_string(tests_dir().join("../Cargo.toml")).unwrap();
    assert!(
        !manifest.contains("[[test]]"),
        "Cargo.toml declares a [[test]] target, which is a binary this gate cannot see by \
         listing tests/"
    );
}

#[test]
fn every_file_in_a_suite_is_declared_by_its_main() {
    let suites = suites();
    let names: BTreeSet<String> = suites
        .iter()
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    // Discovery, checked against what it must at least find, so a scanner that finds nothing
    // cannot pass by having nothing to check.
    for must in ["gates", "suite"] {
        assert!(
            names.contains(must),
            "found no tests/{must}/main.rs; found {names:?}"
        );
    }

    let mut undeclared = Vec::new();
    for dir in &suites {
        let main = std::fs::read_to_string(dir.join("main.rs")).unwrap();
        let mods = declared(&main);
        for stem in rs_files(dir) {
            if stem != "main" && !mods.contains(&stem) {
                undeclared.push(format!(
                    "tests/{}/{stem}.rs",
                    dir.file_name().unwrap().to_string_lossy()
                ));
            }
        }
    }
    assert!(
        undeclared.is_empty(),
        "these are never compiled, so their tests do not run:\n  {}\n\nDeclare each as a \
         `mod` in its directory's main.rs.",
        undeclared.join("\n  ")
    );
}

#[test]
fn the_declaration_reader_sees_what_it_is_looking_for() {
    let got = declared(
        "//! doc mentioning mod nope;\n#[path = \"../common/mod.rs\"]\nmod common;\n\
         mod alpha;\npub mod beta;\n// mod commented;\n",
    );
    let want: BTreeSet<String> = ["common", "alpha", "beta"]
        .into_iter()
        .map(String::from)
        .collect();
    assert_eq!(got, want);
}
