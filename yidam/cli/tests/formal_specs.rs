//! `mise run verify` checks every formal spec, and something runs it.
//!
//! The task named three files and there are four. That is survivable only while the task
//! runs; #461 found it had never run at all — no `dir`, so `lake build` looked for a lakefile
//! four directories above the one that has it; a lakefile that did not elaborate; and neither
//! tool installed by anything. A spec added tomorrow would have been verified by nobody, in
//! silence, exactly as these were.
//!
//! **Both sides are discovered.** The specs come from walking the directory and the checked
//! set comes from parsing the task, and neither is a list in this file — the rule
//! `cli_reference.rs` states one level up:
//!
//! > A hardcoded roster is the same rot one level up: it would stop covering new commands
//! > without ever going red.
//!
//! The two languages need different questions asked of them. Dafny takes a file per
//! invocation, so the task must name each `.dfy`. Lake takes a library and follows imports,
//! so naming each `.lean` would be wrong — the property there is *reachability*: every source
//! is a declared root or is imported, transitively, from one. A `.lean` that is neither still
//! compiles for whoever opens it in an editor and is checked by no build.
//!
//! What this does not check is that the specs say anything true. Two of the six claims their
//! headers carry are an axiom or a `True`, which is #499's subject, not this file's.

use std::collections::{BTreeSet, VecDeque};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const SPEC_DIR: &str = "yidam/prelude/sdks/spec";

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// The `[tasks.verify]` table, parsed rather than grepped.
///
/// Parsed, because the three things asserted below are structure: which directory the task
/// runs in, which files its command lines name, and that CI invokes the task rather than
/// reproducing it. A regex over the file answers none of those and passes on a task that has
/// been commented out.
fn verify_task() -> toml::Table {
    let mise: toml::Table = read("mise.toml")
        .parse()
        .expect("mise.toml does not parse as TOML");
    mise.get("tasks")
        .and_then(|t| t.as_table())
        .and_then(|t| t.get("verify"))
        .and_then(|t| t.as_table())
        .cloned()
        .expect("mise.toml declares no [tasks.verify]")
}

fn verify_run_lines() -> Vec<String> {
    match verify_task().get("run") {
        Some(toml::Value::Array(a)) => a
            .iter()
            .map(|v| {
                v.as_str()
                    .expect("a non-string in verify's run list")
                    .to_string()
            })
            .collect(),
        Some(toml::Value::String(s)) => vec![s.clone()],
        other => panic!("[tasks.verify] has no usable `run` ({other:?})"),
    }
}

/// Files directly under the spec directory with the given extension, sorted.
fn spec_files(ext: &str) -> Vec<String> {
    let dir = repo_root().join(SPEC_DIR);
    let mut out: Vec<String> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", dir.display()))
        .map(|e| e.expect("a directory entry").path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|x| x == ext))
        .map(|p| {
            p.file_name()
                .expect("a file with no name")
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    out.sort();
    assert!(
        !out.is_empty(),
        "no *.{ext} under {SPEC_DIR} — this test is asserting nothing"
    );
    out
}

/// The task runs where the specs are, so its command lines can name them plainly.
///
/// Without this the task ran from the repository root, `lake` found no lakefile, and the
/// error was about a missing configuration file rather than about a spec.
#[test]
fn the_verify_task_runs_in_the_directory_that_holds_the_specs() {
    let dir = verify_task()
        .get("dir")
        .and_then(|d| d.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            panic!(
                "[tasks.verify] declares no `dir`, so mise runs it from the repository root \
                 where there is no lakefile — the #461 failure exactly"
            )
        });

    assert_eq!(
        dir.trim_end_matches('/'),
        SPEC_DIR,
        "[tasks.verify] runs in `{dir}`, which is not where the specs live"
    );
}

/// Every Dafny spec is named by the task. Dafny verifies one file per invocation, so a spec
/// the task does not name is a spec nothing checks.
#[test]
fn every_dafny_spec_is_verified_by_the_task() {
    let run = verify_run_lines();
    let dafny_args: BTreeSet<String> = run
        .iter()
        .filter(|line| line.split_whitespace().next() == Some("dafny"))
        .flat_map(|line| {
            line.split_whitespace()
                .skip(2) // `dafny verify`
                .filter(|a| !a.starts_with('-'))
                .map(str::to_string)
        })
        .collect();

    let present: BTreeSet<String> = spec_files("dfy").into_iter().collect();

    let unchecked: Vec<&String> = present.difference(&dafny_args).collect();
    assert!(
        unchecked.is_empty(),
        "{SPEC_DIR} holds {} Dafny spec(s) that [tasks.verify] does not name: {:?}\n\
         `mise run verify` is green without ever reading them.",
        unchecked.len(),
        unchecked
    );

    // And the other direction: a name the task carries for a file that is gone verifies
    // nothing and fails loudly only when somebody runs it.
    let stale: Vec<&String> = dafny_args.difference(&present).collect();
    assert!(
        stale.is_empty(),
        "[tasks.verify] names {stale:?}, which {SPEC_DIR} does not have"
    );
}

/// Every LEAN source is reached by the build.
///
/// Lake compiles a library from its declared `roots` and whatever they import, so the
/// question is not whether the task names each file — it names none of them — but whether
/// each file is reachable. An unreachable `.lean` type-checks in an editor and is compiled by
/// no CI, which is the same silence as an unnamed `.dfy` wearing different clothes.
#[test]
fn every_lean_source_is_reachable_from_a_declared_root() {
    let lakefile = read(&format!("{SPEC_DIR}/lakefile.lean"));

    // `roots := #[`Core]` — module names, backtick-quoted, in an array literal.
    let roots: BTreeSet<String> = lakefile
        .lines()
        .find(|l| l.trim_start().starts_with("roots"))
        .map(|l| {
            l.split('`')
                .skip(1)
                .step_by(2)
                .map(|m| m.trim_end_matches(']').trim().to_string())
                .filter(|m| !m.is_empty())
                .collect()
        })
        .unwrap_or_default();
    assert!(
        !roots.is_empty(),
        "lakefile.lean declares no `roots`, so `lake build Yidam` compiles nothing"
    );

    // Walk imports from the roots. A module name maps to `<name>.lean` in this flat package
    // (`srcDir := "."`), which is the only layout this directory has ever had.
    let module_path = |m: &str| repo_root().join(SPEC_DIR).join(format!("{m}.lean"));
    let mut reached: BTreeSet<String> = BTreeSet::new();
    let mut queue: VecDeque<String> = roots.iter().cloned().collect();
    while let Some(m) = queue.pop_front() {
        if !reached.insert(m.clone()) {
            continue;
        }
        let p = module_path(&m);
        assert!(
            p.exists(),
            "lakefile.lean reaches module `{m}`, and {SPEC_DIR}/{m}.lean does not exist"
        );
        let text = std::fs::read_to_string(&p).expect("a module that exists is readable");
        for line in text.lines() {
            if let Some(rest) = line.trim_start().strip_prefix("import ") {
                if let Some(name) = rest.split_whitespace().next() {
                    queue.push_back(name.to_string());
                }
            }
        }
    }

    let orphans: Vec<String> = spec_files("lean")
        .into_iter()
        .filter(|f| f != "lakefile.lean")
        .filter(|f| !reached.contains(f.trim_end_matches(".lean")))
        .collect();

    assert!(
        orphans.is_empty(),
        "{SPEC_DIR} holds {} LEAN source(s) no root reaches: {:?}\n\
         `lake build Yidam` never compiles them.",
        orphans.len(),
        orphans
    );
}

/// The toolchain the specs are checked under is written down.
///
/// The lakefile was written against a Lake that accepted `name := \"Yidam\"` as a String and
/// stopped elaborating under one that wants a `Name`. Nothing recorded which, so the file
/// could not even be dated. `lean-toolchain` is elan's own pin and the only one — see the
/// note beside `[tasks.verify]` for why there is not a second.
#[test]
fn the_lean_toolchain_is_pinned() {
    let pin = read(&format!("{SPEC_DIR}/lean-toolchain"));
    let pin = pin.trim();
    assert!(
        pin.starts_with("leanprover/lean4:"),
        "{SPEC_DIR}/lean-toolchain reads `{pin}`, which names no toolchain"
    );
}

/// Workflow text with its comments removed.
///
/// Necessary, and found the way these things are found: the first version of the test below
/// scanned the whole file, and deleting the `- run: mise run verify` step left it green —
/// because the job's own comment says the words "mise run verify" while explaining why the
/// step is there. A guard satisfied by prose about the code is not reading the code.
fn code_only(yaml: &str) -> String {
    yaml.lines()
        .map(|line| match line.split_once('#') {
            // A `#` at the start of the content, or after a space, opens a comment. Inside a
            // quoted value it would not — no workflow here has one, and the assertions below
            // are about step commands, which are the lines least likely to grow one.
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => {
                before.to_string()
            }
            _ => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// A workflow runs the task, and runs *the task* rather than a copy of it.
///
/// This is the whole of #461 in one assertion. `verify` was declared, was documented in
/// `prelude/sdks/README.md`, and no workflow mentioned dafny, lake, or lean — so the
/// specification had a checker on paper and none in fact.
///
/// It must be `mise run verify` and not the three commands spelled out again, because a
/// workflow that reproduces the task is a second answer to "what is verified" and drifts from
/// the first without either going red. `ci.yml` already carries that argument about the
/// cross-compile matrix mirroring `release.yml`.
#[test]
fn a_workflow_runs_the_verification_task() {
    let workflows = repo_root().join(".github/workflows");
    let mut runners: Vec<(String, String)> = Vec::new();
    for entry in std::fs::read_dir(&workflows).expect("no .github/workflows") {
        let path = entry.expect("a directory entry").path();
        if path.extension().is_none_or(|e| e != "yml") {
            continue;
        }
        let text = code_only(&std::fs::read_to_string(&path).expect("an unreadable workflow"));
        let name = path
            .file_name()
            .expect("a file with no name")
            .to_string_lossy()
            .into_owned();
        if text.contains("mise run verify") {
            runners.push((name, text));
        }
    }

    assert!(
        !runners.is_empty(),
        "no workflow runs `mise run verify`. The specs are checked by nothing, which is \
         the state #461 found and the state this test exists to keep the repository out of."
    );

    // The Lean version is pinned once, in `lean-toolchain`. A workflow naming its own is a
    // second pin, and two pins for one toolchain drift silently — the shape #397 cost a
    // release when `releases/latest` answered for four layers at once.
    for (name, text) in &runners {
        let hardcoded: Vec<&str> = text
            .lines()
            .filter(|l| l.contains("leanprover/lean4:"))
            .map(str::trim)
            .collect();
        assert!(
            hardcoded.is_empty(),
            "{name} names a Lean toolchain of its own ({hardcoded:?}); `lean-toolchain` is \
             the pin, and a second one drifts from it in silence"
        );
    }
}

/// The prose that documents the task documents the task that exists.
///
/// `prelude/sdks/README.md` carries a copy of `[tasks.verify]` as an example. It carried the
/// broken one — same missing `dir`, same three-of-four file list — which is how a reader
/// checked the documentation against the config and found them in agreement, both wrong.
#[test]
fn the_documented_verify_task_names_the_specs_the_real_one_does() {
    let readme = read("yidam/prelude/sdks/README.md");
    for spec in spec_files("dfy") {
        assert!(
            readme.contains(&spec),
            "prelude/sdks/README.md shows a `verify` task that does not mention {spec}"
        );
    }
}
