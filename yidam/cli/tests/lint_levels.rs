//! A lint level the crate cannot satisfy is a build nobody can run.
//!
//! `[lints]` in `Cargo.toml` applies to the whole package under every feature set, and this
//! gate compiles one of them. So a level that only the *gated* code violates is invisible
//! here and fails on `ci (cli · full features)` — a job that runs on main and the weekly
//! schedule and never on a pull request.
//!
//! That is not hypothetical. #464 set `unsafe_code = "forbid"` on the reasoning that this
//! crate has no business writing unsafe. It has exactly one `unsafe` block, in
//! `export_sqlite.rs`, behind `--features export-sqlite`; `forbid` cannot be lifted by
//! `#[allow]`, which is precisely what separates it from `deny`. `--all-features` stopped
//! compiling and **main stayed red for four merges** before anybody looked at that job.
//!
//! The same phase measured `clippy::unwrap_used` before rejecting it — 1,831 warnings, all
//! from tests — and added this one by assuming. The difference between the two decisions is
//! the whole lesson, so the check is derived from the tree rather than from a reading of it.

use std::path::PathBuf;

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// The `[lints.rust]` table, as `name = "level"` pairs.
fn rust_lints() -> Vec<(String, String)> {
    let manifest: toml::Value = toml::from_str(
        &std::fs::read_to_string(crate_root().join("Cargo.toml")).expect("manifest"),
    )
    .expect("Cargo.toml parses");
    let table = manifest
        .get("lints")
        .and_then(|l| l.get("rust"))
        .and_then(|r| r.as_table())
        .expect("no [lints.rust] table; the levels this file grades are gone");
    table
        .iter()
        .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_string())))
        .collect()
}

/// Every `.rs` under `src/`, gated or not — which is the point.
fn source_files() -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![crate_root().join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

/// Source with `//` comments dropped, so prose about `unsafe` is not a use of it.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|l| match l.split_once("//") {
            Some((before, _)) if !before.ends_with(':') => before,
            _ => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Nothing is `forbid` that the crate's own code needs an exception to.
///
/// `forbid` is `deny` that `#[allow]` cannot lift. It is the right level for a rule with no
/// legitimate exception and the wrong one the moment a legitimate exception exists — and the
/// failure mode is a build that cannot compile at all, under whatever feature set admits the
/// offending file.
#[test]
fn nothing_is_forbidden_that_the_crate_actually_does() {
    let forbidden: Vec<String> = rust_lints()
        .into_iter()
        .filter(|(_, level)| level == "forbid")
        .map(|(name, _)| name)
        .collect();

    // What a `forbid`-level rust lint would be violated by, spelled as what to look for in
    // the source. Only the ones this crate has an opinion about; a lint added to the table
    // that is not here fails the completeness check below rather than passing silently.
    let probes: &[(&str, &[&str])] =
        &[("unsafe_code", &["unsafe {", "unsafe extern", "unsafe fn"])];

    let mut trouble = Vec::new();
    for name in &forbidden {
        let Some((_, needles)) = probes.iter().find(|(lint, _)| lint == name) else {
            trouble.push(format!(
                "  {name} is `forbid` and this test does not know what violates it. Add a \
                 probe to `probes`, or use `deny` so a justified exception stays possible."
            ));
            continue;
        };
        for path in source_files() {
            let text = code_only(&std::fs::read_to_string(&path).unwrap_or_default());
            for (i, line) in text.lines().enumerate() {
                if needles.iter().any(|n| line.contains(n)) {
                    let rel = path.strip_prefix(crate_root()).unwrap_or(&path);
                    trouble.push(format!("  {}:{} violates `{name}`", rel.display(), i + 1));
                }
            }
        }
    }

    assert!(
        trouble.is_empty(),
        "`forbid` cannot be lifted by `#[allow]`, so each of these is a feature set that \
         does not compile — and only `ci (cli · full features)` would notice, on main, after \
         the merge:\n{}\n\nUse `deny` and put an `#[allow]` with a reason at the site.",
        trouble.join("\n")
    );
}

/// The levels the table sets are levels cargo understands.
///
/// A typo is not a stricter lint, it is no lint: cargo rejects the manifest, or the entry
/// silently means nothing. Cheap to check and it keeps the table honest.
#[test]
fn every_lint_level_is_one_cargo_accepts() {
    const LEVELS: &[&str] = &["allow", "warn", "deny", "forbid", "expect"];
    let bad: Vec<String> = rust_lints()
        .into_iter()
        .filter(|(_, level)| !LEVELS.contains(&level.as_str()))
        .map(|(name, level)| format!("  {name} = {level:?}"))
        .collect();
    assert!(
        bad.is_empty(),
        "not lint levels cargo knows:\n{}",
        bad.join("\n")
    );
}

/// The crate's one `unsafe` block is annotated, so the `deny` is doing work everywhere else.
///
/// Without this the level could be quietly relaxed to `allow` and nothing would notice —
/// which is how a lint stops being a rule while still appearing in the table.
#[test]
fn the_unsafe_that_exists_is_the_one_that_is_allowed() {
    let level = rust_lints()
        .into_iter()
        .find(|(name, _)| name == "unsafe_code")
        .map(|(_, level)| level)
        .expect("`unsafe_code` is no longer in [lints.rust]");
    assert_eq!(
        level, "deny",
        "`unsafe_code` is `{level}`. `allow` is not a rule, and `forbid` is what broke \
         `--all-features` for four merges."
    );

    let mut sites = Vec::new();
    for path in source_files() {
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let code = code_only(&text);
        if !code.contains("unsafe {") && !code.contains("unsafe extern") {
            continue;
        }
        let rel = path.strip_prefix(crate_root()).unwrap_or(&path);
        sites.push(rel.display().to_string());
        assert!(
            text.contains("#[allow(unsafe_code)]"),
            "{} uses unsafe and carries no `#[allow(unsafe_code)]`, so it does not compile \
             under the crate's own lint table",
            rel.display()
        );
    }
    assert_eq!(
        sites.len(),
        1,
        "the crate has {} files using unsafe ({sites:?}). One is the documented FFI \
         registration; a second is a decision somebody should make deliberately, and this \
         test is where they are asked to.",
        sites.len()
    );
}
