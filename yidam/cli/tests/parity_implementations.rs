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
//! **And all three SDKs answer each one.** `parity-check` asks whether every function has a
//! fixture and whether every fixture directory has a runner. Both passed for as long as
//! `find_reachable` and `find_citations` existed in Rust alone: the directories were there and
//! one runner read them, so two thirds of what `parity/README.md` calls "all three SDKs must
//! implement identically" was missing with every gate green. The two tests at the foot of this
//! file are the third question — is each function implemented in each SDK, and does each SDK's
//! runner load each fixture directory (#530).

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

// ── and all three SDKs answer each one (#530) ─────────────────────────────────

const PRELUDE_SDKS: &str = "yidam/prelude/sdks";

/// The SDK directories: every subdirectory of `yidam/prelude/sdks` that carries a test suite.
///
/// Discovered, not named. `parity/` holds fixtures and `spec/` holds proofs; neither has a
/// `tests/`, and a fourth SDK is covered the day it grows one. A list here would be a list
/// that stops covering what arrives after it — which is the failure this file is about, one
/// directory up.
fn sdk_dirs() -> Vec<String> {
    let root = repo_root();
    let mut dirs: Vec<String> = std::fs::read_dir(root.join(PRELUDE_SDKS))
        .expect("yidam/prelude/sdks is readable")
        .filter_map(Result::ok)
        .filter(|e| e.path().join("tests").is_dir())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    dirs.sort();
    assert!(
        dirs.len() >= 3,
        "found {} SDK(s) under {PRELUDE_SDKS} with a tests/ directory: {dirs:?} — the parity \
         surface is a promise about three, so this scan is looking in the wrong place",
        dirs.len()
    );
    dirs
}

/// Source with its comments removed, so a name in prose cannot answer for a name in code.
///
/// `defines` above stays off prose by anchoring at the start of a line, where the keyword
/// would have to be. The thing looked for below is a call in the middle of one, so anchoring
/// is not available and the comments have to go instead — including Python docstrings, which
/// are prose held in a string. A guard that greps a whole file is satisfied by the paragraph
/// explaining what it is looking for; that is the fault `formal_specs.rs` records.
fn strip_comments(text: &str, ext: &str) -> String {
    let py = ext == "py";
    // `'` opens a string in TypeScript and Python. In Rust it opens a lifetime, and a scanner
    // that read `&'a str` as a string would swallow everything after it.
    let quotes: &[char] = match ext {
        "rs" => &['"'],
        "ts" => &['"', '\'', '`'],
        _ => &['"', '\''],
    };
    let c: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < c.len() {
        let (this, next) = (c[i], c.get(i + 1).copied().unwrap_or('\0'));

        if (py && this == '#') || (!py && this == '/' && next == '/') {
            while i < c.len() && c[i] != '\n' {
                i += 1;
            }
            continue;
        }
        if !py && this == '/' && next == '*' {
            i += 2;
            while i + 1 < c.len() && !(c[i] == '*' && c[i + 1] == '/') {
                i += 1;
            }
            i = (i + 2).min(c.len());
            continue;
        }
        if py && quotes.contains(&this) && next == this && c.get(i + 2) == Some(&this) {
            i += 3;
            while i + 2 < c.len() && !(c[i] == this && c[i + 1] == this && c[i + 2] == this) {
                i += 1;
            }
            i = (i + 3).min(c.len());
            continue;
        }
        // A string literal is copied through — the fixture directory name is one, and a `//`
        // inside a URL is not a comment.
        if quotes.contains(&this) {
            out.push(this);
            i += 1;
            while i < c.len() {
                if c[i] == '\\' {
                    out.push(c[i]);
                    if let Some(&e) = c.get(i + 1) {
                        out.push(e);
                    }
                    i += 2;
                    continue;
                }
                out.push(c[i]);
                let closed = c[i] == this;
                i += 1;
                if closed {
                    break;
                }
            }
            continue;
        }
        out.push(this);
        i += 1;
    }
    out
}

/// Whether `text` passes `name` to a fixture loader.
///
/// The directory name as the sole argument of a call — `load_fixtures("find_reachable")`,
/// `loadFixtures('find_reachable')` — which is the one thing that goes away when a function's
/// cases are deleted from a runner. Weaker forms do not: the Rust runner's failure messages
/// carry `"find_reachable({node_path})"`, and a suite that merely *mentions* the name is
/// exactly the vacuous pass this is looking for.
fn loads_fixtures_for(text: &str, name: &str) -> bool {
    text.contains(&format!("(\"{name}\")")) || text.contains(&format!("('{name}')"))
}

/// Every authored test source under one SDK's `tests/`, comments removed.
///
/// The whole directory, not a file named `parity`. "Does this SDK read that fixture
/// directory" is a question about the suite `mise run parity` runs, and a name pattern here
/// would be one more list to keep in step with the filenames three languages happen to use.
fn runner_sources(sdk: &str) -> Vec<String> {
    let root = repo_root().canonicalize().expect("the repo root exists");
    authored_sources(&root.join(PRELUDE_SDKS).join(sdk).join("tests"))
        .into_iter()
        .filter_map(|p| {
            let ext = p.extension()?.to_string_lossy().into_owned();
            Some(strip_comments(&std::fs::read_to_string(&p).ok()?, &ext))
        })
        .collect()
}

/// Every SDK defines every parity function.
///
/// `parity-check` asks two questions — every function has a fixture, every fixture directory
/// has a runner — and neither is this one. Both passed for as long as `find_reachable` and
/// `find_citations` existed in Rust alone: the fixtures were there and the Rust runner read
/// them, so two thirds of a promise the README calls "all three SDKs must implement
/// identically" was missing with every gate green (#530).
#[test]
fn every_sdk_implements_every_parity_function() {
    let found = definitions();
    let sdks = sdk_dirs();

    let mut missing: Vec<String> = Vec::new();
    for (function, files) in &found {
        for sdk in &sdks {
            let prefix = format!("{PRELUDE_SDKS}/{sdk}/");
            let implemented = files
                .iter()
                .any(|f| f.starts_with(&prefix) && !f.contains("/tests/"));
            if !implemented {
                missing.push(format!("  {sdk} does not define `{function}`"));
            }
        }
    }

    assert!(
        missing.is_empty(),
        "{} (function, SDK) pair(s) on the parity surface with no implementation:\n{}\n\n\
         The parity fixtures grade whoever calls them. An SDK that does not define a surface \
         function is not compared to the other two — it passes vacuously, which is not what \
         `parity/README.md` promises about it. Implement it, or take the function off the \
         surface: out of `parity-check`'s `functions` list, into the exceptions, and with a \
         `parity/VERSION` bump, because a function leaving the surface is a contract change.",
        missing.len(),
        missing.join("\n")
    );
}

/// And every SDK's runner reads every fixture directory.
///
/// The third question, and the one the other two cannot reach: a function can be implemented
/// in all three and still be graded in one. Both sides are discovered — the functions from
/// `parity-check`'s own loop, the runners by walking each SDK's `tests/` — so this cannot rot
/// into naming one SDK and forgetting the others.
#[test]
fn every_sdk_runner_reads_every_fixture_directory() {
    let functions = parity_functions();
    let sdks = sdk_dirs();

    let mut ungraded: Vec<String> = Vec::new();
    let mut read = 0usize;
    for sdk in &sdks {
        let sources = runner_sources(sdk);
        assert!(
            !sources.is_empty(),
            "no test sources found under {PRELUDE_SDKS}/{sdk}/tests"
        );
        for f in &functions {
            if sources.iter().any(|text| loads_fixtures_for(text, f)) {
                read += 1;
            } else {
                ungraded.push(format!("  {sdk} reads no fixtures for `{f}`"));
            }
        }
    }

    assert!(
        read > 0,
        "the scan matched no fixture-loader call in any of {} SDK(s), which means it is \
         reading the wrong files or the wrong call shape",
        sdks.len()
    );
    assert!(
        ungraded.is_empty(),
        "{} fixture director(ies) that an SDK never reads:\n{}\n\n\
         A fixture nobody loads looks exactly like one that is doing work. `parity-check` \
         asks whether each function has a fixture and whether each fixture has *a* runner; \
         both pass while one runner does all the reading, which is how two functions were \
         graded in Rust and nowhere else for as long as they existed.",
        ungraded.len(),
        ungraded.join("\n")
    );
}

/// The runner scan reads calls, not the prose around them.
///
/// Otherwise the test above passes because a comment names the function, which is the exact
/// substitution `formal_specs.rs` found: prose answering for code in a check that greps a
/// whole file.
#[test]
fn the_runner_scan_reads_calls_and_not_prose() {
    assert!(loads_fixtures_for(
        r#"let fixtures = load_fixtures("find_reachable");"#,
        "find_reachable"
    ));
    assert!(loads_fixtures_for(
        "  const fixtures = loadFixtures('find_reachable')",
        "find_reachable"
    ));

    // A mention is not a call, and neither is a failure message that interpolates the name.
    assert!(!loads_fixtures_for(
        r#"assert_eq!(got, want, "find_reachable({node_path})");"#,
        "find_reachable"
    ));
    assert!(!loads_fixtures_for(
        r#"assert!(!fixtures.is_empty(), "no find_reachable fixtures found");"#,
        "find_reachable"
    ));
    // …nor a longer name that contains it.
    assert!(!loads_fixtures_for(
        r#"load_fixtures("find_reachable_v2")"#,
        "find_reachable"
    ));

    // Comments go before the search, in all three languages.
    assert!(!loads_fixtures_for(
        &strip_comments(
            "// load_fixtures(\"find_reachable\") used to be here\n",
            "rs"
        ),
        "find_reachable"
    ));
    assert!(!loads_fixtures_for(
        &strip_comments("/** loadFixtures('find_reachable') */\n", "ts"),
        "find_reachable"
    ));
    assert!(!loads_fixtures_for(
        &strip_comments("# load_fixtures(\"find_reachable\")\n", "py"),
        "find_reachable"
    ));
    assert!(!loads_fixtures_for(
        &strip_comments(
            "\"\"\"Reads load_fixtures(\"find_reachable\").\"\"\"\n",
            "py"
        ),
        "find_reachable"
    ));

    // …and the call survives stripping when it is code.
    assert!(loads_fixtures_for(
        &strip_comments(
            "let f = load_fixtures(\"find_reachable\"); // the graph one\n",
            "rs"
        ),
        "find_reachable"
    ));
    // A `//` inside a string is not a comment, and a Rust lifetime is not a string.
    assert!(strip_comments(r#"let u = "https://example.com/x";"#, "rs").contains("example.com/x"));
    assert!(
        loads_fixtures_for(
            &strip_comments(
                "fn f<'a>(x: &'a str) { load_fixtures(\"find_reachable\"); }",
                "rs"
            ),
            "find_reachable"
        ),
        "a lifetime was read as an unterminated string and swallowed the rest of the file"
    );
}
