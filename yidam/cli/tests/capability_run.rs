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

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use common::{examples, Example};

/// One capability, in the two fields this file needs of it.
#[derive(serde::Deserialize, Default)]
struct Declared {
    #[serde(default)]
    after: Vec<String>,
}

/// The capabilities an example declares, read from its manifest.
fn declared(corpus: &Path) -> BTreeMap<String, Declared> {
    #[derive(serde::Deserialize, Default)]
    struct Manifest {
        #[serde(default)]
        capability: BTreeMap<String, Declared>,
    }
    let Ok(text) = std::fs::read_to_string(corpus.join(".yidam/capabilities.toml")) else {
        return BTreeMap::new();
    };
    toml::from_str::<Manifest>(&text)
        .unwrap_or_default()
        .capability
}

/// Every `(example, step)` this repository ships, materialized one corpus at a time.
fn every_capability() -> Vec<(String, String)> {
    let mut all = Vec::new();
    for name in examples() {
        let e = Example::materialize(&name);
        for step in declared(&e.path()).into_keys() {
            all.push((name.clone(), step));
        }
    }
    all
}

/// Every `(example, step)` that nothing else declares itself `after`.
///
/// The verb mutations below need one of these and cannot use any capability. Making a step
/// epistemic changes where its commit goes, and a step something waits for **cannot** be
/// epistemic at all: the manifest refuses the pair, because what an epistemic run writes lands
/// on `propose/<head>` and is not in the tree a dependent would be materialized from. Mutating
/// an upstream would therefore assert something about the manifest's dependency rule rather
/// than about the route.
///
/// Discovered like everything else here. A corpus whose capabilities were all upstream of one
/// another would yield exactly one terminal step, and the guard below says so rather than
/// letting the loop run zero times.
fn every_terminal_capability() -> Vec<(String, String)> {
    let mut all = Vec::new();
    for name in examples() {
        let e = Example::materialize(&name);
        let caps = declared(&e.path());
        let waited_for: BTreeSet<&str> = caps
            .values()
            .flat_map(|c| c.after.iter().map(String::as_str))
            .collect();
        for step in caps.keys() {
            if !waited_for.contains(step.as_str()) {
                all.push((name.clone(), step.clone()));
            }
        }
    }
    all
}

use common::git::out as git;

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
        assert_eq!(e.run(&["run", &step]).2, 0);

        let path = format!(".yidam/runs/{step}.yml");
        // The commit this step's inputs were materialized from, which is the parent of its own
        // commit and **not** the HEAD the command was invoked at. A step that waits for another
        // is computed from the tree its dependency just landed, so pinning the pre-run HEAD
        // here would assert that dependencies do not work.
        let landed = git(&e.path(), &["log", "-1", "--format=%H", "--", &path]);
        let input = git(&e.path(), &["rev-parse", &format!("{landed}^")]);
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
            "{path} in {example} names a commit other than the one its own commit was built \
             on, so the receipt is provenance for a tree the step did not read"
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
        assert_eq!(v["requested"], step.as_str());

        // The step asked for is in the plan, and so is everything it waits for. A report
        // carrying only what was named would be describing a different act from the one the
        // command performed.
        let steps = v["steps"].as_array().expect("a plan");
        let planned: Vec<&str> = steps.iter().filter_map(|s| s["step"].as_str()).collect();
        assert!(planned.contains(&step.as_str()), "{out}");

        let asked = steps
            .iter()
            .find(|s| s["step"] == step.as_str())
            .expect("the step asked for");
        assert_eq!(asked["verb"], "compute");
        assert_eq!(asked["route"], "branch");
        assert!(asked["committed"]["commit"].is_string(), "{asked}");
        assert!(asked["receipt"].is_string(), "{asked}");
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

        // The refused step landed nothing. Asserted about *that step* rather than about HEAD,
        // which a plan may legitimately have advanced: an upstream this step comes `after` is
        // a separate act that succeeded, and rolling it back because a later step overreached
        // would be this command deciding to undo a commit it already wrote.
        let receipt = common::git::succeeded(
            &root,
            &["cat-file", "-e", &format!("HEAD:.yidam/runs/{step}.yml")],
        );
        assert!(
            !receipt,
            "a refused `{step}` in {example} committed a receipt, so something it wrote landed"
        );
        assert!(
            !git(&root, &["log", "--format=%s", &format!("{before}..HEAD")])
                .lines()
                .any(|l| l.contains(&format!(": {step} "))),
            "a refused `{step}` in {example} landed a commit of its own"
        );
    }
}

// ── what #472 added ───────────────────────────────────────────────────────────

/// Dependency order, asserted through git rather than through the report.
///
/// The claim is not that the report printed the steps in an order — it is that the downstream
/// step was materialized from a tree that **held its upstream's output**, which is the whole
/// of what `after` buys. So the evidence is ancestry: the downstream's commit descends from
/// the upstream's, and the file the downstream read is in the upstream's tree and not in the
/// one the plan started from.
#[test]
fn a_dependent_step_runs_against_the_commit_its_dependency_landed() {
    for name in examples() {
        let e = Example::materialize(&name);
        let caps = declared(&e.path());
        let dependents: Vec<(&String, &String)> = caps
            .iter()
            .flat_map(|(step, c)| c.after.iter().map(move |dep| (step, dep)))
            .collect();
        if dependents.is_empty() {
            continue;
        }
        let genesis = git(&e.path(), &["rev-parse", "HEAD"]);
        let (out, err, code) = e.run(&["run"]);
        assert_eq!(code, 0, "`yidam run` failed in {name}:\n{out}{err}");

        for (step, dep) in dependents {
            let subject = |s: &str| format!(": {s} ");
            let commit = |s: &str| {
                let log = git(
                    &e.path(),
                    &["log", "--format=%H %s", &format!("{genesis}..HEAD")],
                );
                log.lines()
                    .find(|l| l.contains(&subject(s)))
                    .map(|l| l.split_whitespace().next().unwrap().to_string())
                    .unwrap_or_else(|| panic!("no commit for `{s}` in {name}:\n{log}"))
            };
            let downstream = commit(step);
            let upstream = commit(dep);
            assert!(
                common::git::succeeded(
                    &e.path(),
                    &["merge-base", "--is-ancestor", &upstream, &downstream]
                ),
                "`{step}` does not descend from `{dep}` in {name}, so it was computed from a \
                 tree that did not hold what it declares it reads"
            );
        }
    }
}

/// Asking for one step brings what it waits for up to date first.
///
/// Against a corpus where nothing has run, which is the case that would otherwise fail at
/// `materialize` — a dependent's declared `reads` resolve to a file only its dependency writes.
#[test]
fn asking_for_a_dependent_step_runs_its_dependency_first() {
    for name in examples() {
        let e = Example::materialize(&name);
        let caps = declared(&e.path());
        let Some((step, deps)) = caps
            .iter()
            .find(|(_, c)| !c.after.is_empty())
            .map(|(s, c)| (s.clone(), c.after.clone()))
        else {
            continue;
        };

        let genesis = git(&e.path(), &["rev-parse", "HEAD"]);
        let (out, err, code) = e.run(&["run", &step]);
        assert_eq!(code, 0, "`yidam run {step}` failed in {name}:\n{out}{err}");

        let log = git(
            &e.path(),
            &["log", "--format=%s", &format!("{genesis}..HEAD")],
        );
        for dep in deps {
            assert!(
                log.contains(&format!(": {dep} ")),
                "`yidam run {step}` landed no commit for `{dep}`, which it declares it comes \
                 after:\n{log}"
            );
        }
    }
}

/// A fresh step is skipped and a stale one is re-run, and the report says which and why.
#[test]
fn the_report_says_which_steps_were_fresh_and_which_were_stale() {
    for name in examples() {
        let e = Example::materialize(&name);
        if declared(&e.path()).is_empty() {
            continue;
        }
        let (out, err, code) = e.run(&["run", "--format", "json"]);
        assert_eq!(code, 0, "{out}{err}");
        let first: serde_json::Value = serde_json::from_str(&out).unwrap();
        let steps = first["steps"].as_array().expect("a plan");
        assert!(!steps.is_empty(), "{out}");
        for s in steps {
            assert_eq!(s["freshness"], "stale", "nothing has run yet: {s}");
            assert_eq!(s["outcome"], "ran", "{s}");
            assert!(
                s["because"].as_str().is_some_and(|w| !w.is_empty()),
                "a verdict with no reason: {s}"
            );
        }

        // And again, against the corpus those commits left behind.
        let (out, err, code) = e.run(&["run", "--format", "json"]);
        assert_eq!(code, 0, "{out}{err}");
        let second: serde_json::Value = serde_json::from_str(&out).unwrap();
        for s in second["steps"].as_array().expect("a plan") {
            assert_eq!(
                s["freshness"], "fresh",
                "a second run against an unchanged corpus found a step stale: {s}"
            );
            assert_eq!(
                s["outcome"], "skipped",
                "a fresh step was invoked anyway: {s}"
            );
        }
        assert_eq!(second["ran"], 0, "{out}");
        assert_eq!(second["committed"], 0, "{out}");
    }
}

/// `--dry-run` plans and writes nothing — no commit, no ref, no file.
///
/// Every ref rather than just HEAD, because an epistemic step's commit would land on
/// `propose/*` and leave the branch exactly where a narrower check would look for it.
#[test]
fn a_dry_run_writes_nothing_at_all() {
    for name in examples() {
        let e = Example::materialize(&name);
        if declared(&e.path()).is_empty() {
            continue;
        }
        let root = e.path();
        let refs_before = git(&root, &["show-ref"]);
        let tree_before = tree(&root);
        let status_before = git(&root, &["status", "--porcelain"]);

        let (out, err, code) = e.run(&["run", "--dry-run"]);
        assert_eq!(code, 0, "a dry run failed in {name}:\n{out}{err}");

        assert_eq!(
            git(&root, &["show-ref"]),
            refs_before,
            "a dry run moved a ref in {name}"
        );
        assert_eq!(tree(&root), tree_before, "a dry run wrote a file in {name}");
        assert_eq!(
            git(&root, &["status", "--porcelain"]),
            status_before,
            "a dry run touched the index in {name}"
        );

        let report: serde_json::Value = {
            let (out, _, code) = e.run(&["run", "--dry-run", "--format", "json"]);
            assert_eq!(code, 0);
            serde_json::from_str(&out).unwrap()
        };
        assert_eq!(report["dry_run"], true, "{report}");
        for s in report["steps"].as_array().expect("a plan") {
            assert_ne!(s["outcome"], "ran", "a dry run invoked something: {s}");
            assert!(
                s["committed"].is_null(),
                "a dry run committed something: {s}"
            );
        }
    }
}

/// A plan holding a step this binary cannot invoke is refused before any of it runs.
///
/// The failure this is against is a partial run: a connector declared behind two calculators
/// would land two commits and then refuse, leaving the corpus advanced by a run that never had
/// a chance of finishing. So the refusal is a pre-pass, and the evidence is that HEAD did not
/// move — not merely that the exit code was nonzero.
#[test]
fn a_plan_holding_a_connector_is_refused_before_anything_runs() {
    for name in examples() {
        let e = Example::materialize(&name);
        let caps = declared(&e.path());
        // The last step in the plan, so there is something ahead of it that would otherwise
        // have run and committed before the refusal was reached.
        let Some(step) = caps
            .iter()
            .find(|(_, c)| !c.after.is_empty())
            .map(|(s, _)| s.clone())
            .or_else(|| caps.keys().next().cloned())
        else {
            continue;
        };

        let manifest = e.path().join(".yidam/capabilities.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        let head = format!("[capability.{step}]");
        let at = text.find(&head).expect("the step is declared");
        let rest = &text[at + head.len()..];
        let end = at + head.len() + rest.find("\n[").unwrap_or(rest.len());
        let table = text[at..end].replace(r#"kind   = "calculator""#, r#"kind   = "connector""#);
        assert!(
            table.contains("connector"),
            "the kind was not mutated, so this asserts nothing"
        );
        std::fs::write(&manifest, format!("{}{table}{}", &text[..at], &text[end..])).unwrap();

        let before = git(&e.path(), &["rev-parse", "HEAD"]);
        let (out, err, code) = e.run(&["run"]);
        assert_ne!(code, 0, "a connector was invoked in {name}:\n{out}");
        assert!(
            err.contains("calculators only") && err.contains(step.as_str()),
            "the refusal does not name the step or why:\n{err}"
        );
        assert_eq!(
            git(&e.path(), &["rev-parse", "HEAD"]),
            before,
            "a plan that could not finish landed a commit anyway in {name}"
        );
    }
}

/// A cycle is refused, the cycle is named, and nothing runs.
///
/// Built by mutation against the manifest the example ships rather than against a fixture
/// written to be refused: the steps in the message are the corpus's own, so a reader meeting
/// this failure meets it in the words their manifest uses.
#[test]
fn a_cycle_in_the_manifest_is_refused_with_the_cycle_named() {
    for name in examples() {
        let e = Example::materialize(&name);
        let caps = declared(&e.path());
        let Some((step, dep)) = caps
            .iter()
            .find_map(|(s, c)| c.after.first().map(|d| (s.clone(), d.clone())))
        else {
            continue;
        };

        // Close the loop: the step its dependency already waits for now waits for it back.
        let manifest = e.path().join(".yidam/capabilities.toml");
        let text = std::fs::read_to_string(&manifest).unwrap();
        let head = format!("[capability.{dep}]");
        let at = text.find(&head).expect("the dependency is declared");
        let end = at + head.len();
        std::fs::write(
            &manifest,
            format!(
                "{}{head}\nafter  = [{:?}]{}",
                &text[..at],
                step,
                &text[end..]
            ),
        )
        .unwrap();

        let before = git(&e.path(), &["rev-parse", "HEAD"]);
        let (out, err, code) = e.run(&["run"]);
        assert_ne!(code, 0, "a cycle was accepted in {name}:\n{out}");
        assert!(
            err.contains("cycle"),
            "the refusal does not say it is a cycle:\n{err}"
        );
        for named in [&step, &dep] {
            assert!(
                err.contains(named.as_str()),
                "the refusal does not name `{named}`:\n{err}"
            );
        }
        assert_eq!(
            git(&e.path(), &["rev-parse", "HEAD"]),
            before,
            "a manifest that cannot be ordered ran something anyway in {name}"
        );
    }
}

/// Re-declare **one** capability's verb, leaving everything else the example ships.
///
/// Scoped to the named step, which a plain `replace` over the file is not: with two
/// capabilities declared, re-declaring every verb at once would also move an upstream into the
/// epistemic family, and the manifest refuses a step that comes `after` one of those. The test
/// would then be asserting the dependency rule while claiming to assert the route.
///
/// The mutation is asserted rather than assumed — an edit that matched nothing would hand every
/// test below an unmutated manifest and a green run about the wrong thing, which is a failure
/// this suite has seen in other shapes.
fn with_verb(e: &Example, step: &str, verb: &str) {
    let manifest = e.path().join(".yidam/capabilities.toml");
    let before = std::fs::read_to_string(&manifest).unwrap();

    // The capability's own table, and nothing past it. TOML closes a table at the next header,
    // so the slice from this step's header to the following one is exactly its declaration.
    let head = format!("[capability.{step}]");
    let at = before
        .find(&head)
        .unwrap_or_else(|| panic!("{} declares no `{step}`", manifest.display()));
    let rest = &before[at + head.len()..];
    let end = at + head.len() + rest.find("\n[").unwrap_or(rest.len());

    let table = &before[at..end];
    let mutated = table.replace("verb   = \"compute\"", &format!("verb   = \"{verb}\""));
    assert_ne!(
        table,
        mutated,
        "`{step}` in {} declares no `compute` verb to re-declare, so the test asserts nothing",
        manifest.display()
    );
    std::fs::write(
        &manifest,
        format!("{}{mutated}{}", &before[..at], &before[end..]),
    )
    .unwrap();
}

/// Run everything the manifest declares, so the next run has only the epistemic step to do.
///
/// Called **after** the verb is re-declared and not before, because the edit is what makes
/// settling necessary: the manifest's digest is in every capability's input state, so changing
/// one line makes every step in the corpus stale. Without this, the run being measured would
/// bring an operational dependency up to date and advance the branch — and the assertion that
/// the branch did not move would fail for a reason that is nothing to do with the route.
///
/// The epistemic step runs here too, and lands on the proposal branch for *this* HEAD. The
/// measured run stands at a later HEAD, finds no receipt for it on the branch, and does it
/// again — which is the act being measured.
fn settle(e: &Example) {
    let (out, err, code) = e.run(&["run"]);
    assert_eq!(code, 0, "settling the corpus failed:\n{out}{err}");
}

/// Delete every proposal branch, so an epistemic step has its act to perform again.
///
/// [`settle`] runs the epistemic step too, and its result is then committed on `propose/<head>`
/// — which is a landed result like any other, so the next run reads the receipt there and
/// reports the step **fresh**. That is correct behaviour and it leaves nothing for the measured
/// run to do.
///
/// Deleting the branch is the act a person performs to reject a proposal, and it is what makes
/// the step owed again. Discovered from the refs rather than composed from a head sha: the
/// branch name is `propose`'s to decide, and a test that rebuilt it here would be a second
/// implementation of that rule.
fn discard_proposals(e: &Example) {
    let refs = git(
        &e.path(),
        &["for-each-ref", "--format=%(refname)", "refs/heads/propose/"],
    );
    let mut found = 0;
    for name in refs.lines().filter(|l| !l.trim().is_empty()) {
        git(&e.path(), &["update-ref", "-d", name]);
        found += 1;
    }
    assert!(
        found > 0,
        "settling landed no proposal, so the epistemic step never ran and the measured run \
         below asserts nothing"
    );
}

/// The whole safety argument, asserted where it is enforced — **and it is now a destination
/// rather than a refusal**.
///
/// This replaces `a_capability_declaring_an_epistemic_verb_does_not_load`, which pinned the
/// holding action: for as long as nothing could route an epistemic commit anywhere, refusing
/// the verb at load was the only way to hold RFC-0026 §2's second sentence. It also made the
/// epistemic half of the vocabulary undeclarable.
///
/// What must not weaken is the property, not the refusal: **no path, and no config value, by
/// which a run advances the current branch with an epistemic commit.** So this asserts the run
/// succeeds, the commit exists, it is epistemic, it is on `propose/*` — and the branch the run
/// was invoked from is byte-for-byte where it was.
#[test]
fn an_epistemic_run_lands_on_a_proposal_branch_and_the_branch_does_not_move() {
    for (example, step) in every_terminal_capability() {
        let e = Example::materialize(&example);
        with_verb(&e, &step, "establish");
        settle(&e);
        discard_proposals(&e);

        let branch = git(&e.path(), &["rev-parse", "--abbrev-ref", "HEAD"]);
        let before = git(&e.path(), &["rev-parse", "HEAD"]);
        let short = git(&e.path(), &["rev-parse", "--short", "HEAD"]);

        let (out, err, code) = e.run(&["run", &step, "--format", "json"]);
        assert_eq!(code, 0, "an epistemic run failed in {example}:\n{out}{err}");

        // The branch is exactly where it was. This is the invariant; everything else below is
        // evidence that something nonetheless happened.
        assert_eq!(
            before,
            git(&e.path(), &["rev-parse", "HEAD"]),
            "an epistemic run advanced {branch} in {example}"
        );

        let report: serde_json::Value = serde_json::from_str(&out).unwrap();
        let entry = report["steps"]
            .as_array()
            .expect("a plan")
            .iter()
            .find(|s| s["step"] == step.as_str())
            .unwrap_or_else(|| panic!("`{step}` is not in the plan in {example}:\n{out}"));
        assert_eq!(entry["route"], "proposal", "{out}");
        let landed = entry["committed"]["branch"]
            .as_str()
            .unwrap_or_else(|| panic!("nothing was committed in {example}:\n{out}"));
        assert_eq!(landed, format!("propose/{short}"), "{out}");

        // Every other step in the plan was fresh, so the branch had nothing to do. Asserted so
        // that this test fails loudly rather than vacuously if `settle` ever stops settling.
        for other in report["steps"].as_array().expect("a plan") {
            if other["step"] != step.as_str() {
                assert_eq!(
                    other["outcome"], "skipped",
                    "`{}` ran during the measured run, so the branch was free to move for a \
                     reason this test is not about: {out}",
                    other["step"]
                );
            }
        }

        // The commit is there, it is a run's, and it is epistemic — decided by the classifier
        // the CLI ships rather than by re-reading the verb here.
        let subject = git(&e.path(), &["log", "-1", "--format=%s", landed]);
        assert!(subject.starts_with("establish: "), "{subject}");
        assert_eq!(
            git(&e.path(), &["log", "-1", "--format=%ae", landed]),
            "run@yidam"
        );
        assert_eq!(
            yidam_core::git::classify_commit("", &subject).kind,
            yidam_core::git::CommitKind::Epistemic,
            "{subject}"
        );

        // And nothing merged itself: the proposal is not reachable from the branch.
        let tip = git(&e.path(), &["rev-parse", landed]);
        let merged =
            common::git::succeeded(&e.path(), &["merge-base", "--is-ancestor", &tip, &before]);
        assert!(
            !merged,
            "{landed} is an ancestor of {branch} in {example} — something merged itself"
        );
    }
}

/// The same property over the **whole** epistemic family, not over one verb.
///
/// `establish` is the verb the argument is usually made about, and a test that only ever ran
/// that one would stop covering a family member the day the vocabulary gained one — the
/// guard-list shape this suite's own header warns about. Every epistemic verb is tried, and
/// the branch must be untouched by all of them.
///
/// One example rather than every example: the claim is about the verb partition, and the
/// per-example behaviour is what the test above establishes.
#[test]
fn no_epistemic_verb_in_the_vocabulary_can_advance_the_branch() {
    let Some((example, step)) = every_terminal_capability().into_iter().next() else {
        panic!("no example declares a terminal capability, so this asserts nothing");
    };
    assert!(
        !yidam_core::git::EPISTEMIC_VERBS.is_empty(),
        "the epistemic family is empty, so the loop below runs zero times"
    );
    for verb in yidam_core::git::EPISTEMIC_VERBS {
        let e = Example::materialize(&example);
        with_verb(&e, &step, verb);
        settle(&e);
        discard_proposals(&e);
        let before = git(&e.path(), &["rev-parse", "HEAD"]);

        let (out, err, code) = e.run(&["run", &step]);
        assert_eq!(code, 0, "`{verb}` was refused in {example}:\n{out}{err}");
        assert_eq!(
            before,
            git(&e.path(), &["rev-parse", "HEAD"]),
            "`{verb}` advanced the branch in {example}"
        );
    }
}

/// A manifest cannot state where its output goes, end to end.
///
/// The unit test pins that the field does not deserialize. This pins what that buys: the three
/// shapes somebody would reach for to license the old behaviour all stop the run rather than
/// being read as effective, which is the difference between a refused permission and an
/// ignored one.
#[test]
fn a_manifest_that_tries_to_declare_a_destination_does_not_run() {
    for (example, step) in every_terminal_capability() {
        for line in [
            "route  = \"branch\"",
            "allow_direct = true",
            "epistemic = false",
        ] {
            let e = Example::materialize(&example);
            with_verb(&e, &step, "establish");
            let manifest = e.path().join(".yidam/capabilities.toml");
            let text = std::fs::read_to_string(&manifest).unwrap();
            std::fs::write(&manifest, format!("{text}{line}\n")).unwrap();

            let (out, err, code) = e.run(&["run", &step]);
            assert_ne!(
                code, 0,
                "`{line}` was accepted in {example}, so a corpus can write a destination the \
                 executor does not read:\n{out}{err}"
            );
        }
    }
}
