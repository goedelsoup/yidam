//! A parity function is implemented in the SDKs and nowhere else.
//!
//! The parity surface exists because there are exactly three answers to each of these
//! questions and fixtures keep them in step. `yidam/cli/src/regen.rs` carried a fourth
//! `update_regen`, and it disagreed: given empty content it wrote a blank line between the
//! markers where the contract — `parity/fixtures/update_regen/empty-new-content.toml`, and
//! `graph.dfy`'s `ClearingASectionLeavesNoBlankLine` — requires none.
//!
//! It survived because nothing compared them. The parity suite grades the three SDKs against
//! each other; a copy that is not an SDK sits outside the comparison whose whole purpose is
//! that there are three. That is the hole this file closes (#528).
//!
//! **Both sides are discovered.** The function list is parsed out of `parity-check`'s own
//! `functions` loop in `mise.toml` — which `VERSIONING.md` names as authoritative and
//! deliberately does not restate — and the definitions come from walking the tree. Neither is
//! a list in this file.
//!
//! What this does *not* check is that all three SDKs implement each one. Two of them do not,
//! which is #530.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SDK_DIR: &str = "yidam/prelude/sdks";

/// The parity functions, read out of `parity-check`'s `functions=` line.
///
/// Parsed rather than restated. `VERSIONING.md` makes the point about itself: it used to copy
/// this list, and said "the nine" while the loop walked ten. A copy here would be the same
/// drift one file further out.
fn parity_functions() -> Vec<String> {
    let mise: toml::Table = std::fs::read_to_string(repo_root().join("mise.toml"))
        .expect("mise.toml is readable")
        .parse()
        .expect("mise.toml parses");
    let run = mise["tasks"]["parity-check"]["run"]
        .as_str()
        .expect("[tasks.parity-check] has a string `run`");
    let line = run
        .lines()
        .find(|l| l.trim_start().starts_with("functions="))
        .expect("parity-check declares no `functions=` line");
    let names: Vec<String> = line
        .split_once('=')
        .expect("a `functions=` line with no `=`")
        .1
        .trim()
        .trim_matches('"')
        .split_whitespace()
        .map(str::to_string)
        .collect();
    assert!(
        names.len() >= 8,
        "parity-check names {} function(s); this test would be asserting almost nothing",
        names.len()
    );
    names
}

/// `find_reachable` → `findReachable`, the name TypeScript spells it with.
fn camel(snake: &str) -> String {
    let mut parts = snake.split('_');
    let head = parts.next().unwrap_or_default().to_string();
    parts.fold(head, |mut acc, w| {
        let mut cs = w.chars();
        if let Some(c) = cs.next() {
            acc.push(c.to_ascii_uppercase());
            acc.push_str(cs.as_str());
        }
        acc
    })
}

/// Whether a line *defines* `name` in the language of `ext`.
///
/// Anchored at the start of the line, after whitespace only. That is what keeps the check off
/// prose: a doc comment reading `/// fn update_regen(…)` has `///` where the keyword would
/// have to be, and a Python `# def update_regen(` the same. A scan that matched anywhere on
/// the line would be satisfied by the paragraph explaining the function — the fault
/// `formal_specs.rs` records from the other direction.
fn defines(line: &str, name: &str, ext: &str) -> bool {
    let t = line.trim_start();
    let after = |rest: &str| -> bool {
        rest.strip_prefix(name)
            .is_some_and(|r| r.starts_with('(') || r.starts_with('<') || r.starts_with(' '))
    };
    match ext {
        "rs" => {
            let t = t.strip_prefix("pub ").unwrap_or(t);
            let t = t.strip_prefix("async ").unwrap_or(t);
            t.strip_prefix("fn ").is_some_and(after)
        }
        "py" => {
            let t = t.strip_prefix("async ").unwrap_or(t);
            t.strip_prefix("def ").is_some_and(after)
        }
        "ts" => {
            let t = t.strip_prefix("export ").unwrap_or(t);
            let t = t.strip_prefix("async ").unwrap_or(t);
            t.strip_prefix("function ").is_some_and(after)
        }
        _ => false,
    }
}

/// Authored source, skipping everything a build or a package manager put there.
///
/// Dot-directories plus the build directories that do not start with one — the rule
/// `prelude_commit_vocabulary.rs` arrived at after a name list missed `.venv` by two orders of
/// magnitude. `yidam/tests/results/` is evaluation output: derived repositories captured as
/// they were, and a duplicate in one of those is a finding about that run, not about this tree.
fn authored_sources(root: &Path) -> Vec<PathBuf> {
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_string_lossy();
            let generated = name.starts_with('.')
                || name == "node_modules"
                || name == "target"
                || name == "dist"
                || name == "results"
                || name == "__pycache__"
                || name.ends_with(".egg-info");
            !(generated && e.file_type().is_dir())
        })
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path().to_owned())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x == "rs" || x == "ts" || x == "py")
        })
        .collect()
}

/// Every definition of every parity function, by repo-relative path.
fn definitions() -> BTreeMap<String, Vec<String>> {
    let root = repo_root().canonicalize().expect("the repo root exists");
    let functions = parity_functions();
    let mut found: BTreeMap<String, Vec<String>> =
        functions.iter().map(|f| (f.clone(), Vec::new())).collect();

    for path in authored_sources(&root) {
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().into_owned())
            .unwrap_or_default();
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        for f in &functions {
            let spelled = if ext == "ts" { camel(f) } else { f.clone() };
            if text.lines().any(|l| defines(l, &spelled, &ext)) {
                found
                    .get_mut(f)
                    .expect("a known function")
                    .push(rel.clone());
            }
        }
    }
    found
}

/// No parity function is implemented outside the SDKs.
#[test]
fn a_parity_function_lives_in_the_sdks_and_nowhere_else() {
    let found = definitions();

    // A scan that matched nothing would pass the assertion below while checking no code at
    // all — the shape every guard in this repository is written against.
    let total: usize = found.values().map(Vec::len).sum();
    assert!(
        total >= 8,
        "the scan found {total} definition(s) of {} parity function(s), which means it is \
         reading the wrong files or the wrong keywords",
        found.len()
    );

    let mut strays: Vec<String> = Vec::new();
    for (function, files) in &found {
        assert!(
            !files.is_empty(),
            "no implementation of `{function}` was found anywhere — either it was renamed \
             without `parity-check`'s `functions` list moving with it, or this scan stopped \
             recognising a definition"
        );
        for f in files {
            if !f.starts_with(SDK_DIR) {
                strays.push(format!("  {f} defines `{function}`"));
            }
        }
    }

    assert!(
        strays.is_empty(),
        "{} implementation(s) of a parity function outside {SDK_DIR}:\n{}\n\n\
         The parity fixtures hold the SDKs to one answer each. A copy they do not grade is \
         free to disagree with them, which is what `yidam/cli/src/regen.rs`'s `update_regen` \
         did about empty content for as long as it existed. Call the SDK.",
        strays.len(),
        strays.join("\n")
    );
}

/// The scan recognises a definition in each of the three languages.
///
/// Otherwise the test above passes because the scan is blind to a language, and the whole
/// point is the one place a fourth implementation could hide.
#[test]
fn the_scan_recognises_all_three_languages() {
    assert!(defines(
        "pub fn update_regen(text: &str) -> String {",
        "update_regen",
        "rs"
    ));
    assert!(defines(
        "    fn update_regen<T>(x: T) {",
        "update_regen",
        "rs"
    ));
    assert!(defines(
        "def update_regen(text: str) -> str:",
        "update_regen",
        "py"
    ));
    assert!(defines(
        "export function updateRegen(text: string): string {",
        "updateRegen",
        "ts"
    ));

    // …and not prose about one.
    assert!(!defines(
        "/// fn update_regen(…) is the one in yidam-core",
        "update_regen",
        "rs"
    ));
    assert!(!defines("// def update_regen(x)", "update_regen", "py"));
    assert!(!defines("  * function updateRegen()", "updateRegen", "ts"));
    // …nor a call, nor a longer name.
    assert!(!defines(
        "let out = update_regen(text, cmd, body);",
        "update_regen",
        "rs"
    ));
    assert!(!defines(
        "pub fn update_regen_block(t: &str) {}",
        "update_regen",
        "rs"
    ));

    assert_eq!(camel("find_reachable"), "findReachable");
    assert_eq!(camel("update_regen"), "updateRegen");
    assert_eq!(camel("parse_node"), "parseNode");
}
