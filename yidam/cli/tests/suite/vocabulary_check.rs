//! `yidam vocabulary --check` as a gate (#1518).
//!
//! The check printed its verdict and exited 0 either way, so nothing that read the exit code
//! — a hook, a CI step — could refuse a subject with it. And it judged a merge subject without
//! knowing a merge was being written: `lint --commits` exempts git's merge subjects at two
//! parents, and `--check` runs before the commit exists, so it read one parent and refused the
//! subject git itself wrote. These are the two halves, against a real repository.

use crate::common;

fn check(example: &common::Example, subject: &str) -> (String, i32) {
    let (stdout, stderr, code) = example.run(&["vocabulary", "--check", subject]);
    (format!("{stdout}{stderr}"), code)
}

#[test]
fn a_violation_exits_nonzero_and_a_legal_subject_does_not() {
    let example = common::Example::materialize("streamflow");
    let (out, code) = check(&example, "establish: the tailwater node");
    assert_eq!(code, 0, "{out}");
    for bad in ["did stuff", "lift: something", "fix(cli): a thing"] {
        let (out, code) = check(&example, bad);
        assert_eq!(code, 1, "{bad}: {out}");
    }
    // The verdict is the same in JSON, and so is the exit code.
    let (_, _, code) = example.run(&["vocabulary", "--check", "did stuff", "--format", "json"]);
    assert_eq!(code, 1);
    // Listing mode has no verdict to fail on.
    let (out, _, code) = example.run(&["vocabulary"]);
    assert_eq!(code, 0, "{out}");
}

#[test]
fn subjects_git_writes_pass_the_check() {
    let example = common::Example::materialize("streamflow");
    for subject in [
        "Revert \"establish: the tailwater node\"",
        "Reapply \"establish: the tailwater node\"",
        "fixup! establish: the tailwater node",
        "squash! establish: the tailwater node",
        "amend! establish: the tailwater node",
    ] {
        let (out, code) = check(&example, subject);
        assert_eq!(code, 0, "{subject}: {out}");
        assert!(out.contains("exempt:"), "{subject}: {out}");
    }
}

#[test]
fn a_merge_subject_passes_only_while_a_merge_is_in_progress() {
    let example = common::Example::materialize("streamflow");
    let dir = example.path();
    let subject = "Merge branch 'side'";

    // No merge in progress: an authored `Merge branch '…'` has no verb, exactly as
    // `lint --commits` reads it at one parent.
    let (out, code) = check(&example, subject);
    assert_eq!(code, 1, "{out}");

    common::git::git(&dir, &["checkout", "-q", "-b", "side"]);
    std::fs::write(dir.join("side.txt"), "side\n").unwrap();
    common::git::git(&dir, &["add", "side.txt"]);
    common::git::git(&dir, &["commit", "-q", "-m", "note: side"]);
    common::git::git(&dir, &["checkout", "-q", "-"]);
    common::git::git(&dir, &["merge", "-q", "--no-ff", "--no-commit", "side"]);

    let (out, code) = check(&example, subject);
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("exempt:"), "{out}");
    // An authored subject on the merge is still the author's, and still checked.
    let (out, code) = check(&example, "did stuff");
    assert_eq!(code, 1, "{out}");

    common::git::git(&dir, &["merge", "--abort"]);
    let (out, code) = check(&example, subject);
    assert_eq!(code, 1, "{out}");
}
