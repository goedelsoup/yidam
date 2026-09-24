//! What `yidam cycle` must not do, mostly.
//!
//! Two failures it is one wrong line away from. The first is `due`'s — reading as a gate, a
//! report whose numbers a person learns to treat as defects — and it inherits that risk
//! whole, because it composes `due`. The second is its own: citing a declaration that does
//! not exist. #576 specifies the fourth half as *"the kuten's declared phase order"*, no
//! profile declares one, and the assertions below hold the report to saying so.

use super::*;
use std::path::Path;

use crate::git::fixture::{git, init};

fn commit(dir: &Path, day: &str, msg: &str) {
    crate::git::fixture::commit_at(dir, msg, &format!("{day}T00:00:00Z"));
}

fn node(dir: &Path, rel: &str, body: &str) {
    let p = dir.join(".yidam/corpus").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

/// A derived repository with two mutually linked nodes and one commit, and no configuration.
///
/// **A pair pointing at each other, because the gate has two orphan checks and a fixture must
/// clear both.** A lone node is an `orphan-in`; a node at the end of an edge with no edge of
/// its own is an `orphan-out`, which is how the first shape of this fixture — `a` linking to a
/// bare `b` — still arrived blocked. A fixture blocked by construction cannot exercise the
/// difference between blocked and clear, which is most of what this file asserts.
fn repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    init(root);
    node(
        root,
        "concept/a.yml",
        "class: concept\nlabel: A\nlinks:\n  - target: ../concept/b.yml\n    relationship: reads\n",
    );
    node(
        root,
        "concept/b.yml",
        "class: concept\nlabel: B\nlinks:\n  - target: ../concept/a.yml\n    relationship: reads\n",
    );
    commit(root, "2026-01-01", "establish: a and b");
    tmp
}

/// The fixture is clear, asserted rather than assumed.
///
/// Every test below that distinguishes blocked from clear rests on this, and the first shape
/// of `repo()` was blocked without anything saying so — the assertions that needed a clear
/// corpus were the ones that failed, one level away from where the fault was.
#[test]
fn the_fixture_starts_clear() {
    let tmp = repo();
    let r = read(tmp.path());
    assert!(r.blocked.is_empty(), "{:?}", r.blocked);
    assert_eq!(r.due, 0);
}

/// 2026-06-01, as a day number. Every reading here is taken against it.
fn today() -> i64 {
    crate::dates::days_from_civil_str("2026-06-01").unwrap()
}

fn read(root: &Path) -> CycleReport {
    read_cycle(root, false, today()).unwrap()
}

// ── it is not a gate ──────────────────────────────────────────────────────────

/// Being owed never fails a run, which is `due`'s rule and the reason this command exists
/// beside the gates rather than in them.
#[test]
fn an_owed_corpus_passes() {
    let tmp = repo();
    let root = tmp.path();
    std::fs::write(
        root.join(".yidam/config.toml"),
        "[due]\nquestions_after = 1\nindex_after = 1\n",
    )
    .unwrap();

    let r = read(root);
    assert!(
        r.due > 0,
        "the fixture is meant to owe something: {:?}",
        r.owed
    );
    assert!(r.passed, "being owed is not a failure");
}

/// `--strict` is the whole of the difference, and it says so in the rendering.
#[test]
fn strict_is_what_fails_and_the_report_says_so() {
    let tmp = repo();
    let root = tmp.path();
    std::fs::write(root.join(".yidam/config.toml"), "[due]\nindex_after = 1\n").unwrap();

    let r = read_cycle(root, true, today()).unwrap();
    assert!(!r.passed);
    let text = render(&r, root);
    assert!(
        text.contains("because you asked it to"),
        "a strict failure must say it was asked for:\n{text}"
    );
}

/// The sentence that keeps this from being read as a gate is on the quiet run too — the run
/// whose reader most needs to be told what a clean report means.
#[test]
fn the_quiet_run_still_says_being_owed_is_not_being_broken() {
    let tmp = repo();
    let text = render(&read(tmp.path()), tmp.path());
    assert!(text.contains("Being owed is not being broken"), "{text}");
    assert!(
        text.contains("Nothing is due and nothing is blocked"),
        "{text}"
    );
}

// ── it composes, and does not recompute ───────────────────────────────────────

/// The owed half is `due`'s four clocks, by the ids `due` publishes — not a second set
/// measured here.
#[test]
fn the_owed_half_is_dues_own_clocks() {
    let tmp = repo();
    let r = read(tmp.path());
    let ids: Vec<&str> = r.owed.iter().map(|c| c.id).collect();
    assert_eq!(ids, vec!["index", "catalog", "questions", "phases"]);
}

/// A due clock contributes the remedy `due` computed, verbatim. Three of the four clocks are
/// discharged by something that is not a command, and inventing a remedy here would be this
/// report claiming an agent can do what `due.rs`'s table says it cannot.
#[test]
fn an_owed_act_is_dues_remedy_verbatim() {
    let tmp = repo();
    let root = tmp.path();
    std::fs::write(root.join(".yidam/config.toml"), "[due]\nindex_after = 1\n").unwrap();

    let r = read(root);
    let clock = r.owed.iter().find(|c| c.id == "index").unwrap();
    assert_eq!(clock.state, State::Due);
    let act = r
        .next
        .iter()
        .find(|a| a.source == Source::Owed)
        .expect("a due clock names an act");
    assert_eq!(Some(&act.act), clock.remedy.as_ref());
}

/// A phase that has already landed is not in flight. `phases` publishes the classification
/// and this reads it; deriving it here again is how `status` came to report 26 active phases
/// against a true count of 1.
#[test]
fn a_settled_phase_is_not_in_flight() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "phase/landed"]);
    node(root, "concept/c.yml", "class: concept\nlabel: C\n");
    commit(root, "2026-02-01", "establish: c");
    git(root, &["checkout", "-q", "main"]);
    git(
        root,
        &[
            "merge",
            "--no-ff",
            "-q",
            "-m",
            "phase: landed — one node",
            "phase/landed",
        ],
    );

    let r = read(root);
    assert!(
        r.in_flight.is_empty(),
        "a merged phase is settled: {:?}",
        r.in_flight
    );
}

/// The one genuinely unsettled ref is in flight, and it names an act.
#[test]
fn an_unsettled_phase_is_in_flight_and_names_an_act() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "phase/open-work"]);
    node(root, "concept/c.yml", "class: concept\nlabel: C\n");
    commit(root, "2026-02-01", "open: where the line starts");
    git(root, &["checkout", "-q", "main"]);

    let r = read(root);
    assert_eq!(r.in_flight.len(), 1, "{:?}", r.in_flight);
    assert_eq!(r.in_flight[0].ref_name, "phase/open-work");
    let act = r
        .next
        .iter()
        .find(|a| a.source == Source::InFlight)
        .unwrap();
    assert!(act.act.contains("phase/open-work"), "{}", act.act);
}

// ── blocked is what a gate would fail on, not what a corpus was forgiven ──────

/// An orphan is a `lint` error and blocks.
#[test]
fn a_finding_against_no_baseline_blocks() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/orphan.yml",
        "class: concept\nlabel: Orphan\n",
    );
    commit(root, "2026-02-01", "establish: an orphan");

    let r = read(root);
    assert!(
        r.blocked.iter().any(|b| b.gate == "lint"),
        "an orphan against no baseline is a block: {:?}",
        r.blocked
    );
    assert!(r.next.iter().any(|a| a.act == "yidam lint"));
}

/// **The same finding, blessed, does not block** — and this is the whole meaning of the
/// blocked half. A corpus carrying inherited debt it agreed to is not a corpus with things
/// to fix; `yidam lint` is green over it, and a report that listed them would tell a reader
/// that a repository doing exactly what it agreed to is blocked.
#[test]
fn a_blessed_finding_does_not_block() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/orphan.yml",
        "class: concept\nlabel: Orphan\n",
    );
    commit(root, "2026-02-01", "establish: an orphan");

    let before = read(root);
    assert!(!before.blocked.is_empty(), "the fixture must start blocked");

    let all = crate::cmd::lint::run_checks(root, &crate::cmd::lint::Options::default());
    let previous = crate::cmd::lint::baseline::Baseline::default();
    let head = crate::cmd::lint::history::corpus_commits(root)
        .last()
        .cloned()
        .unwrap_or_default();
    crate::cmd::lint::baseline::Baseline::from_checks(&all, &previous, &head)
        .write(root)
        .unwrap();

    let after = read(root);
    assert!(
        after.blocked.iter().all(|b| b.gate != "lint"),
        "a blessed finding is inherited debt, not a block: {:?}",
        after.blocked
    );
}

/// One act per gate, however many findings it holds. Forty rows under "what is next" is the
/// shape of list a reader skims, and there is one thing to do about them.
#[test]
fn many_findings_name_one_act() {
    let tmp = repo();
    let root = tmp.path();
    for n in 0..5 {
        node(
            root,
            &format!("concept/orphan{n}.yml"),
            &format!("class: concept\nlabel: Orphan {n}\n"),
        );
    }
    commit(root, "2026-02-01", "establish: five orphans");

    let r = read(root);
    assert!(r.blocked.len() >= 5, "{:?}", r.blocked);
    assert_eq!(
        r.next.iter().filter(|a| a.act == "yidam lint").count(),
        1,
        "one act per gate: {:?}",
        r.next
    );
}

// ── the half that has no declaration ──────────────────────────────────────────

/// The definition of done's own case: a repository holding no kuten gets the other three
/// halves and is *told* this one needs a declaration, rather than getting a blank.
#[test]
fn a_repository_with_no_kuten_is_told_so_and_keeps_the_rest() {
    let tmp = repo();
    let root = tmp.path();
    let r = read(root);

    assert!(r.practice.kuten.is_none());
    assert!(r.practice.phase_types.is_empty());
    let why = r.practice.undeclared.as_deref().expect("it must say why");
    assert!(why.contains("holds no kuten"), "{why}");
    assert!(why.contains("supported state"), "{why}");

    // The other three halves are unaffected — they need no declaration at all.
    assert_eq!(r.owed.len(), 4);
    assert!(render(&r, root).contains("in flight"));
    assert!(render(&r, root).contains("blocked"));
}

/// A declared kuten contributes its phase types and its question pressure, and the report
/// names them as what they are.
#[test]
fn a_declared_kuten_names_its_types_and_its_pressure() {
    let tmp = repo();
    let root = tmp.path();
    let profile = root.join(".yidam/.vendor/prelude/kuten/inquiry");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(
        profile.join("kuten.yml"),
        "kuten: inquiry\nrevision: 2\ngloss: the corpus grows through sustained inquiry\n\
         phases:\n  types: [Investigation, Extraction, Synthesis, Assessment]\n  \
         commit_share: {low: 0.12, high: 0.27}\nquestion_pressure:\n  kind: epistemic\n",
    )
    .unwrap();
    std::fs::create_dir_all(root.join(".yidam/decisions")).unwrap();
    std::fs::write(
        root.join(".yidam/decisions/kuten.yml"),
        "kuten: inquiry\nrevision: 2\n",
    )
    .unwrap();

    let r = read(root);
    assert_eq!(r.practice.kuten.as_deref(), Some("inquiry"));
    assert_eq!(r.practice.revision, Some(2));
    assert_eq!(r.practice.phase_types.len(), 4);
    assert_eq!(r.practice.question_pressure.as_deref(), Some("epistemic"));
    assert!(r.practice.undeclared.is_none());

    let act = r
        .next
        .iter()
        .find(|a| a.source == Source::Practice)
        .expect("a declared pressure names an act");
    assert!(act.act.contains("epistemic"), "{}", act.act);
}

/// **No profile declares a phase order, and the report must not imply one.** The `phases`
/// slot is `{types, commit_share}` and `types` is a set; #576's "declared phase order" does
/// not exist. If a future profile grows an ordering, this assertion is the thing that should
/// go red — it is not a test of prose, it is the guard on a claim.
#[test]
fn the_ranking_is_this_commands_and_the_report_says_no_profile_declares_one() {
    let tmp = repo();
    let root = tmp.path();
    let text = render(&read(root), root);
    assert!(
        text.contains("no profile declares a phase order"),
        "the report must not cite an order it does not have:\n{text}"
    );

    // And the shipped profile is checked rather than assumed: it is the only one there is.
    let shipped =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../prelude/kuten/inquiry/kuten.yml");
    let profile =
        crate::kuten::Profile::parse(&std::fs::read_to_string(&shipped).unwrap()).unwrap();
    let phases = profile.phases.expect("the shipped profile declares phases");
    assert_eq!(phases.types.len(), 4);
    // A `Vec<String>` and a band. Nothing here is a sequence, and nothing reads it as one.
    assert!(
        phases
            .types
            .iter()
            .all(|t| !t.contains("->") && !t.contains("→")),
        "a type carrying an arrow would be an order smuggled into a name: {:?}",
        phases.types
    );
}

// ── it authors nothing ────────────────────────────────────────────────────────

/// The invariant #572 and RFC-0026 carry, asserted rather than argued: this command writes
/// no file and adds no commit. `cycle` reports.
#[test]
fn it_writes_nothing() {
    let tmp = repo();
    let root = tmp.path();
    node(
        root,
        "concept/orphan.yml",
        "class: concept\nlabel: Orphan\n",
    );
    commit(root, "2026-02-01", "establish: an orphan");

    let before = tree(root);
    let head = crate::git::fixture::git_out(root, &["rev-parse", "HEAD"]);

    let r = read(root);
    let _ = render(&r, root);

    assert_eq!(tree(root), before, "cycle wrote to the working tree");
    let after = crate::git::fixture::git_out(root, &["rev-parse", "HEAD"]);
    assert_eq!(after, head, "cycle moved HEAD");
}

/// Every file under the repository, with its bytes — so a rewrite in place is caught as well
/// as a creation.
fn tree(root: &Path) -> std::collections::BTreeMap<String, Vec<u8>> {
    let mut out = std::collections::BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            // `.git` holds a reflog and index that move for reasons that are not writes to
            // the repository — but HEAD is checked separately above, which is the claim.
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                stack.push(p);
            } else if let Ok(bytes) = std::fs::read(&p) {
                out.insert(p.strip_prefix(root).unwrap().display().to_string(), bytes);
            }
        }
    }
    out
}
