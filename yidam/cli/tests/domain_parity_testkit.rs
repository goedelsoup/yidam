//! The vacuity guard is not a line anybody has to remember to write.
//!
//! Every domain's `rust/tests/parity.rs` used to open with the same `fixture_dir` +
//! `load_fixtures` pair — hashing them with comments stripped gave **one hash across all
//! fourteen crates**, and the only difference anywhere was that six carried two extra comment
//! lines naming their own directory. ~30 lines × 14.
//!
//! The copies agreed, so this is not a drift report. It is about *what* was duplicated:
//!
//! ```ignore
//! assert!(!fixtures.is_empty(), "no <domain>.<fn> fixtures found");
//! ```
//!
//! That line is the whole distance between this suite and fourteen tests that iterate an empty
//! vector and pass. A fifteenth domain added by copying a neighbour can lose it, and **a parity
//! test that loads no fixtures reports success identically to one that loads twelve** — the run
//! is green and the function under test was never called.
//!
//! `yidam-domain-testkit` moved the assertion inside `load_fixtures`, which shares the code.
//! Sharing is the smaller half. A fifteenth domain can still write its own `read_dir` loop and
//! never call the testkit at all, and then the duplication has moved rather than the hole
//! closing. **This file is what makes the testkit unskippable**: it walks the domains rather
//! than listing them, and fails one that loads fixtures by hand.
//!
//! `domain-parity-check` (mise.toml) is the complement from the other side — it fails on a
//! fixture *directory* that is empty, and structurally cannot see a test that never asks for a
//! directory.
//!
//! Note what this file does **not** do: it does not restate the fourteen domain names. A list
//! here would stop covering the fifteenth domain on the day it is added, without ever going
//! red — which is the same defect one level out, and the one this repository has shipped
//! before.

use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const DOMAINS: &str = "yidam/prelude/domains";

/// The crate every domain suite must load its fixtures through.
const TESTKIT: &str = "yidam-domain-testkit";

/// `parity` holds the fixtures and the testkit; it is not a domain.
const NOT_A_DOMAIN: &str = "parity";

/// Every domain that ships a Rust implementation, discovered by walking `domains/`.
///
/// A directory with a `rust/Cargo.toml`, minus `parity`. Sorted, so a failure names the same
/// domain first on every machine.
fn rust_domains() -> Vec<(String, PathBuf)> {
    let root = repo_root().join(DOMAINS);
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(&root)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", root.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .filter_map(|p| {
            let name = p.file_name()?.to_string_lossy().to_string();
            (name != NOT_A_DOMAIN && p.join("rust/Cargo.toml").is_file()).then_some((name, p))
        })
        .collect();
    out.sort();
    out
}

/// The floor the discovery has to clear before any assertion over it means anything.
///
/// Fourteen domains ship today. A walk that finds none — a renamed directory, a test run from
/// somewhere unexpected — satisfies every `for` loop below and reports success, which is the
/// exact failure mode this file was written about. So it is asserted rather than assumed, and
/// as a floor rather than an equality: a fifteenth domain is the event this file exists for
/// and must not be the event that breaks it.
const AT_LEAST: usize = 14;

#[test]
fn every_rust_domain_is_discovered() {
    let found = rust_domains();
    assert!(
        found.len() >= AT_LEAST,
        "discovered {} domain crates under {DOMAINS}, expected at least {AT_LEAST} — a walk \
         that finds nothing passes every other test in this file",
        found.len()
    );
}

#[test]
fn every_domain_suite_loads_fixtures_through_the_testkit() {
    let mut offenders: Vec<String> = Vec::new();

    for (name, dir) in rust_domains() {
        let suite = dir.join("rust/tests/parity.rs");
        if !suite.is_file() {
            offenders.push(format!("{name}: no rust/tests/parity.rs"));
            continue;
        }
        let src = std::fs::read_to_string(&suite).unwrap();
        let code = strip_comments(&src);

        let krate = TESTKIT.replace('-', "_");
        if !code.contains(&format!("{krate}::")) {
            offenders.push(format!("{name}: parity.rs never names {TESTKIT}"));
        }
        // The bypass this file exists to catch: a hand-rolled loader, with or without the
        // emptiness assertion. `read_dir` is the operation; a suite that does its own is
        // outside the guard whether or not it currently remembers to check.
        if code.contains("fn load_fixtures") || code.contains("read_dir") {
            offenders.push(format!(
                "{name}: parity.rs loads fixtures itself — use {TESTKIT}::load_fixtures, whose \
                 emptiness assertion is the point"
            ));
        }

        let manifest = std::fs::read_to_string(dir.join("rust/Cargo.toml")).unwrap();
        if !manifest.contains(TESTKIT) {
            offenders.push(format!(
                "{name}: Cargo.toml has no {TESTKIT} dev-dependency"
            ));
        }
    }

    assert!(
        offenders.is_empty(),
        "these domain suites are outside the fixture-emptiness guard:\n  {}",
        offenders.join("\n  ")
    );
}

/// The testkit's guard is where the suites stopped keeping it.
///
/// Asserted against the source rather than exercised, because exercising it means emptying a
/// fixture directory — which is the mutation this change was verified with by hand and is not
/// a thing a test may do to the tree it is running in.
#[test]
fn the_testkit_refuses_to_load_nothing() {
    let lib = repo_root().join(DOMAINS).join("parity/testkit/src/lib.rs");
    let src = std::fs::read_to_string(&lib)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", lib.display()));
    let code = strip_comments(&src);
    assert!(
        code.contains("assert!") && code.contains("is_empty()"),
        "{} no longer asserts that it loaded something — every domain suite is now vacuous \
         and green",
        lib.display()
    );
}

/// Source with `//` comments removed.
///
/// Every check above asks whether the *code* does something. This file's own subject is prose
/// that looks like code: the doc comment on `load_fixtures` quotes the assertion it is
/// describing, and a scan that counted that would be satisfied by a crate whose guard had been
/// deleted and whose comment about it had not.
fn strip_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
