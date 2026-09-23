//! Every declared capability is implemented, and everything implemented is declared.
//!
//! `.yidam/capabilities.toml` says what may run and `.yidam/capabilities/` holds what runs.
//! Nothing made the two agree. A declaration pointing at a script somebody renamed fails at
//! invocation with `sh: no such file or directory`; a script nobody declares is dead weight a
//! reader will take for a working capability — which is #472's finding one level down from the
//! index column it is mostly about. *A directory is not a capability, and a declaration is not
//! an implementation.*
//!
//! # Discovered, and it never carries a list
//!
//! The rule this file is written against, from #472:
//!
//! > A check that names the known capabilities stops covering new ones without ever going red
//! > — the hole #336 found latent in `orphan-in`, and the reason the `parity-check` task walks
//! > `fixtures/*/` rather than listing directories. **Discover the declared set and compare it
//! > to what is on disk; never carry a list.**
//!
//! So nothing here names `travel-tier`, `disclosure-envelope`, `.yidam/capabilities/`, or
//! `examples/streamflow`. The declared set comes from every example's manifest; the
//! implementations come from `git ls-files`; and even the *directory* implementations live in
//! is discovered — it is wherever the declarations point, so a corpus that put its calculators
//! somewhere else is covered by the same two assertions with nothing edited here.
//!
//! # It reads declarations, not prose
//!
//! The second half of the same instruction is that *a guard grepping a whole file is satisfied
//! by that file's own prose*. Nothing below greps. A capability is resolved through its `run`
//! argv against the set of tracked paths, so a comment mentioning a script is not a script, and
//! a script's own header describing a capability does not declare one.
//!
//! # Vacuity is the failure one level up
//!
//! A discovery predicate that matches nothing passes everything.
//! [`some_example_declares_a_capability_and_implements_it`] is what closes that, and is the
//! test to fix first if this file ever goes quiet.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use common::{examples, Example};

/// One capability, as declared: what it is, and the argv it is invoked through.
#[derive(serde::Deserialize)]
struct Declared {
    run: Vec<String>,
}

#[derive(serde::Deserialize, Default)]
struct Manifest {
    #[serde(default)]
    capability: BTreeMap<String, Declared>,
}

/// What an example declares, read from its own manifest.
///
/// Deserialized into the two fields this file uses rather than into the CLI's own
/// `Capability`, which is `pub(crate)` to `yidam` and not reachable from an integration test.
/// The narrow shape is also the honest one: a guard that deserialized every field would fail
/// on a manifest the executor accepts, for a reason that is nothing to do with what it checks.
fn declared(corpus: &Path) -> BTreeMap<String, Declared> {
    let Ok(text) = std::fs::read_to_string(corpus.join(".yidam/capabilities.toml")) else {
        return BTreeMap::new();
    };
    toml::from_str::<Manifest>(&text)
        .unwrap_or_else(|e| panic!("{} does not parse: {e}", corpus.display()))
        .capability
}

/// Every path the example's git index holds, repository-relative.
///
/// `git ls-files` and not a directory walk, for the reason `example_corpus.rs` gives about
/// these fixtures: an untracked file under `examples/*/.yidam/` is invisible to
/// `Example::materialize`, which builds its tree from the index. A guard that walked the
/// filesystem would call a capability implemented that no run could ever be given.
fn tracked(corpus: &Path) -> BTreeSet<String> {
    let out = Command::new("git")
        .current_dir(corpus)
        .args(["ls-files"])
        .output()
        .expect("git ls-files runs");
    assert!(out.status.success(), "git ls-files failed in {corpus:?}");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .map(str::to_string)
        .collect()
}

/// The `run` arguments that name a path in this repository rather than a program or a flag.
///
/// A token counts when the tracked set holds a file by that name, which is a fact about the
/// repository and not a rule about which arguments are paths. `sh`, `cargo`, `-p` and `--` are
/// excluded because no file is called any of those, and nothing here had to decide that.
fn implementation_paths<'a>(cap: &'a Declared, tracked: &BTreeSet<String>) -> Vec<&'a str> {
    cap.run
        .iter()
        .map(String::as_str)
        .filter(|arg| tracked.contains(*arg))
        .collect()
}

/// A `run` token that was meant to be a path in this repository and is not one.
///
/// The complement of the above, and the two together are what make a renamed script a failure
/// rather than a silence: a token holding a `/` is a path by construction — no program on
/// `PATH` is spelled with one — so one that names no tracked file names nothing.
fn dangling_paths<'a>(cap: &'a Declared, tracked: &BTreeSet<String>) -> Vec<&'a str> {
    cap.run
        .iter()
        .map(String::as_str)
        .filter(|arg| arg.contains('/') && !arg.starts_with('-') && !tracked.contains(*arg))
        .collect()
}

/// Every `(example, step, declaration)` this repository ships.
fn every_declaration() -> Vec<(String, String, Declared, BTreeSet<String>)> {
    let mut all = Vec::new();
    for name in examples() {
        let e = Example::materialize(&name);
        let tracked = tracked(&e.path());
        for (step, cap) in declared(&e.path()) {
            all.push((name.clone(), step, cap, tracked.clone()));
        }
    }
    all
}

/// The guard on the discovery: a file that discovers nothing asserts nothing.
///
/// Both halves, because they fail for different reasons. No declaration at all means the
/// manifest is untracked or absent; declarations that resolve to no implementation mean the
/// path rule below stopped matching, which would leave every other test here trivially true.
#[test]
fn some_example_declares_a_capability_and_implements_it() {
    let found = every_declaration();
    assert!(
        !found.is_empty(),
        "no example declares a capability, so every test in this file is vacuous. \
         `.yidam/capabilities.toml` must be git-tracked — an untracked file under \
         `examples/*/.yidam/` is invisible to `Example::materialize`, which builds its tree \
         from `git ls-files`"
    );
    let implemented: usize = found
        .iter()
        .filter(|(_, _, cap, tracked)| !implementation_paths(cap, tracked).is_empty())
        .count();
    assert!(
        implemented > 0,
        "{} capabilities are declared and not one of them resolves to a tracked file, so the \
         path rule in `implementation_paths` matches nothing and every assertion below is \
         vacuously true",
        found.len()
    );
}

/// Nothing declared is missing: every `run` path a declaration names is in the repository.
///
/// This is the half a rename breaks. Left to the executor it surfaces as exit 127 and a shell
/// complaining about a file, in a corpus whose manifest is what is actually wrong.
#[test]
fn every_declared_capability_resolves_to_something_this_repository_holds() {
    for (example, step, cap, tracked) in every_declaration() {
        let dangling = dangling_paths(&cap, &tracked);
        assert!(
            dangling.is_empty(),
            "`{step}` in {example} is invoked as `{}`, and {} names no file this repository \
             tracks.\n  A declaration pointing at nothing is a capability that cannot run, \
             and it fails at invocation rather than at review.",
            cap.run.join(" "),
            dangling.join(", ")
        );
        assert!(
            !implementation_paths(&cap, &tracked).is_empty(),
            "`{step}` in {example} is invoked as `{}`, which names no file in this \
             repository.\n  An example capability implemented somewhere else cannot be run \
             by `capability_run.rs`, which invokes every declared capability against a \
             materialized copy of the corpus and nothing beyond it.",
            cap.run.join(" ")
        );
    }
}

/// Nothing implemented is undeclared: every file beside a capability's implementation is one.
///
/// The directories are **discovered**, not named: each is the parent of a path some
/// declaration points at, so this covers wherever a corpus keeps its calculators. A corpus
/// that moved them needs no edit here, and a corpus that added a second directory is covered
/// the moment one declaration points into it.
///
/// What it catches is the row #472 is about, in the form it takes before a crate exists: a
/// script sitting in the capability directory that nothing runs. A reader finding it there
/// reasonably concludes the corpus has that capability. It does not.
#[test]
fn every_implementation_in_a_capability_directory_is_declared() {
    for name in examples() {
        let e = Example::materialize(&name);
        let tracked = tracked(&e.path());
        let caps = declared(&e.path());

        let mut named: BTreeSet<&str> = BTreeSet::new();
        let mut dirs: BTreeSet<&str> = BTreeSet::new();
        for cap in caps.values() {
            for path in implementation_paths(cap, &tracked) {
                named.insert(path);
                if let Some(dir) = path.rsplit_once('/') {
                    dirs.insert(dir.0);
                }
            }
        }
        if dirs.is_empty() {
            continue; // nothing in this example points into a directory; the test above says so
        }

        let undeclared: Vec<&String> = tracked
            .iter()
            .filter(|p| {
                dirs.iter()
                    .any(|d| p.starts_with(&format!("{d}/")) && !named.contains(p.as_str()))
            })
            .collect();
        assert!(
            undeclared.is_empty(),
            "{name} holds {} in {}, and no capability in .yidam/capabilities.toml is invoked \
             through it.\n  A reader finding it there concludes this corpus has that \
             capability; nothing can run it.\n  Declare it, or delete it.",
            undeclared
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            dirs.iter().copied().collect::<Vec<_>>().join(", ")
        );
    }
}
