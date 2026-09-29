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
        common::git::git(self.path(), args);
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
        !common::git::out(self.path(), &["status", "--porcelain"]).is_empty()
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

    // …and it costs the corpus the count, which `claim-property-undeclared` is what says so.
    // The value is still `inference` and nothing reads it as a claim any more, because the
    // class no longer types the field `claim`. The finding is `Info`, so the gate is still
    // clean — the migration is allowed, and the consequence is not silent. (#1069)
    let (gate_ok, out) = c.run(&["lint"]);
    assert!(gate_ok, "an Info finding must not gate:\n{out}");
    let (_, warned) = c.run(&["lint", "--warn"]);
    assert!(
        warned.contains("claim-property-undeclared"),
        "retyping away from `claim` stopped the counting and nothing said so:\n{warned}"
    );

    // …and back again, because it is still one of the three tokens.
    let (ok, _) = c.run(&["migrate", "retype", "gage", "claim_tag", "claim"]);
    assert!(ok);
    // The declaration is back, so the finding is gone: it was reporting the gap and not the
    // value, and the value never moved.
    assert!(c.gate_is_clean());
}

/// #1044: the retype that was refused for asking an author to do it by hand first.
///
/// A `number` is an unquoted YAML number, so `reach.length_km: 24` retyped to `string` makes
/// every instance a `property-type` violation — and back the other way, `"24"` retyped to
/// `number` did too. Both of `property-type`'s messages name their own repair, *quote it* and
/// *unquote it*, and this carries them out on the instances.
///
/// Run as a round trip because requoting is reversible: the two corpus files must come back
/// byte for byte, which no assertion on one direction alone can say.
#[test]
fn a_retype_across_the_quoting_boundary_requotes_every_instance() {
    let c = Corpus::new();
    let instances = [
        (".yidam/corpus/reach/lower-canyon.yml", "24"),
        (".yidam/corpus/reach/tailwater.yml", "6"),
    ];
    let before: Vec<String> = instances.iter().map(|(f, _)| c.read(f)).collect();

    let (ok, out) = c.run(&["migrate", "retype", "reach", "length_km", "string"]);
    assert!(ok, "{out}");
    assert!(c
        .read(".yidam/corpus/reach.ont.yml")
        .contains("type: string"));
    for (file, n) in instances {
        let text = c.read(file);
        assert!(
            text.contains(&format!("length_km: \"{n}\"")),
            "{file} kept a bare number under a `string` declaration:\n{text}"
        );
    }
    assert!(
        c.gate_is_clean(),
        "a performed retype left the corpus failing its own gate"
    );

    let (ok, out) = c.run(&["migrate", "retype", "reach", "length_km", "number"]);
    assert!(ok, "{out}");
    assert!(c
        .read(".yidam/corpus/reach.ont.yml")
        .contains("type: number"));
    for ((file, _), original) in instances.iter().zip(&before) {
        assert_eq!(&c.read(file), original, "{file} did not come back");
    }
    assert!(c.gate_is_clean());
}

/// The refusal #1044 asked to keep, and it is not the quoting that decides it.
///
/// `"00060"` unquoted is still text to YAML — the leading zero disqualifies it — so dropping
/// the quotes would leave `property-type` reporting the instance it reported before. The
/// candidate is parsed and put back through the gate's predicate, which is how a value that
/// looks like a number and is not stays a refusal.
#[test]
fn a_value_that_only_looks_numeric_is_refused_by_name() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "retype", "gage", "parameter", "number"]);
    assert!(!ok, "{out}");
    assert!(out.contains("no mechanical conversion"), "{out}");
    // The refusal names the instance and what was tried, because `property-type` on its own
    // would tell the author to unquote a value the migration has just declined to unquote.
    assert!(out.contains("canyon-outlet.yml"), "{out}");
    assert!(out.contains("unquoting it gives `00060`"), "{out}");
    assert!(out.contains("nothing was written"), "{out}");
    assert!(!c.dirty(), "a blocked retype touched the tree");
    assert!(c.gate_is_clean());
}

/// And the case the issue names: prose that mentions no number at all.
#[test]
fn prose_under_a_string_is_not_unquoted_into_a_number() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "retype", "reach", "regulated", "number"]);
    assert!(!ok, "{out}");
    assert!(out.contains("no mechanical conversion"), "{out}");
    assert!(!c.dirty());
}

/// RFC-0044: a `values:` set binds on `string` and is carried on any other type, so a retype
/// into `string` is where it starts to bind. `reach.regulated` holds
/// `"yes — discharge set by outlet works"` — prose in a token field, the issue's own shape —
/// and a retype that admitted it against `[yes, no]` would be a migration into a red build.
#[test]
fn a_retype_into_a_string_with_a_declared_set_is_held_to_the_set() {
    let c = Corpus::new();
    let ont = ".yidam/corpus/reach.ont.yml";
    let text = c.read(ont).replace(
        "  - name: regulated\n    type: string\n",
        "  - name: regulated\n    type: flow-control\n    values: [yes, no]\n",
    );
    assert!(
        text.contains("flow-control"),
        "the fixture drifted:\n{text}"
    );
    std::fs::write(c.path().join(ont), text).unwrap();
    c.git(&[
        "commit",
        "-qam",
        "migrate: regulated is a coined type with a set it does not bind",
    ]);
    // The premise: on a coined type the set is carried and ignored, so the gate is clean.
    assert!(c.gate_is_clean(), "a set on a coined type must not bind");

    let (ok, out) = c.run(&["migrate", "retype", "reach", "regulated", "string"]);
    assert!(
        !ok,
        "a retype into a set the instances contradict must refuse:\n{out}"
    );
    for instance in ["lower-canyon", "tailwater"] {
        assert!(
            out.contains(instance),
            "{instance} is outside the set and unnamed:\n{out}"
        );
    }
    assert!(out.contains("widen `values:`"), "{out}");
    assert!(out.contains("nothing was written"), "{out}");
    assert!(!c.dirty(), "a blocked retype touched the tree");
    assert!(c.gate_is_clean());
}

// ── value rename ──────────────────────────────────────────────────────────────

/// Give `gage.units` the closed set its two instances already agree on, committed, so the
/// corpus starts clean under RFC-0044 and every assertion below is about the rename.
fn with_units_set(c: &Corpus, declaration: &str) {
    let ont = ".yidam/corpus/gage.ont.yml";
    let text = c.read(ont).replace(
        "  - name: units\n    type: string\n",
        &format!("  - name: units\n    type: string\n{declaration}"),
    );
    assert!(text.contains("values:"), "the fixture drifted:\n{text}");
    std::fs::write(c.path().join(ont), text).unwrap();
    c.git(&["commit", "-qam", "migrate: units declares its set"]);
    assert!(
        c.gate_is_clean(),
        "the declared set must match the instances"
    );
}

/// #1120: a value of a closed set is renamed on the declaration and on every instance holding
/// it, as one event — the set is closed, so neither half can go first.
#[test]
fn a_value_rename_rewrites_the_declaration_and_every_instance_holding_it() {
    let c = Corpus::new();
    with_units_set(
        &c,
        "    values: [cubic feet per second, cubic metres per second]\n",
    );

    let (ok, out) = c.run(&[
        "migrate",
        "value",
        "gage",
        "units",
        "cubic feet per second",
        "cfs",
    ]);
    assert!(ok, "{out}");
    assert!(
        out.contains("3 edit(s) across 3 file(s)"),
        "one declaration item and two instances:\n{out}"
    );
    let ont = c.read(".yidam/corpus/gage.ont.yml");
    assert!(
        ont.contains("values: [cfs, cubic metres per second]"),
        "the other item stays:\n{ont}"
    );
    for instance in ["valley-bridge", "canyon-outlet"] {
        let text = c.read(&format!(".yidam/corpus/gage/{instance}.yml"));
        assert!(text.contains("  units: cfs\n"), "{instance}:\n{text}");
    }
    assert!(
        c.gate_is_clean(),
        "the rename left the corpus outside its own set"
    );

    let dir = c.path().join(".yidam/migrations");
    let record = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&record).unwrap();
    assert!(text.contains("operation: value-rename"), "{text}");
}

/// The block spelling of the same list, with the item quoted: the quotes are kept, and a
/// new value YAML would read as something other than a string is quoted on its own account.
#[test]
fn a_value_rename_keeps_the_quoting_of_a_block_list_item() {
    let c = Corpus::new();
    with_units_set(&c, "    values:\n      - \"cubic feet per second\"\n");

    let (ok, out) = c.run(&[
        "migrate",
        "value",
        "gage",
        "units",
        "cubic feet per second",
        "ft3/s",
    ]);
    assert!(ok, "{out}");
    let ont = c.read(".yidam/corpus/gage.ont.yml");
    assert!(ont.contains("      - \"ft3/s\"\n"), "{ont}");
    let text = c.read(".yidam/corpus/gage/valley-bridge.yml");
    assert!(text.contains("  units: ft3/s\n"), "{text}");
    assert!(c.gate_is_clean());

    // `24` bare would parse as a number and leave the set it was renamed into.
    let (ok, out) = c.run(&["migrate", "value", "gage", "units", "ft3/s", "24"]);
    assert!(ok, "{out}");
    let ont = c.read(".yidam/corpus/gage.ont.yml");
    assert!(ont.contains("      - \"24\"\n"), "{ont}");
    let text = c.read(".yidam/corpus/gage/valley-bridge.yml");
    assert!(text.contains("  units: \"24\"\n"), "{text}");
    assert!(c.gate_is_clean());
}

/// The refusals, each of which leaves the tree as it found it: no set, a value outside it,
/// and a rename onto a value already in it, which would merge two.
#[test]
fn a_value_rename_outside_the_declared_set_is_refused() {
    let c = Corpus::new();
    let (ok, out) = c.run(&[
        "migrate",
        "value",
        "gage",
        "units",
        "cubic feet per second",
        "cfs",
    ]);
    assert!(!ok, "{out}");
    assert!(out.contains("declares no `values:`"), "{out}");
    assert!(!c.dirty());

    with_units_set(
        &c,
        "    values: [cubic feet per second, cubic metres per second]\n",
    );
    let cases: [(&[&str], &str); 4] = [
        (
            &["migrate", "value", "gage", "units", "cfs", "cumecs"],
            "declares no value `cfs`",
        ),
        (
            &[
                "migrate",
                "value",
                "gage",
                "units",
                "cubic feet per second",
                "cubic metres per second",
            ],
            "would merge two values",
        ),
        (
            &["migrate", "value", "gage", "nonesuch", "a", "b"],
            "declares no property",
        ),
        (
            &[
                "migrate",
                "value",
                "gage",
                "units",
                "cubic feet per second",
                "cubic feet per second",
            ],
            "the same",
        ),
    ];
    for (args, why) in cases {
        let (ok, out) = c.run(args);
        assert!(!ok, "{args:?} should be refused:\n{out}");
        assert!(out.contains("Cannot migrate"), "{args:?}:\n{out}");
        assert!(out.contains(why), "{args:?}:\n{out}");
        assert!(!c.dirty(), "{args:?} touched the tree");
    }
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
    let status = common::git::out(c.path(), &["status", "--porcelain"]);
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

// ── the findings lift ─────────────────────────────────────────────────────────

/// The node the lift is run against, carrying a paragraph as a released binary wrote one.
///
/// Written literally rather than drafted: #712 deleted the machinery that composed these, so no
/// version of this tree can produce one to migrate.
const NODE: &str = ".yidam/corpus/gage/canyon-outlet.yml";

fn carrying_a_legacy_paragraph(c: &Corpus) -> String {
    let before = c.read(NODE);
    let text = before.replace(
        "properties:",
        "\n  Opened by `yidam propose` at 8d35441 [orphan-in] — nothing links to this node. \
         What follows\n  from that is unresolved here: `yidam propose` carries findings into \
         the corpus and does\n  not answer them. [open]\nproperties:",
    );
    assert_ne!(text, before, "the fixture node has no `properties:` key");
    std::fs::write(c.path().join(NODE), &text).unwrap();
    c.git(&["add", "-A"]);
    c.git(&[
        "commit",
        "-q",
        "-m",
        "establish: a question an earlier release opened",
    ]);
    before
}

#[test]
fn a_dry_run_of_a_findings_lift_prints_the_plan_and_changes_nothing() {
    let c = Corpus::new();
    let with_paragraph = {
        carrying_a_legacy_paragraph(&c);
        c.read(NODE)
    };
    let (ok, out) = c.run(&["migrate", "--dry-run", "findings"]);
    assert!(ok, "{out}");
    assert!(out.contains("Would migrate"), "{out}");
    assert!(out.contains("orphan-in"), "{out}");
    assert_eq!(c.read(NODE), with_paragraph, "a dry run rewrote the node");
    assert!(!c.dirty(), "a dry run wrote to the tree");
    assert!(
        !c.path().join(".yidam/migrations").exists(),
        "a dry run wrote a record"
    );
}

/// The lift itself: the paragraph becomes a record, the corpus still gates, and a record of the
/// migration names the node it touched.
#[test]
fn a_findings_lift_replaces_the_paragraph_and_the_corpus_still_gates() {
    let c = Corpus::new();
    let before = carrying_a_legacy_paragraph(&c);

    let (ok, out) = c.run(&["migrate", "findings"]);
    assert!(ok, "{out}");
    assert!(out.contains("Migrated"), "{out}");

    let after = c.read(NODE);
    assert!(
        !after.contains("Opened by `yidam propose` at"),
        "the paragraph survived:\n{after}"
    );
    assert!(after.contains("check: orphan-in"), "{after}");
    assert!(after.contains("opened_at: 8d35441"), "{after}");
    // Everything the node said before the paragraph arrived is still there, byte for byte, up
    // to the record appended at the end.
    for line in before.lines().filter(|l| !l.trim().is_empty()) {
        assert!(after.contains(line), "the lift dropped `{line}`:\n{after}");
    }
    assert!(c.gate_is_clean(), "the lift left the corpus failing");

    let dir = c.path().join(".yidam/migrations");
    let record = std::fs::read_dir(&dir)
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    let text = std::fs::read_to_string(&record).unwrap();
    assert!(text.contains("operation: findings"), "{text}");
    assert!(text.contains(NODE), "{text}");
}

/// Run twice, and the second run has nothing to do — the property that makes this safe to run
/// over a corpus part-way through the lift.
#[test]
fn a_findings_lift_is_idempotent() {
    let c = Corpus::new();
    carrying_a_legacy_paragraph(&c);
    assert!(c.run(&["migrate", "findings"]).0);
    let once = c.read(NODE);

    let (ok, out) = c.run(&["migrate", "findings"]);
    assert!(ok, "{out}");
    assert!(out.contains("Nothing to lift"), "{out}");
    assert_eq!(c.read(NODE), once, "a second run rewrote the node");
}

/// A corpus that never ran the old `propose` gets told so, rather than being told it migrated
/// nothing.
#[test]
fn a_corpus_with_no_paragraphs_says_there_is_nothing_to_lift() {
    let c = Corpus::new();
    let (ok, out) = c.run(&["migrate", "findings"]);
    assert!(ok, "{out}");
    assert!(out.contains("Nothing to lift"), "{out}");
    assert!(!c.dirty(), "it wrote to a corpus with nothing to migrate");
}

// ── routes ───────────────────────────────────────────────────────────────────

/// `AGENTS.md` as a derivation made before RFC-0039 carries it: the whole-file list, a wrapped
/// bullet, prose on both sides, and an owner's section after.
const OLD_AGENTS: &str = "\
# AGENTS.md

## Before taking substantive action

Read the vendored prelude. It is the model this repository runs on, and it is not
negotiable from inside the repo:

- [Identity](.yidam/.vendor/prelude/IDENTITY.md) — what this kind of repository is
- [Agent conduct](.yidam/.vendor/prelude/guidelines/agent-conduct.md) — behavioral norms,
  including the `[verified]` / `[inference]` / `[open]` claim tags
- [Phases](.yidam/.vendor/prelude/PHASES.md) — how a unit of inquiry is bounded and committed

Files under `.yidam/.vendor/` are read-only.

## Domain gates

An owner's section, edited by hand.
";

/// The example corpus with an old `AGENTS.md` and, unless `vendored` is false, the routes the
/// template ships vendored beside it.
fn with_old_agents(c: &Corpus, agents: &str, vendored: bool) {
    std::fs::write(c.path().join("AGENTS.md"), agents).unwrap();
    if vendored {
        let to = c.path().join(".yidam/.vendor/prelude/routes.yml");
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(repo_root().join("yidam/prelude/routes.yml"), &to).unwrap();
    }
    c.git(&["add", "-A"]);
    c.git(&[
        "commit",
        "-q",
        "-m",
        "vendor: a prelude that carries routes",
    ]);
}

#[test]
fn a_dry_run_of_migrate_routes_names_the_list_and_changes_nothing() {
    let c = Corpus::new();
    with_old_agents(&c, OLD_AGENTS, true);
    let (ok, out) = c.run(&["migrate", "--dry-run", "routes"]);
    assert!(ok, "{out}");
    assert!(out.contains("Would migrate"), "{out}");
    assert!(out.contains("AGENTS.md:8-11"), "{out}");
    // The commit-subject special case: its unit is a bullet, and a generic edit count would
    // report that it did nothing.
    assert!(
        out.contains("(3 bullet(s) across 1 file(s))"),
        "the commit subject does not count the bullets: {out}"
    );
    assert!(!c.dirty(), "a dry run wrote to the tree");
}

/// The migration itself: the list becomes a block the generator already agrees with, and no
/// other line of the file moves.
#[test]
fn migrate_routes_replaces_the_list_and_nothing_else() {
    let c = Corpus::new();
    with_old_agents(&c, OLD_AGENTS, true);

    let (ok, out) = c.run(&["migrate", "routes"]);
    assert!(ok, "{out}");
    assert!(out.contains("Migrated"), "{out}");
    assert!(out.contains("record: .yidam/migrations/routes-"), "{out}");

    let after = c.read("AGENTS.md");
    assert!(after.contains("<!-- REGEN: yidam routes\n"), "{after}");
    assert!(after.contains("### On every occasion"), "{after}");
    assert!(
        !after.contains("claim tags\n- [Phases]"),
        "the old list survived:\n{after}"
    );
    for kept in [
        "## Before taking substantive action\n\nRead the vendored prelude.",
        "negotiable from inside the repo:\n\n<!-- REGEN: yidam routes\n",
        "<!-- /REGEN -->\n\nFiles under `.yidam/.vendor/` are read-only.",
        "## Domain gates\n\nAn owner's section, edited by hand.\n",
    ] {
        assert!(after.contains(kept), "`{kept}` did not survive:\n{after}");
    }

    // Rendered, not a placeholder: the generator finds the block and has nothing to change.
    let (ok, out) = c.run(&["routes"]);
    assert!(ok, "{out}");
    assert_eq!(
        c.read("AGENTS.md"),
        after,
        "`yidam routes` rewrote the block"
    );

    // And it reaches it: a hand-edited route line is put back.
    let edited = after.replacen("### On every occasion", "### On most occasions", 1);
    std::fs::write(c.path().join("AGENTS.md"), &edited).unwrap();
    assert!(c.run(&["routes"]).0);
    assert_eq!(
        c.read("AGENTS.md"),
        after,
        "`yidam routes` did not reach the block"
    );
}

#[test]
fn migrate_routes_is_idempotent() {
    let c = Corpus::new();
    with_old_agents(&c, OLD_AGENTS, true);
    assert!(c.run(&["migrate", "routes"]).0);
    let once = c.read("AGENTS.md");

    let (ok, out) = c.run(&["migrate", "routes"]);
    assert!(ok, "{out}");
    assert!(out.contains("Nothing to migrate"), "{out}");
    assert_eq!(c.read("AGENTS.md"), once, "a second run rewrote the file");
}

/// An owner's line in the list refuses the whole migration, and the refusal hands over the
/// block to paste. This is the `render_migrate` dispatch: the generic refusal prints no block.
#[test]
fn an_owners_line_in_the_list_refuses_and_prints_the_block() {
    let c = Corpus::new();
    let agents = OLD_AGENTS.replace(
        "- [Phases]",
        "- [Our field guide](docs/field-guide.md) — read before any gauge work\n- [Phases]",
    );
    with_old_agents(&c, &agents, true);

    let (ok, out) = c.run(&["migrate", "routes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("Cannot migrate"), "{out}");
    assert!(
        out.contains("Our field guide"),
        "the refusal does not name the line: {out}"
    );
    assert!(
        out.contains("<!-- REGEN: yidam routes\n") && out.contains("### On every occasion"),
        "the refusal does not print the block to paste: {out}"
    );
    assert!(!c.dirty(), "a refused migration wrote to the tree");
}

#[test]
fn a_missing_heading_refuses_and_prints_the_block() {
    let c = Corpus::new();
    with_old_agents(
        &c,
        "# AGENTS.md\n\nThirty-seven lines, no route list.\n",
        true,
    );
    let (ok, out) = c.run(&["migrate", "routes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("Before taking substantive action"), "{out}");
    assert!(out.contains("<!-- /REGEN -->"), "{out}");
    assert!(!c.dirty(), "a refused migration wrote to the tree");
}

/// Before a re-vendor there is nothing to render, and the working list is not traded for a
/// placeholder.
#[test]
fn migrate_routes_before_a_revendor_refuses() {
    let c = Corpus::new();
    with_old_agents(&c, OLD_AGENTS, false);
    let (ok, out) = c.run(&["migrate", "routes"]);
    assert!(!ok, "{out}");
    assert!(out.contains("yidam-vendor-update"), "{out}");
    assert_eq!(c.read("AGENTS.md"), OLD_AGENTS);
    assert!(!c.dirty(), "a refused migration wrote to the tree");
}

// ── scaffold ─────────────────────────────────────────────────────────────────

/// The scaffold's own `ci.yml` and `CLAUDE.md` as a derivation made before #1054 carries them:
/// the markers stripped, and an owner's job and section added outside where they would go.
fn with_old_scaffold(c: &Corpus) -> (String, String) {
    let unmark = |text: String| -> String {
        text.lines()
            .filter(|l| !l.contains("<!-- YIDAM:") && !l.contains("<!-- /YIDAM:"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n"
    };
    let ci = unmark(
        std::fs::read_to_string(repo_root().join("sadhana/github/workflows/ci.yml")).unwrap(),
    ) + "\n  ours:\n    runs-on: ubuntu-latest\n    steps:\n      - run: make ours\n";
    let claude =
        unmark(std::fs::read_to_string(repo_root().join("sadhana/root/CLAUDE.md")).unwrap())
            + "\n## Ours\n\nDomain notes.\n";
    std::fs::create_dir_all(c.path().join(".github/workflows")).unwrap();
    std::fs::create_dir_all(c.path().join(".claude")).unwrap();
    std::fs::write(c.path().join(".github/workflows/ci.yml"), &ci).unwrap();
    std::fs::write(c.path().join(".claude/CLAUDE.md"), &claude).unwrap();
    c.git(&["add", "-A"]);
    c.git(&[
        "commit",
        "-q",
        "-m",
        "vendor: the scaffold before its regions",
    ]);
    (ci, claude)
}

#[test]
fn a_dry_run_of_migrate_scaffold_names_the_regions_and_changes_nothing() {
    let c = Corpus::new();
    with_old_scaffold(&c);
    let (ok, out) = c.run(&["migrate", "--dry-run", "scaffold"]);
    assert!(ok, "{out}");
    assert!(out.contains("Would migrate"), "{out}");
    assert!(
        out.contains(".github/workflows/ci.yml  the region takes in: privacy, corpus"),
        "{out}"
    );
    assert!(out.contains("## The short version"), "{out}");
    assert!(out.contains("(2 file(s) across 2 file(s))"), "{out}");
    assert!(!c.dirty(), "a dry run wrote to the tree");
}

/// The migration puts back the scaffold's regions exactly: every line it wraps is the
/// template's, and every line outside is where the owner left it.
#[test]
fn migrate_scaffold_marks_the_template_s_part_and_nothing_else() {
    let c = Corpus::new();
    let (ci_before, claude_before) = with_old_scaffold(&c);
    let (ok, out) = c.run(&["migrate", "scaffold"]);
    assert!(ok, "{out}");
    assert!(out.contains("record: .yidam/migrations/scaffold-"), "{out}");

    let ci = c.read(".github/workflows/ci.yml");
    let claude = c.read(".claude/CLAUDE.md");
    // Stripping the markers again gives back what was there: nothing added, nothing lost.
    let unmark = |t: &str| {
        t.lines()
            .filter(|l| !l.trim().is_empty())
            .filter(|l| !l.contains("<!-- YIDAM:") && !l.contains("<!-- /YIDAM:"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    assert_eq!(unmark(&claude), unmark(&claude_before), "{claude}");
    let mut had: Vec<&str> = ci_before.lines().filter(|l| !l.trim().is_empty()).collect();
    let mut has: Vec<&str> = ci
        .lines()
        .filter(|l| {
            !l.trim().is_empty() && !l.contains("<!-- YIDAM:CI") && !l.contains("<!-- /YIDAM:CI")
        })
        .collect();
    had.sort_unstable();
    has.sort_unstable();
    assert_eq!(has, had, "the workflow gained or lost a line");

    // And the region holds the gate jobs and not the owner's.
    let region = ci
        .split("<!-- YIDAM:CI -->")
        .nth(1)
        .unwrap()
        .split("<!-- /YIDAM:CI -->")
        .next()
        .unwrap();
    assert!(
        region.contains("\n  privacy:\n") && region.contains("\n  corpus:\n"),
        "{ci}"
    );
    assert!(
        !region.contains("\n  ours:\n") && !region.contains("\n  detect:\n"),
        "{ci}"
    );
    let region = claude
        .split("<!-- YIDAM:CLAUDE -->")
        .nth(1)
        .unwrap()
        .split("<!-- /YIDAM:CLAUDE -->")
        .next()
        .unwrap();
    assert!(
        region.contains("## The short version") && !region.contains("## Ours"),
        "{claude}"
    );

    // Idempotent.
    let (ok, out) = c.run(&["migrate", "scaffold"]);
    assert!(ok, "{out}");
    assert!(out.contains("Nothing to migrate"), "{out}");
}

/// An owner's section between two of the template's refuses: wrapping it would hand it to the
/// re-vendor. hermetic-ch's `CLAUDE.md` has one.
#[test]
fn migrate_scaffold_refuses_an_owner_section_inside_the_template_s() {
    let c = Corpus::new();
    let (_, claude) = with_old_scaffold(&c);
    let claude = claude.replace(
        "## Before committing",
        "## Ours first\n\nx\n\n## Before committing",
    );
    std::fs::write(c.path().join(".claude/CLAUDE.md"), &claude).unwrap();
    c.git(&["commit", "-q", "-am", "edit: our section"]);
    let (ok, out) = c.run(&["migrate", "scaffold"]);
    assert!(!ok, "{out}");
    assert!(out.contains("`## Ours first`"), "{out}");
    assert!(out.contains("Nothing was written"), "{out}");
    assert!(!c.dirty(), "a refused migration wrote to the tree");
}
