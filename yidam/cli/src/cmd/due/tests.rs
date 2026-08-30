//! What `yidam due` must not do, mostly.
//!
//! The failure this command is one wrong line away from is reading as a second `doctor` — a
//! report whose numbers a person learns to treat as defects. Most of what is asserted here is
//! that a clock nobody set never comes due, that being owed never fails a run, and that the
//! sentence saying so is on every rendering including the quiet one.

use super::*;
use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed");
}

/// Commit at a fixed date. Every clock here is read against a `today` this file supplies, so
/// nothing in it can depend on the day it runs.
fn commit(dir: &Path, day: &str, msg: &str) {
    git(dir, &["add", "-A"]);
    let stamp = format!("{day}T00:00:00Z");
    let ok = Command::new("git")
        .current_dir(dir)
        .args(["commit", "-q", "-m", msg])
        .env("GIT_AUTHOR_DATE", &stamp)
        .env("GIT_COMMITTER_DATE", &stamp)
        .status()
        .unwrap()
        .success();
    assert!(ok, "commit failed");
}

fn node(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(".yidam/corpus").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// A derived repository with one node and one commit, and no configuration at all.
fn repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.com"]);
    git(root, &["config", "user.name", "T"]);
    node(root, "concept/a.yml", "class: concept\nlabel: A\n");
    commit(root, "2026-01-01", "establish: a");
    tmp
}

fn config(root: &Path, body: &str) {
    std::fs::create_dir_all(root.join(".yidam")).unwrap();
    std::fs::write(root.join(".yidam/config.toml"), body).unwrap();
}

/// `2026-03-01`, the day every clock here is read on.
fn today() -> i64 {
    crate::dates::days_from_civil_str("2026-03-01").unwrap()
}

fn clocks(root: &Path) -> Vec<Clock> {
    let cfg = crate::config::load_yidam_config(root).unwrap();
    read_clocks(root, &cfg.due, today())
}

fn find<'a>(clocks: &'a [Clock], id: &str) -> &'a Clock {
    clocks.iter().find(|c| c.id == id).expect("clock present")
}

// ── an unset clock ────────────────────────────────────────────────────────────

/// The default state of every derived repository, and it must be silent about being owed.
#[test]
fn a_corpus_that_declared_nothing_is_owed_nothing() {
    let tmp = repo();
    let all = clocks(tmp.path());
    assert_eq!(all.len(), 4, "four clocks: {all:?}");
    for c in &all {
        assert_eq!(
            c.state,
            State::Undeclared,
            "{} came due against an interval nobody set",
            c.id
        );
        assert_eq!(c.overdue, 0, "{}", c.id);
    }
    assert_eq!(DueReport::new(all, false).due, 0);
}

/// An unset clock still reports its measurement. A clock nobody set is not one that reads
/// zero, and the number is what a person needs to pick the interval.
#[test]
fn an_unset_clock_still_says_what_it_measured() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/b.yml",
        "class: concept\nlabel: B\ndescription: it is `[open]`\n",
    );
    commit(root, "2026-02-01", "open: whether b holds");

    let all = clocks(root);
    let q = find(&all, "questions");
    assert!(q.detail.contains('1'), "{}", q.detail);
    // And it names the key that would turn it into a clock.
    assert_eq!(
        q.remedy.as_deref(),
        Some("declare `[due] questions_after` in .yidam/config.toml")
    );
}

// ── the index clock ───────────────────────────────────────────────────────────

/// An index that was never built, against a corpus that asked for one.
#[test]
fn a_corpus_that_wants_an_index_and_has_none_is_due_one() {
    let tmp = repo();
    config(tmp.path(), "[due]\nindex_after = 5\n");
    let all = clocks(tmp.path());
    let index = find(&all, "index");
    assert_eq!(index.state, State::Due);
    assert!(
        index
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("index-build"),
        "{index:?}"
    );
}

// ── the questions clock ───────────────────────────────────────────────────────

/// The clock this issue had no machinery for, end to end: an open question whose residence
/// passes the declared interval.
#[test]
fn a_question_past_its_declared_residence_is_due() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/b.yml",
        "class: concept\nlabel: B\ndescription: it is `[open]`\n",
    );
    commit(root, "2026-01-02", "open: whether b holds");
    for i in 3..8 {
        node(
            root,
            "concept/a.yml",
            &format!("class: concept\nlabel: A{i}\n"),
        );
        commit(root, &format!("2026-01-0{i}"), "revise: a");
    }

    config(root, "[due]\nquestions_after = 3\n");
    let due = find(&clocks(root), "questions").clone();
    assert_eq!(due.state, State::Due, "{due:?}");
    assert_eq!(due.overdue, 1);

    // And the same corpus against an interval it has not reached.
    config(root, "[due]\nquestions_after = 500\n");
    let held = find(&clocks(root), "questions").clone();
    assert_eq!(held.state, State::Ok, "{held:?}");
    assert_eq!(held.overdue, 0);
}

/// Answering an open question is a resolution event, so the remedy must not name `propose`.
///
/// RFC-0020 and `cmd/sangha.rs` both draw that line, and #289 proposed crossing it — "under
/// E4, proposes the commits that would discharge them" is true of exactly one of the four
/// clocks. A remedy pointing a person at `yidam propose` here would be telling them a tool
/// can close a question, which it deliberately cannot.
#[test]
fn the_remedy_for_an_overdue_question_is_not_a_proposal() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/b.yml",
        "class: concept\nlabel: B\ndescription: it is `[open]`\n",
    );
    commit(root, "2026-01-02", "open: whether b holds");
    config(root, "[due]\nquestions_after = 1\n");

    let q = find(&clocks(root), "questions").clone();
    assert_eq!(q.state, State::Due, "{q:?}");
    let remedy = q.remedy.unwrap_or_default();
    assert!(
        !remedy.contains("propose"),
        "a question is not closed by a drafted commit: {remedy}"
    );
}

/// A node whose YAML does not parse still carries its question.
///
/// `open-questions` finds the tag in the prose and lists it; a local walk here that gave up
/// on the parse would not, and the clock would count a set the command it names in its own
/// remedy does not list. That was the near-miss — this walk was reimplemented before it was
/// shared — and the shape has cost this repository three implementations of one predicate
/// before.
#[test]
fn a_node_that_does_not_parse_is_still_counted() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/broken.yml",
        "class: concept\nlabel: B\n\tdescription: it is `[open]`\n",
    );
    commit(root, "2026-01-02", "open: whether b holds");

    let listed =
        crate::cmd::corpus::open_questions_data(root, &crate::paths::yidam_corpus_dir(root))
            .open_questions
            .len();
    assert_eq!(listed, 1, "the fixture's node is not read as a question");

    config(root, "[due]\nquestions_after = 1\n");
    let q = find(&clocks(root), "questions").clone();
    assert_eq!(q.state, State::Due, "{q:?}");
    assert!(q.detail.contains("of 1 "), "{}", q.detail);
}

// ── the phases clock ──────────────────────────────────────────────────────────

/// A `phase/*` ref that has not settled, older than the declared interval.
#[test]
fn a_phase_in_flight_past_its_interval_is_due() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "phase/survey"]);
    node(root, "concept/c.yml", "class: concept\nlabel: C\n");
    commit(root, "2026-01-10", "scope: the survey");
    git(root, &["checkout", "-q", "main"]);
    config(root, "[due]\nphases_after = 30\n");

    // 2026-01-10 to 2026-03-01 is 50 days.
    let p = find(&clocks(root), "phases").clone();
    assert_eq!(p.state, State::Due, "{p:?}");
    assert_eq!(p.overdue, 1);
    assert!(p.detail.contains("phase/survey"), "{}", p.detail);
}

/// A standing elector position is not bounded work and has no settlement to be late for.
///
/// It is `ma/*`'s whole purpose to sit ahead of the baseline. Counting it as an overdue phase
/// is the category error #272 found in the status line, and this asserts it did not move here.
#[test]
fn an_elector_position_is_never_in_flight() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "ma/reader"]);
    node(root, "concept/d.yml", "class: concept\nlabel: D\n");
    commit(root, "2026-01-10", "position: the reader's");
    git(root, &["checkout", "-q", "main"]);
    config(root, "[due]\nphases_after = 1\n");

    let p = find(&clocks(root), "phases").clone();
    assert_eq!(p.state, State::Ok, "a position came due: {p:?}");
    assert_eq!(p.detail, "nothing in flight");
}

/// A merged phase has settled; its ref outliving the settlement is a hygiene question and
/// not a clock.
#[test]
fn a_settled_phase_is_not_in_flight() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "phase/done"]);
    node(root, "concept/e.yml", "class: concept\nlabel: E\n");
    commit(root, "2026-01-10", "scope: done");
    git(root, &["checkout", "-q", "main"]);
    git(root, &["merge", "-q", "--no-edit", "phase/done"]);
    config(root, "[due]\nphases_after = 1\n");

    let p = find(&clocks(root), "phases").clone();
    assert_eq!(p.state, State::Ok, "{p:?}");
}

// ── the catalog clock ─────────────────────────────────────────────────────────

/// The one clock whose interval was already declared where it belongs, and the one whose
/// remedy `propose` genuinely covers.
#[test]
fn an_expired_source_is_due_and_propose_is_what_discharges_it() {
    let tmp = repo();
    let root = tmp.path();
    let dir = root.join(".yidam/catalog");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("gauge.md"),
        "---\nid: gauge\nlocation: https://example.test/gauge\nretrieved: 2026-01-01\n---\n\nA gauge record.\n",
    )
    .unwrap();
    commit(root, "2026-01-01", "cite: the gauge record");
    config(root, "[catalog]\nttl_days = 30\n");

    let c = find(&clocks(root), "catalog").clone();
    assert_eq!(c.state, State::Due, "{c:?}");
    assert_eq!(c.overdue, 1);
    assert!(
        c.remedy.as_deref().unwrap_or_default().contains("propose"),
        "{c:?}"
    );
    // The interval is reported from where the corpus declared it, not restated under `[due]`.
    assert!(
        c.interval
            .as_deref()
            .unwrap_or_default()
            .contains("[catalog] ttl_days"),
        "{c:?}"
    );
}

// ── the verdict, and the sentence about it ────────────────────────────────────

/// Being owed is not a failure. `--strict` is the only thing that makes it one.
#[test]
fn only_strict_turns_a_due_clock_into_a_nonzero_exit() {
    let overdue = vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it")];
    assert!(DueReport::new(overdue.clone(), false).passed);
    assert!(!DueReport::new(overdue, true).passed);
    // And nothing due passes under either.
    let clear = vec![Clock::new("x", "Q", State::Ok, "none")];
    assert!(DueReport::new(clear.clone(), false).passed);
    assert!(DueReport::new(clear, true).passed);
}

/// An unmeasurable clock is not a due one.
///
/// A corpus that asked to be told when a source aged and holds one it cannot date has a gap
/// in its bookkeeping. Calling that due would assert something nobody knows.
#[test]
fn an_unmeasurable_clock_does_not_come_due() {
    let unknown = vec![Clock::new("x", "Q", State::Unmeasurable, "no date")];
    let r = DueReport::new(unknown, true);
    assert_eq!(r.due, 0);
    assert!(r.passed);
}

/// The line separating this report from `doctor` is on every rendering, including the one
/// with nothing to report.
#[test]
fn every_rendering_says_that_owed_is_not_broken() {
    let root = Path::new("/tmp/x");
    let clear = render(
        &DueReport::new(vec![Clock::new("x", "Q", State::Ok, "n")], false),
        root,
    );
    assert!(clear.contains("doctor"), "{clear}");

    let owed = render(
        &DueReport::new(
            vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it")],
            false,
        ),
        root,
    );
    assert!(owed.contains("not being broken"), "{owed}");
    assert!(owed.contains("→ do it"), "{owed}");

    let strict = render(
        &DueReport::new(
            vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it")],
            true,
        ),
        root,
    );
    assert!(strict.contains("--strict"), "{strict}");
}

/// An unset clock's remedy prints; an `ok` clock's does not exist to print.
#[test]
fn a_clean_clock_prints_no_arrow() {
    let out = render(
        &DueReport::new(vec![Clock::new("x", "Q", State::Ok, "nothing")], false),
        Path::new("/tmp/x"),
    );
    assert!(!out.contains('→'), "{out}");
}
