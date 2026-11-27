//! An output that carries the commit its step ran at (RFC-0052 §2.6 and §2.7).
//!
//! Each test adds one step to `examples/streamflow` that writes `$YIDAM_INPUT_COMMIT` into a
//! JSON file, the shape a consumer's schema requires of a pack. Undeclared, every re-run is
//! new bytes, so it commits and `doctor` warns. Declared under `stamps_input_commit`, a re-run
//! whose bytes match once the commit is masked writes a note, and `doctor` reports it as
//! declared.

use std::fmt::Write as _;

use crate::common;

use common::git::out as git;
use common::Example;

const STEP: &str = "stamp";
const OUTPUT: &str = ".yidam/computed/stamp.json";

/// `examples/streamflow` with one more step, `stamp`, owed at every run and writing
/// `{"corpus_commit": "<its input commit>"}`. `declared` adds the output to
/// `stamps_input_commit`.
fn with_stamp(declared: bool) -> Example {
    with_stamp_as(declared, "compute")
}

/// [`with_stamp`] under `verb`, so an epistemic verb puts the step on the proposal route.
fn with_stamp_as(declared: bool, verb: &str) -> Example {
    let e = Example::materialize("streamflow");
    let root = e.path();
    std::fs::write(
        root.join(".yidam/capabilities/stamp.sh"),
        "set -eu\nmkdir -p \"$YIDAM_OUT/.yidam/computed\"\n\
         printf '{\"corpus_commit\": \"%s\"}\\n' \"$YIDAM_INPUT_COMMIT\" \
         > \"$YIDAM_OUT/.yidam/computed/stamp.json\"\n",
    )
    .unwrap();
    let manifest = root.join(".yidam/capabilities.toml");
    let mut text = std::fs::read_to_string(&manifest).unwrap();
    let _ = write!(
        text,
        "\n[capability.stamp]\nkind   = \"calculator\"\n\
         run    = [\"sh\", \".yidam/capabilities/stamp.sh\"]\n\
         reads  = [\".yidam/capabilities/**\"]\nwrites = [\".yidam/computed/**\"]\n\
         verb   = \"{verb}\"\nageing_days = 0\n{}",
        match declared {
            true => format!("stamps_input_commit = [\"{OUTPUT}\"]\n"),
            false => String::new(),
        }
    );
    std::fs::write(&manifest, text).unwrap();
    git(&root, &["add", "-A"]);
    git(
        &root,
        &["commit", "-qm", "fix: a step that stamps its input commit"],
    );
    e
}

/// Run the step and hand back its entry in the JSON report.
fn run(e: &Example) -> serde_json::Value {
    let (out, err, code) = e.run(&["run", STEP, "--format", "json"]);
    assert_eq!(code, 0, "the step failed:\n{out}{err}");
    let report: serde_json::Value = serde_json::from_str(&out).unwrap();
    report["steps"]
        .as_array()
        .expect("a plan")
        .iter()
        .find(|s| s["step"] == STEP)
        .cloned()
        .unwrap_or_else(|| panic!("`{STEP}` is not in the plan:\n{out}"))
}

/// Rewrite the step's script with `edit` and commit it.
fn edit_script(e: &Example, edit: impl Fn(String) -> String) {
    let script = e.path().join(".yidam/capabilities/stamp.sh");
    std::fs::write(&script, edit(std::fs::read_to_string(&script).unwrap())).unwrap();
    git(&e.path(), &["commit", "-qam", "fix: edit the stamp step"]);
}

/// The `run-output-stamps-input-commit` entry in a JSON `doctor` report.
fn doctor(e: &Example) -> serde_json::Value {
    git(
        &e.path(),
        &[
            "restore",
            "--source=HEAD",
            "--worktree",
            "--staged",
            "--",
            ".yidam/",
        ],
    );
    let (out, err, _) = e.run(&["doctor", "--format", "json"]);
    let v: serde_json::Value =
        serde_json::from_str(&out).unwrap_or_else(|x| panic!("not JSON: {x}\n{out}{err}"));
    v["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"] == "run-output-stamps-input-commit")
        .cloned()
        .unwrap_or_else(|| panic!("no stamp check:\n{out}"))
}

#[test]
fn a_declared_stamp_is_masked_so_a_rerun_writes_a_note_and_no_commit() {
    let e = with_stamp(true);
    let first = run(&e);
    assert!(
        first["committed"].is_object(),
        "the first run landed nothing: {first}"
    );
    let landed = git(&e.path(), &["rev-parse", "HEAD"]);
    let held = git(&e.path(), &["show", &format!("HEAD:{OUTPUT}")]);

    let second = run(&e);
    assert_eq!(second["outcome"], "ran", "{second}");
    assert!(second["committed"].is_null(), "{second}");
    assert!(second["noted"].is_object(), "{second}");
    assert_eq!(
        git(&e.path(), &["rev-parse", "HEAD"]),
        landed,
        "a re-run whose only news was the commit moved the branch"
    );
    assert_eq!(
        git(&e.path(), &["show", &format!("HEAD:{OUTPUT}")]),
        held,
        "the committed output is not the one the first run wrote"
    );

    let c = doctor(&e);
    assert_eq!(c["verdict"], "ok", "{c}");
    let detail = c["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(OUTPUT) && detail.contains("declared"),
        "{c}"
    );
}

#[test]
fn an_undeclared_stamp_commits_every_rerun_and_is_warned_on() {
    let e = with_stamp(false);
    run(&e);
    let landed = git(&e.path(), &["rev-parse", "HEAD"]);

    let second = run(&e);
    assert!(second["committed"].is_object(), "{second}");
    assert!(second["noted"].is_null(), "{second}");
    assert_ne!(git(&e.path(), &["rev-parse", "HEAD"]), landed);

    let c = doctor(&e);
    assert_eq!(c["verdict"], "warn", "{c}");
    assert!(
        c["detail"].as_str().unwrap_or_default().contains(OUTPUT),
        "{c}"
    );
}

#[test]
fn a_declared_stamp_that_moved_otherwise_still_commits() {
    let e = with_stamp(true);
    run(&e);
    let script = e.path().join(".yidam/capabilities/stamp.sh");
    let text = std::fs::read_to_string(&script)
        .unwrap()
        .replace("corpus_commit", "pack_commit");
    std::fs::write(&script, text).unwrap();
    git(
        &e.path(),
        &["commit", "-qam", "fix: rename the stamp's key"],
    );
    let landed = git(&e.path(), &["rev-parse", "HEAD"]);

    let second = run(&e);
    assert!(second["committed"].is_object(), "{second}");
    assert_ne!(git(&e.path(), &["rev-parse", "HEAD"]), landed);
}

/// §2.7: a declaration whose committed output no longer carries the commit masks nothing, and
/// `doctor` says so with the path named.
#[test]
fn a_declared_stamp_the_output_no_longer_carries_is_warned_on_as_stale() {
    let e = with_stamp(true);
    run(&e);
    edit_script(&e, |t| t.replace("\"$YIDAM_INPUT_COMMIT\"", "\"none\""));
    let after = run(&e);
    assert!(after["committed"].is_object(), "{after}");

    let c = doctor(&e);
    assert_eq!(c["verdict"], "warn", "{c}");
    let detail = c["detail"].as_str().unwrap_or_default();
    assert!(
        detail.contains(OUTPUT) && detail.contains("masks nothing"),
        "{c}"
    );
}

/// §2.7 on the proposal route with no pending proposal: the parent is HEAD, which holds the
/// merged proposal, and a re-run whose only news is the commit opens no second proposal.
#[test]
fn a_declared_stamp_on_the_proposal_route_opens_no_second_proposal() {
    let e = with_stamp_as(true, "establish");
    let first = run(&e);
    let proposal = first["committed"]["branch"]
        .as_str()
        .unwrap_or_else(|| panic!("the first run proposed nothing: {first}"))
        .to_string();
    git(&e.path(), &["merge", "-q", "--ff-only", &proposal]);
    let head = git(&e.path(), &["rev-parse", "HEAD"]);

    let second = run(&e);
    assert_eq!(second["route"], "proposal", "{second}");
    assert!(second["committed"].is_null(), "{second}");
    assert!(second["noted"].is_object(), "{second}");
    assert_eq!(git(&e.path(), &["rev-parse", "HEAD"]), head);
    assert_eq!(
        git(&e.path(), &["branch", "--list", "propose/*"])
            .lines()
            .count(),
        1,
        "a re-run whose only news was the commit opened a proposal"
    );
}
