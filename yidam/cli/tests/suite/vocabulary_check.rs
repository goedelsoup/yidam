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
fn a_wrapped_subject_is_checked_by_what_it_holds() {
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
        assert!(
            out.contains("checked: \"establish: the tailwater node\""),
            "{subject}: {out}"
        );
    }
    // The wrapper is no way past the gate.
    for subject in ["Revert \"wip\"", "fixup! did stuff"] {
        let (out, code) = check(&example, subject);
        assert_eq!(code, 1, "{subject}: {out}");
    }
    // `--fixup=amend:` keeps the body's first line, and that is the line judged.
    let (out, code) = check(&example, "amend! establish: the tailwater node\n\nwip\n");
    assert_eq!(code, 1, "{out}");
}

#[test]
fn a_commit_touching_only_object_paths_passes() {
    let example = common::Example::materialize("streamflow");
    let dir = example.path();
    let config = dir.join(".yidam/config.toml");
    let mut text = std::fs::read_to_string(&config).unwrap_or_default();
    text.push_str("\n[object]\npaths = [\"web/**\"]\n");
    std::fs::create_dir_all(config.parent().unwrap()).unwrap();
    std::fs::write(&config, text).unwrap();

    // Nothing staged: the subject is the author's, as `lint --commits` reads a commit with
    // corpus paths.
    let (out, code) = check(&example, "did stuff");
    assert_eq!(code, 1, "{out}");

    std::fs::create_dir_all(dir.join("web")).unwrap();
    std::fs::write(dir.join("web/index.html"), "<p>hi</p>\n").unwrap();
    common::git::git(&dir, &["add", "web/index.html"]);
    let (out, code) = check(&example, "did stuff");
    assert_eq!(code, 0, "{out}");
    assert!(out.contains("exempt:"), "{out}");

    // One corpus path makes it corpus work again.
    common::git::git(&dir, &["add", ".yidam/config.toml"]);
    let (out, code) = check(&example, "did stuff");
    assert_eq!(code, 1, "{out}");
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
