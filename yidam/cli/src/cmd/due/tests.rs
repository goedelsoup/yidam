//! What `yidam due` must not do, mostly.
//!
//! The failure this command is one wrong line away from is reading as a second `doctor` — a
//! report whose numbers a person learns to treat as defects. Most of what is asserted here is
//! that a clock nobody set never comes due, that being owed never fails a run, and that the
//! sentence saying so is on every rendering including the quiet one.

use super::*;
use std::path::Path;

use crate::git::fixture::{git, init};

/// Commit at a fixed date. Every clock here is read against a `today` this file supplies, so
/// nothing in it can depend on the day it runs.
fn commit(dir: &Path, day: &str, msg: &str) {
    crate::git::fixture::commit_at(dir, msg, &format!("{day}T00:00:00Z"));
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
    init(root);
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
    assert_eq!(all.len(), 6, "six clocks: {all:?}");
    for c in &all {
        assert_ne!(
            c.state,
            State::Due,
            "{} came due against an interval nobody set",
            c.id
        );
        assert_eq!(c.overdue, 0, "{}", c.id);
        // The row set does not vary by build. `Unbuildable` was added instead of dropping the
        // clock precisely so that a reader comparing two `due` runs is comparing five rows to
        // five rows — see [`State::Unbuildable`].
        let expected = match (c.id, cfg!(feature = "vector-read")) {
            ("index", false) => State::Unbuildable,
            _ => State::Undeclared,
        };
        assert_eq!(c.state, expected, "{}", c.id);
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

/// An index that was never built, against a corpus that asked for one — **in a build that can
/// build one**.
///
/// `clock_index` is called directly with `can_build = true` rather than through
/// [`clocks`], and that is the whole reason the argument exists. Under `cargo test` with the
/// default features this assertion would otherwise be about the *other* arm, and the arm it
/// means to check would be compiled-and-verified only by `ci (cli · full features)` — which
/// runs on `main` and a Monday cron, never on the pull request that changes it.
#[test]
fn a_corpus_that_wants_an_index_and_has_none_is_due_one() {
    let tmp = repo();
    let index = clock_index(tmp.path(), Some(5), true);
    assert_eq!(index.state, State::Due);
    assert_eq!(index.overdue, 1);
    assert_eq!(index.remedy.as_deref(), Some("yidam index-build"));
    // And it is a tool act, because this binary runs the command it names (#1442).
    assert!(
        matches!(&index.actor, Some(Actor::Tool(argv)) if argv == &["yidam", "index-build"]),
        "{index:?}"
    );
    // And it does not hedge about a feature the reader has. The parenthetical was #1061's
    // other half: the remedy named `index` whether or not the binary carried it.
    assert!(
        !index
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("feature"),
        "a build that can do it should not be told which feature would let it: {index:?}"
    );
}

/// The same corpus, read by a binary that cannot build an index: **not** due (#1061).
///
/// The row this replaces could not be discharged by anything the reader had. `index-build`
/// refuses in this build and refusing is correct, so the clock reported a corpus doing nothing
/// wrong as permanently owed — and both repositories that met it silenced the clock rather than
/// repairing an index.
#[test]
fn a_build_that_cannot_make_an_index_is_not_owed_one() {
    let tmp = repo();
    let index = clock_index(tmp.path(), Some(5), false);
    assert_eq!(index.state, State::Unbuildable, "{index:?}");
    // Owed nothing. `overdue` counts subjects past an interval and there is no subject here.
    assert_eq!(index.overdue, 0);
    // Which means `--strict` passes over it, which is the cost the old row was really carrying.
    assert!(DueReport::new(vec![index], true).passed);
}

/// It says which of the two facts is in the way, and what to do about it.
///
/// Asserted on the sentence rather than on the state, because a reader meeting this row has to
/// be able to tell *my corpus has no index* from *my binary cannot make one*, and the state tag
/// alone says neither.
#[test]
fn the_unbuildable_row_names_the_binary_and_the_install() {
    let tmp = repo();
    let index = clock_index(tmp.path(), Some(5), false);
    assert!(
        index.detail.contains("no index has been built")
            && index.detail.contains("this binary cannot build one"),
        "both facts, not one: {}",
        index.detail
    );
    let remedy = index.remedy.as_deref().unwrap_or_default();
    assert!(remedy.contains("--features vector-read"), "{remedy}");
    // The other route, because it needs no install at all: an index built elsewhere can be
    // pulled into this corpus.
    assert!(remedy.contains("vault pull"), "{remedy}");
    // A person's act: `index-build` refuses in this binary, so naming it as the command that
    // discharges the clock would hand a loop a command that cannot run (#1442).
    assert!(matches!(index.actor, Some(Actor::Person)), "{index:?}");
    // And the row prints it. `render` shows a remedy on three states, and a state added to the
    // enum without being added there is a row that names a problem and withholds the fix.
    let out = render(&DueReport::new(vec![index], false), tmp.path());
    assert!(out.contains("--features vector-read"), "{out}");
}

/// A corpus that never declared the interval is told about the build **before** it is told to
/// declare one.
///
/// This is the arm that made the defect self-inflicting. `Undeclared`'s remedy is *declare
/// `[due] index_after`*, and following it in this build is what produces the permanently red
/// row — so a reader of the light build must not be handed that advice first.
#[test]
fn an_undeclared_index_clock_does_not_advise_a_build_that_cannot_act_on_it() {
    let tmp = repo();
    let index = clock_index(tmp.path(), None, false);
    assert_eq!(index.state, State::Unbuildable, "{index:?}");
    assert!(
        !index
            .remedy
            .as_deref()
            .unwrap_or_default()
            .contains("declare"),
        "it advised declaring an interval it cannot satisfy: {index:?}"
    );
    // The build that can act on it still gets the advice.
    let capable = clock_index(tmp.path(), None, true);
    assert_eq!(capable.state, State::Undeclared);
    assert_eq!(
        capable.remedy.as_deref(),
        Some("declare `[due] index_after` in .yidam/config.toml")
    );
}

/// A declared interval is recorded on the unbuildable row, so a decline over it still reads as
/// the contradiction it is.
///
/// `declined()` finds a corpus that declared and declined the same clock by reading
/// `interval.is_some()`. Dropping the interval under this arm would take that contradiction out
/// of view over a detail of the build the report happened to be run from.
#[test]
fn an_unbuildable_clock_still_carries_the_interval_the_corpus_declared() {
    let tmp = repo();
    assert_eq!(
        clock_index(tmp.path(), Some(5), false).interval.as_deref(),
        Some("[due] index_after = 5")
    );
    assert_eq!(clock_index(tmp.path(), None, false).interval, None);
}

/// A light build holding an index reads its freshness for real, and is due a rebuild.
///
/// The asymmetry this repository has to keep: `stale_nodes` is a `meta.json` and a walk of file
/// times, so *reading* an index needs no feature. A corpus that pulled one with
/// `yidam vault pull --index` gets the true verdict, and the act that would discharge it is
/// available — the route it arrived by. Widening #1061's fix to every arm would have turned
/// that into silence.
#[test]
fn a_build_that_cannot_make_an_index_still_grades_one_it_holds() {
    let tmp = repo();
    let root = tmp.path();
    // Built before the node below is written, so the node is stale against it.
    let dir = root.join(".yidam/index");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("meta.json"),
        r#"{"generated_at":1,"model_name":"m","embedding_dim":1,"node_count":1}"#,
    )
    .unwrap();
    node(
        root,
        "concept/c.yml",
        "class: concept
label: C
",
    );

    let index = clock_index(root, Some(1), false);
    assert_eq!(index.state, State::Due, "{index:?}");
    assert!(index.overdue >= 1, "{index:?}");
    // And the remedy is the one this build can reach, not `index-build`.
    let remedy = index.remedy.as_deref().unwrap_or_default();
    assert!(remedy.contains("vault pull"), "{remedy}");
    // A person's act: `index-build` refuses in this binary, so naming it as the command that
    // discharges the clock would hand a loop a command that cannot run (#1442).
    assert!(matches!(index.actor, Some(Actor::Person)), "{index:?}");
}

/// The state tag is its own spelling.
///
/// A tag reused from another state is how two facts come to render identically, which is the
/// whole argument `Declined` was added under.
#[test]
fn every_clock_state_has_its_own_tag() {
    let tags = [
        State::Due,
        State::Ok,
        State::Undeclared,
        State::Declined,
        State::Unmeasurable,
        State::Unbuildable,
    ]
    .map(State::tag);
    let unique: std::collections::BTreeSet<&str> = tags.iter().copied().collect();
    assert_eq!(unique.len(), tags.len(), "{tags:?}");
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

/// A renamed question is still overdue (#1180).
///
/// The clock filters on the count before it sorts. So when a rename restarted the count, the
/// question did not just drop to the bottom of the list. It dropped off it, and the clock
/// read Ok over a question nobody had answered.
#[test]
fn a_question_moved_under_a_declared_origin_stays_overdue() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/b.yml",
        "class: concept\nlabel: B\ndescription: it is `[open]`\n",
    );
    commit(root, "2026-01-02", "open: whether b holds");
    node(root, "concept/a.yml", "class: concept\nlabel: A2\n");
    commit(root, "2026-01-03", "revise: a");
    git(
        root,
        &[
            "mv",
            ".yidam/corpus/concept/b.yml",
            ".yidam/corpus/concept/bee.yml",
        ],
    );
    node(
        root,
        "concept/bee.yml",
        "class: concept\nlabel: B\ndescription: it is `[open]`\nmoved-from: ../concept/b.yml\n",
    );
    commit(
        root,
        "2026-01-04",
        "migrate: concept/b.yml → concept/bee.yml",
    );

    config(root, "[due]\nquestions_after = 3\n");
    let due = find(&clocks(root), "questions").clone();
    assert_eq!(due.state, State::Due, "{due:?}");
    assert!(
        due.detail
            .ends_with("longest 3 (.yidam/corpus/concept/bee.yml)"),
        "{due:?}"
    );
}

/// Answering an open question is a resolution event, so the remedy must not name `propose`.
///
/// RFC-0020 and `cmd/sangha.rs` both draw that line, and #289 proposed crossing it — "under
/// E4, proposes the commits that would discharge them" is true of exactly one of the five
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

    let listed = crate::cmd::corpus::open_questions_data(&crate::corpus::Corpus::open(root))
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

// ── the superseded clock ──────────────────────────────────────────────────────

/// A catalog entry for `gauge`, holding one record per digest, each fetched from location 0.
fn gauge(root: &Path, digests: &[&str]) {
    let records: String = digests
        .iter()
        .map(|d| format!("  - sha256: {d}\n    retrieved: 2026-01-01\n    from: 0\n"))
        .collect();
    let dir = root.join(".yidam/catalog");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("gauge.md"),
        format!("---\nid: gauge\nlocation:\n  - kind: file\n    value: sources/gauge.csv\nartifacts:\n{records}---\n\nA gauge record.\n"),
    )
    .unwrap();
}

fn citing(root: &Path, rel: &str, label: &str) {
    node(
        root,
        rel,
        &format!("class: concept\nlabel: {label}\nlinks:\n  - target: ../../catalog/gauge.md\n    relationship: sourced-from\n"),
    );
}

/// The case #1200 names: one entry, two versions, a node citing it on each side of the new
/// digest. The node read before it is owed; the node committed to after it is not.
fn two_versions_one_node_either_side() -> tempfile::TempDir {
    let tmp = repo();
    let root = tmp.path();
    gauge(root, &["aaaaaaaaaaaaaaaa"]);
    citing(root, "concept/before.yml", "Before");
    citing(root, "concept/after.yml", "After");
    commit(root, "2026-01-10", "cite: the gauge, twice");
    gauge(root, &["aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"]);
    commit(root, "2026-02-01", "fetch: gauge changed");
    citing(root, "concept/after.yml", "After, re-read");
    commit(root, "2026-02-05", "revise: after, against the new gauge");
    tmp
}

#[test]
fn a_node_read_before_a_new_version_is_owed_and_one_touched_after_is_not() {
    let tmp = two_versions_one_node_either_side();
    let root = tmp.path();
    config(root, "[due]\nsuperseded_after = 14\n");

    let c = find(&clocks(root), "superseded").clone();
    assert_eq!(c.state, State::Due, "{c:?}");
    assert_eq!(c.overdue, 1, "{c:?}");
    assert_eq!(c.subjects.len(), 1, "{c:?}");
    let s = &c.subjects[0];
    assert!(s.starts_with(".yidam/corpus/concept/before.yml"), "{s}");
    assert!(s.contains("gauge"), "{s}");
    assert!(s.contains("28 day(s)"), "{s}");
    assert!(s.contains("sha256:bbbbbbbbbbbb"), "{s}");
    assert!(!c.subjects.iter().any(|s| s.contains("after.yml")), "{c:?}");
}

/// Undeclared, the clock still names what it measured, and never comes due.
#[test]
fn an_unset_superseded_clock_measures_and_owes_nothing() {
    let tmp = two_versions_one_node_either_side();
    let c = find(&clocks(tmp.path()), "superseded").clone();
    assert_eq!(c.state, State::Undeclared, "{c:?}");
    assert_eq!(c.overdue, 0);
    assert!(c.subjects.is_empty(), "{c:?}");
    assert!(c.detail.contains("1 node(s) cite 1 of 1"), "{}", c.detail);
}

/// Inside its interval the node is not yet owed. The interval runs from the superseding
/// commit, not from the node's own last touch.
#[test]
fn a_new_version_inside_its_interval_is_not_yet_due() {
    let tmp = two_versions_one_node_either_side();
    let root = tmp.path();
    config(root, "[due]\nsuperseded_after = 60\n");
    let c = find(&clocks(root), "superseded").clone();
    assert_eq!(c.state, State::Ok, "{c:?}");
    assert!(c.subjects.is_empty(), "{c:?}");
}

/// A commit to the node is what discharges it — any commit, since the corpus cannot tell a
/// re-read from a typo fix, and a pin to the new digest is one such commit.
#[test]
fn a_commit_to_the_owed_node_discharges_it() {
    let tmp = two_versions_one_node_either_side();
    let root = tmp.path();
    citing(root, "concept/before.yml", "Before, re-read");
    commit(root, "2026-02-20", "revise: before, against the new gauge");
    config(root, "[due]\nsuperseded_after = 14\n");
    let c = find(&clocks(root), "superseded").clone();
    assert_eq!(c.state, State::Ok, "{c:?}");
    assert!(c.subjects.is_empty(), "{c:?}");
}

/// One version is not a supersession, however old the node citing it.
#[test]
fn a_source_with_one_version_owes_nothing() {
    let tmp = repo();
    let root = tmp.path();
    gauge(root, &["aaaaaaaaaaaaaaaa"]);
    citing(root, "concept/before.yml", "Before");
    commit(root, "2026-01-10", "cite: the gauge");
    config(root, "[due]\nsuperseded_after = 1\n");
    let c = find(&clocks(root), "superseded").clone();
    assert_eq!(c.state, State::Ok, "{c:?}");
    assert!(
        c.detail.contains("no source holds a second version"),
        "{}",
        c.detail
    );
}

// ── the verdict, and the sentence about it ────────────────────────────────────

/// Being owed is not a failure. `--strict` is the only thing that makes it one.
#[test]
fn only_strict_turns_a_due_clock_into_a_nonzero_exit() {
    let overdue = vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it", Actor::Person)];
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
            vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it", Actor::Person)],
            false,
        ),
        root,
    );
    assert!(owed.contains("not being broken"), "{owed}");
    assert!(owed.contains("→ do it"), "{owed}");

    let strict = render(
        &DueReport::new(
            vec![Clock::new("x", "Q", State::Due, "one").owing(1, "do it", Actor::Person)],
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

// ── a clock a corpus declined ─────────────────────────────────────────────────

/// A decision record, by the identity `decisions-log` gives one.
fn decision(root: &Path, stem: &str, summary: &str) {
    let dir = root.join(".yidam/decisions");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join(format!("{stem}.yml")),
        format!("summary: {summary}\n"),
    )
    .unwrap();
}

/// The state the report had no way to express: a corpus that decided against the thing a
/// clock measures, rather than one that has not thought about it.
///
/// Both read as `undeclared` before this key, and the only remedy on offer to the first was
/// to declare an interval for work nobody intends.
#[test]
fn a_clock_declined_in_writing_is_not_a_clock_nobody_set() {
    let tmp = repo();
    let root = tmp.path();
    decision(
        root,
        "due-clocks",
        "no index; typed retrieval answers these",
    );
    config(root, "[due.declined]\nindex = \"due-clocks\"\n");

    let all = clocks(root);
    let index = find(&all, "index");
    assert_eq!(index.state, State::Declined, "{index:?}");
    assert_eq!(
        index.record.as_deref(),
        Some(".yidam/decisions/due-clocks.yml"),
        "the decline must carry where to read why"
    );
    assert_eq!(index.remedy, None, "nothing discharges a decline");

    // And it leaves the sentence about unset clocks to the clocks that really are unset.
    let report = DueReport::new(all, false);
    assert_eq!(report.declined, 1);
    assert_eq!(report.undeclared, 5, "a decline was counted as an absence");
    assert_eq!(report.due, 0);
}

/// A declined clock still reports its measurement, for [`an_unset_clock_still_says_what_it_measured`]'s
/// reason: the reader who may one day revisit the decision is the one who needs the number.
#[test]
fn a_declined_clock_still_says_what_it_measured() {
    let tmp = repo();
    let root = tmp.path();
    decision(root, "due-clocks", "declined");
    config(root, "[due.declined]\ncatalog = \"due-clocks\"\n");

    let catalog = find(&clocks(root), "catalog").clone();
    assert_eq!(
        catalog.detail, "0 source(s), none under a TTL",
        "{catalog:?}"
    );
}

/// A decline is honoured over a clock this build could not have discharged anyway.
///
/// The two reasons a row goes quiet compose in one direction only. `Unbuildable` is a fact
/// about the binary and says nothing about whether the corpus wants the clock, so a written
/// decline still outranks it — and it must, or a corpus that argued its way out of the index
/// would see its record go unread on exactly the build that cannot act on one.
#[test]
fn a_decline_outranks_a_build_that_cannot_act() {
    let tmp = repo();
    let root = tmp.path();
    decision(root, "due-clocks", "declined");

    // `can_build = false` passed directly, and the decline composed by hand, for exactly the
    // reason [`a_corpus_that_wants_an_index_and_has_none_is_due_one`] gives. Read through
    // [`clocks`] this asserted the *other* arm's detail: with the feature on there is no
    // `Unbuildable` to outrank, so it passed on every pull request and failed on `main` under
    // `ci (cli · full features)`. The `[due.declined] index` config route is not lost with it —
    // [`a_clock_declined_in_writing_is_not_a_clock_nobody_set`] holds that, on assertions no
    // build feature can move.
    let index = clock_index(root, None, false)
        .declined("due-clocks", &crate::paths::yidam_decisions_dir(root));

    assert_eq!(index.state, State::Declined, "{index:?}");
    assert!(index.record.is_some(), "{index:?}");
    assert_eq!(index.remedy, None, "nothing discharges a decline");
    // The measurement survives, build fact and all, for the reader revisiting the decision.
    assert!(
        index.detail.contains("this binary cannot build one"),
        "{}",
        index.detail
    );
}

/// The property that keeps this from being a mute button, and the one `.yidam/lint-baseline.yml`
/// has: a decline whose record is gone stops being honoured and says so.
///
/// The record could be renamed, or never written. Either way what is left is a key asserting
/// an argument nobody can read, and the clock reverts to what it actually is.
#[test]
fn a_decline_whose_record_is_missing_is_not_honoured() {
    let tmp = repo();
    let root = tmp.path();
    config(root, "[due.declined]\ncatalog = \"due-clocks\"\n");

    let catalog = find(&clocks(root), "catalog").clone();
    assert_eq!(catalog.state, State::Undeclared, "{catalog:?}");
    assert_eq!(catalog.record, None);
    let remedy = catalog.remedy.unwrap_or_default();
    assert!(remedy.contains("due-clocks"), "{remedy}");
    assert!(remedy.contains(".yidam/decisions/"), "{remedy}");
    assert!(
        catalog.detail.contains("does not hold"),
        "{}",
        catalog.detail
    );
}

/// Declaring an interval and declining the same clock is a contradiction, and the interval
/// wins.
///
/// The direction matters: a corpus that set a clock is asking to be told, and resolving the
/// contradiction by going quiet would let a decline silence a live interval. The clock reads
/// as declared and names the contradiction, so the person who wrote both finds out.
#[test]
fn an_interval_beats_a_decline_of_the_same_clock() {
    let tmp = repo();
    let root = tmp.path();
    decision(root, "due-clocks", "declined");
    let dir = root.join(".yidam/catalog");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("gauge.md"),
        "---\nid: gauge\nlocation: https://example.test/gauge\nretrieved: 2026-01-01\n---\n\nA gauge record.\n",
    )
    .unwrap();
    commit(root, "2026-01-01", "cite: the gauge record");
    config(
        root,
        "[catalog]\nttl_days = 30\n\n[due.declined]\ncatalog = \"due-clocks\"\n",
    );

    let catalog = find(&clocks(root), "catalog").clone();
    assert_eq!(
        catalog.state,
        State::Due,
        "a decline silenced an interval: {catalog:?}"
    );
    assert!(catalog.detail.contains("contradicts"), "{}", catalog.detail);
    assert_eq!(catalog.record, None);
}

/// A decline keyed on a clock that does not exist does nothing, so the report says so.
///
/// A typo here is the same failure as a missing record arriving through the other half of
/// the key: a declaration that reads as deliberate and has no effect.
#[test]
fn a_decline_naming_no_clock_is_reported_rather_than_ignored() {
    let tmp = repo();
    let root = tmp.path();
    decision(root, "due-clocks", "declined");
    config(root, "[due.declined]\nindices = \"due-clocks\"\n");

    let cfg = crate::config::load_yidam_config(root).unwrap();
    let all = read_clocks(root, &cfg.due, today());
    let unknown = unknown_declines(&cfg.due, &all);
    assert_eq!(unknown, vec!["indices".to_string()]);

    let out = render(&DueReport::new(all, false).noting(unknown), root);
    assert!(out.contains("names no clock called `indices`"), "{out}");
    assert!(out.contains("index, catalog"), "{out}");
}

/// The row prints where the decision is recorded. Without it the report says only that
/// somebody switched a clock off, which is the reading `Declined` exists to prevent.
#[test]
fn a_declined_row_prints_the_record_behind_it() {
    let declined = Clock {
        record: Some(".yidam/decisions/due-clocks.yml".to_string()),
        ..Clock::new("x", "Q", State::Declined, "no index has been built")
    };
    let report = DueReport::new(vec![declined], true);
    let out = render(&report, Path::new("/tmp/x"));
    assert!(
        out.contains("→ declined in .yidam/decisions/due-clocks.yml"),
        "{out}"
    );
    assert!(out.contains("1 clock(s) declined"), "{out}");
    // And it is not owed, under `--strict` or otherwise.
    assert_eq!(report.due, 0);
    assert!(report.passed);
    assert!(!out.contains("can never come due"), "{out}");
}

// ── asks ──────────────────────────────────────────────────────────────────────

const ASKED_AT: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const REPINNED: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn pin_upstream(root: &Path, commit: &str) {
    std::fs::create_dir_all(root.join(".yidam/tonpa")).unwrap();
    std::fs::write(
        root.join(".yidam/tonpa.toml"),
        format!("[dependencies.up]\npath = \"../up\"\nrev = \"{commit}\"\n"),
    )
    .unwrap();
    std::fs::write(
        root.join(".yidam/tonpa/tonpa.lock"),
        format!(
            "[[package]]\nname = \"up\"\nurl = \"../up\"\nsha256 = \"x\"\nrev = \"{commit}\"\n"
        ),
    )
    .unwrap();
}

fn ask(root: &Path, stem: &str, header: &str) {
    let p = root.join(".yidam/asks/up").join(format!("{stem}.md"));
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, format!("---\n{header}\n---\n# {stem}?\n")).unwrap();
}

/// One ask read at the first pin, and the dependency re-pinned past it on 2026-02-01 — 28 days
/// before `today()`. A second ask asked at the new pin, and a third already answered.
fn repinned_past_an_ask() -> tempfile::TempDir {
    let tmp = repo();
    let root = tmp.path();
    pin_upstream(root, ASKED_AT);
    ask(
        root,
        "stale",
        &format!("pin: {}\nstatus: open", &ASKED_AT[..8]),
    );
    ask(
        root,
        "settled",
        &format!("pin: {ASKED_AT}\nstatus: answered\nanswered_at: {REPINNED}"),
    );
    commit(root, "2026-01-10", "ask: up, twice");
    pin_upstream(root, REPINNED);
    commit(root, "2026-02-01", "pin: up moved");
    ask(
        root,
        "current",
        &format!("pin: {}\nstatus: open", &REPINNED[..12]),
    );
    commit(root, "2026-02-03", "ask: up, at the new pin");
    tmp
}

#[test]
fn an_ask_its_corpus_has_moved_past_is_due_from_the_re_pin() {
    let tmp = repinned_past_an_ask();
    let root = tmp.path();
    config(root, "[due]\nasks_after = 7\n");

    let c = find(&clocks(root), "asks").clone();
    assert_eq!(c.state, State::Due, "{c:?}");
    assert_eq!(c.overdue, 1, "{c:?}");
    assert_eq!(c.subjects.len(), 1, "{c:?}");
    let s = &c.subjects[0];
    assert!(s.starts_with(".yidam/asks/up/stale.md"), "{s}");
    assert!(s.contains("28 day(s)"), "{s}");
    assert!(s.contains("bbbbbbbbbbbb"), "{s}");
    assert_eq!(c.actor, Some(Actor::Person), "{c:?}");
    assert!(
        !c.remedy.unwrap_or_default().contains("propose"),
        "whether a corpus answered is a reading, not a drafted commit"
    );
}

/// A pin that still matches is waiting on the far side, however old, and an answered ask is
/// not open — neither is owed, at any interval.
#[test]
fn an_ask_at_the_current_pin_or_answered_is_owed_nothing() {
    let tmp = repinned_past_an_ask();
    let root = tmp.path();
    std::fs::remove_file(root.join(".yidam/asks/up/stale.md")).unwrap();
    config(root, "[due]\nasks_after = 0\n");
    let c = find(&clocks(root), "asks").clone();
    assert_eq!(c.state, State::Ok, "{c:?}");
    assert!(
        c.detail.contains("1 open ask(s), 0 read against"),
        "{}",
        c.detail
    );
}

#[test]
fn an_ask_inside_its_interval_is_not_yet_due() {
    let tmp = repinned_past_an_ask();
    let root = tmp.path();
    config(root, "[due]\nasks_after = 60\n");
    let c = find(&clocks(root), "asks").clone();
    assert_eq!(c.state, State::Ok, "{c:?}");
    assert!(c.subjects.is_empty(), "{c:?}");
}

#[test]
fn an_unset_asks_clock_measures_and_owes_nothing() {
    let tmp = repinned_past_an_ask();
    let c = find(&clocks(tmp.path()), "asks").clone();
    assert_eq!(c.state, State::Undeclared, "{c:?}");
    assert_eq!(c.overdue, 0);
    assert!(
        c.detail
            .contains("2 open ask(s), 1 read against a pin since moved"),
        "{}",
        c.detail
    );
}

/// A re-pin nobody committed has no date. It is owed on the day at `0`, and unmeasurable
/// past it rather than either due or clean.
#[test]
fn an_uncommitted_re_pin_is_unmeasurable_past_zero() {
    let tmp = repo();
    let root = tmp.path();
    pin_upstream(root, REPINNED);
    ask(root, "stale", &format!("pin: {ASKED_AT}\nstatus: open"));

    config(root, "[due]\nasks_after = 7\n");
    let c = find(&clocks(root), "asks").clone();
    assert_eq!(c.state, State::Unmeasurable, "{c:?}");

    config(root, "[due]\nasks_after = 0\n");
    let c = find(&clocks(root), "asks").clone();
    assert_eq!(c.state, State::Due, "{c:?}");
}

/// A dependency read live from a sibling's tree has nothing to have moved past, and the clock
/// says so rather than counting the ask clean.
#[test]
fn an_ask_to_a_live_dependency_is_said_to_have_no_pin() {
    let tmp = repo();
    let root = tmp.path();
    std::fs::create_dir_all(root.join(".yidam")).unwrap();
    std::fs::write(
        root.join(".yidam/tonpa.toml"),
        "[dependencies.up]\npath = \"../up\"\n",
    )
    .unwrap();
    ask(root, "q", &format!("pin: {ASKED_AT}\nstatus: open"));
    let c = find(&clocks(root), "asks").clone();
    assert!(
        c.detail.contains("1 to a dependency with no pin"),
        "{}",
        c.detail
    );
}
