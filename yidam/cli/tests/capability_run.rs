//! `yidam run` — the first code path in this repository that can author a `compute:` commit.
//!
//! `prelude/GRAPH.md` closes the commit vocabulary and four of its operational verbs named
//! acts nothing could perform. What makes this a vertical slice rather than a format is that
//! everything below runs the real binary against a real corpus and reads the commits it
//! wrote: a manifest with no executor would pass a schema test while asserting nothing.
//!
//! # Discovered, not listed
//!
//! Every test that runs a capability reads the declared set out of the example's own
//! manifest. A suite naming `travel-tier` in nine places would stop covering a second
//! capability the day one is declared, without ever going red — the guard-list shape #448
//! found, and the one #472 is explicitly written against.
//!
//! Discovery has the failure one level up, which is that a predicate matching nothing passes
//! everything. [`an_example_declares_a_capability_for_these_tests_to_run`] is what closes it
//! and is the test to fix first if this suite ever goes quiet.

mod common;

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use common::{examples, Example};

/// The capabilities an example declares, read from its manifest.
fn declared(corpus: &Path) -> Vec<String> {
    #[derive(serde::Deserialize, Default)]
    struct Manifest {
        #[serde(default)]
        capability: BTreeMap<String, toml::Value>,
    }
    let Ok(text) = std::fs::read_to_string(corpus.join(".yidam/capabilities.toml")) else {
        return Vec::new();
    };
    toml::from_str::<Manifest>(&text)
        .unwrap_or_default()
        .capability
        .into_keys()
        .collect()
}

/// Every `(example, step)` this repository ships, materialized one corpus at a time.
fn every_capability() -> Vec<(String, String)> {
    let mut all = Vec::new();
    for name in examples() {
        let e = Example::materialize(&name);
        for step in declared(&e.path()) {
            all.push((name.clone(), step));
        }
    }
    all
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
    assert!(
        out.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// Every file under `root` except git's own, by relative path.
///
/// The working-tree comparison is over bytes and over the *set* of paths, because the two
/// failures are different: a run that rewrote a node and a run that dropped a scratch file
/// beside it are both "the working tree moved" and only one of them is visible in a diff of
/// files that exist in both snapshots.
fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != std::ffi::OsStr::new(".git") || e.depth() == 0)
    {
        let entry = entry.unwrap();
        if !entry.file_type().is_file() {
            continue;
        }
        let rel = entry
            .path()
            .strip_prefix(root)
            .unwrap()
            .to_string_lossy()
            .to_string();
        out.insert(rel, std::fs::read(entry.path()).unwrap());
    }
    out
}

/// The guard on the discovery above: a suite that runs nothing passes everything.
#[test]
fn an_example_declares_a_capability_for_these_tests_to_run() {
    let found = every_capability();
    assert!(
        !found.is_empty(),
        "no example declares a capability, so every test in this file is vacuous. \
         `.yidam/capabilities.toml` must be git-tracked — an untracked file under \
         `examples/*/.yidam/` is invisible to `Example::materialize`, which builds its \
         tree from `git ls-files`"
    );
}

/// The definition of done's first two bullets, over whatever is declared.
#[test]
fn a_run_lands_a_commit_the_vocabulary_gate_accepts() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let before = git(&e.path(), &["rev-parse", "HEAD"]);

        let (out, err, code) = e.run(&["run", &step]);
        assert_eq!(
            code, 0,
            "`yidam run {step}` failed in {example}:\n{out}{err}"
        );

        let after = git(&e.path(), &["rev-parse", "HEAD"]);
        assert_ne!(before, after, "`{step}` landed no commit in {example}");

        let subject = git(&e.path(), &["log", "-1", "--format=%s"]);
        assert!(
            subject.starts_with("compute: "),
            "`{subject}` is the commit a calculator wrote and does not lead with `compute:`"
        );

        // Asked of the gate rather than of the vocabulary constant, because the gate is what a
        // derived repository runs over its whole log — and `unrecognized-verb` is warn
        // severity, so a subject outside the vocabulary leaves the exit code at zero. The
        // finding is the answer here; the exit code is not.
        let (out, err, code) = e.run(&["lint", "--commits", "--format", "json"]);
        assert_eq!(
            code, 0,
            "`lint --commits` failed after `{step}`:\n{out}{err}"
        );
        let report: serde_json::Value = serde_json::from_str(&out).unwrap();
        let unrecognized = report["checks"]
            .as_array()
            .expect("lint checks")
            .iter()
            .find(|c| c["id"] == "unrecognized-verb")
            .expect("the vocabulary check ran");
        assert_eq!(
            unrecognized["violations"].as_array().map(Vec::len),
            Some(0),
            "`{subject}` is reported off-vocabulary: {}",
            unrecognized["violations"]
        );
    }
}

/// The invariant RFC-0026 §2 states, asserted through the classifier the CLI ships rather
/// than by re-deciding here what an epistemic verb is.
///
/// *"A run authors operational commits directly. Every epistemic commit it produces goes to a
/// proposal branch, and nothing merges itself."* On the baseline there is nothing to permit:
/// every commit a run wrote is operational, and the manifest that licensed it could not have
/// declared otherwise.
#[test]
fn no_commit_a_run_wrote_on_the_baseline_is_epistemic() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        assert_eq!(e.run(&["run", &step]).2, 0);

        let authored = git(&e.path(), &["log", "--author=run@yidam", "--format=%H"]);
        let by_a_run: Vec<&str> = authored.lines().collect();
        assert!(
            !by_a_run.is_empty(),
            "no commit on the baseline is authored by a run, so this asserts nothing — \
             either the author changed or nothing was landed"
        );

        let (out, _, code) = e.run(&["log", "--format", "json"]);
        assert_eq!(code, 0);
        let report: serde_json::Value = serde_json::from_str(&out).unwrap();
        let entries = report["entries"].as_array().expect("log entries");
        for hash in &by_a_run {
            let entry = entries
                .iter()
                .find(|e| e["hash"] == *hash)
                .unwrap_or_else(|| panic!("{hash} is not in `yidam log`"));
            assert_eq!(
                entry["kind"], "operational",
                "a run wrote {hash} — {} — and it classifies as epistemic",
                entry["subject"]
            );
        }
    }
}

/// The bullet that is tested by running rather than by reading: against a **dirty** tree,
/// diffing before and after.
///
/// Tracked-and-modified, staged, and untracked all at once, because a command that touched
/// the index would be invisible to a check that only compared file bytes.
#[test]
fn a_run_leaves_a_dirty_working_tree_exactly_as_it_found_it() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let root = e.path();

        std::fs::write(
            root.join("README.md"),
            "an edit nobody asked this command to keep\n",
        )
        .unwrap();
        std::fs::write(root.join("scratch.txt"), "untracked\n").unwrap();
        std::fs::write(root.join("staged.txt"), "staged\n").unwrap();
        git(&root, &["add", "staged.txt"]);

        let before = tree(&root);
        let index_before = git(&root, &["status", "--porcelain"]);

        let (out, err, code) = e.run(&["run", &step]);
        assert_eq!(code, 0, "{out}{err}");

        assert_eq!(
            tree(&root),
            before,
            "`yidam run {step}` changed the working tree of {example}"
        );
        assert_eq!(
            git(
                &root,
                &[
                    "status",
                    "--porcelain",
                    "--",
                    ":!.yidam/computed",
                    ":!.yidam/runs"
                ]
            ),
            index_before,
            "`yidam run {step}` changed the index of {example}"
        );
    }
}

/// `format_version` in the receipt from this first commit, not added later.
///
/// The order is the bullet. A field added after release strands every producer already
/// shipped: a consumer reading it crashes against the binary that never wrote it.
#[test]
fn the_receipt_carries_a_format_version_and_the_commit_it_was_computed_from() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let input = git(&e.path(), &["rev-parse", "HEAD"]);
        assert_eq!(e.run(&["run", &step]).2, 0);

        let path = format!(".yidam/runs/{step}.yml");
        let text = git(&e.path(), &["show", &format!("HEAD:{path}")]);
        let r: serde_yaml::Value = serde_yaml::from_str(&text)
            .unwrap_or_else(|err| panic!("{path} in {example} is not YAML: {err}"));

        assert_eq!(
            r["format_version"].as_u64(),
            Some(1),
            "{path} carries no format_version"
        );
        assert_eq!(
            r["input"]["commit"].as_str(),
            Some(input.as_str()),
            "{path} names a commit other than the one it was computed from"
        );
        assert!(
            r["input"]["files"]
                .as_sequence()
                .is_some_and(|f| !f.is_empty()),
            "{path} records no input files, so it is provenance for nothing"
        );
        assert!(
            r["outputs"].as_sequence().is_some_and(|o| !o.is_empty()),
            "{path} records no outputs"
        );
    }
}

/// The report contract, on the same terms as every other surface.
#[test]
fn the_json_report_carries_the_envelope_every_other_report_carries() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let (out, err, code) = e.run(&["run", &step, "--format", "json"]);
        assert_eq!(code, 0, "{out}{err}");
        let v: serde_json::Value =
            serde_json::from_str(&out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}"));
        assert_eq!(v["format_version"], "1");
        assert!(v["yidam"]["version"].is_string());
        assert!(v["root"].is_string());
        assert_eq!(v["step"], step.as_str());
        assert_eq!(v["verb"], "compute");
        assert!(v["committed"]["commit"].is_string());
    }
}

/// A re-run against an unchanged corpus records that nothing happened by writing nothing.
#[test]
fn a_second_run_against_an_unchanged_corpus_writes_no_commit() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        assert_eq!(e.run(&["run", &step]).2, 0);
        let after_first = git(&e.path(), &["rev-parse", "HEAD"]);

        let (out, err, code) = e.run(&["run", &step]);
        assert_eq!(code, 0, "{out}{err}");
        assert_eq!(
            git(&e.path(), &["rev-parse", "HEAD"]),
            after_first,
            "a second `run {step}` in {example} wrote a commit, so the log records how often \
             somebody ran the command rather than what changed"
        );
    }
}

/// `writes` is load-bearing rather than documentation — RFC-0026 §4.
///
/// Built by mutation rather than by a fixture manifest: the step that overreaches is the one
/// the example actually declares, edited to write one file more, so the refusal is shown
/// against the real path and not against a case invented to be refused.
#[test]
fn a_step_that_writes_outside_its_declaration_is_refused_and_lands_nothing() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let root = e.path();

        let script = root.join(format!(".yidam/capabilities/{step}.sh"));
        if !script.exists() {
            continue; // a capability implemented some other way; #472 generalises this
        }
        let mut text = std::fs::read_to_string(&script).unwrap();
        text.push_str(
            "mkdir -p \"$YIDAM_OUT/docs\"\nprintf 'overreach\\n' > \"$YIDAM_OUT/docs/oops.md\"\n",
        );
        std::fs::write(&script, text).unwrap();
        git(
            &root,
            &[
                "commit",
                "-qam",
                "fix: the step now writes where it may not",
            ],
        );
        let before = git(&root, &["rev-parse", "HEAD"]);

        let (out, err, code) = e.run(&["run", &step]);
        assert_ne!(code, 0, "an overreaching `{step}` was accepted:\n{out}");
        assert!(
            err.contains("docs/oops.md") && err.contains("declared"),
            "the refusal names neither what was written nor what was declared:\n{err}"
        );
        assert_eq!(
            git(&root, &["rev-parse", "HEAD"]),
            before,
            "a refused `{step}` landed a commit anyway"
        );
    }
}

/// The whole safety argument, asserted where it is enforced.
///
/// A corpus that could declare an epistemic verb here could license its own runs to author
/// `establish:` on the baseline. RFC-0026 §3 puts the rule in Rust with no override path, and
/// this is the mutation that shows the rule is load-bearing rather than described.
#[test]
fn a_capability_declaring_an_epistemic_verb_does_not_load() {
    for (example, step) in every_capability() {
        let e = Example::materialize(&example);
        let manifest = e.path().join(".yidam/capabilities.toml");
        let text = std::fs::read_to_string(&manifest)
            .unwrap()
            .replace("verb   = \"compute\"", "verb   = \"establish\"");
        assert!(
            text.contains("establish"),
            "the manifest of {example} was not mutated, so this test asserts nothing"
        );
        std::fs::write(&manifest, text).unwrap();

        let (out, err, code) = e.run(&["run", &step]);
        assert_ne!(code, 0, "an epistemic verb was accepted:\n{out}");
        assert!(
            err.contains("epistemic verb"),
            "the refusal does not say why:\n{err}"
        );
    }
}
