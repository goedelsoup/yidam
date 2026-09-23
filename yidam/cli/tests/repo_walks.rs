//! One rule for what is not authored, and one place that knows it.
//!
//! #900: `yidam/web/docs/test/quality-render.mjs` builds into `dist-quality-test` and removes
//! it when the block ends, so an interrupted `npm test` leaves a Starlight build behind.
//! `.gitignore` has declared `dist-*/` since #467 — so `git status` shows nothing — while the
//! cargo scanners' hand-written skip lists said `dist` and not `dist-*`. Three `design_tokens`
//! assertions then failed on hundreds of Starlight's own compiled custom properties, during a
//! gate that has nothing to do with the docs, naming files the developer did not write in a
//! directory they cannot see. The natural reading is "my change broke the design-token gate".
//!
//! Twenty-two walk sites across thirteen files carried nine different answers to the same
//! question, and two of them were not scanning too much but answering wrong: a `#[cfg(test)]`
//! walk in `lint/checks.rs` graded two class files out of a gitignored foreign checkout, and
//! `yidam clone` copied `dist-quality-*` debris into every derived repository. This file
//! asserts there is now one answer, in [`common::repo_walk`], which reads it from git.
//!
//! What it does **not** cover: a walk of a temporary directory. Those are the other half of
//! the `WalkDir` calls in this suite — a materialized derived repo, a fixture corpus, a vault
//! store — and they are not this working tree, have no `.gitignore` of their own, and prune
//! `.git` at most. The rule below is about walks of *this repository*, which is where a
//! gitignored build directory can appear.

use std::path::{Path, PathBuf};

mod common;

/// Names that only a hand-written exclusion list has a reason to spell.
///
/// `node_modules` is the reliable tell: no list of build directories has ever been written
/// without it, so a file that names one of these is either pruning a walk or is one of the
/// two exceptions below.
const BUILD_DIR_NAMES: &[&str] = &["node_modules", "__pycache__", ".venv", ".pytest_cache"];

/// The one file that names a build directory for a reason that is not pruning a walk of
/// this repository.
///
/// Each is re-checked below rather than merely permitted: an entry whose file no longer
/// contains the name it excuses is stale, and a stale exemption is how an allowlist stops
/// describing the thing it was written about.
const NOT_A_PRUNE: &[(&str, &str)] = &[
    (
        "tests/derived_repo_smoke.rs",
        "asserts build junk was NOT copied into a derived repo — the names are the finding, \
         not the filter, and the tree it walks is a tempdir",
    ),
    (
        "tests/hook_claims.rs",
        "filters paths out of `git ls-files` output, which never walks a working tree",
    ),
    (
        "src/cmd/copy.rs",
        "`yidam clone` copies a tree into a destination that is not a repository, so it \
         cannot ask git what to leave behind; it names the conventions and reads \
         `CACHEDIR.TAG` for the rest",
    ),
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Every `.rs` in the crate, as a path relative to `yidam/cli/`.
///
/// `src/` and not only `tests/`, because `checks.rs` is where the eleventh copy of the list
/// was found: `collect_ont_files` is a `#[cfg(test)]` walk of this repository living beside
/// the code it checks, pruned by `.git | target | node_modules | dist`. It reached
/// `.local/ext-fixture/.yidam/corpus/` — a staged foreign repository — so a test whose name
/// says *in this repository* was grading two class files this repository does not ship. A
/// guard that had looked only under `tests/` would have reported all clear.
fn suite_sources() -> Vec<(String, String)> {
    let dir = crate_dir();
    let mut out: Vec<(String, String)> = ["src", "tests"]
        .iter()
        .flat_map(|sub| common::repo_walk(&dir.join(sub)))
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

/// Prose, so the rule is read from code rather than from the comment explaining the rule.
///
/// This file names every excluded directory it forbids, in the paragraphs above and in
/// [`BUILD_DIR_NAMES`]; a scan that did not strip them would find itself and nothing else.
/// Line comments and doc comments only — a `//` inside a string literal is rare here and
/// stripping it would cost more than it saves.
fn code_of(text: &str) -> String {
    text.lines()
        .filter(|l| !l.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_detector_sees_what_it_is_looking_for() {
    let sources = suite_sources();
    assert!(
        sources.len() > 150,
        "only {} sources found across src/ and tests/; this crate has many more, so every \
         assertion below is scanning the wrong directory",
        sources.len()
    );
    let helper = sources
        .iter()
        .find(|(p, _)| p == "tests/common/mod.rs")
        .expect("common/mod.rs is the shared helper and must be in the scan");
    // The known positive. Without this, a `code_of` that returned the empty string — or a
    // scan that read no files at all — would satisfy every assertion below in silence.
    assert!(
        code_of(&helper.1).contains("filter_entry"),
        "common/mod.rs no longer prunes a walk; either the helper moved, in which case this \
         file must follow it, or `code_of` is eating the code"
    );
    // And what the helper must *not* contain: the names themselves. It asks git, which is
    // the whole repair — a helper holding one more hand-written list would satisfy
    // `only_the_shared_helper_prunes_a_walk_of_this_repository` while changing nothing.
    for name in BUILD_DIR_NAMES {
        assert!(
            !code_of(&helper.1).contains(name),
            "common/mod.rs names {name:?} in code. The point of #900 is that the rule is \
             git's; a list in the helper is the same defect with one copy instead of eight"
        );
    }
    assert!(
        code_of(&helper.1).contains("--ignored"),
        "common/mod.rs no longer reads the exclusion set from git, so nothing here knows \
         what `.gitignore` says"
    );

    // The detector itself, on text whose answer is known. Without this a `code_of` that
    // returned the empty string would make the scan above pass over every file in the suite.
    let named = BUILD_DIR_NAMES[0];
    assert!(
        code_of(&format!("let x = \"{named}\";")).contains(named),
        "the scan cannot see a build directory named in code"
    );
    assert!(
        !code_of(&format!("// a comment about {named}")).contains(named),
        "the scan grades comments, so prose describing the rule reads as an instance of it"
    );
}

#[test]
fn only_the_shared_helper_prunes_a_walk_of_this_repository() {
    let excused: Vec<&str> = NOT_A_PRUNE.iter().map(|(f, _)| *f).collect();
    let mut offenders = Vec::new();

    for (path, text) in suite_sources() {
        // This file, which has to spell the vocabulary it forbids in order to look for it.
        if path == "tests/common/mod.rs"
            || path == "tests/repo_walks.rs"
            || excused.contains(&path.as_str())
        {
            continue;
        }
        let code = code_of(&text);
        for name in BUILD_DIR_NAMES {
            if code.contains(name) {
                offenders.push(format!("{path}: names {name:?}"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "these carry their own idea of what is not authored:\n  {}\n\nWhich is #900: the \
         lists disagree, and the one that is wrong is wrong silently. Walk with \
         `common::repo_walk` (or `repo_walk_keeping` for an exclusion that is a judgement \
         about the question, not about build output) and delete the names.",
        offenders.join("\n  ")
    );
}

#[test]
fn every_exemption_is_still_describing_its_file() {
    for (file, why) in NOT_A_PRUNE {
        let path = crate_dir().join(file);
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{file} is exempted and unreadable: {e} — {why}"));
        let code = code_of(&text);
        assert!(
            BUILD_DIR_NAMES.iter().any(|n| code.contains(n)),
            "{file} is exempted for naming a build directory ({why}) and no longer names \
             one. Delete the entry: an exemption nobody needs is a hole nobody is watching."
        );
    }
}

/// The helper's own rule, checked against git rather than against itself.
///
/// [`only_the_shared_helper_prunes_a_walk_of_this_repository`] proves there is one list;
/// this proves the list is right. The debris in #900 was a real directory with a real
/// `.gitignore` entry, and the failure was that the walk did not agree with the entry.
#[test]
fn the_walk_agrees_with_gitignore() {
    let root = common::repo_root();
    let debris = root.join("yidam/web/docs/dist-quality-repo-walks");
    let file = debris.join("_astro/common.CouWXoG5.css");
    std::fs::create_dir_all(file.parent().unwrap()).expect("scratch dir is writable");
    std::fs::write(&file, "a{color:var(--sl-color-text-accent)}\n").expect("scratch is writable");

    let seen = |p: &Path| common::repo_walk(&root.join("yidam/web/docs")).any(|e| e.path() == p);
    let found = seen(&file);
    std::fs::remove_dir_all(&debris).expect("scratch dir is removable");

    assert!(
        !found,
        "an interrupted `npm test` leaves `dist-quality-*` behind — `.gitignore:55` says \
         `dist-*/` and the walk reached into it anyway. That is #900 exactly: hundreds of \
         Starlight's own custom properties graded as if this repository had written them."
    );
}

/// A walk of this repository, written the way the guard can see it.
fn walks_the_repo(line: &str) -> bool {
    line.contains("WalkDir::new(") && line.contains("repo_root()")
}

/// A test that walks *this repository* does it through the shared walker.
///
/// [`only_the_shared_helper_prunes_a_walk_of_this_repository`] asks whether a walk carries
/// its own exclusion list. That is the defect #900 reported, but it is not the whole shape:
/// a walk can reach build output by carrying no list at all, and a walk can carry one whose
/// name is too ordinary to put in [`BUILD_DIR_NAMES`]. `toolchain_pins.rs` was both — it
/// filtered on `target`, which is also what half this crate calls a function parameter, so
/// the name check could never have been widened to see it.
///
/// The rule is deliberately literal: `WalkDir::new(…)` and `repo_root()` on one line. It
/// cannot see a root that arrived through a variable, and it does not try — a rule that
/// guessed would have to guess about `src/`, where `repo_root()` means the *corpus* being
/// operated on and a hand-rolled walk is correct. Scoped to `tests/` for exactly that
/// reason. Measured when written: one offender, no false positives.
#[test]
fn a_test_that_walks_this_repository_uses_the_shared_walker() {
    // The rule, before it is pointed at anything real. Two substrings are easy to mistype
    // and a mistyped pair matches nothing, which reads exactly like a clean repository.
    assert!(
        walks_the_repo("    WalkDir::new(repo_root().join(dir))"),
        "the rule does not match the shape it was written for"
    );
    assert!(
        !walks_the_repo("    common::repo_walk(&repo_root().join(dir))"),
        "the rule matches the fix it is asking for"
    );
    assert!(
        !walks_the_repo("    WalkDir::new(tmp.path())"),
        "the rule matches a tempdir walk, which is not its business"
    );

    let mut offenders = Vec::new();
    let mut scanned = 0usize;
    let mut saw_a_walk = 0usize;

    for (path, text) in suite_sources() {
        if !path.starts_with("tests/") || path == "tests/repo_walks.rs" {
            continue;
        }
        scanned += 1;
        for (i, line) in code_of(&text).lines().enumerate() {
            if line.contains("WalkDir::new(") {
                saw_a_walk += 1;
            }
            if walks_the_repo(line) {
                offenders.push(format!("{path}:{}: {}", i + 1, line.trim()));
            }
        }
    }

    // A clean tree and a scan that read nothing produce the same empty `offenders`, so the
    // absence of findings is only evidence once the scan is known to have looked. The floor
    // is on files, and the second count is the one that matters: `WalkDir` is still the
    // right tool for a tempdir, so those lines exist and must be visible from here. If they
    // are not, the walk, the path prefix or `code_of` is broken and this proves nothing.
    assert!(
        scanned > 75,
        "only {scanned} test sources reached the rule; this suite has many more, so an          empty finding list says nothing"
    );
    assert!(
        saw_a_walk > 0,
        "no `WalkDir::new(` anywhere in {scanned} test sources. The tempdir walks are still          written that way, so the scan is reading the wrong text — not a clean repository"
    );

    assert!(
        offenders.is_empty(),
        "these walk this repository without the shared walker:\n  {}\n\nA bare `WalkDir` \
         descends whatever a build left behind — `target/`, `dist-quality-*`, `.venv` — \
         and reads it as though this repository had authored it. Use \
         `common::repo_walk`.",
        offenders.join("\n  ")
    );
}
