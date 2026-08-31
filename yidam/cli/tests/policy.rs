//! `yidam policy`, end to end.
//!
//! The unit tests hold the engine to the contract and `policy_equivalence` holds the default
//! rules to the Rust guards they will replace. This holds the *command* to what somebody at a
//! terminal needs from it: what the rules are, what they say about one situation, and whether
//! they still say what was decided.
//!
//! Assertions are on **exit codes** wherever the command is a gate. `policy check` that prints
//! a disallowed builtin and returns 0 is broken in the way that matters.
//!
//! No repository is required by any of these, and one test pins that: the compiled-in default
//! is a complete rule set, and somebody working out why a push was refused should not need a
//! checkout to ask.

use std::path::Path;
use std::process::Command;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn ok(&self) -> &Run {
        assert_eq!(
            self.code, 0,
            "expected success\n{}{}",
            self.stdout, self.stderr
        );
        self
    }
    fn failed(&self) -> &Run {
        assert_ne!(
            self.code, 0,
            "expected nonzero\n{}{}",
            self.stdout, self.stderr
        );
        self
    }
    fn said(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
    fn json(&self) -> serde_json::Value {
        serde_json::from_str(&self.stdout)
            .unwrap_or_else(|e| panic!("stdout is not JSON ({e}):\n{}", self.stdout))
    }
}

fn run(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .expect("running yidam");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn run_stdin(dir: &Path, args: &[&str], stdin: &str) -> Run {
    use std::io::Write;
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("running yidam");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A directory with a `.yidam/policy/` holding `files`.
fn repo(files: &[(&str, &str)]) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join(".yidam/policy");
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    tmp
}

// ── check ────────────────────────────────────────────────────────────────────

#[test]
fn check_reports_every_decision_as_inherited_when_nothing_is_overridden() {
    let tmp = tempfile::tempdir().unwrap();
    let said = run(tmp.path(), &["policy", "check"]).ok().said();
    for decision in ["disclose/at_rest", "disclose/record", "disclose/derived"] {
        assert!(said.contains(decision), "{decision} unlisted:\n{said}");
    }
    assert!(
        said.contains("Every decision is the one this binary shipped"),
        "{said}"
    );
}

/// The compiled-in default is a complete rule set, so the command answers outside a repository.
#[test]
fn check_needs_no_repository() {
    let tmp = tempfile::tempdir().unwrap();
    run(tmp.path(), &["policy", "check"]).ok();
}

#[test]
fn check_names_an_override_and_the_file_it_came_from() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
    )]);
    let said = run(tmp.path(), &["policy", "check"]).ok().said();
    assert!(said.contains("local"), "{said}");
    assert!(said.contains("record.rego"), "{said}");
    // Reported, never treated as a defect: a repository is entitled to decide.
    assert!(said.contains("supersede"), "{said}");
}

/// **`check` compares text and says so.** Whether a local rule is more permissive is a question
/// about every possible input, and claiming to have answered it would be worse than silence.
#[test]
fn check_does_not_claim_to_know_which_way_an_override_moved() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
    )]);
    let said = run(tmp.path(), &["policy", "check"]).ok().said();
    assert!(said.contains("compares text"), "{said}");
}

/// A builtin this build does not carry is found *here* rather than at the moment a decision is
/// needed — which is the whole reason the scan exists.
#[test]
fn check_refuses_a_policy_that_names_a_builtin_this_build_does_not_carry() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": [], \"x\": \
         http.send({})}\n",
    )]);
    let r = run(tmp.path(), &["policy", "check"]);
    r.failed();
    assert!(r.said().contains("http.send"), "{}", r.said());
}

/// A comment naming a forbidden builtin is prose, not a call. The shipped policy contains
/// exactly such a comment, so this is the case that keeps the scan from tripping over its own
/// documentation.
#[test]
fn check_does_not_mistake_a_comment_for_a_call() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\n# never call http.send here\ndecision := {\"allow\": \
         true, \"deny\": []}\n",
    )]);
    run(tmp.path(), &["policy", "check"]).ok();
}

#[test]
fn check_json_carries_the_origin_of_every_decision() {
    let tmp = tempfile::tempdir().unwrap();
    let v = run(tmp.path(), &["policy", "check", "--format", "json"])
        .ok()
        .json();
    assert_eq!(v["ok"], serde_json::json!(true));
    let rows = v["decisions"].as_array().expect("decisions is an array");
    assert_eq!(rows.len(), 3, "{v:#?}");
    assert!(rows.iter().all(|r| r["origin"] == "inherited"), "{v:#?}");
}

// ── eval ─────────────────────────────────────────────────────────────────────

const PRIVATE_RECORD: &str = r#"{
  "repo": {"private_paths": [{"path": "dossier", "holds_content": true}]},
  "subject": {"rel": "dossier/evidence.md", "redistributable": true}
}"#;

#[test]
fn eval_reads_its_input_from_stdin_and_refuses_with_a_reason_somebody_can_act_on() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run_stdin(
        tmp.path(),
        &["policy", "eval", "--decision", "disclose/record"],
        PRIVATE_RECORD,
    );
    r.ok();
    let said = r.said();
    assert!(said.contains("refuse"), "{said}");
    assert!(said.contains("private-paths"), "{said}");
    assert!(said.contains("outlives the access"), "{said}");
}

#[test]
fn eval_reads_its_input_from_a_file() {
    let tmp = tempfile::tempdir().unwrap();
    let p = tmp.path().join("in.json");
    std::fs::write(&p, PRIVATE_RECORD).unwrap();
    let said = run(
        tmp.path(),
        &[
            "policy",
            "eval",
            "--decision",
            "disclose/record",
            "--input",
            p.to_str().unwrap(),
        ],
    )
    .ok()
    .said();
    assert!(said.contains("refuse"), "{said}");
}

/// `--explain` names the rule, which is what makes a refusal debuggable rather than final.
#[test]
fn eval_explain_names_the_rule_that_fired() {
    let tmp = tempfile::tempdir().unwrap();
    let said = run_stdin(
        tmp.path(),
        &[
            "policy",
            "eval",
            "--decision",
            "disclose/record",
            "--explain",
        ],
        PRIVATE_RECORD,
    )
    .ok()
    .said();
    assert!(said.contains("[private-path]"), "{said}");
}

#[test]
fn eval_json_carries_the_verdict_and_every_reason() {
    let tmp = tempfile::tempdir().unwrap();
    let v = run_stdin(
        tmp.path(),
        &[
            "policy",
            "eval",
            "--decision",
            "disclose/record",
            "--format",
            "json",
        ],
        PRIVATE_RECORD,
    )
    .ok()
    .json();
    assert_eq!(v["allow"], serde_json::json!(false));
    assert_eq!(v["deny"][0]["rule"], "private-path");
}

#[test]
fn eval_names_an_unknown_decision_rather_than_allowing_it() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run_stdin(
        tmp.path(),
        &["policy", "eval", "--decision", "disclose/nope"],
        "{}",
    );
    r.failed();
    assert!(r.said().contains("is not a decision"), "{}", r.said());
}

/// A malformed input is refused rather than answered. A decision made about a document nobody
/// could parse is not a decision.
#[test]
fn eval_refuses_an_input_that_is_not_json() {
    let tmp = tempfile::tempdir().unwrap();
    run_stdin(
        tmp.path(),
        &["policy", "eval", "--decision", "disclose/record"],
        "not json",
    )
    .failed();
}

// ── test ─────────────────────────────────────────────────────────────────────

#[test]
fn test_runs_the_shipped_cases_and_they_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let r = run(tmp.path(), &["policy", "test"]);
    r.ok();
    assert!(r.said().contains("0 failed"), "{}", r.said());
    assert!(r.said().contains("0 changed"), "{}", r.said());
}

/// **The signal `policy check` cannot give.** An override that changes what a decision does
/// shows up as the inherited expectations it no longer meets — and does not fail, because the
/// repository is entitled to decide.
#[test]
fn test_reports_an_override_as_changed_expectations_rather_than_as_failures() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
    )]);
    let r = run(tmp.path(), &["policy", "test"]);
    r.ok();
    let said = r.said();
    assert!(said.contains("changed"), "{said}");
    assert!(said.contains("0 failed"), "{said}");
    // The decisions nobody overrode still hold, so their cases still pass.
    assert!(said.contains("test_no_overlap_permits"), "{said}");
}

/// A repository's own case that fails is a failure, and it gates.
#[test]
fn test_fails_on_a_local_case_that_does_not_hold() {
    let tmp = repo(&[(
        "mine_test.rego",
        "package yidam.mine_test\n\ntest_something if { false }\n",
    )]);
    let r = run(tmp.path(), &["policy", "test"]);
    r.failed();
    assert!(r.said().contains("test_something"), "{}", r.said());
}

/// **Undefined is a failure, not a skip.** A body that did not hold asserted nothing, and
/// counting it as a pass is how a suite comes to cover less than it claims.
#[test]
fn test_treats_an_undefined_case_as_a_failure() {
    let tmp = repo(&[(
        "mine_test.rego",
        "package yidam.mine_test\n\ntest_undefined if { input.nothing == 1 }\n",
    )]);
    let r = run(tmp.path(), &["policy", "test"]);
    r.failed();
    assert!(r.said().contains("undefined"), "{}", r.said());
}

#[test]
fn test_json_separates_failures_from_overridden_expectations() {
    let tmp = repo(&[(
        "record.rego",
        "package yidam.disclose.record\n\ndecision := {\"allow\": true, \"deny\": []}\n",
    )]);
    let v = run(tmp.path(), &["policy", "test", "--format", "json"])
        .ok()
        .json();
    assert_eq!(v["failed"], serde_json::json!(0));
    assert!(v["changed_by_override"].as_u64().unwrap() > 0, "{v:#?}");
}
