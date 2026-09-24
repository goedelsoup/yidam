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
    for key in ["dry_run", "plan", "completed", "remaining", "resumed"] {
        assert!(run.get(key).is_some(), "run is missing `{key}`: {run}");
    }
    for key in [
        "baseline",
        "commits",
        "outputs",
        "remaining",
        "ready",
        "subject",
    ] {
        assert!(
            settle.get(key).is_some(),
            "settle is missing `{key}`: {settle}"
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
