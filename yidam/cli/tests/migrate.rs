//! `yidam migrate` against the corpus it teaches from.
//!
//! Every case runs the real binary over a materialized copy of `examples/streamflow` and
//! then runs the **gate** over the result. That is the property worth holding: a migration
//! is not "the edits I intended" but "a corpus its own checks still accept". The class
//! rename shipped with three defects that every unit test would have passed and the first
//! run against a real corpus caught — a doubled `.yml`, a rebuilt link that had not changed,
//! and the `instance-of` edge every instance carries pointing at a file that had moved.

use std::path::Path;
use std::process::Command;

mod common;

use common::{repo_root, tracked_under};

struct Corpus {
    dir: tempfile::TempDir,
}

impl Corpus {
    fn new() -> Self {
        let root = repo_root();
        let dir = tempfile::tempdir().unwrap();
        let prefix = "examples/streamflow/";
        let files = tracked_under(&root, prefix);
        assert!(!files.is_empty(), "no tracked files under {prefix}");
        for tracked in &files {
            let to = dir.path().join(tracked.strip_prefix(prefix).unwrap());
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(tracked), &to).unwrap();
        }
        let me = Self { dir };
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "t@t.test"],
            vec!["config", "user.name", "T"],
            vec!["add", "-A"],
            vec!["commit", "-q", "-m", "genesis: the example corpus"],
        ] {
            me.git(&args);
        }
        // The premise every assertion below rests on.
        assert!(
            me.gate_is_clean(),
            "the example corpus does not start clean"
        );
        me
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) {
        let ok = Command::new("git")
            .current_dir(self.path())
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?}");
    }

    fn run(&self, args: &[&str]) -> (bool, String) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.path())
            .args(args)
            .output()
            .unwrap();
        let mut text = String::from_utf8_lossy(&out.stdout).to_string();
        text.push_str(&String::from_utf8_lossy(&out.stderr));
        (out.status.success(), text)
    }

    /// `lint` empty at every severity **and** `graph-check` clean — the standard the
    /// example corpus is held to elsewhere, applied to the corpus a migration produced.
    fn gate_is_clean(&self) -> bool {
        let (_, lint) = self.run(&["lint", "--warn"]);
        let (graph_ok, _) = self.run(&["graph-check"]);
        lint.contains("0 finding(s)") && graph_ok
    }

    fn read(&self, rel: &str) -> String {
        std::fs::read_to_string(self.path().join(rel)).unwrap_or_default()
    }

    fn dirty(&self) -> bool {
        let out = Command::new("git")
            .current_dir(self.path())
            .args(["status", "--porcelain"])
            .output()
            .unwrap();
        !out.stdout.is_empty()
    }
}

// ── property rename ───────────────────────────────────────────────────────────

#[test]
fn a_property_rename_moves_the_declaration_and_every_instance() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "property", "gage", "parameter", "parameter_code"]);
    assert!(ok, "{out}");

    assert!(c
        .read(".yidam/corpus/gage.ont.yml")
        .contains("name: parameter_code"));
    for instance in ["canyon-outlet", "valley-bridge"] {
        let text = c.read(&format!(".yidam/corpus/gage/{instance}.yml"));
        assert!(
            text.contains("parameter_code:"),
            "{instance} kept the old key"
        );
        assert!(
            !text.contains("\n  parameter:"),
            "{instance} carries both names"
        );
    }
    assert!(c.gate_is_clean(), "the migration left the corpus failing");
}

/// The class and the instances have to move together. Renaming only the declaration is the
/// hand-edit this command exists to replace, and it produces `undeclared-property` on every
/// instance at once.
#[test]
fn renaming_only_the_declaration_would_have_broken_the_gate() {
    let c = Corpus::new();
    let ont = ".yidam/corpus/gage.ont.yml";
    let text = c
        .read(ont)
        .replace("name: parameter", "name: parameter_code");
    std::fs::write(c.path().join(ont), text).unwrap();
    assert!(
        !c.gate_is_clean(),
        "editing the class alone must break the gate — otherwise this test proves nothing"
    );
}

// ── retype ────────────────────────────────────────────────────────────────────

/// The operation with a wrong answer available. `cubic feet per second` is not a date, and
/// writing it back as one — or as a string, and calling it migrated — would leave the corpus
/// in a state its own gate rejects while reporting success.
#[test]
fn a_retype_with_no_mechanical_conversion_is_refused_and_writes_nothing() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "retype", "gage", "units", "date"]);
    assert!(!ok, "a refused migration must exit nonzero:\n{out}");
    assert!(out.contains("no mechanical conversion"), "{out}");
    assert!(out.contains("nothing was written"), "{out}");
    assert!(!c.dirty(), "a blocked migration touched the tree");
    assert!(c.gate_is_clean());
}

#[test]
fn a_retype_every_value_satisfies_is_performed() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "retype", "gage", "units", "text"]);
    assert!(ok, "{out}");
    assert!(c.read(".yidam/corpus/gage.ont.yml").contains("type: text"));
    assert!(c.gate_is_clean());
}

/// The refusal must use the gate's own predicate. A migration that disagreed with
/// `property-type` about what a valid value is would be a migration into a broken build —
/// so a type the *checks* accept is a type the migration performs.
#[test]
fn the_refusal_agrees_with_the_check_that_gates() {
    let c = Corpus::new();
    // `claim` accepts the three tokens and nothing else; `claim_tag: inference` satisfies
    // it, so retyping the field that already holds one is allowed…
    let (ok, _) = c.run(&["migrate", "retype", "gage", "claim_tag", "string"]);
    assert!(ok, "a claim token is a valid string");
    assert!(c.gate_is_clean());

    // …and back again, because it is still one of the three tokens.
    let (ok, _) = c.run(&["migrate", "retype", "gage", "claim_tag", "claim"]);
    assert!(ok);
    assert!(c.gate_is_clean());
}

// ── class rename ──────────────────────────────────────────────────────────────

/// The operation with the most ways to be subtly wrong, and all three of them were.
#[test]
fn a_class_rename_leaves_the_corpus_passing_its_own_gate() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "class", "gage", "station"]);
    assert!(ok, "{out}");

    assert!(c.path().join(".yidam/corpus/station.ont.yml").is_file());
    assert!(!c.path().join(".yidam/corpus/gage.ont.yml").exists());
    assert!(c
        .path()
        .join(".yidam/corpus/station/canyon-outlet.yml")
        .is_file());
    // The doubled extension that produced `canyon-outlet.yml.yml`.
    assert!(
        !c.path()
            .join(".yidam/corpus/station/canyon-outlet.yml.yml")
            .exists(),
        "the move appended a second .yml"
    );
    // The empty directory left behind reads as a class with no instances.
    assert!(!c.path().join(".yidam/corpus/gage").exists());
    assert!(c.gate_is_clean(), "{out}");
}

/// Every instance points at its class file. The class file is being renamed with the
/// directory, and missing it left all of them dangling.
#[test]
fn a_class_rename_follows_the_instance_of_edge_to_the_class_file() {
    let c = Corpus::new();
    let (ok, _) = c.run(&["migrate", "class", "gage", "station"]);
    assert!(ok);
    let text = c.read(".yidam/corpus/station/canyon-outlet.yml");
    assert!(text.contains("../station.ont.yml"), "{text}");
    assert!(!text.contains("../gage.ont.yml"), "{text}");
}

/// A link that leaves the corpus is not affected by a class rename — every instance sits at
/// `<class>/<file>`, so the rename preserves depth. Rebuilding it through `normalize`, which
/// swallows a `..` that escapes the corpus, returned it one level short.
#[test]
fn a_class_rename_does_not_touch_a_link_out_of_the_corpus() {
    let c = Corpus::new();
    let before = c.read(".yidam/corpus/gage/canyon-outlet.yml");
    assert!(
        before.contains("../../catalog/usgs-nwis.md"),
        "the fixture must carry a catalog citation for this test to mean anything"
    );

    let (ok, _) = c.run(&["migrate", "class", "gage", "station"]);
    assert!(ok);
    assert!(
        c.read(".yidam/corpus/station/canyon-outlet.yml")
            .contains("../../catalog/usgs-nwis.md"),
        "the catalog citation was rewritten and should not have been"
    );
}

/// An edge is declared from both ends. A rename that fixed only its own side would leave
/// the other class naming one that no longer exists.
#[test]
fn a_class_rename_updates_the_edge_declared_at_the_other_end() {
    let c = Corpus::new();
    assert!(c
        .read(".yidam/corpus/reach.ont.yml")
        .contains("target: gage"));
    let (ok, _) = c.run(&["migrate", "class", "gage", "station"]);
    assert!(ok);
    let reach = c.read(".yidam/corpus/reach.ont.yml");
    assert!(reach.contains("target: station"), "{reach}");
    assert!(!reach.contains("target: gage"), "{reach}");
}

#[test]
fn a_class_rename_moves_files_with_git_so_history_follows() {
    let c = Corpus::new();
    let (ok, _) = c.run(&["migrate", "class", "gage", "station"]);
    assert!(ok);
    let out = Command::new("git")
        .current_dir(c.path())
        .args(["status", "--porcelain"])
        .output()
        .unwrap();
    let status = String::from_utf8_lossy(&out.stdout);
    // `RM`, not `R `: the content edits land before the move, so the staged rename carries
    // a worktree modification with it. What matters is the `R` — git recorded a rename
    // rather than a delete and an add, so `--follow` reaches the node's earlier history.
    assert!(
        status
            .lines()
            .any(|l| l.starts_with('R') && l.contains("gage/canyon-outlet.yml")),
        "the move was not staged as a rename:\n{status}"
    );
}

// ── edge re-target ────────────────────────────────────────────────────────────

/// What the migration *creates* and cannot do. Which instances should now point elsewhere
/// is a decision about the corpus, and the report's job is to name every one of them.
#[test]
fn an_edge_retarget_predicts_exactly_the_violations_the_gate_then_reports() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "edge", "reach", "measured-by", "concept"]);
    assert!(ok, "{out}");
    assert!(out.contains("2 instance(s) now in violation"), "{out}");

    let (_, lint) = c.run(&["lint", "--warn"]);
    for node in ["reach/lower-canyon.yml", "reach/tailwater.yml"] {
        assert!(out.contains(node), "the migration did not predict {node}");
        assert!(lint.contains(node), "the gate did not report {node}");
    }
    assert!(
        lint.contains("edge-target-class"),
        "the violations must be the ones `edge-target-class` gates on:\n{lint}"
    );
}

#[test]
fn an_edge_retarget_at_a_class_that_does_not_exist_is_refused() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "edge", "reach", "measured-by", "nonesuch"]);
    assert!(!ok, "{out}");
    assert!(out.contains("has no nonesuch.ont.yml"), "{out}");
    assert!(!c.dirty());
}

// ── refusals and the record ───────────────────────────────────────────────────

#[test]
fn migrations_that_cannot_proceed_write_nothing() {
    let c = Corpus::new();
    let cases: [&[&str]; 6] = [
        &["migrate", "class", "nonesuch", "other"],
        &["migrate", "class", "gage", "reach"],
        &["migrate", "property", "gage", "nonesuch", "x"],
        &["migrate", "property", "gage", "parameter", "units"],
        &["migrate", "retype", "gage", "parameter", "string"],
        &["migrate", "edge", "gage", "nonesuch", "concept"],
    ];
    for args in cases {
        let (ok, out) = c.run(args);
        assert!(!ok, "{args:?} should be refused:\n{out}");
        assert!(out.contains("Cannot migrate"), "{args:?}:\n{out}");
        assert!(!c.dirty(), "{args:?} touched the tree");
    }
}

#[test]
fn a_dry_run_prints_the_plan_and_changes_nothing() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "--dry-run", "class", "gage", "station"]);
    assert!(ok, "{out}");
    assert!(out.contains("Would migrate"), "{out}");
    assert!(!c.dirty(), "a dry run wrote to the tree");
    assert!(
        !c.path().join(".yidam/migrations").exists(),
        "and wrote a record"
    );
}

/// The mechanical half of the event, kept because a migration is otherwise a wave of edits
/// under one subject with nothing saying which operation produced them.
#[test]
fn an_applied_migration_writes_a_record_naming_what_it_touched() {
    let c = Corpus::new();
    let (ok, _) = c.run(&["migrate", "edge", "reach", "measured-by", "concept"]);
    assert!(ok);

    let dir = c.path().join(".yidam/migrations");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&file).unwrap();

    assert!(text.contains("operation: edge-retarget"), "{text}");
    assert!(text.contains(".yidam/corpus/reach.ont.yml"), "{text}");
    // The violations belong in the record: the next reader of this file is the person who
    // has to deal with them.
    assert!(text.contains("lower-canyon.yml"), "{text}");
    // And it says where the ARGUMENT lives, which is not here.
    assert!(text.contains(".yidam/decisions/"), "{text}");
}

/// The commit subject must be in the closed vocabulary — `lint --commits` reports anything
/// else, and `classify_commit` files an unrecognized verb as Epistemic.
#[test]
fn the_suggested_commit_subject_uses_a_recognized_verb() {
    let c = Corpus::new();
    let (ok, out) = c.run(&[
        "migrate",
        "--dry-run",
        "property",
        "gage",
        "parameter",
        "code",
    ]);
    assert!(ok, "{out}");
    let subject = out
        .lines()
        .find_map(|l| l.strip_prefix("commit: "))
        .expect("a commit subject");
    let verb = subject.split(':').next().unwrap();
    assert!(
        yidam_core::git::is_recognized_verb(verb),
        "`{verb}` is not in the closed vocabulary"
    );
}
