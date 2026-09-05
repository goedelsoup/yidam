//! `kuten check` must recognize the six repositories that defined `inquiry` — #574's proof
//! obligation, and the end-to-end plumbing that carries it.
//!
//! > A declared `inquiry` that fails to recognise the six repositories which defined it is a
//! > wrong extraction.
//!
//! # What is and is not proved here
//!
//! Those six corpora are not in this checkout and cannot be, so the obligation is encoded as
//! their measured **shapes**: the cluster A0 published is four ranges — phase commits 13–26%,
//! nodes per commit 0.50–1.11, median node 35–62 lines, off-vocabulary exactly 0% — over six
//! repositories in six unrelated domains, and no per-repository table was published. So the
//! fixtures under `fixtures/kuten/` sit on the cluster's endpoints and through its interior,
//! and what they pin is that a repository anywhere inside the published cluster is recognized
//! and that the arithmetic between a raw count and a band never loses one.
//!
//! **The run against the real six is the owner's to do.** These fixtures cannot substitute
//! for it: they can show the extraction is faithful to the numbers A0 reported, and they
//! cannot show the numbers A0 reported are what those repositories still are.
//!
//! # The controls are the other half
//!
//! Four fixtures are deliberately outside the cluster, and the pair that matters is
//! `control-vintage-prelude` and `control-stopped-phasing`: identical zero phase commits,
//! opposite readings. One repository could not have run a phase; the other could and did not.
//! Collapsing those two is the error that produced a second cluster which was not there.
//!
//! The fixture set is **discovered**, and a fixture whose expectations do not name a metric
//! must conform on it — so adding a fixture is a decision about every metric, not only the
//! one it was added for.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use yidam::kuten::{compare, Measurement, Profile, Verdict, Vintage};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/kuten")
}

/// The profile under test is the one this repository ships, read off disk.
///
/// Not a copy in this file. A test holding its own copy of the bands would go on passing
/// after somebody changed the shipped profile, which is the extraction it exists to check.
fn inquiry() -> Profile {
    let path = repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    Profile::parse(&text).expect("the shipped inquiry profile parses")
}

struct Fixture {
    name: String,
    note: String,
    cluster: bool,
    measurement: Measurement,
    vintage: Vintage,
    /// Metric id → expected verdict. A metric absent here must conform.
    expect: BTreeMap<String, String>,
}

fn fixtures() -> Vec<Fixture> {
    let mut out = Vec::new();
    let entries = std::fs::read_dir(fixture_dir()).expect("tests/fixtures/kuten/ exists");
    let mut paths: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "toml"))
        .collect();
    paths.sort();

    for path in paths {
        let raw = std::fs::read_to_string(&path).expect("a fixture");
        let doc: toml::Value =
            toml::from_str(&raw).unwrap_or_else(|e| panic!("{} is not TOML ({e})", path.display()));
        let name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        let measurement: Measurement = doc["measurement"]
            .clone()
            .try_into()
            .unwrap_or_else(|e| panic!("{name}: measurement ({e})"));
        let vintage_table = &doc["vintage"];
        let flag = |key: &str| {
            vintage_table
                .get(key)
                .and_then(toml::Value::as_bool)
                .unwrap_or_else(|| panic!("{name}: vintage.{key}"))
        };
        let expect = doc
            .get("expect")
            .and_then(toml::Value::as_table)
            .map(|t| {
                t.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            })
            .unwrap_or_default();
        out.push(Fixture {
            note: doc["note"].as_str().expect("a note").to_string(),
            cluster: doc["cluster"].as_bool().expect("cluster"),
            measurement,
            vintage: Vintage {
                has_phase_verb: flag("has_phase_verb"),
                vocabulary_is_closed: flag("vocabulary_is_closed"),
                graph_present: flag("graph_present"),
            },
            expect,
            name,
        });
    }
    out
}

fn tag(v: Verdict) -> &'static str {
    match v {
        Verdict::Conforming => "conforming",
        Verdict::Divergent => "divergent",
        Verdict::Vintage => "vintage",
        Verdict::Unmeasurable => "unmeasurable",
    }
}

/// The floor, and the count.
///
/// Six is not decoration: the cluster is six repositories, and a fixture set that quietly
/// dropped to four would still pass every assertion below.
#[test]
fn six_shapes_define_the_cluster_and_the_rest_are_controls() {
    let found = fixtures();
    let cluster: Vec<&Fixture> = found.iter().filter(|f| f.cluster).collect();
    assert_eq!(
        cluster.len(),
        6,
        "the cluster is six repositories; {} fixture(s) claim to be one of them: {:?}",
        cluster.len(),
        cluster.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
    assert!(
        found.len() > cluster.len(),
        "no control fixtures — nothing here shows the check can tell a non-member apart"
    );
    for f in &found {
        assert!(
            !f.note.trim().is_empty(),
            "{}: a fixture says what it is",
            f.name
        );
    }
}

/// The obligation, run backward: every shape inside the cluster is recognized.
#[test]
fn every_shape_in_the_cluster_conforms() {
    let profile = inquiry();
    for f in fixtures().iter().filter(|f| f.cluster) {
        let findings = compare(&profile, &f.measurement, &f.vintage);
        assert_eq!(findings.len(), 4, "{}: four bands, four findings", f.name);
        for finding in &findings {
            assert_eq!(
                finding.verdict,
                Verdict::Conforming,
                "{} ({}) is inside the cluster and `{}` reports {} — the extraction is wrong, \
                 not the repository. Measured {}, declared {}.",
                f.name,
                f.note,
                finding.metric,
                tag(finding.verdict),
                finding.measured,
                finding.declared
            );
        }
    }
}

/// Every fixture's every metric lands on the verdict the fixture declares.
///
/// The expectations are per metric and default to `conforming`, so a control that names one
/// divergence is also asserting the other three metrics were left alone. A control that made
/// everything divergent would prove nothing about the metric it was written for.
#[test]
fn every_fixture_lands_on_the_verdict_it_declares() {
    let profile = inquiry();
    for f in fixtures() {
        let findings = compare(&profile, &f.measurement, &f.vintage);
        let ids: BTreeSet<&str> = findings.iter().map(|x| x.metric).collect();
        for named in f.expect.keys() {
            assert!(
                ids.contains(named.as_str()),
                "{}: expects `{named}`, which no finding carries. Known: {ids:?}",
                f.name
            );
        }
        for finding in &findings {
            let want = f
                .expect
                .get(finding.metric)
                .map(String::as_str)
                .unwrap_or("conforming");
            assert_eq!(
                tag(finding.verdict),
                want,
                "{} ({}): `{}` measured {} against a declared {}",
                f.name,
                f.note,
                finding.metric,
                finding.measured,
                finding.declared
            );
        }
    }
}

/// **A vintage artifact is never reported as divergence** — #574's second prohibition, and
/// the whole lesson of A0's retraction, asserted on the pair that isolates it.
///
/// The two fixtures carry the same zero. One repository's vendored prelude has the `phase`
/// verb and one's does not, and that is the only difference between them.
#[test]
fn the_same_zero_reads_two_ways_and_only_the_prelude_decides() {
    let profile = inquiry();
    let all = fixtures();
    let by = |name: &str| {
        all.iter()
            .find(|f| f.name == name)
            .unwrap_or_else(|| panic!("the {name} fixture"))
    };
    let old = by("control-vintage-prelude");
    let current = by("control-stopped-phasing");
    assert_eq!(old.measurement.phase_commits, 0);
    assert_eq!(current.measurement.phase_commits, 0);

    let verdict_of = |f: &Fixture, metric: &str| {
        compare(&profile, &f.measurement, &f.vintage)
            .into_iter()
            .find(|x| x.metric == metric)
            .map(|x| x.verdict)
            .expect("the metric")
    };
    assert_eq!(verdict_of(old, "phase-commit-share"), Verdict::Vintage);
    assert_eq!(
        verdict_of(current, "phase-commit-share"),
        Verdict::Divergent
    );

    // And no vintage fixture is ever divergent anywhere. A blanket exemption would be just
    // as wrong in the other direction, so the node-shape metrics must still be read.
    for finding in compare(&profile, &old.measurement, &old.vintage) {
        assert_ne!(
            finding.verdict,
            Verdict::Divergent,
            "`{}` reports a repository whose prelude could not have done this as divergent",
            finding.metric
        );
    }
    assert!(
        compare(&profile, &old.measurement, &old.vintage)
            .iter()
            .any(|f| f.verdict == Verdict::Conforming),
        "a vintage prelude exempted every metric, including the ones it does not gate"
    );
}

// ── end to end, through the binary ────────────────────────────────────────────

struct Run {
    stdout: String,
    code: i32,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("running yidam");
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A repository holding a kuten: the vendored profile, the decision record, a corpus, and a
/// history built out of the closed vocabulary.
fn stage(declared_revision: u32) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let write = |rel: &str, body: &str| {
        let p = root.join(rel);
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, body).unwrap();
    };

    // The vendored prelude, at a vintage that carries both gated capabilities.
    write(
        ".yidam/.vendor/prelude/GRAPH.md",
        "# Graph\n\n## Commit vocabulary\n\nThis list is closed.\n\n| Verb | When |\n\
         |---|---|\n| `phase` | A phase settled |\n",
    );
    let profile =
        std::fs::read_to_string(repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml"))
            .expect("the shipped profile");
    write(".yidam/.vendor/prelude/kuten/inquiry/kuten.yml", &profile);
    write(
        ".yidam/decisions/kuten.yml",
        &format!("kuten: inquiry\nrevision: {declared_revision}\n"),
    );

    // Four nodes of 48 lines each: a 48-line median, inside the 35–62 band.
    let node = format!("title: a node\ndescription: |\n{}", "  line\n".repeat(46));
    for n in 0..4 {
        write(&format!(".yidam/corpus/concept/n{n}.yml"), &node);
    }
    write(
        "AGENTS.md",
        "# Agents\n\n<!-- REGEN: yidam kuten\n-->\n_stale_\n<!-- /REGEN -->\n",
    );

    let git = |args: &[&str]| {
        Command::new("git")
            .current_dir(root)
            .args(args)
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .status()
            .unwrap();
    };
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "fixture@yidam.test"]);
    git(&["config", "user.name", "Fixture"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: the corpus"]);
    // Five more commits, one of them settling a phase: 1 of 6 is 17%, inside 13–26%. Four
    // nodes over six commits is 0.67, inside 0.50–1.11.
    for (n, subject) in [
        "establish: a node",
        "open: a question",
        "phase: the first unit settles",
        "revise: the node",
        "synthesize: two threads",
    ]
    .iter()
    .enumerate()
    {
        write(&format!("notes/{n}.md"), "a step\n");
        git(&["add", "-A"]);
        git(&["commit", "-q", "-m", subject]);
    }
    tmp
}

/// The whole command, over a repository that holds one: read-only, exit zero, conforming.
#[test]
fn a_repository_holding_inquiry_is_read_against_it_and_nothing_is_written() {
    let tmp = stage(1);
    let before = std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();

    let r = run(tmp.path(), &["kuten", "check", "--format", "json"]);
    assert_eq!(r.code, 0, "check exits zero: {}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    assert_eq!(v["kuten"]["held"], true);
    assert_eq!(v["kuten"]["name"], "inquiry");
    assert_eq!(v["kuten"]["revision_skew"], false);
    assert_eq!(v["kuten"]["conforming"], true, "{}", r.stdout);
    assert_eq!(v["kuten"]["findings"].as_array().unwrap().len(), 4);

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap(),
        before,
        "`kuten check` wrote to the repository it was pointed at"
    );
}

/// A comparison across revisions is annotated rather than silently made.
#[test]
fn a_revision_the_profile_has_moved_past_is_annotated() {
    let tmp = stage(9);
    let r = run(tmp.path(), &["kuten", "check", "--format", "json"]);
    assert_eq!(r.code, 0, "annotating is not refusing to exit zero");
    let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
    assert_eq!(v["kuten"]["revision_skew"], true);
    assert_eq!(v["kuten"]["declared_revision"], 9);

    let text = run(tmp.path(), &["kuten", "check"]);
    assert!(
        text.stdout.contains("across revisions"),
        "the annotation has to be legible in prose too: {}",
        text.stdout
    );
}

/// `yidam kuten` fills its block, and `regen --check` then agrees.
#[test]
fn the_agents_block_is_written_by_the_generator_and_checked_by_the_gate() {
    let tmp = stage(1);
    let stale = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(stale.code, 1, "a stale block has to fail: {}", stale.stdout);
    assert!(stale.stdout.contains("(kuten)"), "{}", stale.stdout);

    let wrote = run(tmp.path(), &["kuten"]);
    assert_eq!(wrote.code, 0, "{}", wrote.stdout);
    let agents = std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap();
    assert!(agents.contains("`inquiry`"), "{agents}");
    assert!(agents.contains("revision 1"), "{agents}");
    assert!(!agents.contains("_stale_"), "{agents}");

    let after = run(tmp.path(), &["regen", "--check"]);
    assert_eq!(
        after.code, 0,
        "the gate has to be satisfiable: {}",
        after.stdout
    );
}

/// `doctor` names the kuten and its revision — RFC-0028 §9.
#[test]
fn doctor_reports_which_kuten_is_held() {
    let tmp = stage(1);
    let r = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    let check = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "kuten")
        .expect("a kuten check");
    assert_eq!(check["verdict"], "ok");
    assert!(
        check["detail"].as_str().unwrap().contains("revision 1"),
        "{check}"
    );
}

/// Holding none is a supported state, and `doctor` does not call it a fault.
#[test]
fn a_repository_holding_no_kuten_reports_as_one() {
    let tmp = stage(1);
    std::fs::remove_file(tmp.path().join(".yidam/decisions/kuten.yml")).unwrap();

    let r = run(tmp.path(), &["kuten", "check"]);
    assert_eq!(r.code, 0);
    assert!(r.stdout.contains("supported state"), "{}", r.stdout);

    let d = run(tmp.path(), &["doctor", "--format", "json"]);
    let v: serde_json::Value = serde_json::from_str(&d.stdout).unwrap();
    let check = v["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "kuten")
        .unwrap();
    assert_eq!(check["verdict"], "ok", "holding none is not a warning");
}
