//! A round's memory, held to #1443's definition of done.
//!
//! Two obligations, and the second is the one a careless implementation fails. A two-round
//! fixture that resolves one finding and introduces one yields exactly that delta; and with no
//! record, `--since-last` says so instead of reporting an empty delta as no change.

use super::*;
use crate::cmd::cycle::{read_cycle, render, render_brief, with_round, CycleReport, RoundOpts};
use crate::git::fixture::{commit_at, git, git_out, init};

fn node(root: &Path, rel: &str, body: &str) {
    let p = root.join(".yidam/corpus").join(rel);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, body).unwrap();
}

fn commit(root: &Path, day: &str, msg: &str) {
    commit_at(root, msg, &format!("{day}T00:00:00Z"));
}

/// 2026-06-01, as a day number.
fn today() -> i64 {
    crate::dates::days_from_civil_str("2026-06-01").unwrap()
}

/// `a` and `b` linked to each other, which is clear, and `lonely` linked to nothing, which
/// is not: one node carrying findings, so a round can resolve them.
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
    node(
        root,
        "concept/lonely.yml",
        "class: concept\nlabel: Lonely\n",
    );
    commit(root, "2026-01-01", "establish: a, b and lonely");
    tmp
}

fn round(root: &Path, opts: RoundOpts) -> (CycleReport, Round) {
    let mut report = read_cycle(root, false, today()).unwrap();
    let now = with_round(root, &mut report, opts).unwrap();
    (report, now)
}

fn read_with(root: &Path, opts: RoundOpts) -> CycleReport {
    round(root, opts).0
}

const SINCE: RoundOpts = RoundOpts {
    since_last: true,
    record: false,
    brief: None,
};
const RECORD: RoundOpts = RoundOpts {
    since_last: false,
    record: true,
    brief: None,
};

fn nodes(fs: &[Finding]) -> Vec<&str> {
    let mut out: Vec<&str> = fs.iter().map(|f| f.node.as_str()).collect();
    out.dedup();
    out
}

/// The definition of done's first half: resolve one, introduce one, and the delta is that.
///
/// Asserted by node rather than by check id, because which checks a lone node trips is lint's
/// business — but the fixture's own findings are asserted first, or "everything on `lonely`
/// left" would pass over a corpus where nothing was ever found.
#[test]
fn a_round_that_resolves_one_and_introduces_one_reports_exactly_that() {
    let tmp = repo();
    let root = tmp.path();
    let (first, _) = round(root, RECORD);
    assert!(
        !first.findings.is_empty()
            && first
                .findings
                .iter()
                .all(|f| f.node.ends_with("lonely.yml")),
        "the fixture should carry findings on `lonely` and nowhere else: {:#?}",
        first.findings
    );

    // Resolve: link `lonely` into the pair, both ways. Introduce: a new lone node.
    node(
        root,
        "concept/lonely.yml",
        "class: concept\nlabel: Lonely\nlinks:\n  - target: ../concept/a.yml\n    relationship: reads\n",
    );
    node(
        root,
        "concept/a.yml",
        "class: concept\nlabel: A\nlinks:\n  - target: ../concept/b.yml\n    relationship: reads\n  - target: ../concept/lonely.yml\n    relationship: reads\n",
    );
    node(root, "concept/stray.yml", "class: concept\nlabel: Stray\n");
    commit(root, "2026-01-02", "establish: lonely joins, stray arrives");

    let (second, _) = round(root, SINCE);
    assert!(second.since_last_unread.is_none());
    let d = second.since_last.expect("a round was recorded");

    // Sorted, because a report lists findings in gate order and a round keeps them sorted.
    let shape = |fs: &[Finding]| {
        let mut s = fs
            .iter()
            .map(|f| (f.gate.clone(), f.check.clone()))
            .collect::<Vec<_>>();
        s.sort();
        s
    };
    let stray = first.findings[0].node.replace("lonely", "stray");
    assert_eq!(nodes(&d.findings.arrived), [stray.as_str()], "{d:#?}");
    assert_eq!(
        nodes(&d.findings.left),
        [first.findings[0].node.as_str()],
        "{d:#?}"
    );
    // The same checks on both sides, because the two nodes were in the same shape.
    assert_eq!(shape(&d.findings.arrived), shape(&first.findings));
    assert_eq!(shape(&d.findings.left), shape(&first.findings));

    // Nothing else moved, and saying so is part of "exactly".
    for (half, m) in [
        ("due", &d.due),
        ("stale_steps", &d.stale_steps),
        ("in_flight", &d.in_flight),
        ("proposals", &d.proposals),
    ] {
        assert!(
            m.arrived.is_empty() && m.left.is_empty(),
            "{half} moved: {m:?}"
        );
    }
    assert_eq!(
        d.head,
        git_out(root, &["rev-list", "--max-parents=0", "HEAD"]),
        "the delta names the head the recorded round stood on"
    );
}

/// The definition of done's second half: no record is said, never shown as an empty delta.
#[test]
fn with_no_record_the_delta_is_unread_and_not_empty() {
    let tmp = repo();
    let r = read_with(tmp.path(), SINCE);
    assert!(r.since_last.is_none(), "{:?}", r.since_last);
    let why = r
        .since_last_unread
        .clone()
        .expect("--since-last with no record says why");
    assert!(why.contains("no round is recorded"), "{why}");
    assert!(why.contains("branch `main`"), "{why}");

    let text = render(&r, tmp.path());
    assert!(text.contains("no round is recorded"), "{text}");
    assert!(!text.contains("nothing moved"), "{text}");
}

/// A round compared with an unchanged record moved nothing, and says so: the other side of the
/// test above.
#[test]
fn a_round_recorded_and_unchanged_moved_nothing() {
    let tmp = repo();
    round(tmp.path(), RECORD);
    let r = read_with(tmp.path(), SINCE);
    let d = r.since_last.as_ref().expect("a round was recorded");
    assert!(d.lines().is_empty(), "{d:#?}");
    let text = render(&r, tmp.path());
    assert!(text.contains("nothing moved"), "{text}");
}

/// `--since-last --record` compares and then records, so the next comparison is against this
/// round, not the one before.
#[test]
fn since_last_with_record_compares_before_it_records() {
    let tmp = repo();
    let root = tmp.path();
    round(root, RECORD);
    node(root, "concept/stray.yml", "class: concept\nlabel: Stray\n");
    commit(root, "2026-01-02", "establish: stray");

    let both = RoundOpts {
        since_last: true,
        record: true,
        brief: None,
    };
    let d = read_with(root, both)
        .since_last
        .expect("compared against the first round");
    assert!(!d.findings.arrived.is_empty(), "stray arrived: {d:#?}");

    let again = read_with(root, SINCE).since_last.expect("recorded");
    assert!(again.lines().is_empty(), "{again:#?}");
}

/// The record is never staged: the directory ignores itself, in a repository whose own
/// `.gitignore` knows nothing of it — every derived repository scaffolded before this.
#[test]
fn a_recorded_round_leaves_the_working_tree_clean() {
    let tmp = repo();
    let root = tmp.path();
    assert_eq!(git_out(root, &["status", "--porcelain"]), "");
    round(root, RECORD);
    assert!(path(root, Some("main")).exists());
    assert_eq!(
        git_out(root, &["status", "--porcelain", "--untracked-files=all"]),
        "",
        "the round record would be staged by `git add -A`"
    );
}

/// Keyed by branch: a round on one branch is not the last round on another.
#[test]
fn a_round_on_another_branch_is_not_this_branchs_last() {
    let tmp = repo();
    let root = tmp.path();
    round(root, RECORD);
    git(root, &["checkout", "-q", "-b", "phase/other"]);

    let r = read_with(root, SINCE);
    assert!(r.since_last.is_none());
    assert!(
        r.since_last_unread
            .as_deref()
            .unwrap_or_default()
            .contains("branch `phase/other`"),
        "{:?}",
        r.since_last_unread
    );

    // And recording there leaves `main`'s alone, under the branch's own path.
    round(root, RECORD);
    assert!(path(root, Some("phase/other")).exists());
    git(root, &["checkout", "-q", "main"]);
    assert!(read_with(root, SINCE).since_last.is_some());
}

/// A detached HEAD has a record of its own, at a name no branch can take.
#[test]
fn a_detached_head_records_under_head() {
    let tmp = repo();
    let root = tmp.path();
    git(root, &["checkout", "-q", "--detach"]);
    let (_, now) = round(root, RECORD);
    assert_eq!(now.branch, None);
    assert!(path(root, None).ends_with(".yidam/loop/HEAD.json"));
    assert!(path(root, None).exists());
    assert!(read_with(root, SINCE).since_last.is_some());
}

/// A record this binary cannot read is reported unread, with the reason — not as no record,
/// and not as a delta against whatever half-parsed.
#[test]
fn a_record_in_another_format_is_unread_and_says_why() {
    let tmp = repo();
    let root = tmp.path();
    round(root, RECORD);
    let p = path(root, Some("main"));
    let text = std::fs::read_to_string(&p).unwrap();
    assert!(text.contains("\"format\": 1"), "{text}");
    std::fs::write(&p, text.replace("\"format\": 1", "\"format\": 99")).unwrap();

    let why = read_with(root, SINCE).since_last_unread.unwrap();
    assert!(why.contains("format 99"), "{why}");
    assert!(!why.contains("no round is recorded"), "{why}");

    std::fs::write(&p, "{ not json").unwrap();
    let why = read_with(root, SINCE).since_last_unread.unwrap();
    assert!(why.contains("not JSON"), "{why}");
}

/// A proposal head opened between rounds arrives, by its branch name.
#[test]
fn a_proposal_opened_between_rounds_arrives() {
    let tmp = repo();
    let root = tmp.path();
    round(root, RECORD);
    git(root, &["branch", "propose/2026-01-02-x"]);
    let d = read_with(root, SINCE).since_last.unwrap();
    assert_eq!(d.proposals.arrived, ["propose/2026-01-02-x"]);
    assert!(d.proposals.left.is_empty());
}

/// Multiset accounting, the baseline's: one more occurrence of one finding is an arrival.
#[test]
fn a_second_occurrence_of_one_finding_is_an_arrival() {
    let f = |n: &str| Finding {
        gate: "lint".into(),
        check: "c".into(),
        node: n.into(),
    };
    let m = Moved::between(&[f("x")], &[f("x"), f("x")]);
    assert_eq!(m.arrived, [f("x")]);
    assert!(m.left.is_empty());
    let m = Moved::between(&[f("x"), f("y")], &[f("y")]);
    assert_eq!(m.left, [f("x")]);
    assert!(m.arrived.is_empty());
}

/// The wire shape of a delta, asserted at the key level: `report.schema.json` declares these
/// names and no golden reaches them (`UNREACHED` in `report_goldens.rs` points here).
#[test]
fn the_delta_serializes_under_the_schemas_names() {
    let tmp = repo();
    round(tmp.path(), RECORD);
    let v = serde_json::to_value(read_with(tmp.path(), SINCE)).unwrap();
    let keys = |v: &serde_json::Value| {
        let mut k: Vec<String> = v.as_object().unwrap().keys().cloned().collect();
        k.sort();
        k
    };
    assert_eq!(
        keys(&v["since_last"]),
        [
            "due",
            "findings",
            "head",
            "in_flight",
            "proposals",
            "stale_steps"
        ]
    );
    for half in ["due", "findings", "in_flight", "proposals", "stale_steps"] {
        assert_eq!(keys(&v["since_last"][half]), ["arrived", "left"], "{half}");
    }
    assert!(v.get("since_last_unread").is_none());
    assert!(
        v.get("findings").is_none(),
        "a round's findings are not on the wire"
    );

    // And neither field is present when nothing asked: absent is "not asked".
    let plain = serde_json::to_value(read_cycle(tmp.path(), false, today()).unwrap()).unwrap();
    assert!(plain.get("since_last").is_none());
    assert!(plain.get("since_last_unread").is_none());
}

/// The brief stays under its budget, keeps its header and closing line, and says what it cut.
///
/// Run over a corpus with many findings, because a brief that fits when there is little to say
/// has not been tested against its budget.
#[test]
fn the_brief_fits_its_budget_and_says_what_it_cut() {
    let tmp = repo();
    let root = tmp.path();
    for i in 0..30 {
        node(
            root,
            &format!("concept/stray{i}.yml"),
            "class: concept\nlabel: S\n",
        );
    }
    commit(root, "2026-01-02", "establish: strays");
    let brief_of = |budget: usize| {
        let opts = RoundOpts {
            brief: Some(budget),
            ..SINCE
        };
        let (r, now) = round(root, opts);
        render_brief(&r, &now, budget)
    };

    for budget in [120, 250, 500] {
        let text = brief_of(budget);
        assert!(
            text.len() / 4 <= budget,
            "{} tokens over {budget}:\n{text}",
            text.len() / 4
        );
        assert!(
            text.starts_with("yidam cycle --brief — branch main at "),
            "{text}"
        );
        assert!(
            text.ends_with("`yidam cycle` is the full report."),
            "{text}"
        );
        assert!(
            text.contains("next:"),
            "the acts come first and survive a cut:\n{text}"
        );
    }
    let tight = brief_of(120);
    assert!(tight.contains("past the 120-token budget"), "{tight}");
    // One long half is capped, so it cannot spend the budget alone.
    let roomy = brief_of(5000);
    assert!(roomy.contains("…and "), "{roomy}");
    assert!(!roomy.contains("past the"), "{roomy}");
    // And with no record, the brief says so rather than going quiet about it.
    assert!(roomy.contains("no round is recorded"), "{roomy}");
}
