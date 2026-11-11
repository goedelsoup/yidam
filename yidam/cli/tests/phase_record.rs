//! `yidam phase` — the acts `prelude/PHASES.md` has specified since before anything held one.
//!
//! A phase was a convention about ref names. `cmd/phases.rs` derived `state` from a ref's
//! namespace, so *active* meant *a ref matching a glob*: a phase whose run died halfway
//! through and a phase somebody opened this morning were the same row, and neither the
//! repository nor a reader could tell them apart.
//!
//! Everything here runs the real binary against a real repository and reads the commits it
//! wrote. A record format with no writer, or a resumability claim asserted against a mock,
//! would pass while asserting nothing — the shape this repository keeps finding.
//!
//! # The fixture counts invocations
//!
//! [`corpus`] builds a two-step manifest whose steps append a line to a file outside the
//! repository before doing anything else. So *"a step already recorded is not re-invoked"* is
//! checked by counting how many times the process actually started, rather than by reading
//! the report that claims it did not — which is the assertion a resumable executor owes and
//! the one a report cannot make on its own behalf.

use std::path::PathBuf;
use std::process::Command;

mod common;

use common::git::out as git;

/// A corpus, the probe file its steps append to, and the temp dir holding both.
struct Fixture {
    dir: tempfile::TempDir,
}

impl Fixture {
    fn root(&self) -> PathBuf {
        self.dir.path().join("repo")
    }

    fn probe(&self) -> PathBuf {
        self.dir.path().join("invocations")
    }

    /// How many times each step has been invoked, in order.
    fn invocations(&self) -> Vec<String> {
        std::fs::read_to_string(self.probe())
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect()
    }

    fn run(&self, args: &[&str]) -> (String, String, bool) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.root())
            .args(args)
            .env("PROBE", self.probe())
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .env("GIT_CONFIG_SYSTEM", "/dev/null")
            .output()
            .unwrap_or_else(|e| panic!("yidam {args:?}: {e}"));
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.success(),
        )
    }

    /// The same, asserting it succeeded and returning stdout.
    fn ok(&self, args: &[&str]) -> String {
        let (stdout, stderr, ok) = self.run(args);
        assert!(ok, "yidam {args:?} failed:\n{stderr}\n{stdout}");
        stdout
    }

    fn json(&self, args: &[&str]) -> serde_json::Value {
        let mut with_format = args.to_vec();
        with_format.extend(["--format", "json"]);
        serde_json::from_str(&self.ok(&with_format)).expect("the report is JSON")
    }

    /// The record as the *ref* carries it.
    ///
    /// Never off the working tree. Every commit `yidam phase` lands moves the ref and leaves
    /// the checkout where it was, so the file on disk is the record from before the last act
    /// — reading it here would make this suite agree with a bug rather than catch one.
    fn record(&self, slug: &str) -> serde_json::Value {
        let text = git(
            &self.root(),
            &["show", &format!("phase/{slug}:.yidam/phases/{slug}.yml")],
        );
        serde_yaml::from_str(&text).expect("the record is YAML")
    }

    /// Bring the checkout up to the commits the run landed.
    ///
    /// `cmd/run` states this rather than leaving it to be discovered: a run advances the
    /// branch and never touches the index, so afterwards your checkout is behind HEAD by as
    /// many commits as the plan landed. Not a defect — RFC-0026 §5 forbids the fix that
    /// would fail halfway on a dirty tree — but a test that switched branches without doing
    /// it would be testing git's refusal.
    fn resync(&self) {
        git(&self.root(), &["reset", "-q", "--hard", "HEAD"]);
    }

    /// Make `second` fail, or stop it failing. The mechanism an interruption is simulated by:
    /// a step that exits nonzero stops the run exactly where a kill would, and unlike a kill
    /// it is deterministic on every machine this suite runs on.
    fn second_fails(&self, fail: bool) {
        let path = self.root().join(".yidam/capabilities/second.sh");
        let body = if fail { "exit 3\n" } else { "" };
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\nset -eu\nprintf 'second\\n' >> \"$PROBE\"\n{body}\
                 mkdir -p \"$YIDAM_OUT/.yidam/computed\"\n\
                 printf 'second: done\\n' > \"$YIDAM_OUT/.yidam/computed/second.yml\"\n"
            ),
        )
        .unwrap();
        git(&self.root(), &["add", "-A"]);
        git(
            &self.root(),
            &[
                "commit",
                "-q",
                "-m",
                if fail {
                    "fix: second exits nonzero"
                } else {
                    "fix: second completes"
                },
            ],
        );
    }
}

impl Fixture {
    /// Declare a connector and a calculator that runs after it, beside the two steps that run.
    ///
    /// The shape #1463 was filed from: a derived repository whose manifest still declares a
    /// bootstrap connector. This binary refuses `kind = "connector"` by design, so there is no
    /// fix to make to the step — only a plan that has to be resolved around it. Each step
    /// would append to the probe if it ever started, which is how "never invoked" is counted.
    fn declare_connector(&self, only: bool) {
        let manifest = self.root().join(".yidam/capabilities.toml");
        let mut text = if only {
            String::new()
        } else {
            std::fs::read_to_string(&manifest).unwrap()
        };
        text.push_str(
            r#"
[capability.fetch]
kind   = "connector"
run    = ["sh", "-c", "printf 'fetch\n' >> \"$PROBE\""]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "compute"

[capability.after-fetch]
kind   = "calculator"
run    = ["sh", "-c", "printf 'after-fetch\n' >> \"$PROBE\""]
reads  = [".yidam/corpus/**"]
writes = [".yidam/computed/**"]
verb   = "compute"
after  = ["fetch"]
"#,
        );
        std::fs::write(&manifest, text).unwrap();
        git(&self.root(), &["add", "-A"]);
        git(
            &self.root(),
            &["commit", "-q", "-m", "fix: declare the bootstrap connector"],
        );
    }
}

/// A repository with a corpus and a two-step manifest, the second waiting on the first.
///
/// Built by hand rather than from `examples/`: the shipped examples' capabilities are real
/// calculators over a real corpus, and what this suite needs is a step it can make fail on
/// demand and count the invocations of. Mutating a shipped example to get that would make
/// every assertion here about whatever that example happens to declare tomorrow.
fn corpus() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    let caps = root.join(".yidam/capabilities");
    std::fs::create_dir_all(&caps).unwrap();
    std::fs::create_dir_all(root.join(".yidam/corpus")).unwrap();

    std::fs::write(
        root.join(".yidam/corpus/seed.yml"),
        "class: concept\nname: seed\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/capabilities.toml"),
        r#"
[capability.first]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/first.sh"]
reads  = [".yidam/corpus/**", ".yidam/capabilities/first.sh"]
writes = [".yidam/computed/**"]
verb   = "compute"

[capability.second]
kind   = "calculator"
run    = ["sh", ".yidam/capabilities/second.sh"]
reads  = [".yidam/computed/first.yml", ".yidam/capabilities/second.sh"]
writes = [".yidam/computed/**"]
verb   = "compute"
after  = ["first"]
"#,
    )
    .unwrap();
    std::fs::write(
        caps.join("first.sh"),
        "#!/bin/sh\nset -eu\nprintf 'first\\n' >> \"$PROBE\"\n\
         mkdir -p \"$YIDAM_OUT/.yidam/computed\"\n\
         printf 'first: done\\n' > \"$YIDAM_OUT/.yidam/computed/first.yml\"\n",
    )
    .unwrap();
    std::fs::write(
        caps.join("second.sh"),
        "#!/bin/sh\nset -eu\nprintf 'second\\n' >> \"$PROBE\"\n\
         mkdir -p \"$YIDAM_OUT/.yidam/computed\"\n\
         printf 'second: done\\n' > \"$YIDAM_OUT/.yidam/computed/second.yml\"\n",
    )
    .unwrap();

    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.email", "t@yidam.test"],
        vec!["config", "user.name", "Tester"],
        vec!["config", "commit.gpgsign", "false"],
    ] {
        git(&root, &args);
    }
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-q", "-m", "genesis: the fixture corpus"],
    );

    Fixture { dir }
}

// ── the snapshot ──────────────────────────────────────────────────────────────

/// `start` pins RFC-0026 §1's input state and lands it where a *ref* carries it.
#[test]
fn start_snapshots_the_input_state_onto_the_phase_branch() {
    let f = corpus();
    let baseline = git(&f.root(), &["rev-parse", "HEAD"]);

    let report = f.json(&["phase", "start", "outcome-axis", "--type", "Investigation"]);
    assert_eq!(report["phase"], "outcome-axis");
    assert_eq!(report["branch"], "phase/outcome-axis");
    assert_eq!(report["type"], "Investigation");
    assert_eq!(report["input"]["commit"], baseline);

    // The record is on the branch, readable from a ref rather than from a checkout — which is
    // what lets `yidam phases` answer for a phase held only as `origin/phase/<slug>`.
    let on_ref = git(
        &f.root(),
        &["show", "phase/outcome-axis:.yidam/phases/outcome-axis.yml"],
    );
    let rec: serde_json::Value = serde_yaml::from_str(&on_ref).unwrap();
    assert_eq!(rec["format_version"], 1);
    assert_eq!(rec["type"], "Investigation");
    assert_eq!(rec["input"]["commit"], baseline);
    assert!(
        rec["input"]["manifest_sha256"].as_str().unwrap().len() == 64,
        "the manifest digest is a sha256: {rec:?}"
    );

    // Operational, so RFC-0026 §2's invariant is untouched: the one verb this layer might
    // have wanted is `phase:`, which is epistemic and which only a person writes.
    let subject = git(
        &f.root(),
        &["log", "-1", "--format=%s", "phase/outcome-axis"],
    );
    assert!(subject.starts_with("scaffold: "), "{subject}");
    assert_eq!(
        git(
            &f.root(),
            &["log", "-1", "--format=%an", "phase/outcome-axis"]
        ),
        "yidam phase"
    );
}

/// The checkout does not move, which is RFC-0026 §5's property and the reason `propose` went
/// onto a temporary index in the first place.
#[test]
fn start_moves_neither_the_checkout_nor_the_working_tree() {
    let f = corpus();
    let before = git(&f.root(), &["rev-parse", "HEAD"]);
    std::fs::write(f.root().join("scratch.txt"), "somebody's work").unwrap();

    f.ok(&["phase", "start", "outcome-axis", "--type", "Investigation"]);

    assert_eq!(git(&f.root(), &["rev-parse", "HEAD"]), before);
    assert_eq!(
        git(&f.root(), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "main"
    );
    assert_eq!(
        std::fs::read_to_string(f.root().join("scratch.txt")).unwrap(),
        "somebody's work",
        "a command that updated the checkout would leave somebody's work where they did not \
         put it"
    );
    assert!(!f.root().join(".yidam/phases").exists());
}

/// A phase is opened once. The second `start` is a refusal, not a second branch.
#[test]
fn start_refuses_a_phase_that_is_already_open() {
    let f = corpus();
    f.ok(&["phase", "start", "outcome-axis", "--type", "Investigation"]);
    let (_, stderr, ok) = f.run(&["phase", "start", "outcome-axis", "--type", "Investigation"]);
    assert!(!ok);
    assert!(stderr.contains("already exists"), "{stderr}");
}

/// The name is refused rather than kebab-ified — the slug is a ref, a file stem and the
/// record's identity at once, and a command that renamed what it was given would make those
/// three agree with each other and with nothing the person typed.
#[test]
fn start_refuses_a_name_that_is_not_the_ref_it_would_become() {
    let f = corpus();
    let (_, stderr, ok) = f.run(&["phase", "start", "Outcome Axis", "--type", "Investigation"]);
    assert!(!ok);
    assert!(
        stderr.contains("outcome-axis"),
        "names the repair: {stderr}"
    );
    assert!(!f.root().join(".git/refs/heads/phase").exists());
}

// ── the type, and the kuten it is validated against ───────────────────────────

/// RFC-0028 §3 puts the enforcing consumer here: *"#473's `phase start`, validating a declared
/// type against the vendored list."*
#[test]
fn a_declared_type_is_checked_against_the_vendored_kuten() {
    let f = corpus();
    let root = f.root();
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::create_dir_all(root.join(".yidam/.vendor/prelude/kuten/inquiry")).unwrap();
    std::fs::write(
        root.join(".yidam/decisions/kuten.yml"),
        "kuten: inquiry\nrevision: 2\n",
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/.vendor/prelude/kuten/inquiry/kuten.yml"),
        "kuten: inquiry\nrevision: 2\nphases:\n  types: [Investigation, Synthesis]\n  \
         commit_share: {low: 0.12, high: 0.27}\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-q", "-m", "decide: adopt the inquiry kuten"],
    );

    let (_, stderr, ok) = f.run(&["phase", "start", "outcome-axis", "--type", "Extraction"]);
    assert!(!ok, "a type the vendored list omits is refused");
    assert!(stderr.contains("Investigation, Synthesis"), "{stderr}");

    f.ok(&["phase", "start", "outcome-axis", "--type", "Synthesis"]);
    // The revision is recorded beside the sha although the sha already pins it: comparing two
    // integers is RFC-0026's equality check, where re-reading the profile out of a commit is
    // the heuristic in a costume.
    assert_eq!(f.record("outcome-axis")["input"]["kuten"], "inquiry");
    assert_eq!(f.record("outcome-axis")["input"]["kuten_revision"], 2);
}

/// Every repository today holds no kuten — 0 of 18 derived corpora at A0 — and a layer that
/// refused to open a phase without one would be unusable in all of them. RFC-0028 Erratum 1
/// is why there is no default list to fall back to: implementing one is the only way to
/// create the four-element phase-type list in the binary that the section objects to.
#[test]
fn a_repository_holding_no_kuten_records_the_type_and_validates_it_against_nothing() {
    let f = corpus();
    f.ok(&["phase", "start", "outcome-axis", "--type", "Excavation"]);
    let rec = f.record("outcome-axis");
    assert_eq!(rec["type"], "Excavation");
    assert!(rec["input"]["kuten"].is_null(), "{rec:?}");
}

// ── the run record, and resumption ────────────────────────────────────────────

fn open_and_switch(f: &Fixture) {
    f.ok(&["phase", "start", "outcome-axis", "--type", "Investigation"]);
    git(&f.root(), &["switch", "-q", "phase/outcome-axis"]);
}

/// The plan is recorded before the first step runs, and each step as it completes.
#[test]
fn a_run_records_its_plan_and_then_its_steps() {
    let f = corpus();
    open_and_switch(&f);

    let report = f.json(&["phase", "run"]);
    assert_eq!(
        report["plan"].as_array().unwrap().len(),
        2,
        "the plan orders every dependency before what declares it: {report:?}"
    );
    assert_eq!(report["plan"][0], "first");
    assert_eq!(report["plan"][1], "second");
    assert!(report["remaining"].as_array().unwrap().is_empty());
    assert_eq!(f.invocations(), ["first", "second"]);

    let rec = f.record("outcome-axis");
    let completed: Vec<&str> = rec["completed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["step"].as_str().unwrap())
        .collect();
    assert_eq!(completed, ["first", "second"]);
}

/// **The resumability claim, counted rather than reported.**
///
/// #473's definition of done: *"`phase run` is resumable: killing it mid-run and re-running
/// completes rather than restarting, with a test."* A step that exits nonzero stops the run
/// exactly where a kill would and is deterministic on every machine, and the probe file says
/// how many times each step's process actually started.
#[test]
fn an_interrupted_run_resumes_at_the_step_it_stopped_on() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);

    let (_, stderr, ok) = f.run(&["phase", "run"]);
    assert!(!ok, "the run stops where the step failed:\n{stderr}");
    assert_eq!(f.invocations(), ["first", "second"]);

    // The first step's completion is committed before the second is invoked, so the record
    // survives the failure. A record written at the end of a run would have nothing here.
    let rec = f.record("outcome-axis");
    assert_eq!(rec["completed"].as_array().unwrap().len(), 1);
    assert_eq!(rec["completed"][0]["step"], "first");
    assert_eq!(rec["plan"][1], "second");

    // The checkout is behind HEAD by the commits the run landed, so the fixture syncs before
    // it commits again — a `git add -A` over a stale tree would stage the record from before
    // the run and silently undo the very thing this test is about.
    f.resync();
    f.second_fails(false);
    let report = f.json(&["phase", "run"]);
    assert_eq!(report["resumed"], 1);
    let recorded: Vec<&str> = report["completed"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["step"].as_str().unwrap())
        .collect();
    assert_eq!(
        recorded,
        ["second"],
        "only the outstanding step is recorded"
    );

    assert_eq!(
        f.invocations(),
        ["first", "second", "second"],
        "`first` was invoked once across both runs — a restart would show it twice"
    );
    assert!(report["remaining"].as_array().unwrap().is_empty());
}

/// #1463: a manifest declaring a step this binary refuses still runs a phase, and the phase
/// can finish.
///
/// Before, the plan was committed with the connector in it and the run stopped on reaching
/// it, so the phase read `interrupted` and nothing could move it on. Now the refused step and
/// its dependent are named in the report, left out of the record's plan, and never started.
#[test]
fn a_refused_step_is_reported_and_left_out_of_the_plan_it_records() {
    let f = corpus();
    f.declare_connector(false);
    open_and_switch(&f);

    let report = f.json(&["phase", "run"]);
    assert_eq!(report["plan"], serde_json::json!(["first", "second"]));
    let refused: Vec<&str> = report["refused"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["step"].as_str().unwrap())
        .collect();
    assert_eq!(refused, ["fetch", "after-fetch"], "{report}");
    assert!(
        report["refused"][0]["because"]
            .as_str()
            .unwrap()
            .contains(r#"`kind = "connector"`"#),
        "{report}"
    );
    assert_eq!(
        f.invocations(),
        ["first", "second"],
        "a refused step, and the step after it, were started"
    );

    let rec = f.record("outcome-axis");
    assert_eq!(rec["plan"], serde_json::json!(["first", "second"]));
    let rows = f.json(&["phases"]);
    assert_eq!(rows["phases"][0]["state"], "active", "{rows}");
}

/// A plan in which nothing can run is refused before the plan commit, so it lands nothing.
#[test]
fn a_plan_nothing_can_run_lands_no_commit() {
    let f = corpus();
    f.declare_connector(true);
    open_and_switch(&f);

    let before = git(&f.root(), &["rev-parse", "HEAD"]);
    let (_, stderr, ok) = f.run(&["phase", "run"]);
    assert!(!ok, "a plan nothing can run succeeded");
    assert!(stderr.contains("nothing was written"), "{stderr}");
    assert!(
        stderr.contains("fetch"),
        "the refusal names the step: {stderr}"
    );
    assert_eq!(git(&f.root(), &["rev-parse", "HEAD"]), before);
    assert!(f.invocations().is_empty());
}

/// A run that stops partway says its commits are not in the checkout, and how to take them.
///
/// The ref moves and the index does not, so after the failure the index reads as a staged
/// revert of the plan commit — the thing #1463 found easy to commit by accident.
#[test]
fn a_run_that_stops_says_how_to_take_what_it_committed() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);

    let (_, stderr, ok) = f.run(&["phase", "run"]);
    assert!(!ok);
    let line = stderr
        .lines()
        .find(|l| l.contains("git restore --source=HEAD --worktree --staged --"))
        .unwrap_or_else(|| panic!("no restore line:\n{stderr}"));
    for path in [
        ".yidam/phases/outcome-axis.yml",
        ".yidam/computed/first.yml",
    ] {
        assert!(line.contains(path), "`{path}` is not named: {line}");
    }

    // The line it prints is the repair: run it and the checkout is HEAD again.
    let args: Vec<&str> = line.split_whitespace().skip(1).collect();
    git(&f.root(), &args);
    assert_eq!(git(&f.root(), &["status", "--porcelain"]), "");
}

/// The legibility half: an interrupted phase says so where a reader looks.
#[test]
fn an_interrupted_phase_reads_interrupted_and_a_finished_one_reads_active() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);
    let (_, _, ok) = f.run(&["phase", "run"]);
    assert!(!ok);

    let rows = f.json(&["phases"]);
    let row = &rows["phases"][0];
    assert_eq!(row["ref_name"], "phase/outcome-axis");
    assert_eq!(
        row["state"], "interrupted",
        "ref shape cannot see this — the record is the only evidence there is"
    );
    assert_eq!(row["source"], "record");
    assert_eq!(row["type"], "Investigation");

    f.resync();
    f.second_fails(false);
    f.ok(&["phase", "run"]);
    let rows = f.json(&["phases"]);
    assert_eq!(rows["phases"][0]["state"], "active");
}

/// RFC-0028 §3's ranking, in the case that decides it: the record is authoritative only where
/// ref shape says `active`. A record is committed on the phase's own branch, strictly before
/// the merge that settles it, so it could claim settlement only by predicting one.
#[test]
fn a_merged_phase_reads_settled_however_its_record_stands() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);
    let (_, _, ok) = f.run(&["phase", "run"]);
    assert!(!ok, "the record is left mid-plan on purpose");

    f.resync();
    git(&f.root(), &["switch", "-q", "main"]);
    git(
        &f.root(),
        &[
            "merge",
            "--no-ff",
            "-q",
            "-m",
            "phase: outcome-axis — what it produced",
            "phase/outcome-axis",
        ],
    );

    let row = &f.json(&["phases"])["phases"][0];
    assert_eq!(row["state"], "settled");
    assert_eq!(
        row["source"], "ref",
        "settlement is a fact about the baseline, which is the question the ref answers"
    );
}

/// The arm every phase in every existing repository is in, and which RFC-0028 §3 keeps
/// forever: *"refs without run records exist forever."*
#[test]
fn a_phase_opened_by_hand_is_still_listed_and_says_its_state_is_inferred() {
    let f = corpus();
    git(&f.root(), &["switch", "-q", "-c", "phase/by-hand"]);
    std::fs::write(f.root().join(".yidam/corpus/note.yml"), "class: concept\n").unwrap();
    git(&f.root(), &["add", "-A"]);
    git(&f.root(), &["commit", "-q", "-m", "establish: a note"]);
    git(&f.root(), &["switch", "-q", "main"]);

    let report = f.json(&["phases"]);
    let row = &report["phases"][0];
    assert_eq!(row["state"], "active");
    assert_eq!(row["source"], "ref");
    assert!(row.get("type").is_none(), "nothing invents a type: {row:?}");

    assert!(
        f.ok(&["phases"]).contains("inferred"),
        "the table says how many rows are an inference and what writes the evidence instead"
    );
}

/// A branch with no record cannot be run: nothing snapshotted what it began from.
#[test]
fn run_refuses_a_phase_branch_carrying_no_record() {
    let f = corpus();
    git(&f.root(), &["switch", "-q", "-c", "phase/by-hand"]);
    let (_, stderr, ok) = f.run(&["phase", "run"]);
    assert!(!ok);
    assert!(stderr.contains("phase start"), "{stderr}");
}

/// RFC-0028 §2 applied to a phase in flight across a re-vendor: annotate, never silently
/// proceed, and never re-validate the type against a list that changed under it.
#[test]
fn a_kuten_that_moves_under_a_phase_is_reported_and_the_phase_still_runs() {
    let f = corpus();
    let root = f.root();
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::write(
        root.join(".yidam/decisions/kuten.yml"),
        "kuten: inquiry\nrevision: 2\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-q", "-m", "decide: adopt the inquiry kuten"],
    );
    open_and_switch(&f);

    std::fs::write(
        root.join(".yidam/decisions/kuten.yml"),
        "kuten: inquiry\nrevision: 3\n",
    )
    .unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &[
            "commit",
            "-q",
            "-m",
            "vendor: the prelude at a later revision",
        ],
    );

    let report = f.json(&["phase", "run"]);
    assert_eq!(report["revision_skew"]["recorded"], 2);
    assert_eq!(report["revision_skew"]["held"], 3);
    assert!(
        report["remaining"].as_array().unwrap().is_empty(),
        "annotated, not refused — refusing would strand the work rather than the mismatch"
    );
}

// ── settle ────────────────────────────────────────────────────────────────────

/// **The invariant, on this layer's second surface.** `phase:` is epistemic, RFC-0026 §2
/// forbids a run authoring one, and `cmd/due.rs` reached the same limit first: *"A person.
/// Merging a phase, or abandoning it, is not a mechanical consequence of a finding."*
#[test]
fn settle_authors_no_commit_and_moves_no_ref() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);

    let refs_before = git(
        &f.root(),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    let stdout = f.ok(&["phase", "settle"]);
    let refs_after = git(
        &f.root(),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    assert_eq!(refs_before, refs_after, "settle moved a ref");

    // Not one `phase:` commit anywhere in the repository — the assertion #473's definition of
    // done asks for, made over the whole object graph rather than over the branch.
    let subjects = git(&f.root(), &["log", "--all", "--format=%s"]);
    assert!(
        !subjects.lines().any(|s| s.starts_with("phase:")),
        "a run wrote a `phase:` commit:\n{subjects}"
    );

    assert!(stdout.contains("git merge --no-ff"), "{stdout}");
    assert!(stdout.contains("a person writes"), "{stdout}");
}

/// The draft is checked against the closed vocabulary, because a subject this tool hands a
/// person to paste is one it is answerable for.
#[test]
fn settle_drafts_a_subject_in_the_commit_vocabulary() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);

    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["ready"], true);
    let subject = report["subject"].as_str().unwrap();
    assert!(subject.starts_with("phase: outcome-axis — "), "{subject}");
    assert!(report["commits"].as_u64().unwrap() > 0);
    assert!(!report["outputs"].as_array().unwrap().is_empty());
}

/// `PHASES.md`: *a phase that never produces commits is an open inquiry thread, not a settled
/// phase.* And a phase whose plan never completed is one whose outputs are not all there.
#[test]
fn settle_reports_a_phase_that_produced_nothing_as_not_ready() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);
    let (_, _, ok) = f.run(&["phase", "run"]);
    assert!(!ok);

    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["ready"], false);
    assert_eq!(report["remaining"][0], "second");
    assert!(
        f.ok(&["phase", "settle"]).contains("open inquiry thread"),
        "the refusal names PHASES.md's own remedy"
    );
}

// ── settle and the upstream question (#1469) ──────────────────────────────────

/// Commit `files` on the open phase branch under `subject`.
fn commit_on_phase(f: &Fixture, files: &[&str], subject: &str) {
    for p in files {
        let path = f.root().join(p);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "x\n").unwrap();
        git(&f.root(), &["add", p]);
    }
    git(&f.root(), &["commit", "-q", "-m", subject]);
}

/// **#1469.** One corpus carried four workarounds across ~4,500 commits and filed none,
/// because the norm was prose read at session start and nothing asked when a workaround was
/// written. A phase that adds files outside every laid-out directory is asked at settle, with
/// a drafted `from:derived-repo` body — and still files nothing, moves no ref and gates nothing.
#[test]
fn settle_asks_about_files_added_outside_the_laid_out_directories() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    commit_on_phase(
        &f,
        &[
            "mirror/claim.ont.yml",
            "mirror/source.ont.yml",
            "corpora.toml",
        ],
        "scaffold: mirror the sibling's ontology",
    );

    let refs_before = git(
        &f.root(),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    let report = f.json(&["phase", "settle"]);
    let up = &report["upstream"];
    assert_eq!(
        report["ready"], true,
        "the question is not a gate: {report}"
    );
    assert_eq!(up["repository"], "goedelsoup/yidam", "{up}");
    assert_eq!(
        up["unlaid"],
        serde_json::json!([
            "corpora.toml",
            "mirror/claim.ont.yml",
            "mirror/source.ont.yml"
        ]),
        "{up}"
    );
    let draft = up["draft"].as_str().expect("a draft when nothing is cited");
    assert!(draft.contains("- `mirror/` (2 file(s))"), "{draft}");

    let text = f.ok(&["phase", "settle"]);
    assert!(text.contains("nothing was filed"), "{text}");
    assert!(
        text.contains("gh issue create --repo goedelsoup/yidam --label \"from:derived-repo\""),
        "{text}"
    );
    let refs_after = git(
        &f.root(),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    );
    assert_eq!(refs_before, refs_after, "settle moved a ref");
}

/// A phase whose commits already cite the template's tracker has reported, and is asked
/// nothing. The tracker is `.yidam.toml`'s `origin`, so a derivation of a fork cites the fork.
#[test]
fn settle_asks_nothing_of_a_phase_that_cites_the_templates_tracker() {
    let f = corpus();
    std::fs::write(
        f.root().join(".yidam.toml"),
        "[yidam]\norigin = \"git@github.com:someone/fork.git\"\ncommit = \"abc123\"\n",
    )
    .unwrap();
    git(&f.root(), &["add", ".yidam.toml"]);
    git(&f.root(), &["commit", "-q", "-m", "vendor: pin the fork"]);
    open_and_switch(&f);
    f.ok(&["phase", "run"]);

    // Cites the wrong tracker: still asked, and the draft carries this repository's pin.
    commit_on_phase(
        &f,
        &["tools/fetch.sh"],
        "scaffold: fetch around goedelsoup/yidam#1466",
    );
    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["upstream"]["repository"], "someone/fork");
    assert!(
        report["upstream"]["draft"]
            .as_str()
            .unwrap()
            .contains("commit = \"abc123\""),
        "{report}"
    );

    commit_on_phase(
        &f,
        &["tools/pin.sh"],
        "scaffold: pin until someone/fork#7 lands",
    );
    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["upstream"]["cited"], serde_json::json!([7]));
    assert!(report["upstream"].get("draft").is_none(), "{report}");
    assert!(
        !f.ok(&["phase", "settle"]).contains("nothing was filed"),
        "a phase that reported is asked nothing"
    );
}

/// Domain tooling in a laid-out directory is the corpus's own compute layer, not a workaround,
/// and asking about it would have fired on half the phases of a working corpus.
#[test]
fn settle_asks_nothing_of_a_phase_that_added_only_laid_out_paths() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    commit_on_phase(
        &f,
        &[
            "crates/ground/src/lib.rs",
            "web/index.html",
            "docs/notes.md",
        ],
        "scaffold: a crate, a page, a note",
    );
    let report = f.json(&["phase", "settle"]);
    assert_eq!(
        report["upstream"]["unlaid"],
        serde_json::json!([]),
        "{report}"
    );
    assert!(report["upstream"].get("draft").is_none());
}

// ── settle and the REGEN blocks ───────────────────────────────────────────────

/// A README carrying one REGEN block with its placeholder, committed on the baseline before
/// the phase opens — the state every scaffolded block is in until something refreshes it.
fn with_block(f: &Fixture, command: &str) {
    std::fs::write(
        f.root().join("README.md"),
        format!(
            "# fixture\n\n<!-- REGEN: yidam {command} -->\n_Run `yidam regen` to populate._\n\
             <!-- /REGEN -->\n"
        ),
    )
    .unwrap();
    git(&f.root(), &["add", "README.md"]);
    git(
        &f.root(),
        &[
            "commit",
            "-q",
            "-m",
            "scaffold: a README with a REGEN block",
        ],
    );
}

/// **#1066.** Closing a phase requires the blocks to be current, and `settle` never touched
/// them, so every repository ran `regen` by hand one commit before the phase commit — 101 of
/// 118 phase commits in the reporting corpus. Now `settle` refreshes what is stale, stages
/// it, and prints the `regen:` commit ahead of the merge. It still writes no commit and
/// moves no ref, which is the invariant the test above pins and this one re-asserts.
#[test]
fn settle_refreshes_a_stale_block_stages_it_and_writes_no_commit() {
    let f = corpus();
    with_block(&f, "status");
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();

    // The block is stale before settle, or nothing below measures anything.
    let (_, _, current) = f.run(&["regen", "--check"]);
    assert!(!current, "the placeholder block passes `regen --check`");

    let head = git(&f.root(), &["rev-parse", "HEAD"]);
    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["ready"], true, "{report}");
    assert_eq!(report["regen"]["passed"], true, "{report}");
    assert_eq!(report["regen"]["staged"], serde_json::json!(["README.md"]));
    assert!(report["regen"]["stale"].as_array().unwrap().is_empty());
    assert!(report["regen"]["dirty"].as_array().unwrap().is_empty());

    assert_eq!(
        git(&f.root(), &["rev-parse", "HEAD"]),
        head,
        "settle wrote a commit"
    );
    assert_eq!(
        git(&f.root(), &["diff", "--cached", "--name-only"]).trim(),
        "README.md",
        "the refreshed block is staged, and nothing else is"
    );
    f.ok(&["regen", "--check"]);

    // The merge carries the `regen:` commit, so the draft counts it: the branch's commits
    // plus one, and the README among the outputs.
    let on_branch: usize = git(&f.root(), &["rev-list", "--count", "HEAD", "^main"])
        .parse()
        .unwrap();
    assert_eq!(report["commits"], on_branch + 1, "{report}");
    assert!(
        report["outputs"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("README.md")),
        "{report}"
    );
    assert!(
        report["subject"]
            .as_str()
            .unwrap()
            .contains(&format!("{} commits", on_branch + 1)),
        "{report}"
    );

    let stdout = f.ok(&["phase", "settle"]);
    let lines: Vec<&str> = stdout.lines().map(str::trim).collect();
    let commit = lines
        .iter()
        .position(|l| l.starts_with("git commit -m \"regen:"))
        .unwrap_or_else(|| panic!("no `regen:` commit in the sequence:\n{stdout}"));
    let switch = lines
        .iter()
        .position(|l| l.starts_with("git switch main"))
        .unwrap();
    assert!(
        commit < switch,
        "the `regen:` commit goes before the merge:\n{stdout}"
    );
}

/// A second `settle` before the person has run the sequence finds nothing stale — the
/// refresh is on disk — and still prints the `regen:` commit, because the index holds it and
/// `git merge --no-ff` refuses a dirty index. A sequence that dropped the line would hand
/// back a merge git refuses.
#[test]
fn settle_keeps_printing_the_regen_commit_while_it_is_staged() {
    let f = corpus();
    with_block(&f, "status");
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();
    f.ok(&["phase", "settle"]);

    let report = f.json(&["phase", "settle"]);
    assert!(report["regen"]["stale"].as_array().unwrap().is_empty());
    assert_eq!(report["regen"]["staged"], serde_json::json!(["README.md"]));
    assert!(
        f.ok(&["phase", "settle"])
            .contains("git commit -m \"regen:"),
        "the staged refresh dropped out of the sequence"
    );
}

/// A block is generated from the tree, and CI checks it against the commit's. A checkout
/// that differs from `HEAD` — here, the one `phase run` leaves behind — would yield a block
/// the gate calls stale, so nothing is refreshed and the sync is named instead.
#[test]
fn settle_refuses_to_refresh_over_a_checkout_that_is_not_the_commit() {
    let f = corpus();
    with_block(&f, "status");
    open_and_switch(&f);
    f.ok(&["phase", "run"]);

    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["ready"], false, "{report}");
    assert_eq!(report["regen"]["passed"], false);
    assert!(!report["regen"]["stale"].as_array().unwrap().is_empty());
    assert!(!report["regen"]["dirty"].as_array().unwrap().is_empty());
    assert!(report["regen"]["staged"].as_array().unwrap().is_empty());
    assert!(
        std::fs::read_to_string(f.root().join("README.md"))
            .unwrap()
            .contains("_Run `yidam regen` to populate._"),
        "the block was refreshed from a tree that is not the commit's"
    );
    assert!(
        git(&f.root(), &["diff", "--cached", "--name-only"])
            .lines()
            .all(|l| l != "README.md"),
        "settle staged a block it did not refresh"
    );
    let stdout = f.ok(&["phase", "settle"]);
    assert!(stdout.contains("Not ready"), "{stdout}");
    assert!(stdout.contains("git reset --hard HEAD"), "{stdout}");

    f.resync();
    assert_eq!(f.json(&["phase", "settle"])["ready"], true);
}

/// A block no generator writes cannot be made current by a refresh, so the phase is not
/// ready with `regen --check`'s own remedy — rather than ready, and red on the first push.
#[test]
fn settle_refuses_a_block_no_generator_writes() {
    let f = corpus();
    with_block(&f, "statsu");
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();

    let report = f.json(&["phase", "settle"]);
    assert_eq!(report["ready"], false, "{report}");
    assert_eq!(report["regen"]["passed"], false);
    assert_eq!(report["regen"]["unclaimed"][0]["generator"], "statsu");
    let stdout = f.ok(&["phase", "settle"]);
    assert!(stdout.contains("no generator writes"), "{stdout}");
    assert!(!stdout.contains("git merge --no-ff"), "{stdout}");
}

// ── the report contract ───────────────────────────────────────────────────────

/// Every `--format json` here carries the envelope, and its fields are the documented ones.
///
/// **Asserted here because nothing else can reach it.** `report_goldens.rs` requires every
/// `--format`-bearing command to have its fields checked, and it discovers those commands by
/// scanning `yidam <name> --help` for a `--format` line. A subcommand group does not print its
/// children's flags, so `phase` never enters that roster and its exemption would go stale
/// unnoticed — the going-green-while-covering-nothing shape that file is written against.
/// **#893** is that hole, filed rather than widened here: eight subcommands sit outside the
/// roster and four of them predate these three.
///
/// So this is `capability_run.rs`'s bargain, which `NO_REPORT` states for `run`: a writing
/// command stays out of the shared fixture and asserts the envelope and its own fields in its
/// own suite, against a corpus it built.
#[test]
fn every_phase_report_carries_the_envelope_and_its_documented_fields() {
    let f = corpus();

    let start = f.json(&["phase", "start", "outcome-axis", "--type", "Investigation"]);
    git(&f.root(), &["switch", "-q", "phase/outcome-axis"]);
    let run = f.json(&["phase", "run"]);
    let settle = f.json(&["phase", "settle"]);

    for (name, report) in [("start", &start), ("run", &run), ("settle", &settle)] {
        assert_eq!(report["format_version"], "1", "{name}");
        assert!(report["yidam"]["version"].is_string(), "{name}");
        assert!(report["yidam"]["commit"].is_string(), "{name}");
        assert!(report["root"].is_string(), "{name}");
        // Every phase report names the phase and its type, so a consumer holding three of
        // them can key on one thing rather than on where each came from.
        assert_eq!(report["phase"], "outcome-axis", "{name}");
        assert_eq!(report["type"], "Investigation", "{name}");
        assert_eq!(report["branch"], "phase/outcome-axis", "{name}");
    }

    for key in ["from", "commit", "record", "input"] {
        assert!(
            start.get(key).is_some(),
            "start is missing `{key}`: {start}"
        );
    }
    for key in [
        "dry_run",
        "plan",
        "completed",
        "remaining",
        "resumed",
        "refused",
    ] {
        assert!(run.get(key).is_some(), "run is missing `{key}`: {run}");
    }
    for key in [
        "baseline",
        "commits",
        "outputs",
        "remaining",
        "ready",
        "subject",
        "regen",
        "upstream",
    ] {
        assert!(
            settle.get(key).is_some(),
            "settle is missing `{key}`: {settle}"
        );
    }
    for key in ["passed", "stale", "unclaimed", "staged", "dirty"] {
        assert!(
            settle["regen"].get(key).is_some(),
            "settle's `regen` is missing `{key}`: {settle}"
        );
    }

    // `revision_skew` is absent rather than null where there is none — the distinction the
    // record's own `Option` fields keep, so a consumer never has to tell "no kuten moved"
    // from "this binary does not report it".
    assert!(settle.get("revision_skew").is_none(), "{settle}");
}

/// `--dry-run` resolves the plan and writes nothing — `cmd/run`'s rule, on this surface.
#[test]
fn a_dry_run_plans_and_writes_nothing() {
    let f = corpus();
    open_and_switch(&f);

    let before = git(&f.root(), &["rev-parse", "HEAD"]);
    let report = f.json(&["phase", "run", "--dry-run"]);
    assert_eq!(report["dry_run"], true);
    assert_eq!(report["plan"].as_array().unwrap().len(), 2);
    assert_eq!(report["remaining"].as_array().unwrap().len(), 2);
    assert!(report["completed"].as_array().unwrap().is_empty());

    assert_eq!(git(&f.root(), &["rev-parse", "HEAD"]), before);
    assert!(f.invocations().is_empty(), "a dry run invoked a step");
}

// ── settle --merge ────────────────────────────────────────────────────────────

/// Every ref and the sha it stands at, for asserting a refusal moved nothing.
fn refs(f: &Fixture) -> String {
    git(
        &f.root(),
        &["for-each-ref", "--format=%(refname) %(objectname)"],
    )
}

/// **#1452, owner decision 1 on #1438.** `--merge` writes the `--no-ff` merge PHASES.md asks
/// for: two parents, the baseline first; the phase tip's tree exactly; `yidam phase` as author
/// and the person as committer. Then the checkout is on the baseline and the branch is gone.
#[test]
fn settle_merge_writes_the_merge_under_the_phase_author() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();
    let base_before = git(&f.root(), &["rev-parse", "main"]);
    let tip = git(&f.root(), &["rev-parse", "HEAD"]);

    let report = f.json(&["phase", "settle", "--merge"]);
    let merged = &report["merged"];
    assert_eq!(merged["switched"], true, "{report}");
    assert_eq!(merged["deleted"], true, "{report}");
    assert!(merged["regen"].is_null(), "{report}");

    let root = f.root();
    assert_eq!(git(&root, &["rev-parse", "--abbrev-ref", "HEAD"]), "main");
    assert_eq!(
        git(&root, &["rev-parse", "main^1"]),
        base_before,
        "the baseline is the first parent"
    );
    assert_eq!(
        git(&root, &["rev-parse", "main^2"]),
        tip,
        "the phase is the second"
    );
    assert_eq!(
        git(&root, &["rev-parse", "main^{tree}"]),
        git(&root, &["rev-parse", &format!("{tip}^{{tree}}")]),
        "the merge introduced something the phase did not hold"
    );
    assert_eq!(
        git(&root, &["log", "-1", "--format=%an <%ae>|%cn", "main"]),
        "yidam phase <phase@yidam>|Tester"
    );
    let subject = git(&root, &["log", "-1", "--format=%s", "main"]);
    assert_eq!(subject, report["subject"].as_str().unwrap());
    assert!(subject.starts_with("phase: outcome-axis — "), "{subject}");
    assert!(
        git(&root, &["branch", "--list", "phase/outcome-axis"]).is_empty(),
        "the phase branch was left behind"
    );
    assert!(git(&root, &["status", "--porcelain"]).is_empty());

    // And `phases` reads it settled, from the ref shape alone.
    let phases = f.json(&["phases"]);
    assert!(
        !phases.to_string().contains("\"active\""),
        "a merged phase still reads active: {phases}"
    );
}

/// **The rework #1438 set out to remove.** After a hand merge, a `regen` on the baseline
/// followed about twice a round. `--merge` commits the staged refresh on the phase branch and
/// merges a tree equal to the phase tip's, and every block is a function of the tree (#647),
/// so `regen --check` holds on the baseline with nothing left to do and no commit after the
/// merge.
#[test]
fn settle_merge_leaves_the_baseline_passing_regen_check() {
    let f = corpus();
    with_block(&f, "status");
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();

    let report = f.json(&["phase", "settle", "--merge"]);
    let merged = &report["merged"];
    assert!(merged["regen"].is_string(), "{report}");

    let root = f.root();
    let (stdout, stderr, current) = f.run(&["regen", "--check"]);
    assert!(
        current,
        "the baseline is stale after --merge:\n{stdout}\n{stderr}"
    );
    assert!(git(&root, &["status", "--porcelain"]).is_empty());

    // The `regen:` commit is on the phase side of the merge, under the same author.
    let second_parent = git(&root, &["log", "-1", "--format=%s|%an", "main^2"]);
    assert_eq!(
        second_parent,
        "regen: REGEN blocks refreshed to settle outcome-axis|yidam phase"
    );
    assert_eq!(
        git(&root, &["log", "-1", "--format=%s", "main"]),
        report["merged"]["subject"].as_str().unwrap(),
        "something was committed after the merge"
    );
}

/// `--summary` replaces the tally, which PHASES.md asks a person to do with the draft.
#[test]
fn settle_merge_takes_the_summary_in_place_of_the_tally() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();
    f.ok(&[
        "phase",
        "settle",
        "--merge",
        "--summary",
        "the outcome axis is computed",
    ]);
    assert_eq!(
        git(&f.root(), &["log", "-1", "--format=%s", "main"]),
        "phase: outcome-axis — the outcome axis is computed"
    );
}

/// **Article V, the case it rules out.** A baseline that moved on since the phase branched
/// would make the merge combine two trees, and the combination is something neither side
/// held. Refused, with nothing written.
#[test]
fn settle_merge_refuses_a_baseline_that_moved_on() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();
    let root = f.root();
    git(&root, &["switch", "-q", "main"]);
    std::fs::write(root.join("NOTES.md"), "elsewhere\n").unwrap();
    git(&root, &["add", "NOTES.md"]);
    git(&root, &["commit", "-q", "-m", "scaffold: notes"]);
    git(&root, &["switch", "-q", "phase/outcome-axis"]);

    let before = refs(&f);
    let (stdout, stderr, ok) = f.run(&["phase", "settle", "--merge"]);
    assert!(!ok, "a moved baseline was merged:\n{stdout}");
    assert!(stderr.contains("has moved on"), "{stderr}");
    assert!(stderr.contains("Nothing was merged"), "{stderr}");
    assert_eq!(refs(&f), before, "a refusal moved a ref");
}

/// A checkout that is not the commit — the one `phase run` leaves — is not refreshed by
/// `settle`, so it is not ready; and an edit in progress beside staged blocks is somebody's
/// work, which the switch at the end would carry onto the baseline.
#[test]
fn settle_merge_refuses_a_checkout_that_is_not_the_commit() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    let before = refs(&f);
    let (_, stderr, ok) = f.run(&["phase", "settle", "--merge"]);
    assert!(!ok);
    assert!(stderr.contains("git reset --hard HEAD"), "{stderr}");
    assert_eq!(refs(&f), before);

    f.resync();
    std::fs::write(
        f.root().join(".yidam/corpus/seed.yml"),
        "class: concept\nname: edited\n",
    )
    .unwrap();
    let (_, stderr, ok) = f.run(&["phase", "settle", "--merge"]);
    assert!(!ok, "an edit in progress was carried into a merge");
    assert!(stderr.contains(".yidam/corpus/seed.yml"), "{stderr}");
    assert_eq!(refs(&f), before);
}

/// Not ready is not merged: `--merge` adds a writer, not a looser gate.
#[test]
fn settle_merge_refuses_a_phase_that_is_not_ready() {
    let f = corpus();
    f.second_fails(true);
    open_and_switch(&f);
    let (_, _, ok) = f.run(&["phase", "run"]);
    assert!(!ok);
    f.resync();
    let before = refs(&f);
    let (_, stderr, ok) = f.run(&["phase", "settle", "--merge"]);
    assert!(!ok);
    assert!(stderr.contains("Not ready"), "{stderr}");
    assert!(stderr.contains("Nothing was merged"), "{stderr}");
    assert_eq!(refs(&f), before);
}

/// Moving the baseline's ref under a checkout of it elsewhere would leave that checkout
/// reading as a revert of the phase.
#[test]
fn settle_merge_refuses_a_baseline_checked_out_in_another_worktree() {
    let f = corpus();
    open_and_switch(&f);
    f.ok(&["phase", "run"]);
    f.resync();
    let other = f.dir.path().join("other");
    git(
        &f.root(),
        &["worktree", "add", "-q", other.to_str().unwrap(), "main"],
    );
    let before = refs(&f);
    let (_, stderr, ok) = f.run(&["phase", "settle", "--merge"]);
    assert!(!ok);
    assert!(stderr.contains("another worktree"), "{stderr}");
    assert_eq!(refs(&f), before);
}
