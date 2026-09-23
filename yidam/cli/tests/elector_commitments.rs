//! A seat's commitments, read across its own history (#294).
//!
//! The unit tests in `lint::commitments` are pure over one revision of one file. They cannot
//! reach the part with teeth: `elector-commitment-vanished` is a statement about **two**
//! revisions, and the thing that decides it is a git walk. So these build a real repository, put
//! a real seat on a real branch, and move the file.
//!
//! Each case is a mutation of the sanctioned path and not an independent scenario, which is the
//! point — [`a_withdrawal_is_the_sanctioned_path`] and
//! [`a_ground_that_leaves_without_being_withdrawn_is_reported`] differ by one line of one
//! revision, and a check that cannot tell those two apart has no teeth at all.

use std::path::Path;
use std::process::Command;

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .env("GIT_AUTHOR_DATE", "@1700000000 +0000")
        .env("GIT_COMMITTER_DATE", "@1700000000 +0000")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

const HOLDS: &str = "## What this seat holds";
const WITHDRAWN: &str = "## What this seat has withdrawn";

const FILE: &str = ".yidam/sangha/commitments/advocate.md";

/// A commitments file naming each list of positions under each section.
fn commitments(holds: &[&str], withdrawn: &[&str]) -> String {
    let items = |ps: &[&str]| {
        ps.iter()
            .map(|p| format!("- a ground — [{p}](../positions/{p}.md)\n"))
            .collect::<String>()
    };
    format!(
        "# Commitments: ma/advocate\n\n{HOLDS}\n\n{}\n{WITHDRAWN}\n\n{}",
        items(holds),
        items(withdrawn)
    )
}

/// A repository with two positions filed by one seat, and a `ma/advocate` branch.
///
/// The corpus is empty on purpose: every check here reads `.yidam/sangha/` and refs, and a
/// corpus would only contribute findings that belong to other tests.
fn fixture() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join(".yidam/corpus")).unwrap();
    std::fs::create_dir_all(root.join(".yidam/sangha/positions")).unwrap();
    for p in ["advocate-a", "advocate-b"] {
        std::fs::write(
            root.join(format!(".yidam/sangha/positions/{p}.md")),
            format!("# Position: {p}\n\nThe case.\n"),
        )
        .unwrap();
    }
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "tester@example.org"]);
    git(root, &["config", "user.name", "Tester"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "genesis: two positions"]);
    git(root, &["switch", "-q", "-c", "ma/advocate"]);
    dir
}

/// Commit each revision of the commitments file onto `ma/advocate`, then return to the baseline.
///
/// `None` deletes the file, which is a revision like any other and is the case a seat would
/// reach for if deleting were the cheap way out.
fn history(root: &Path, revisions: &[Option<String>]) {
    std::fs::create_dir_all(root.join(".yidam/sangha/commitments")).unwrap();
    for (i, revision) in revisions.iter().enumerate() {
        match revision {
            Some(text) => std::fs::write(root.join(FILE), text).unwrap(),
            None => std::fs::remove_file(root.join(FILE)).unwrap(),
        }
        git(root, &["add", "-A"]);
        git(root, &["commit", "-q", "-m", &format!("revise: round {i}")]);
    }
    git(root, &["switch", "-q", "main"]);
}

/// Every finding `yidam lint` reports for one check, as `node — detail`.
fn findings(root: &Path, id: &str) -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["lint", "--format", "json", "--warn"])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap_or_else(|e| {
        panic!(
            "lint --format json: {e}\nstdout: {stdout}\nstderr: {}",
            String::from_utf8_lossy(&out.stderr)
        )
    });
    let check = report["checks"]
        .as_array()
        .expect("checks")
        .iter()
        .find(|c| c["id"].as_str() == Some(id))
        .unwrap_or_else(|| {
            panic!(
                "no check `{id}` in the report — the registry has {} checks",
                report["checks"].as_array().map_or(0, Vec::len)
            )
        });
    check["violations"]
        .as_array()
        .expect("violations")
        .iter()
        .map(|v| {
            format!(
                "{} — {}",
                v["node"].as_str().unwrap_or("?"),
                v["detail"].as_str().unwrap_or("?")
            )
        })
        .collect()
}

/// The shape the protocol prescribes: the ground moves to the other section, and nothing fires.
#[test]
fn a_withdrawal_is_the_sanctioned_path() {
    let dir = fixture();
    let root = dir.path();
    history(
        root,
        &[
            Some(commitments(&["advocate-a", "advocate-b"], &[])),
            Some(commitments(&["advocate-a"], &["advocate-b"])),
        ],
    );
    assert!(
        findings(root, "elector-commitment-vanished").is_empty(),
        "withdrawing a ground is the act the loop exists to produce and must never be reported"
    );
    assert!(
        findings(root, "elector-position-unindexed").is_empty(),
        "both positions are named"
    );
    assert!(findings(root, "elector-commitments-malformed").is_empty());
    assert!(findings(root, "elector-commitments-absent").is_empty());
}

/// One line different from the case above, and it gates.
#[test]
fn a_ground_that_leaves_without_being_withdrawn_is_reported() {
    let dir = fixture();
    let root = dir.path();
    history(
        root,
        &[
            Some(commitments(&["advocate-a", "advocate-b"], &[])),
            Some(commitments(&["advocate-a"], &[])),
        ],
    );
    let vanished = findings(root, "elector-commitment-vanished");
    assert_eq!(vanished.len(), 1, "{vanished:?}");
    assert!(
        vanished[0].starts_with(".yidam/sangha/positions/advocate-b.md — "),
        "the finding is about the position that argued the ground: {vanished:?}"
    );
    assert!(
        vanished[0].contains("ma/advocate"),
        "and it names the seat: {vanished:?}"
    );
    let unindexed = findings(root, "elector-position-unindexed");
    assert_eq!(
        unindexed.len(),
        1,
        "the position is also no longer indexed: {unindexed:?}"
    );
}

/// Deleting the file is the cheapest way out, and it is routed to the check that gates.
#[test]
fn deleting_the_file_reports_every_ground_it_held() {
    let dir = fixture();
    let root = dir.path();
    history(
        root,
        &[Some(commitments(&["advocate-a", "advocate-b"], &[])), None],
    );
    let vanished = findings(root, "elector-commitment-vanished");
    assert_eq!(
        vanished.len(),
        2,
        "both held grounds went with the file: {vanished:?}"
    );
    assert!(
        findings(root, "elector-commitments-absent").len() == 1,
        "and the seat is also reported as carrying no index"
    );
}

/// A file with no headings cannot be believed, so the vanish walk does not believe it.
///
/// This is the mutation that matters: if the malformed revision were read as two empty
/// sections, both grounds would report as vanished and the Error would be one the seat caused
/// by writing a worse file rather than by discarding anything.
#[test]
fn a_malformed_revision_gates_on_its_own_and_is_not_read_as_empty() {
    let dir = fixture();
    let root = dir.path();
    history(
        root,
        &[
            Some(commitments(&["advocate-a", "advocate-b"], &[])),
            Some("# Commitments: ma/advocate\n\nnothing structured here\n".to_string()),
        ],
    );
    let malformed = findings(root, "elector-commitments-malformed");
    assert_eq!(malformed.len(), 1, "{malformed:?}");
    assert!(
        malformed[0].contains(HOLDS) && malformed[0].contains(WITHDRAWN),
        "it names both missing headings: {malformed:?}"
    );
    assert!(
        findings(root, "elector-commitment-vanished").is_empty(),
        "a revision that cannot be read is skipped, not treated as having lost everything"
    );
}

/// A seat that has argued something and kept no index is reported, and only at Info.
#[test]
fn a_seat_with_positions_and_no_commitments_file_is_reported_once() {
    let dir = fixture();
    let root = dir.path();
    git(root, &["switch", "-q", "main"]);
    let absent = findings(root, "elector-commitments-absent");
    assert_eq!(absent.len(), 1, "{absent:?}");
    assert!(absent[0].starts_with("ma/advocate — "), "{absent:?}");
    assert!(
        absent[0].contains("2 position"),
        "it says how many positions the seat has filed: {absent:?}"
    );
    assert!(
        findings(root, "elector-position-unindexed").is_empty(),
        "there is no index to be missing from — the absent check is the whole finding"
    );
}

/// Wording is not identity: an item may be rewritten freely as long as the link stays.
#[test]
fn rewording_an_item_is_not_a_vanished_ground() {
    let dir = fixture();
    let root = dir.path();
    history(
        root,
        &[
            Some(commitments(&["advocate-a", "advocate-b"], &[])),
            Some(format!(
                "# Commitments: ma/advocate\n\n{HOLDS}\n\n\
                 - an entirely different sentence — [a](../positions/advocate-a.md)\n\
                 - and another — [b](../positions/advocate-b.md)\n\n{WITHDRAWN}\n"
            )),
        ],
    );
    assert!(
        findings(root, "elector-commitment-vanished").is_empty(),
        "the item's identity is the position it links, and both links stand"
    );
}
