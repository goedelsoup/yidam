//! `kuten check` must recognize the six repositories that defined `inquiry` — #574's proof
//! obligation, and the end-to-end plumbing that carries it.
//!
//! > A declared `inquiry` that fails to recognise the six repositories which defined it is a
//! > wrong extraction.
//!
//! # What is and is not proved here
//!
//! Those six corpora are not in this checkout and cannot be. Their **measurements** can be,
//! and since #644 they are: the profile records the six raw counts its bands were fitted
//! from, and [`every_band_contains_the_measurements_it_was_fitted_from`] reads them back
//! through `compare`. That is the obligation itself, running in CI.
//!
//! It did not run before, because A0 published the cluster only as four ranges and no
//! per-repository table. Two of those four ranges excluded the very repositories whose
//! numbers had set their endpoints — a ratio quoted to two decimal places and rounded inward
//! — and nothing here could see it, because there was nothing to compare the bands against
//! but themselves.
//!
//! The fixtures under `fixtures/kuten/` keep the job they are good at, which is the other
//! one. The endpoints are now covered exactly, by the members that set them, so the fixtures
//! probe the interior and carry the four **controls** — a vintage prelude, a repository that
//! stopped phasing, an object-coupled one, an empty one — which no measurement of the six can
//! supply, because none of the six is any of those things. Their counts sit on A0's published
//! endpoints, which the re-fit made interior; that is where they are useful now, and moving
//! them onto the new ends would only re-assert what the members already assert.
//!
//! What neither half proves is that the recorded numbers are still what those repositories
//! are. A member that changes its practice moves off a correctly fitted band, and that is
//! divergence rather than a wrong extraction. Re-fitting is a dated act; the date is
//! `measured.fitted`.
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
///
/// Five findings and not four since A3: the four bands, plus the kind-shaped
/// `question-pressure` finding, which is read against the corpus's open questions rather than
/// against an interval. The obligation covers it exactly as it covers the bands — a rule that
/// reports divergence against a repository which defined the profile is a wrong extraction,
/// whatever shape the rule is.
#[test]
fn every_shape_in_the_cluster_conforms() {
    let profile = inquiry();
    for f in fixtures().iter().filter(|f| f.cluster) {
        let findings = compare(&profile, &f.measurement, &f.vintage);
        assert_eq!(
            findings.len(),
            5,
            "{}: four bands and the question-pressure kind, five findings",
            f.name
        );
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

// ── the obligation, on the measurements themselves ────────────────────────────

/// One row of `measured.members` — a repository's raw counts, as the profile records them.
///
/// The field names are not [`Measurement`]'s. `commits` and `nodes` are key names the
/// encoding prohibition reserves at any depth in a profile, so the record spells them
/// `authored` and `instances` and the mapping is made here, once, in the open.
#[derive(serde::Deserialize)]
struct Member {
    authored: usize,
    phase: usize,
    off_vocabulary: usize,
    instances: usize,
    median_lines: f64,
    open_questions: usize,
}

impl Member {
    fn measurement(&self) -> Measurement {
        Measurement {
            commits: self.authored,
            phase_commits: self.phase,
            off_vocabulary_commits: self.off_vocabulary,
            // The stored members were measured before this field existed; zero is the
            // honest reading, and it never reaches a band — nothing compares against it.
            suffixed_commits: 0,
            nodes: self.instances,
            median_node_lines: Some(self.median_lines),
            open_questions: self.open_questions,
        }
    }
}

/// The six measurements the bands were fitted from, read off the shipped profile.
fn measured_members() -> Vec<Member> {
    let path = repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&text).expect("the shipped inquiry profile is YAML");
    let rows = doc
        .get("measured")
        .and_then(|m| m.get("members"))
        .cloned()
        .expect("the shipped profile records the members its bands were fitted from");
    serde_yaml::from_value(rows).expect("`measured.members` is a list of raw counts")
}

/// **The proof obligation, on the numbers rather than on shapes standing in for them.**
///
/// The fixtures above encode the cluster A0 *published* — four ranges — because A0 published
/// no per-repository table. That absence is not a detail: it is why two of the four bands
/// shipped excluding the very repositories whose measurements set their endpoints, and why
/// finding it out took a re-measurement two days later (#644). A0 quoted a ratio to two
/// decimal places and rounded inward, so a member measured at 0.2623 sat outside a ceiling of
/// 0.26 that its own number had defined, and a member at 0.0164 sat outside a zero.
///
/// So the profile now carries the six measurements, and this reads them back through
/// [`compare`] — the same function `kuten check` runs — and holds every band to containing
/// its own evidence. **This is the half that can run in CI.** The six corpora are not in this
/// checkout and never will be; their measurements can be, and a band that stops containing
/// them goes red here on the commit that moves it.
///
/// What this cannot do is notice that the recorded numbers have gone stale — a repository
/// that changes its practice moves away from a band that is still correctly fitted, and that
/// is divergence, which is the instrument working. Re-fitting is a dated act, and
/// `measured.fitted` is where the date lives.
#[test]
fn every_band_contains_the_measurements_it_was_fitted_from() {
    let profile = inquiry();
    let members = measured_members();
    assert_eq!(
        members.len(),
        6,
        "the cluster is six repositories and the profile records {} — a table that lost a row \
         would go on passing this while the band it no longer constrains drifted off it",
        members.len()
    );

    // Every one of the six carries the `phase` verb and a closed vocabulary in the prelude it
    // has vendored — measured, not assumed, and it is what makes the two vintage-gated
    // metrics answerable for them at all. A vintage-exempt probe here would make this test
    // pass on a band it never read.
    let vintage = Vintage {
        has_phase_verb: true,
        vocabulary_is_closed: true,
        graph_present: true,
    };

    for (i, member) in members.iter().enumerate() {
        let findings = compare(&profile, &member.measurement(), &vintage);
        assert_eq!(
            findings.len(),
            5,
            "member {i}: four bands and the question-pressure kind, five findings"
        );
        for finding in &findings {
            assert_eq!(
                finding.verdict,
                Verdict::Conforming,
                "member {i} of the six this profile was fitted from measured {} on `{}`, and \
                 the declared {} reports {}. A band that excludes its own evidence is a wrong \
                 extraction — re-fit it by `measured.estimator`, which rounds outward.",
                finding.measured,
                finding.metric,
                finding.declared,
                tag(finding.verdict)
            );
        }
    }
}

/// The guard above, mutated: a recorded measurement moved outside its band is caught.
///
/// A test that reads a table and compares it against bands fitted from that same table can
/// pass by reading nothing — an empty list, a renamed key, a `Vintage` that exempts the two
/// gated metrics. This moves one row's phase count until it sits below the floor and asserts
/// the comparison notices, so the assertion above is known to be load-bearing rather than
/// believed to be.
#[test]
fn the_containment_guard_catches_a_member_outside_its_band() {
    let profile = inquiry();
    let vintage = Vintage {
        has_phase_verb: true,
        vocabulary_is_closed: true,
        graph_present: true,
    };
    let mut measurement = measured_members()[0].measurement();
    let floor = profile
        .phases
        .as_ref()
        .expect("the phases slot is populated")
        .commit_share
        .low;
    measurement.phase_commits = 0;
    assert!(
        floor > 0.0,
        "a floor of zero would make this mutation conform, and the mutation would prove nothing"
    );

    let verdicts: Vec<Verdict> = compare(&profile, &measurement, &vintage)
        .into_iter()
        .filter(|f| f.metric == "phase-commit-share")
        .map(|f| f.verdict)
        .collect();
    assert_eq!(
        verdicts,
        vec![Verdict::Divergent],
        "a member driven to zero phase commits still reads as conforming, so the containment \
         assertion above is not reading the band"
    );
}

/// **G6 — the question-pressure rule recognizes all six, and it was measured before it was
/// written.**
///
/// RFC-0028 §5's example rule counts `open:` commits, and it does not survive its own
/// population: two of the six — bitlocker and hermetic-ch — have written **zero** while
/// holding open questions in the corpus. So the rule reads corpus state through
/// `claims::is_open_question`, and this is the assertion that it recognizes the six.
///
/// The counts in the six fixtures are not invented. Running this repository's own
/// `kuten::measure` over the six repositories on 2026-09-06, read-only, gave 14 / 26 / 13 /
/// 51 / 115 / 436 open questions against 73 / 82 / 122 / 157 / 247 / 1,278 authored commits —
/// every one non-zero, at 11% to 47% of commits. The fixtures sit on the ends and through the
/// interior of that measured range.
#[test]
fn the_question_pressure_rule_recognizes_every_repository_that_defined_the_profile() {
    let profile = inquiry();
    assert!(
        profile.question_pressure.is_some(),
        "the shipped profile no longer declares question pressure, so this asserts nothing"
    );
    for f in fixtures().iter().filter(|f| f.cluster) {
        assert!(
            f.measurement.open_questions > 0,
            "{}: a cluster fixture with no open questions is not one of the six — every one \
             of them holds some",
            f.name
        );
        let finding = compare(&profile, &f.measurement, &f.vintage)
            .into_iter()
            .find(|x| x.metric == "question-pressure")
            .unwrap_or_else(|| panic!("{}: no question-pressure finding", f.name));
        assert_eq!(
            finding.verdict,
            Verdict::Conforming,
            "{} ({}) defined this profile and the question-pressure rule reports {} against \
             it — the rule is wrong, not the repository",
            f.name,
            f.note,
            tag(finding.verdict)
        );
        assert!(finding.question.is_none());
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

// ── the reader, against a document ────────────────────────────────────────────

/// **The probes are read against the prelude this repository ships**, not against a fixture.
///
/// Every other test here states `Vintage` as data — deliberately, because the six corpora
/// that defined the cluster are not in this checkout — and that is exactly the hole #638 fell
/// through: `Vintage::read` had no test with a document in it, so a probe that matched
/// nothing in any real `GRAPH.md` could not fail anything. `vocabulary_is_closed` was matched
/// against the raw text of a sentence that had since been rewrapped, and read `false` for the
/// template and all eighteen derived corpora.
///
/// The document this loads is the one every derived repository vendors, so if a reflow, a
/// rewording, or a table edit puts either probe out of reach again, this goes red here rather
/// than silently exempting a metric in every corpus downstream.
#[test]
fn both_probes_read_true_against_the_shipped_prelude() {
    let path = repo_root().join("yidam/prelude/GRAPH.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let vintage = Vintage::read(&text);

    assert!(
        vintage.graph_present,
        "a document was read, so the prelude is present"
    );
    assert!(
        vintage.has_phase_verb,
        "{} no longer carries a `| `phase`` row in the vocabulary table, so every corpus \
         reads as unable to have settled a phase",
        path.display()
    );
    assert!(
        vintage.vocabulary_is_closed,
        "{} no longer reads as closing the commit vocabulary, so `off-vocabulary-share` is \
         gated off in every corpus and the band can never be read",
        path.display()
    );
}

/// The closed-list sentence is prose, and prose gets rewrapped. Neither the wrapping this
/// repository happens to ship nor any other may decide the answer.
#[test]
fn the_closed_list_is_read_however_the_paragraph_is_wrapped() {
    let path = repo_root().join("yidam/prelude/GRAPH.md");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));

    // One line per paragraph — the shape a formatter with no column limit would leave.
    let unwrapped = text
        .split("\n\n")
        .map(|para| para.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect::<Vec<_>>()
        .join("\n\n");
    assert!(
        Vintage::read(&unwrapped).vocabulary_is_closed,
        "unwrapping the paragraphs lost the closed vocabulary"
    );

    // And every break the sentence itself could be given. Rewrapped from `unwrapped`, where
    // the sentence is contiguous — in the shipped file it is already broken after `This`, so
    // a substitution against `text` would replace nothing and prove nothing.
    let sentence = "This list is closed:";
    assert!(
        unwrapped.contains(sentence),
        "the closed-list sentence has been reworded; this test names the old wording"
    );
    let words: Vec<&str> = sentence.split(' ').collect();
    for at in 1..words.len() {
        let broken = format!("{}\n{}", words[..at].join(" "), words[at..].join(" "));
        assert!(
            Vintage::read(&unwrapped.replace(sentence, &broken)).vocabulary_is_closed,
            "a break after word {at} of `{sentence}` lost the closed vocabulary"
        );
    }
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

/// A 48-line corpus node, optionally carrying an open claim.
fn node(open: bool) -> String {
    let body = if open {
        format!("{}  whether this holds is [open]\n", "  line\n".repeat(45))
    } else {
        "  line\n".repeat(46)
    };
    format!("title: a node\ndescription: |\n{body}")
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

    // Four nodes of 48 lines each: a 48-line median, inside the 35–62 band. One of them
    // carries an open claim, because a corpus declaring `inquiry` and holding no open
    // question diverges on question pressure — correctly — and this fixture is the
    // conforming one. `[open]` replaces a line rather than adding one, so the median does
    // not move.
    for n in 0..4 {
        write(&format!(".yidam/corpus/concept/n{n}.yml"), &node(n == 0));
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
    assert_eq!(v["kuten"]["findings"].as_array().unwrap().len(), 5);

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("AGENTS.md")).unwrap(),
        before,
        "`kuten check` wrote to the repository it was pointed at"
    );
}

/// Every file under `root`, outside `.git`, as path → contents.
///
/// `.git` is excluded because git's own bookkeeping is not authorship: `git log` may touch a
/// pack index or a ref cache, and counting that as the kuten having written something would
/// make the assertion below flaky in the one direction that matters least.
fn tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut out = BTreeMap::new();
    for entry in walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.file_name() != ".git")
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
    {
        let rel = entry.path().strip_prefix(root).unwrap_or(entry.path());
        out.insert(
            rel.to_path_buf(),
            std::fs::read(entry.path()).unwrap_or_default(),
        );
    }
    out
}

/// Which paths were created, deleted, or edited between two snapshots.
///
/// Named paths rather than a whole-map equality: comparing the maps directly is the same
/// assertion, and it prints two corpora as byte arrays when it fails, which is a report
/// nobody can read.
fn written(before: &BTreeMap<PathBuf, Vec<u8>>, after: &BTreeMap<PathBuf, Vec<u8>>) -> Vec<String> {
    let mut out = Vec::new();
    for (path, body) in after {
        match before.get(path) {
            None => out.push(format!("created {}", path.display())),
            Some(was) if was != body => out.push(format!("modified {}", path.display())),
            Some(_) => {}
        }
    }
    for path in before.keys() {
        if !after.contains_key(path) {
            out.push(format!("deleted {}", path.display()));
        }
    }
    out.sort();
    out
}

/// **G4 — the question-pressure slot authors nothing.** #575's DoD, run rather than asserted.
///
/// This is the constitutional half of the slot and the reason it is licensed at all. Opening
/// a question asserts nothing the work did not already assert, which is why `propose` may
/// draft `open:` and may not draft `establish:` — and a slot that *created* a question would
/// be reaching past the door it came through. So the check may report the gap and may not
/// close it.
///
/// The corpus here holds **no** open question, so the slot is in its divergent arm — the one
/// arm where a tempted implementation would have something to write — and the whole tree
/// outside `.git` is compared byte for byte before and after.
#[test]
fn the_question_pressure_slot_authors_nothing() {
    let tmp = stage(1);
    // Strip the one node that carries an open question, so the check runs with something to
    // say. A repository already holding questions would exercise the silent arm.
    std::fs::write(tmp.path().join(".yidam/corpus/concept/n0.yml"), node(false)).unwrap();

    let before = tree(tmp.path());
    let r = run(tmp.path(), &["kuten", "check", "--format", "json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    let finding = v["kuten"]["findings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["metric"] == "question-pressure")
        .expect("a question-pressure finding");
    assert_eq!(
        finding["verdict"], "divergent",
        "the fixture is meant to be in the arm that has something to say: {finding}"
    );
    assert!(
        finding["question"].as_str().is_some_and(|q| !q.is_empty()),
        "divergence asks a person: {finding}"
    );

    let wrote = written(&before, &tree(tmp.path()));
    assert!(
        wrote.is_empty(),
        "`kuten check` {wrote:?} under a question-pressure divergence. The slot creates \
         pressure toward a kind of question; it does not author one, and anything that did \
         would be reaching past the licence it came through."
    );
}

/// Append commits to a staged repository, one file each, at the fixture's frozen date.
fn commits(root: &Path, subjects: &[&str]) {
    for (n, subject) in subjects.iter().enumerate() {
        std::fs::create_dir_all(root.join("later")).unwrap();
        std::fs::write(root.join(format!("later/{n}.md")), "a step\n").unwrap();
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "--no-gpg-sign", "-m", subject],
        ] {
            Command::new("git")
                .current_dir(root)
                .args(args)
                .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
                .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
                .status()
                .unwrap();
        }
    }
}

/// **One suffix, two questions, and the report answers both at once** — #691.
///
/// `phase(section-nine): …` settles a phase and is spelled wrong. Before this, one parse
/// answered both questions and they contradicted each other in the field: a derived corpus
/// with 73 such commits was told **0% of commits settle a phase** by the same binary whose
/// `yidam phases` listed 22 in flight, while those same 73 commits drove its off-vocabulary
/// divergence. Two of the four divergences it was shown were the one suffix, counted twice
/// in opposite directions.
///
/// So the two halves are pinned in **one** test rather than two. Separate tests would each
/// keep passing while the questions were quietly re-merged — the band going strict again
/// reads as a phase-count test failing, the check going lenient reads as a lint test
/// failing, and nothing would say that the pair is the invariant. Here the same two commits
/// are asserted into both counts, into opposite verdicts, and into `lint --commits`.
#[test]
fn a_scoped_phase_commit_is_counted_by_the_band_and_still_reported_off_vocabulary() {
    let tmp = stage(1);
    let root = tmp.path();

    // Two subjects written the way the field corpus wrote 73 of them, and four in the closed
    // vocabulary. Twelve authored commits: three settle a phase — 0.25, inside 0.12–0.27 —
    // and two are outside the vocabulary, which 0.00–0.02 does not admit.
    commits(
        root,
        &[
            "phase(section-nine): the last checkable block, and it half-answers",
            "phase(per-district-trough): a statewide number becomes 607 of them",
            "establish: a second node",
            "revise: the second node",
            "open: a second question",
            "close: the second question",
        ],
    );

    let r = run(root, &["kuten", "check", "--format", "json"]);
    assert_eq!(r.code, 0, "{}", r.stdout);
    let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
    let m = &v["kuten"]["measurement"];
    assert_eq!(m["commits"], 12, "{m}");
    assert_eq!(
        m["phase_commits"], 3,
        "the band asks whether this corpus bounds its work into phases, and two of these \
         three commits bounded one. Counting only `phase:` answers a question about spelling \
         with a number about practice: {m}"
    );
    assert_eq!(
        m["off_vocabulary_commits"], 2,
        "and the same two commits are still outside the vocabulary. A phase count that \
         looked past the suffix *for recognition* would read zero here, which is A0's \
         extraction error (#644) rebuilt inside the binary: {m}"
    );

    let verdict = |metric: &str| {
        v["kuten"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|f| f["metric"] == metric)
            .unwrap_or_else(|| panic!("a {metric} finding: {}", r.stdout))["verdict"]
            .as_str()
            .unwrap()
            .to_string()
    };
    assert_eq!(verdict("phase-commit-share"), "conforming");
    assert_eq!(verdict("off-vocabulary-share"), "divergent");

    // And the surface a person is asked to act on says so too. The remedy for these commits
    // is the vocabulary finding, and it is the only one that should be naming them.
    let lint = run(root, &["lint", "--commits", "--format", "json"]);
    let report: serde_json::Value =
        serde_json::from_str(&lint.stdout).unwrap_or_else(|e| panic!("{e}: {}", lint.stdout));
    let reported: Vec<String> = report["checks"]
        .as_array()
        .expect("the report lists checks")
        .iter()
        .find(|c| c["id"] == "unrecognized-verb")
        .expect("`lint --commits` runs the vocabulary check")["violations"]
        .as_array()
        .map(|vs| {
            vs.iter()
                .filter_map(|v| v["detail"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    assert_eq!(
        reported.len(),
        2,
        "`lint --commits` must go on reporting a scoped verb whatever the band counts: \
         {reported:?}"
    );
    assert!(
        reported.iter().all(|msg| msg.contains("phase(")),
        "and it must name these two: {reported:?}"
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

/// `doctor` names the kuten, its revision, and which way the arrow runs — RFC-0028 §9 and
/// §6, which adopts #582's acceptance criterion into A3.
///
/// The direction is on the existing `kuten` check's **text** rather than a check of its own:
/// it is part of the answer to *which kuten does this repository hold*, and a consumer keys
/// on the check id. `doctor`'s id set is asserted exhaustively elsewhere, so a new id would
/// be a contract change made in passing.
#[test]
fn doctor_reports_which_kuten_is_held_and_which_way_the_arrow_runs() {
    let tmp = stage(1);
    let read_detail = |root: &Path| {
        let r = run(root, &["doctor", "--format", "json"]);
        let v: serde_json::Value = serde_json::from_str(&r.stdout).expect("valid JSON");
        let check = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == "kuten")
            .expect("a kuten check")
            .clone();
        assert_eq!(check["verdict"], "ok", "{check}");
        check["detail"].as_str().unwrap_or_default().to_string()
    };

    let detail = read_detail(tmp.path());
    assert!(detail.contains("revision 1"), "{detail}");
    assert!(
        detail.contains("authored"),
        "the direction is what closes #582: an undeclared untracked corpus is otherwise \
         indistinguishable from no corpus at all. Got: {detail}"
    );

    // And the other declared state actually reads differently. A line that said `authored`
    // whatever the profile declares would answer #582's question with a constant.
    let vendored = tmp
        .path()
        .join(".yidam/.vendor/prelude/kuten/inquiry/kuten.yml");
    let text = std::fs::read_to_string(&vendored).unwrap();
    std::fs::write(
        &vendored,
        text.replace("direction: authored", "direction: projected"),
    )
    .unwrap();
    let detail = read_detail(tmp.path());
    assert!(
        detail.contains("projected"),
        "a projected corpus is a declared state and `doctor` names it: {detail}"
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
