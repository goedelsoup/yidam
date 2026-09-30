//! #476's definition of done, one test per line of it where a line is testable here.

use std::collections::BTreeMap;
use std::path::Path;

use super::*;
use crate::git::fixture;

const GAGE: &str = "class: gage\nproperties:\n  - name: station\n    type: string\n  \
                    - name: peak_cfs\n    type: number\n  - name: state\n    type: string\n";
const QUESTION: &str = "class: question\n";

/// A peer bundle as `tonpa install` leaves it: a manifest with a commit, and a corpus.
fn peer(
    root: &Path,
    name: &str,
    commit: &str,
    class: &str,
    props: [&str; 2],
    nodes: &[(&str, &str, &str)],
) {
    let base = format!(".yidam/tonpa/{name}");
    fixture::write(
        root,
        &format!("{base}/manifest.yml"),
        &format!("commit: \"{commit}\"\n"),
    );
    fixture::write(
        root,
        &format!("{base}/corpus/{class}.ont.yml"),
        &format!(
            "class: {class}\nproperties:\n  - name: {}\n    type: string\n  - name: {}\n    \
             type: number\n",
            props[0], props[1]
        ),
    );
    for (file, id, peak) in nodes {
        fixture::write(
            root,
            &format!("{base}/corpus/{class}/{file}.yml"),
            &format!(
                "class: {class}\nlabel: {file}\nproperties:\n  {}: \"{id}\"\n  {}: {peak}\n",
                props[0], props[1]
            ),
        );
    }
}

const SPEC: &str = r#"
question = "What peak discharge has each gage recorded?"
query = "gage"
answer = "peak_cfs"
key = "station"
lands_as = "question"

[peers.alpha]
classes = { gage = "station" }
properties = { station = "usgs_id", peak_cfs = "flood_peak" }

[peers.beta]
classes = { gage = "site" }
properties = { station = "id", peak_cfs = "peak" }

[peers.gamma]
classes = { gage = "gauge" }
properties = { station = "gid", peak_cfs = "q_max" }

[peers.delta]
classes = { gage = "gauge" }
properties = { station = "gid", peak_cfs = "q_max" }

[peers.epsilon]
classes = { gage = "gauge" }
properties = { station = "gid" }

[peers.sibling]
classes = { gage = "gage" }
properties = { station = "station", peak_cfs = "peak_cfs" }

[peers.omega]
classes = { gage = "gage" }
properties = { station = "station", peak_cfs = "peak_cfs" }
"#;

/// Three answering peers — two agreeing, one not — plus one of every other outcome.
///
/// Returns the tempdir holding both the repository (`repo/`) and the path dependency's
/// sibling checkout (`sibling/`), which must sit outside it.
fn world() -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("repo");
    std::fs::create_dir_all(&root).unwrap();
    fixture::init(&root);
    fixture::write(&root, ".yidam/corpus/gage.ont.yml", GAGE);
    fixture::write(&root, ".yidam/corpus/question.ont.yml", QUESTION);
    fixture::write(
        &root,
        ".yidam/corpus/gage/local.yml",
        "class: gage\nlabel: A local gage\nproperties:\n  station: \"09999999\"\n",
    );
    peer(
        &root,
        "alpha",
        "aaa1111",
        "station",
        ["usgs_id", "flood_peak"],
        &[
            ("little-falls", "01646500", "12000"),
            ("other", "02000000", "800"),
        ],
    );
    peer(
        &root,
        "beta",
        "bbb2222",
        "site",
        ["id", "peak"],
        &[("potomac", "01646500", "12000")],
    );
    peer(
        &root,
        "gamma",
        "ccc3333",
        "gauge",
        ["gid", "q_max"],
        &[("lf", "01646500", "15500")],
    );
    peer(&root, "delta", "ddd4444", "gauge", ["gid", "q_max"], &[]);
    peer(
        &root,
        "epsilon",
        "eee5555",
        "gauge",
        ["gid", "q_max"],
        &[("x", "01646500", "1")],
    );
    peer(
        &root,
        "zeta",
        "fff6666",
        "gauge",
        ["gid", "q_max"],
        &[("z", "01646500", "2")],
    );

    let sibling = dir.path().join("sibling");
    fixture::write(&sibling, ".yidam/corpus/gage.ont.yml", GAGE);
    fixture::write(
        &sibling,
        ".yidam/corpus/gage/s.yml",
        "class: gage\nlabel: s\nproperties:\n  station: \"01646500\"\n  peak_cfs: 99\n",
    );
    fixture::write(
        &root,
        ".yidam/tonpa.toml",
        "[dependencies.sibling]\npath = \"../sibling\"\n",
    );
    fixture::write(&root, ".yidam/gathers/peaks.toml", SPEC);
    fixture::commit(&root, "genesis: seed");
    (dir, root)
}

fn opts(dry_run: bool, force: bool) -> Options {
    Options {
        name: "peaks".into(),
        dry_run,
        force,
        format: crate::report::Format::Text,
    }
}

fn outcomes(r: &GatherReport) -> BTreeMap<String, Outcome> {
    r.peers
        .iter()
        .map(|p| (p.package.clone(), p.outcome))
        .collect()
}

fn show(root: &Path, spec: &str) -> String {
    fixture::git_out(root, &["show", spec])
}

fn landed_node(root: &Path, r: &GatherReport, path: &str) -> crate::parse::CorpusInstance {
    let branch = &r.landed.as_ref().unwrap().branch;
    serde_yaml::from_str::<crate::parse::CorpusInstance>(&show(root, &format!("{branch}:{path}")))
        .unwrap()
}

#[test]
fn three_pinned_peers_land_a_question_citing_package_node_commit_and_a_verbatim_span() {
    let (_d, root) = world();
    let r = run(&root, &opts(false, false)).unwrap();
    let landed = r.landed.as_ref().expect("answered peers land a branch");
    assert!(
        landed.branch.starts_with("propose/gather/peaks/"),
        "{}",
        landed.branch
    );

    let node = landed_node(&root, &r, ".yidam/corpus/question/gather-peaks.yml");
    assert!(crate::claims::is_question_node(
        node.label.as_deref().unwrap()
    ));
    let cites = node.cites.unwrap();
    let packages: BTreeSet<_> = cites.iter().filter_map(|c| c.package.clone()).collect();
    assert_eq!(
        packages,
        ["alpha", "beta", "gamma"]
            .map(String::from)
            .into_iter()
            .collect()
    );
    for c in &cites {
        let (package, node, commit, span) = (
            c.package.as_deref().unwrap(),
            c.node.as_deref().unwrap(),
            c.commit.as_deref().unwrap(),
            c.span.as_deref().unwrap(),
        );
        let text =
            std::fs::read_to_string(root.join(format!(".yidam/tonpa/{package}/corpus/{node}.yml")))
                .unwrap();
        assert!(
            text.contains(span),
            "{span:?} is not verbatim in {package}::{node}"
        );
        let pin = crate::deps::read_manifest(&root.join(format!(".yidam/tonpa/{package}")))
            .unwrap()
            .commit
            .unwrap();
        assert_eq!(commit, pin);
    }
    // The gate agrees: every landed citation resolves, is pinned and has not drifted.
    let deps = citations::installed(&root);
    for c in &cites {
        assert!(citations::findings(c, &deps).is_empty(), "{c:?}");
    }
}

#[test]
fn a_path_dependency_is_refused_with_the_reason_and_never_read() {
    let (_d, root) = world();
    let r = run(&root, &opts(true, false)).unwrap();
    let sibling = r.peers.iter().find(|p| p.package == "sibling").unwrap();
    assert_eq!(sibling.outcome, Outcome::Refused);
    assert!(sibling
        .reason
        .as_deref()
        .unwrap()
        .contains("path dependency"));
    assert_eq!((sibling.matched, sibling.asked.as_deref()), (0, None));
    assert!(r.cites.iter().all(|c| c.package != "sibling"));
}

#[test]
fn every_peer_is_reported_including_the_ones_that_said_nothing() {
    let (_d, root) = world();
    let r = run(&root, &opts(false, false)).unwrap();
    let expected: BTreeMap<String, Outcome> = [
        ("alpha", Outcome::Answered),
        ("beta", Outcome::Answered),
        ("delta", Outcome::Empty),
        ("epsilon", Outcome::Unaligned),
        ("gamma", Outcome::Answered),
        ("omega", Outcome::Missing),
        ("sibling", Outcome::Refused),
        ("zeta", Outcome::Undeclared),
    ]
    .into_iter()
    .map(|(n, o)| (n.to_string(), o))
    .collect();
    assert_eq!(outcomes(&r), expected);
    let epsilon = r.peers.iter().find(|p| p.package == "epsilon").unwrap();
    assert!(
        epsilon.reason.as_deref().unwrap().contains("peak_cfs"),
        "{epsilon:?}"
    );

    // And the landed node says so itself, so a reader of the corpus alone sees the absences.
    let node = landed_node(&root, &r, ".yidam/corpus/question/gather-peaks.yml");
    let description = node.description.unwrap();
    for (name, outcome) in &expected {
        assert!(
            description.contains(&format!("{name}: {}", outcome.as_str())),
            "{name} is missing from:\n{description}"
        );
    }
}

#[test]
fn a_class_the_peer_does_not_declare_is_unaligned_not_matched_by_name() {
    let (_d, root) = world();
    let spec = SPEC.replace(
        "[peers.delta]\nclasses = { gage = \"gauge\" }",
        "[peers.delta]\nclasses = { gage = \"gage\" }",
    );
    fixture::write(&root, ".yidam/gathers/peaks.toml", &spec);
    fixture::commit(&root, "configure: delta");
    let r = run(&root, &opts(true, false)).unwrap();
    let delta = r.peers.iter().find(|p| p.package == "delta").unwrap();
    assert_eq!(delta.outcome, Outcome::Unaligned);
    assert!(delta
        .reason
        .as_deref()
        .unwrap()
        .contains("declares no class `gage`"));
}

#[test]
fn peers_that_disagree_land_a_question_and_no_resolution() {
    let (_d, root) = world();
    let r = run(&root, &opts(false, false)).unwrap();
    assert_eq!(r.disagreements.len(), 1, "{:?}", r.disagreements);
    let d = &r.disagreements[0];
    assert_eq!(d.key, "01646500");
    let spans: BTreeSet<_> = d
        .answers
        .iter()
        .map(|c| (c.package.as_str(), c.span.as_str()))
        .collect();
    assert_eq!(
        spans,
        [
            ("alpha", "flood_peak: 12000"),
            ("beta", "peak: 12000"),
            ("gamma", "q_max: 15500"),
        ]
        .into_iter()
        .collect()
    );

    let node = landed_node(
        &root,
        &r,
        ".yidam/corpus/question/gather-peaks-01646500.yml",
    );
    assert!(crate::claims::is_question_node(
        node.label.as_deref().unwrap()
    ));
    // No resolution: nothing tags, picks or records a value of its own.
    assert!(node.properties.is_none());
    assert!(node.cites.unwrap().iter().all(|c| c.tag.is_none()));

    // Agreeing peers alone are not a disagreement.
    let spec = SPEC.replace(
        "[peers.gamma]\nclasses = { gage = \"gauge\" }",
        "[peers.gamma]\nclasses = {}",
    );
    fixture::write(&root, ".yidam/gathers/peaks.toml", &spec);
    fixture::commit(&root, "configure: drop gamma");
    let r = run(&root, &opts(true, false)).unwrap();
    assert!(r.disagreements.is_empty(), "{:?}", r.disagreements);
}

#[test]
fn nothing_is_imported_and_no_link_leaves_the_corpus() {
    let (_d, root) = world();
    let r = run(&root, &opts(false, false)).unwrap();
    let branch = &r.landed.as_ref().unwrap().branch;
    let changed = fixture::git_out(
        &root,
        &["diff", "--name-status", &format!("HEAD..{branch}")],
    );
    let (receipts, lines): (Vec<&str>, Vec<&str>) = changed
        .lines()
        .partition(|l| l.contains("\t.yidam/runs/gather/peaks/"));
    assert_eq!(lines.len(), 2, "{changed}");
    // One receipt per peer that was asked and answered or said nothing — and none for a peer
    // that was never asked, so a receipt is never evidence of a question nobody put.
    let receipts: Vec<&str> = receipts
        .iter()
        .map(|l| l.split_once('\t').unwrap().1)
        .collect();
    assert_eq!(
        receipts,
        [
            ".yidam/runs/gather/peaks/alpha.yml",
            ".yidam/runs/gather/peaks/beta.yml",
            ".yidam/runs/gather/peaks/delta.yml",
            ".yidam/runs/gather/peaks/gamma.yml",
        ],
        "{changed}"
    );
    for line in lines {
        let (status, path) = line.split_once('\t').unwrap();
        assert_eq!(status, "A", "only new question nodes: {line}");
        assert!(
            path.starts_with(".yidam/corpus/question/gather-peaks"),
            "{line}"
        );
        let node = serde_yaml::from_str::<crate::parse::CorpusInstance>(&show(
            &root,
            &format!("{branch}:{path}"),
        ))
        .unwrap();
        assert!(node.links.is_none(), "{path} adds a links: entry");
    }
    // Every commit is an `open:` — the one epistemic verb that asserts nothing but a question.
    let subjects = fixture::git_out(&root, &["log", "--format=%s", &format!("HEAD..{branch}")]);
    assert!(
        subjects.lines().all(|s| s.starts_with("open: ? ")),
        "{subjects}"
    );
    // And the working tree was not touched.
    assert_eq!(fixture::git_out(&root, &["status", "--porcelain"]), "");
}

#[test]
fn a_repeat_gather_over_unchanged_peers_writes_nothing() {
    let (_d, root) = world();
    let first = run(&root, &opts(false, false)).unwrap();
    let branch = first.landed.as_ref().unwrap().branch.clone();
    let tip = fixture::git_out(&root, &["rev-parse", &branch]);

    let again = run(&root, &opts(false, false)).unwrap();
    let landed = again.landed.unwrap();
    assert!(landed.unchanged);
    assert!(landed.commits.is_empty());
    assert_eq!(fixture::git_out(&root, &["rev-parse", &branch]), tip);
}

#[test]
fn a_moved_peer_is_refused_over_an_existing_branch_unless_forced() {
    let (_d, root) = world();
    run(&root, &opts(false, false)).unwrap();
    // Refused on the *same* HEAD: amend the bundle commit away and back is not possible, so
    // move the peer in the working tree of a new commit and point the old branch name at it.
    let (_, short) = write::head(&root).unwrap();
    let old = branch_for("peaks", &short);
    peer(
        &root,
        "gamma",
        "ccc4444",
        "gauge",
        ["gid", "q_max"],
        &[("lf", "01646500", "16000")],
    );
    fixture::commit(&root, "tonpa: gamma moved");
    let (_, now) = write::head(&root).unwrap();
    fixture::git(&root, &["branch", &branch_for("peaks", &now), &old]);

    let err = run(&root, &opts(false, false)).unwrap_err().to_string();
    assert!(err.contains("holds a different answer"), "{err}");
    let r = run(&root, &opts(false, true)).unwrap();
    let node = landed_node(&root, &r, ".yidam/corpus/question/gather-peaks.yml");
    assert!(node
        .cites
        .unwrap()
        .iter()
        .any(|c| c.commit.as_deref() == Some("ccc4444")));
}

#[test]
fn a_bundle_that_is_not_the_locked_one_is_refused() {
    let (_d, root) = world();
    fixture::write(
        &root,
        ".yidam/tonpa/beta/bundle.yiz",
        "not the locked bytes",
    );
    fixture::write(
        &root,
        ".yidam/tonpa/tonpa.lock",
        "[[package]]\nname = \"beta\"\nurl = \"https://example.invalid/beta.yiz\"\nsha256 = \"00\"\n",
    );
    fixture::commit(&root, "tonpa: lock");
    let r = run(&root, &opts(true, false)).unwrap();
    assert_eq!(outcomes(&r)["beta"], Outcome::Refused);
}

#[test]
fn a_dry_run_asks_everyone_and_writes_no_ref() {
    let (_d, root) = world();
    let r = run(&root, &opts(true, false)).unwrap();
    assert!(r.landed.is_none());
    assert_eq!(r.cites.len(), 4);
    let refs = fixture::git_out(&root, &["for-each-ref", "refs/heads/propose"]);
    assert_eq!(refs, "");
}

#[test]
fn a_spec_the_local_corpus_cannot_ask_is_refused_before_any_peer_is_read() {
    let (_d, root) = world();
    for (from, to, says) in [
        (
            "query = \"gage\"",
            "query = \"gage -rel-> gage\"",
            "one step",
        ),
        (
            "query = \"gage\"",
            "query = \"widget\"",
            "not a question this corpus can ask",
        ),
        (
            "answer = \"peak_cfs\"",
            "answer = \"flow\"",
            "is not a property",
        ),
        (
            "lands_as = \"question\"",
            "lands_as = \"note\"",
            "not a class this corpus declares",
        ),
    ] {
        fixture::write(&root, ".yidam/gathers/peaks.toml", &SPEC.replace(from, to));
        fixture::commit(&root, "configure: break");
        let err = format!("{:#}", run(&root, &opts(true, false)).unwrap_err());
        assert!(err.contains(says), "{to}: {err}");
    }
}

#[test]
fn an_uncommitted_corpus_is_refused() {
    let (_d, root) = world();
    fixture::write(&root, ".yidam/corpus/gage/new.yml", "class: gage\n");
    assert!(run(&root, &opts(true, false)).is_err());
}

#[test]
fn a_span_is_quoted_only_when_it_is_verbatim() {
    let text = "class: x\nproperties:\n  gid: \"01646500\"\n  q_max: 15500\n";
    assert_eq!(
        quote(text, "q_max", "15500").as_deref(),
        Some("q_max: 15500")
    );
    // The quoted form keeps `gid: 01646500` out of the text; the bare value is still there.
    assert_eq!(quote(text, "gid", "01646500").as_deref(), Some("01646500"));
    assert_eq!(quote(text, "q_max", "16000"), None);
}
