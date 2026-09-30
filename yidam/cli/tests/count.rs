//! `yidam count` — a number in prose, checked against the graph (#1071, RFC-0043).
//!
//! Against `examples/streamflow` rather than a synthetic fixture, for the reason
//! `query_goldens` gives about the same corpus: the generator's answer *is* a query result,
//! and a fixture built to make the query easy would be checking the engine against itself.
//! Three classes and eight instances, small enough that every number below was counted by
//! hand.
//!
//! Its own binary rather than rows in `regen_check`, because these cases need a corpus — the
//! reports fixture that suite stages has no `.yidam/corpus/`, and `count` over it can only
//! ever answer nothing.
//!
//! What the cases are for, in one line each: a number written into prose is the number the
//! read form prints; an inline block stays inline; the gate fails before `regen` and passes
//! after; a block naming no query is reported rather than guessed at; and a query that does
//! not typecheck refuses its block instead of publishing a `0`.

use std::path::Path;
use std::process::Command;

mod common;

use common::{repo_root, tracked_under};

const EXAMPLE: &str = "examples/streamflow/";

/// `examples/streamflow` as a standalone repository, plus `body` as `NOTES.md`.
///
/// From `git ls-files`, matching every other suite here: a directory walk picks up
/// `.DS_Store` and any local scratch, and the test would then be measuring the maintainer's
/// working directory.
///
/// `NOTES.md` is committed, not merely written. `count` discovers its blocks through
/// `tracked::list`, so an untracked file holds no blocks as far as this generator is
/// concerned — which is the behaviour, not an accident of the fixture.
fn stage(body: &str) -> tempfile::TempDir {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in tracked_under(&root, EXAMPLE) {
        let to = dir.path().join(tracked.strip_prefix(EXAMPLE).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    std::fs::write(dir.path().join("NOTES.md"), body).unwrap();
    let git = |args: &[&str]| common::git::git_at(dir.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "count@yidam.test"]);
    git(&["config", "user.name", "Count"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: streamflow and a note"]);
    dir
}

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn notes(root: &Path) -> String {
    std::fs::read_to_string(root.join("NOTES.md")).unwrap()
}

/// A document holding both block shapes, each seeded with a wrong number.
///
/// Seeded wrong on purpose: a block that already holds the right answer cannot tell a
/// generator that wrote it from one that did nothing.
const BOTH_SHAPES: &str = "\
# Notes

The corpus holds <!-- REGEN: yidam count concept -->99<!-- /REGEN --> concepts.

Reaches:

<!-- REGEN: yidam count reach -->
99
<!-- /REGEN -->
";

/// The whole point of the feature: prose carries the graph's number, not a remembered one.
///
/// Both halves asserted — the literal, because four concepts and two reaches is a fact about
/// the corpus a reader can check by listing `.yidam/corpus/`; and the equality with what
/// `yidam count <query>` prints, because *that* is the invariant the generator owes. A drift
/// between the two is the defect #1071 describes, one layer in.
#[test]
fn the_number_in_prose_is_the_number_the_query_answers() {
    let tmp = stage(BOTH_SHAPES);
    let root = tmp.path();

    assert_eq!(run(root, &["count", "concept"]).stdout.trim(), "4");
    assert_eq!(run(root, &["count", "reach"]).stdout.trim(), "2");

    let r = run(root, &["regen"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);

    let after = notes(root);
    assert!(
        after.contains("<!-- REGEN: yidam count concept -->4<!-- /REGEN -->"),
        "{after}"
    );
    assert!(
        after.contains("<!-- REGEN: yidam count reach -->\n2\n<!-- /REGEN -->"),
        "{after}"
    );
}

/// RFC-0043's inline rule, where it is actually visible: the sentence is still a sentence.
///
/// This is the acceptance criterion the issue names. The block form was already supported,
/// and a `count` that could only write one would put the number on a line of its own —
/// which is not a sentence saying how many districts there are.
#[test]
fn an_inline_block_leaves_its_sentence_unbroken() {
    let tmp = stage(BOTH_SHAPES);
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert!(
        notes(tmp.path()).lines().any(|l| l
            == "The corpus holds <!-- REGEN: yidam count concept -->4<!-- /REGEN --> concepts."),
        "{}",
        notes(tmp.path())
    );
}

/// Both arms, per `regen_check`'s rule: a gate is only a gate if it fails.
#[test]
fn a_stale_count_gates_and_a_fresh_one_does_not() {
    let tmp = stage(BOTH_SHAPES);
    let root = tmp.path();

    let before = run(root, &["regen", "--check"]);
    assert_eq!(before.code, 1, "{}", before.stdout);
    assert!(
        before.stdout.contains("(count concept)"),
        "{}",
        before.stdout
    );
    assert!(before.stdout.contains("(count reach)"), "{}", before.stdout);
    assert_eq!(notes(root), BOTH_SHAPES, "--check writes nothing");

    assert_eq!(run(root, &["regen"]).code, 0);
    let after = run(root, &["regen", "--check"]);
    assert_eq!(after.code, 0, "{}", after.stdout);
}

/// A block naming no query is reported, never guessed at.
///
/// The registration hazard this case exists for: `count` is the first generator whose
/// command carries an argument, and putting its bare name in the claimed set would have
/// turned this block — which no run can ever write — from reported into silently claimed.
/// That is precisely the defect #1062's gate exists to close, so a regression here is
/// invisible unless something asserts it.
#[test]
fn a_block_naming_no_query_is_unclaimed() {
    let tmp = stage("# Notes\n\n<!-- REGEN: yidam count -->0<!-- /REGEN -->\n");
    let r = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(r.code, 1, "{}", r.stdout);
    assert!(
        r.stdout.contains("name a command no generator writes"),
        "{}",
        r.stdout
    );
    assert!(r.stdout.contains("(yidam count)"), "{}", r.stdout);
    // The remedy has to show the form that *would* be written, or it tells the reader the
    // command is unknown while listing it.
    assert!(r.stdout.contains("count <query>"), "{}", r.stdout);
}

/// A query that does not typecheck keeps its block and fails the command.
///
/// Three things at once, and the third is the one worth having: the run fails, it says which
/// block and why — and the block still holds what it held. A gate whose failure mode is to
/// publish a plausible `0` is worse than no gate, because the number then reads as measured.
///
/// The good block beside it is written all the same: one bad query must not cost the rest of
/// the document its refresh, or a single typo silently freezes every other number.
#[test]
fn a_query_that_does_not_typecheck_publishes_no_number() {
    let tmp = stage(
        "# Notes\n\nBad: <!-- REGEN: yidam count gauge -->keep me<!-- /REGEN -->.\n\
         \nGood: <!-- REGEN: yidam count concept -->99<!-- /REGEN -->.\n",
    );
    let root = tmp.path();
    let r = run(root, &["regen"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stderr.contains("NOTES.md"), "{}", r.stderr);
    assert!(r.stderr.contains("(gauge)"), "{}", r.stderr);
    assert!(r.stderr.contains("unknown-class"), "{}", r.stderr);

    let after = notes(root);
    assert!(
        after.contains("<!-- REGEN: yidam count gauge -->keep me<!-- /REGEN -->"),
        "the refused block kept its content: {after}"
    );
    assert!(
        after.contains("<!-- REGEN: yidam count concept -->4<!-- /REGEN -->"),
        "the good block beside it was still written: {after}"
    );
}

/// The read form refuses too, and says so rather than printing a number.
#[test]
fn the_read_form_refuses_an_unknown_class() {
    let tmp = stage("# Notes\n");
    let r = run(tmp.path(), &["count", "gauge"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert!(r.stdout.contains("unknown-class"), "{}", r.stdout);
    assert!(
        !r.stdout.lines().any(|l| l.trim() == "0"),
        "a rejected query has not counted zero of anything: {}",
        r.stdout
    );
}

/// A block shown in a fence is prose about the tool, not a block the tool owns.
///
/// `docs/rfcs/0043-inline-regen-and-count.md` and `docs/upgrading.md` both show one, and
/// regenerating a shown example would rewrite the documentation into output.
#[test]
fn a_block_shown_in_a_fence_is_not_regenerated() {
    let shown =
        "# Notes\n\n```markdown\n<!-- REGEN: yidam count concept -->0<!-- /REGEN -->\n```\n";
    let tmp = stage(shown);
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    assert_eq!(notes(tmp.path()), shown);
    assert_eq!(run(tmp.path(), &["regen", "--check"]).code, 0);
}

/// One query that is a textual prefix of another does not overwrite its block.
///
/// The hazard `TheLocatorComparesTheWholeCommand` rules out in the model, and the reason
/// #1094 had to land before this: while every command was a bare generator name, no two
/// stood in this relation and the old prefix search was harmless. `yidam count reach` is a prefix of
/// `yidam count reach[regulated=yes]`, and a corpus can now write that pair by accident.
///
/// `0` for the second is the right answer and not an empty one — `regulated` holds
/// `"yes — inherited from upstream"`, the exact-match case `query_goldens` records as
/// `exact-matches-nothing`. It is used here precisely because it differs from the first.
#[test]
fn a_query_that_prefixes_another_does_not_clobber_it() {
    let tmp = stage(
        "# Notes\n\nAll: <!-- REGEN: yidam count reach -->9<!-- /REGEN -->\n\
         Regulated: <!-- REGEN: yidam count reach[regulated=yes] -->9<!-- /REGEN -->\n",
    );
    assert_eq!(run(tmp.path(), &["regen"]).code, 0);
    let after = notes(tmp.path());
    assert!(
        after.contains("All: <!-- REGEN: yidam count reach -->2<!-- /REGEN -->"),
        "{after}"
    );
    assert!(
        after.contains("<!-- REGEN: yidam count reach[regulated=yes] -->0<!-- /REGEN -->"),
        "the longer command kept its own answer: {after}"
    );
}

/// Two blocks in one document asking the same question are both written.
///
/// The second hazard: `update_regen` used to rewrite the **first** match only, so
/// *"44 districts … of those 44 districts"* — the shape #1071 reports — left the second stale
/// forever, and `--check` could not report it, because a block nothing writes is a block
/// nothing records. Both arms asserted, since a `--check` that passes over an unwritten block
/// is the silent half of that defect.
#[test]
fn two_blocks_asking_the_same_question_are_both_written() {
    let tmp = stage(
        "# Notes\n\nThere are <!-- REGEN: yidam count reach -->9<!-- /REGEN --> reaches, and of \
         those <!-- REGEN: yidam count reach -->9<!-- /REGEN --> none is unregulated.\n",
    );
    let root = tmp.path();
    assert_eq!(run(root, &["regen", "--check"]).code, 1);
    assert_eq!(run(root, &["regen"]).code, 0);
    assert_eq!(
        notes(root)
            .matches("<!-- REGEN: yidam count reach -->2<!-- /REGEN -->")
            .count(),
        2,
        "{}",
        notes(root)
    );
    assert_eq!(run(root, &["regen", "--check"]).code, 0);
}
