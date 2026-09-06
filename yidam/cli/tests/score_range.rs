//! `yidam score <range>` against real histories — #286, A5 of the kuten epic (#572).
//!
//! An integration test rather than unit tests, and for the reason `query_history.rs` gives:
//! every property below is about what git objects say, and there is no way to assert them
//! without a history. The pure scoring is unit-tested in `src/score.rs`; what is here is
//! everything that only a repository can answer.
//!
//! Four fixtures, each built to make one arm reachable:
//!
//! | | `adopting` | `conventional` | `barren` | `revised` |
//! |---|---|---|---|---|
//! | recognized verbs | yes | none | yes | yes |
//! | nodes added in range | 3 | 1 | 0 | 1 |
//! | kuten held | no | no | no | yes, and changed mid-range |
//!
//! # Nothing here hardcodes the criteria
//!
//! The set every assertion runs over is read out of `yidam/prelude/kuten/inquiry/kuten.yml`,
//! which is the document that settles it. A test literal would go on passing while the
//! profile grew a criterion nothing exercised — #661 exactly, one layer over.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

fn repo_root() -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = yidam/cli/
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The criteria the shipped profile declares, read out of the profile.
fn criteria() -> Vec<String> {
    let path = repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml");
    let text = std::fs::read_to_string(&path).expect("the inquiry profile");
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).expect("the profile is YAML");
    let ids: Vec<String> = doc["rubric"]["criteria"]
        .as_sequence()
        .expect("`rubric.criteria` is a sequence")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert!(
        !ids.is_empty(),
        "read no criterion out of {} — this file is asserting nothing",
        path.display()
    );
    ids
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

fn repo() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    git(dir, &["init", "-q", "-b", "main"]);
    git(dir, &["config", "user.email", "t@t.co"]);
    git(dir, &["config", "user.name", "T"]);
    tmp
}

fn write(dir: &Path, rel: &str, body: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

fn commit(dir: &Path, subject: &str) {
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-qm", subject]);
}

/// The ontology every fixture shares: one class that is pointed at, one that points.
fn ontology(dir: &Path) {
    write(
        dir,
        ".yidam/corpus/reach.ont.yml",
        "class: reach\nedges:\n  - relationship: measured-by\n    target: gage\n    \
         direction: out\n",
    );
    write(dir, ".yidam/corpus/gage.ont.yml", "class: gage\n");
}

/// A corpus that adopted the vocabulary: recognized verbs, and three nodes in range.
///
/// The genesis commit is outside the scored range, so `HEAD~3..HEAD` is a session.
fn adopting() -> tempfile::TempDir {
    let tmp = repo();
    let dir = tmp.path();
    ontology(dir);
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon Outlet\n",
    );
    commit(dir, "genesis: the corpus");

    // Linked from a reach, so it lands.
    write(
        dir,
        ".yidam/corpus/gage/valley.yml",
        "class: gage\nlabel: Valley Bridge\n",
    );
    write(
        dir,
        ".yidam/corpus/reach/tailwater.yml",
        "class: reach\nlabel: Tailwater\nlinks:\n  - target: ../gage/valley.yml\n    \
         relationship: measured-by\n",
    );
    commit(
        dir,
        "establish: the tailwater reach and the gage that measures it",
    );

    // Nothing points at it, and it is an open question.
    write(
        dir,
        ".yidam/corpus/gage/unnamed.yml",
        // Quoted, because a bare `? ` opens an explicit key in YAML and the node would not
        // parse at all — which reads here as a node that is simply not a question.
        "class: gage\nlabel: \"? which gage the 1974 record refers to\"\n",
    );
    commit(dir, "open: which gage the 1974 record refers to");

    write(dir, "README.md", "# A repository\n");
    commit(dir, "regen: the readme block");
    tmp
}

/// A corpus that never adopted the vocabulary. Every subject is a conventional commit.
fn conventional() -> tempfile::TempDir {
    let tmp = repo();
    let dir = tmp.path();
    ontology(dir);
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon Outlet\n",
    );
    commit(dir, "chore: initial import");

    write(
        dir,
        ".yidam/corpus/gage/valley.yml",
        "class: gage\nlabel: Valley Bridge\n",
    );
    commit(dir, "feat: a second gage");
    tmp
}

/// A range that touched no corpus node at all.
fn barren() -> tempfile::TempDir {
    let tmp = repo();
    let dir = tmp.path();
    ontology(dir);
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon Outlet\n",
    );
    commit(dir, "genesis: the corpus");
    write(dir, "README.md", "# A repository\n");
    commit(dir, "regen: the readme block");
    tmp
}

/// A corpus that adopted a kuten and then superseded the decision mid-range.
///
/// The profile is **the shipped one**, vendored where genesis would have put it, so the
/// criteria this fixture is scored on are the ones `inquiry` actually declares rather than a
/// second copy written here.
fn revised() -> tempfile::TempDir {
    let tmp = repo();
    let dir = tmp.path();
    ontology(dir);
    write(
        dir,
        ".yidam/corpus/gage/canyon.yml",
        "class: gage\nlabel: Canyon Outlet\n",
    );
    let profile = repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml");
    write(
        dir,
        ".yidam/.vendor/prelude/kuten/inquiry/kuten.yml",
        &std::fs::read_to_string(&profile).expect("the shipped profile"),
    );
    write(
        dir,
        ".yidam/decisions/kuten.yml",
        "kuten: inquiry\nrevision: 1\n",
    );
    commit(dir, "genesis: the corpus");

    write(
        dir,
        ".yidam/corpus/gage/valley.yml",
        "class: gage\nlabel: Valley Bridge\n",
    );
    commit(dir, "establish: a second gage");

    write(
        dir,
        ".yidam/decisions/kuten.yml",
        "kuten: inquiry\nrevision: 2\n",
    );
    commit(dir, "decide: the kuten at revision 2 supersedes revision 1");
    tmp
}

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn score(dir: &Path, args: &[&str]) -> Run {
    let mut argv = vec!["score"];
    argv.extend_from_slice(args);
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(&argv)
        .current_dir(dir)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn json(dir: &Path, args: &[&str]) -> serde_json::Value {
    let mut argv = args.to_vec();
    argv.extend_from_slice(&["--format", "json"]);
    let run = score(dir, &argv);
    assert_eq!(run.code, 0, "{}{}", run.stdout, run.stderr);
    serde_json::from_str(&run.stdout).unwrap_or_else(|e| panic!("{e}\n{}", run.stdout))
}

/// The rows, keyed by criterion.
fn rows(report: &serde_json::Value) -> BTreeMap<String, serde_json::Value> {
    report["score"]["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|r| {
            (
                r["criterion"].as_str().expect("a criterion id").to_string(),
                r.clone(),
            )
        })
        .collect()
}

/// Every file's bytes, keyed by path. `.git/` excluded — nothing here should touch either.
fn tree(root: &Path) -> BTreeMap<std::path::PathBuf, Vec<u8>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| {
            (
                e.path().strip_prefix(root).unwrap().to_path_buf(),
                std::fs::read(e.path()).unwrap(),
            )
        })
        .collect()
}

/// **The default arm.** No repository on earth holds a kuten today, so the neutral arm is
/// what every reading actually exercises: the same criteria, and a report that says whose
/// selection they are.
#[test]
fn a_repository_with_no_kuten_is_scored_on_the_same_criteria_and_told_so() {
    let repo = adopting();
    let report = json(repo.path(), &["HEAD~3..HEAD"]);

    assert_eq!(report["score"]["held"], false);
    assert_eq!(report["score"]["kuten"], serde_json::Value::Null);
    let got: Vec<String> = rows(&report).into_keys().collect();
    let mut want = criteria();
    want.sort();
    assert_eq!(got, want, "the neutral arm must run the declared criteria");

    let text = score(repo.path(), &["HEAD~3..HEAD"]);
    assert_eq!(text.code, 0);
    assert!(text.stdout.contains("holds no kuten"), "{}", text.stdout);
    assert!(
        text.stdout.contains("not this corpus's"),
        "the neutral arm must say whose selection it used: {}",
        text.stdout
    );
}

/// Every declared criterion produces exactly one row, whatever the range holds.
#[test]
fn the_report_is_total_over_the_declared_criteria() {
    for (name, repo) in [
        ("adopting", adopting()),
        ("conventional", conventional()),
        ("barren", barren()),
    ] {
        let report = json(repo.path(), &["HEAD~1..HEAD"]);
        let got: Vec<String> = rows(&report).into_keys().collect();
        let mut want = criteria();
        want.sort();
        assert_eq!(got, want, "{name}");
    }
}

/// **The behavioural rule this criterion turns on, against a real history.**
///
/// `classify_commit` is total — Operational is the listed case and everything else falls
/// through to Epistemic — so over a range of conventional commits the naive share reads 1.00.
/// This must read nothing at all, and must say so.
#[test]
fn register_is_unmeasurable_over_a_range_with_no_recognized_verb() {
    let repo = conventional();
    let report = json(repo.path(), &["HEAD~1..HEAD"]);
    let register = rows(&report)["register"].clone();
    assert_eq!(register["reading"]["kind"], "unmeasurable", "{register}");
    assert!(
        register["reading"]["why"]
            .as_str()
            .unwrap_or_default()
            .contains("recognize"),
        "{register}"
    );
    assert!(
        register["reading"]["value"].is_null(),
        "an unmeasurable reading must carry no number: {register}"
    );
    // And the precondition rides along, so a reader can see *why* there was no denominator.
    assert!(
        register["note"]
            .as_str()
            .unwrap_or_default()
            .contains("legibility 0 of 1"),
        "{register}"
    );
}

/// The three readings, against a range whose answers are known by construction.
#[test]
fn the_readings_are_drawn_from_the_range_and_carry_their_evidence() {
    let repo = adopting();
    let report = json(repo.path(), &["HEAD~3..HEAD"]);
    let rows = rows(&report);

    // Three authored commits, all recognized: `establish` and `open` are epistemic, `regen`
    // is operational.
    let register = &rows["register"];
    assert_eq!(register["reading"]["kind"], "measured");
    assert_eq!(register["reading"]["shown"], "2 of 3 (67%)");

    // Three nodes added: the linked gage lands, the reach is a `reach` (a class nothing
    // points at, so exempt and counted as landed), and the unnamed gage does not.
    let landing = &rows["landing"];
    assert_eq!(landing["reading"]["shown"], "2 of 3 (67%)");

    // One of the three is an open question.
    assert_eq!(rows["questions"]["reading"]["shown"], "1 of 3 (33%)");

    // **The guard.** A number with nothing behind it is a claim nobody can check.
    for (id, row) in &rows {
        if row["reading"]["kind"] == "measured" {
            let evidence = row["evidence"].as_array().expect("evidence");
            assert!(
                !evidence.is_empty(),
                "`{id}` read a number and showed nothing"
            );
        }
    }
}

/// A range that added no node is unmeasurable on the node criteria — never 0.00.
#[test]
fn a_range_that_added_no_node_is_unmeasurable_rather_than_zero() {
    let repo = barren();
    let rows = rows(&json(repo.path(), &["HEAD~1..HEAD"]));
    for id in ["landing", "questions"] {
        assert_eq!(rows[id]["reading"]["kind"], "unmeasurable", "{id}");
    }
    // And the commit criterion still reads, because a range can be barren of nodes and not
    // of work.
    assert_eq!(rows["register"]["reading"]["kind"], "measured");
}

/// **Exit zero however it reads, and write nothing.**
///
/// A kuten binds nobody, and a report that gated would make divergence a defect. The worst
/// reading available — no recognized verb, no landed node, no question — must still exit
/// zero, and every invocation must leave the repository byte-identical.
#[test]
fn score_exits_zero_however_it_reads_and_writes_nothing() {
    for (name, repo) in [
        ("adopting", adopting()),
        ("conventional", conventional()),
        ("barren", barren()),
    ] {
        let before = tree(repo.path());
        assert!(!before.is_empty(), "{name} staged nothing");
        for args in [
            vec!["HEAD~1..HEAD"],
            vec!["HEAD~1..HEAD", "--brief"],
            vec!["HEAD~1..HEAD", "--format", "json"],
            vec!["HEAD~1"],
        ] {
            let run = score(repo.path(), &args);
            assert_eq!(run.code, 0, "{name} {args:?}: {}{}", run.stdout, run.stderr);
        }
        assert_eq!(
            tree(repo.path()),
            before,
            "{name}: score wrote to the repository"
        );
    }
}

/// The judged questions are opt-in, and nothing scores them.
#[test]
fn the_judged_questions_are_emitted_only_by_brief() {
    let repo = adopting();
    let quiet = json(repo.path(), &["HEAD~3..HEAD"]);
    assert_eq!(quiet["score"]["questions"].as_array().unwrap().len(), 0);

    let asked = json(repo.path(), &["HEAD~3..HEAD", "--brief"]);
    let questions = asked["score"]["questions"].as_array().unwrap();
    assert!(!questions.is_empty());
    // None of them is a row, and none of them has a number.
    let scored: Vec<String> = rows(&asked).into_keys().collect();
    for q in questions {
        assert!(
            !scored.contains(&q.as_str().unwrap().to_string()),
            "a judged question is also a scored row: {q}"
        );
    }
}

/// **The refusal, and it fires.**
///
/// #662 is the warning: the harness's own cross-version refusal has been inert for a protocol
/// version because nothing drove a version-to-version case through it. This drives one.
#[test]
fn a_range_spanning_a_kuten_revision_is_refused_and_names_both_ends() {
    let repo = revised();
    let run = score(repo.path(), &["HEAD~2..HEAD"]);
    assert_ne!(
        run.code, 0,
        "a range spanning a revision must refuse rather than answer: {}",
        run.stdout
    );
    let said = format!("{}{}", run.stdout, run.stderr);
    assert!(said.contains("revision 1"), "{said}");
    assert!(said.contains("revision 2"), "{said}");
    assert!(
        said.contains(".yidam/decisions/kuten.yml"),
        "the refusal must say what to look at: {said}"
    );

    // And it refuses *before* comparing: no rows, no reading, nothing to misread.
    assert!(!run.stdout.contains("register"), "{}", run.stdout);
}

/// A range that stays inside one revision is scored, so the refusal is a rule and not a wall.
#[test]
fn a_range_inside_one_revision_is_scored_and_names_the_practice() {
    let repo = revised();
    let report = json(repo.path(), &["HEAD~2..HEAD~1"]);
    assert_eq!(report["score"]["held"], true);
    assert_eq!(report["score"]["kuten"], "inquiry");
    assert_eq!(report["score"]["revision"], 1);
    let got: Vec<String> = rows(&report).into_keys().collect();
    let mut want = criteria();
    want.sort();
    assert_eq!(got, want);
}

/// A range is required, as `diff`'s is. A default range answers about a scope nobody chose.
#[test]
fn a_range_is_required() {
    let repo = adopting();
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .args(["score"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert_ne!(out.status.code(), Some(0));
}
