//! The kuten a corpus holds — RFC-0028, and the reporting half specified in §9.
//!
//! A kuten is a committed, vendored declaration of what a corpus's practice is aimed at. It
//! narrows and parameterizes the loop; it may not widen the model. This module is the model
//! behind `yidam kuten check`: read the **vendored** declaration, measure the repository's
//! own history, and say where the two disagree.
//!
//! # Nothing here decides anything
//!
//! Divergence from a kuten is a question for a person, never a defect. `due`'s argument
//! applies verbatim — *a corpus with three expired sources is not unhealthy, it is owed* — so
//! the comparison produces findings and no verdict, and the command exits zero.
//!
//! # The vintage rule is the whole of A0's lesson
//!
//! A0 measured eighteen derived corpora and reported a second cluster that was not there.
//! Two controls dissolved it, and the first is that **a repository works from the prelude it
//! vendored**. Three candidates had vendored a prelude with no `phase` verb and no closed
//! vocabulary; their zero phase usage and their 43%/73% "violations" were properties of the
//! template they hold. So [`Vintage`] is read from the *vendored* `GRAPH.md`, and a metric
//! that vendored prelude could not have produced is reported as [`Verdict::Vintage`] and
//! never as divergence.
//!
//! # Why the comparison takes data and not a repository
//!
//! [`compare`] is a pure function of a profile, a measurement and a vintage. That is what
//! lets the six repositories which defined the `inquiry` cluster be encoded as their measured
//! shapes and asserted against — the proof obligation #574 sets — without those corpora being
//! in this checkout. [`measure`] is the thin part that reads a working tree.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Where a vendored kuten lives, relative to the repository root.
pub const VENDORED_DIR: &str = ".yidam/.vendor/prelude/kuten";

/// The decision record naming which kuten this corpus adopted, and at what revision.
pub const DECISION_PATH: &str = ".yidam/decisions/kuten.yml";

/// A closed band a measured value is read against.
///
/// Both ends are inclusive. A0's cluster is stated as ranges over six repositories, and a
/// repository sitting exactly on an end is one of the six.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq)]
pub struct Band {
    pub low: f64,
    pub high: f64,
}

impl Band {
    pub fn holds(&self, value: f64) -> bool {
        // A tolerance, because the bands are quoted to two decimal places and a ratio is
        // not. Without it a repository measured at 0.4999 fails a band whose author wrote
        // 0.50 meaning "a half".
        const EPSILON: f64 = 1e-9;
        value >= self.low - EPSILON && value <= self.high + EPSILON
    }

    pub fn describe(&self) -> String {
        format!("{:.2}–{:.2}", self.low, self.high)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Phases {
    pub types: Vec<String>,
    pub commit_share: Band,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vocabulary {
    pub verbs: Vec<String>,
    pub off_vocabulary_share: Band,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Classes {
    pub nodes_per_commit: Band,
    pub median_node_lines: Band,
}

/// A kuten profile, as the vendored `kuten.yml` declares it.
///
/// Unknown keys are accepted here on purpose. The closed slot set is enforced by a guard over
/// the profiles this repository ships, not by a parse in a binary that a derived repository
/// may be running at an older version — a strict parse would turn "upstream added a slot"
/// into "this repository's kuten cannot be read".
#[derive(Debug, Clone, Deserialize)]
pub struct Profile {
    #[serde(rename = "kuten")]
    pub name: String,
    pub revision: u32,
    #[serde(default)]
    pub gloss: String,
    #[serde(default)]
    pub phases: Option<Phases>,
    #[serde(default)]
    pub vocabulary: Option<Vocabulary>,
    #[serde(default)]
    pub classes: Option<Classes>,
}

impl Profile {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(serde_yaml::from_str(text)?)
    }
}

/// What `.yidam/decisions/kuten.yml` records: the selection, and the revision vendored with it.
#[derive(Debug, Clone, Deserialize)]
pub struct Declaration {
    #[serde(rename = "kuten")]
    pub name: String,
    pub revision: u32,
}

impl Declaration {
    pub fn parse(text: &str) -> anyhow::Result<Self> {
        Ok(serde_yaml::from_str(text)?)
    }
}

/// What the *vendored* prelude could have produced.
///
/// Read from the vendored `GRAPH.md` rather than from upstream's current one, and read
/// structurally: the `phase` verb is a row in the vocabulary table, and the closed list
/// announces itself in the sentence that closes it. A repository whose prelude predates
/// either cannot have run the practice this measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Vintage {
    /// Whether the vendored vocabulary carries the `phase` verb.
    pub has_phase_verb: bool,
    /// Whether the vendored vocabulary is declared closed.
    pub vocabulary_is_closed: bool,
    /// Whether a vendored `GRAPH.md` was found at all.
    pub graph_present: bool,
}

impl Vintage {
    /// A repository with no vendored prelude to read. Every vintage-gated metric is then
    /// unanswerable rather than divergent.
    pub fn absent() -> Self {
        Self {
            has_phase_verb: false,
            vocabulary_is_closed: false,
            graph_present: false,
        }
    }

    /// Read a vendored `GRAPH.md`.
    ///
    /// The `phase` row is matched as a table cell — `| `phase` |` — and not as the bare word,
    /// which appears in this document's prose dozens of times in every vintage. The closed
    /// list is matched on the sentence that declares it closed.
    pub fn read(graph_md: &str) -> Self {
        let has_phase_verb = graph_md
            .lines()
            .any(|l| l.trim_start().starts_with("| `phase`"));
        let vocabulary_is_closed = graph_md.contains("This list is closed");
        Self {
            has_phase_verb,
            vocabulary_is_closed,
            graph_present: true,
        }
    }

    pub fn of_repo(root: &Path) -> Self {
        let path = root.join(".yidam/.vendor/prelude/GRAPH.md");
        match std::fs::read_to_string(path) {
            Ok(text) => Self::read(&text),
            Err(_) => Self::absent(),
        }
    }
}

/// What a repository's own history and working tree show.
///
/// A plain struct rather than a set of methods on a repository, so the six shapes that
/// defined the `inquiry` cluster can be stated as data and run through [`compare`].
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
pub struct Measurement {
    /// Commits with an authored subject — git-generated merge subjects are excluded, because
    /// nobody chose their verb.
    pub commits: usize,
    /// Commits whose leading verb is `phase`.
    pub phase_commits: usize,
    /// Commits whose leading verb is outside the closed vocabulary.
    pub off_vocabulary_commits: usize,
    /// Instance nodes in the corpus.
    pub nodes: usize,
    /// Median instance node length, in lines. `None` when there are no nodes.
    pub median_node_lines: Option<f64>,
}

impl Measurement {
    fn share(part: usize, whole: usize) -> Option<f64> {
        (whole > 0).then(|| part as f64 / whole as f64)
    }

    pub fn phase_share(&self) -> Option<f64> {
        Self::share(self.phase_commits, self.commits)
    }

    pub fn off_vocabulary_share(&self) -> Option<f64> {
        Self::share(self.off_vocabulary_commits, self.commits)
    }

    pub fn nodes_per_commit(&self) -> Option<f64> {
        Self::share(self.nodes, self.commits)
    }
}

/// How one measured value stands against what the kuten declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Measured, and inside the declared band.
    Conforming,
    /// Measured, and outside it. A question, never a defect.
    Divergent,
    /// The vendored prelude could not have produced this metric. **Never divergence.**
    Vintage,
    /// Nothing to measure — no commits, no nodes, or the kuten declares no band.
    Unmeasurable,
}

impl Verdict {
    pub fn tag(self) -> &'static str {
        match self {
            Verdict::Conforming => "ok",
            Verdict::Divergent => "diverges",
            Verdict::Vintage => "vintage",
            Verdict::Unmeasurable => "unmeasured",
        }
    }
}

/// One metric, read against one declared band.
#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    /// The slot the band belongs to — `phases`, `vocabulary`, `classes`.
    pub slot: &'static str,
    /// Stable identifier for the metric. A consumer keys on this; the prose may change.
    pub metric: &'static str,
    pub verdict: Verdict,
    /// What the kuten declares, as a reader would write it.
    pub declared: String,
    /// What this repository shows.
    pub measured: String,
    /// The question a person is being asked, where there is one.
    pub question: Option<String>,
}

/// One band, and everything needed to read a measurement against it.
///
/// A struct rather than eight parameters, and it also puts the four metrics' declarations
/// side by side where the vintage gate on two of them is visible as a difference.
struct Metric {
    slot: &'static str,
    /// Stable identifier. A consumer keys on this.
    id: &'static str,
    band: Band,
    /// Whether the vendored prelude could have produced this metric at all. `None` where the
    /// question does not arise — node shape is measurable at every vintage.
    vintage: Option<VintageGate>,
    /// How the measured value is written for a reader.
    render: fn(f64) -> String,
    /// The question a person is asked when the value falls outside the band. Takes the
    /// rendered value and the rendered band.
    question: fn(&str, &str) -> String,
}

/// A vintage precondition: whether it holds, and what to say when it does not.
struct VintageGate {
    holds: bool,
    reason: &'static str,
}

impl Metric {
    fn read(&self, measured: Option<f64>) -> Finding {
        let declared = self.band.describe();
        if let Some(gate) = &self.vintage {
            if !gate.holds {
                return Finding {
                    slot: self.slot,
                    metric: self.id,
                    verdict: Verdict::Vintage,
                    declared,
                    measured: gate.reason.to_string(),
                    question: None,
                };
            }
        }
        let Some(value) = measured else {
            return Finding {
                slot: self.slot,
                metric: self.id,
                verdict: Verdict::Unmeasurable,
                declared,
                measured: "nothing to measure".to_string(),
                question: None,
            };
        };
        let shown = (self.render)(value);
        let holds = self.band.holds(value);
        Finding {
            slot: self.slot,
            metric: self.id,
            verdict: if holds {
                Verdict::Conforming
            } else {
                Verdict::Divergent
            },
            question: (!holds).then(|| (self.question)(&shown, &declared)),
            declared,
            measured: shown,
        }
    }
}

fn percent(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

fn two_places(v: f64) -> String {
    format!("{v:.2}")
}

fn lines(v: f64) -> String {
    format!("{v:.0} lines")
}

/// Read a repository against its declared kuten. **Pure, and total.**
///
/// Every band the profile declares produces exactly one finding, including the ones that
/// conform: a metric that vanishes when it agrees cannot be told from one that was never
/// read. A slot the profile leaves unpopulated produces no finding at all — that is the
/// difference between *this practice makes no claim here* and *this repository was not
/// measured*.
pub fn compare(profile: &Profile, m: &Measurement, vintage: &Vintage) -> Vec<Finding> {
    let mut out = Vec::new();

    if let Some(phases) = &profile.phases {
        out.push(
            Metric {
                slot: "phases",
                id: "phase-commit-share",
                band: phases.commit_share,
                vintage: Some(VintageGate {
                    holds: vintage.has_phase_verb,
                    reason: "the vendored prelude has no `phase` verb, so no phase here could \
                             ever have been settled with one",
                }),
                render: percent,
                question: |got, want| {
                    format!(
                        "{got} of commits settle a phase, against {want}. Is this corpus still \
                         bounding its work into phases?"
                    )
                },
            }
            .read(m.phase_share()),
        );
    }

    if let Some(vocabulary) = &profile.vocabulary {
        out.push(
            Metric {
                slot: "vocabulary",
                id: "off-vocabulary-share",
                band: vocabulary.off_vocabulary_share,
                vintage: Some(VintageGate {
                    holds: vintage.vocabulary_is_closed,
                    reason: "the vendored prelude does not close the vocabulary, so no commit \
                             here is outside it",
                }),
                render: percent,
                question: |got, want| {
                    format!(
                        "{got} of commits use a verb outside the vocabulary, against {want}. Are \
                         those commits this corpus's work, or an artifact's?"
                    )
                },
            }
            .read(m.off_vocabulary_share()),
        );
    }

    if let Some(classes) = &profile.classes {
        // No vintage gate on either: a corpus of any vintage accretes nodes, and both bands
        // read the working tree rather than a capability the prelude had to grant.
        out.push(
            Metric {
                slot: "classes",
                id: "nodes-per-commit",
                band: classes.nodes_per_commit,
                vintage: None,
                render: two_places,
                question: |got, want| {
                    format!(
                        "this corpus accretes {got} nodes per commit, against {want}. Nodes per \
                         commit halves over a repository's life, so read it against this \
                         repository's age."
                    )
                },
            }
            .read(m.nodes_per_commit()),
        );
        out.push(
            Metric {
                slot: "classes",
                id: "median-node-lines",
                band: classes.median_node_lines,
                vintage: None,
                render: lines,
                question: |got, want| {
                    format!("the median node here is {got}, against a declared {want}.")
                },
            }
            .read(m.median_node_lines),
        );
    }

    out
}

/// The whole answer `yidam kuten check` gives.
#[derive(Debug, Serialize)]
pub struct Report {
    /// Whether this repository holds a kuten at all. **`false` is a supported state**, and
    /// was every one of the eighteen corpora A0 measured.
    pub held: bool,
    /// The kuten named by the decision record.
    pub name: Option<String>,
    /// The revision the decision record names.
    pub declared_revision: Option<u32>,
    /// The revision of the profile actually vendored.
    pub vendored_revision: Option<u32>,
    /// Whether those two disagree. A comparison across revisions is annotated, never
    /// silently made — the harness refuses a cross-`PROTOCOL_VERSION` diff for this reason.
    pub revision_skew: bool,
    /// Why there is nothing to compare, when there is nothing to compare.
    pub unresolved: Option<String>,
    pub vintage: Vintage,
    pub measurement: Measurement,
    pub findings: Vec<Finding>,
    /// Whether every measured metric conforms. Not an exit code: this command exits zero.
    pub conforming: bool,
}

impl Report {
    /// A repository that holds no kuten, and says so.
    pub fn unheld(measurement: Measurement, vintage: Vintage) -> Self {
        Self {
            held: false,
            name: None,
            declared_revision: None,
            vendored_revision: None,
            revision_skew: false,
            unresolved: None,
            vintage,
            measurement,
            findings: Vec::new(),
            conforming: true,
        }
    }
}

// ── reading a repository ──────────────────────────────────────────────────────

pub fn decision_path(root: &Path) -> PathBuf {
    root.join(DECISION_PATH)
}

pub fn profile_path(root: &Path, name: &str) -> PathBuf {
    root.join(VENDORED_DIR).join(name).join("kuten.yml")
}

/// The kuten this repository declared, if it declared one.
///
/// A missing record is the supported no-kuten state and not an error. A malformed one is an
/// error a person should see, and is reported rather than swallowed.
pub fn read_declaration(root: &Path) -> anyhow::Result<Option<Declaration>> {
    let path = decision_path(root);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(Some(Declaration::parse(&text).map_err(|e| {
        anyhow::anyhow!("{} is not a kuten decision record: {e}", path.display())
    })?))
}

/// The vendored profile the declaration names.
pub fn read_profile(root: &Path, name: &str) -> anyhow::Result<Option<Profile>> {
    let path = profile_path(root, name);
    if !path.exists() {
        return Ok(None);
    }
    let text = std::fs::read_to_string(&path)?;
    Ok(Some(Profile::parse(&text).map_err(|e| {
        anyhow::anyhow!("{} is not a kuten profile: {e}", path.display())
    })?))
}

/// Measure a repository: its authored history, and the corpus it has accreted.
///
/// The commit half goes through the same reader **and the same merge predicate** that
/// `lint --commits` uses, so the two cannot disagree about what a verb is or which merge
/// subjects git wrote rather than a person. A second copy of `is_merge` here would be a
/// second answer to a question the model already settled — a bare `Merge <ref>` is
/// git-generated and a `phase: …` merge is not, and one repository wrote ten of the first.
pub fn measure(root: &Path) -> Measurement {
    let subjects = crate::cmd::lint::commits::read_subjects(root, None);
    let authored: Vec<&crate::cmd::lint::commits::Subject> = subjects
        .iter()
        .filter(|s| !crate::cmd::lint::commits::is_merge(&s.text, s.parents))
        .collect();

    let mut lines: Vec<usize> =
        crate::walk::walk_corpus_instances(&crate::paths::yidam_corpus_dir(root))
            .iter()
            .map(|p| crate::walk::line_count(p))
            .collect();
    lines.sort_unstable();

    Measurement {
        commits: authored.len(),
        phase_commits: authored.iter().filter(|s| s.verb == "phase").count(),
        off_vocabulary_commits: authored
            .iter()
            .filter(|s| !yidam_core::git::is_recognized_verb(&s.verb))
            .count(),
        nodes: lines.len(),
        median_node_lines: median(&lines),
    }
}

/// The median of a sorted slice, averaging the two middles on an even count.
fn median(sorted: &[usize]) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let mid = sorted.len() / 2;
    Some(if sorted.len() % 2 == 1 {
        sorted[mid] as f64
    } else {
        (sorted[mid - 1] + sorted[mid]) as f64 / 2.0
    })
}

/// Assemble the report for `root`.
pub fn check(root: &Path) -> anyhow::Result<Report> {
    let measurement = measure(root);
    let vintage = Vintage::of_repo(root);

    let Some(declaration) = read_declaration(root)? else {
        return Ok(Report::unheld(measurement, vintage));
    };

    let Some(profile) = read_profile(root, &declaration.name)? else {
        return Ok(Report {
            held: true,
            name: Some(declaration.name.clone()),
            declared_revision: Some(declaration.revision),
            vendored_revision: None,
            revision_skew: false,
            unresolved: Some(format!(
                "{} names `{}`, and no profile is vendored at {}",
                DECISION_PATH,
                declaration.name,
                profile_path(Path::new(""), &declaration.name).display()
            )),
            vintage,
            measurement,
            findings: Vec::new(),
            conforming: true,
        });
    };

    let findings = compare(&profile, &measurement, &vintage);
    let conforming = !findings.iter().any(|f| f.verdict == Verdict::Divergent);
    Ok(Report {
        held: true,
        name: Some(profile.name.clone()),
        declared_revision: Some(declaration.revision),
        vendored_revision: Some(profile.revision),
        revision_skew: declaration.revision != profile.revision,
        unresolved: None,
        vintage,
        measurement,
        findings,
        conforming,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn inquiry() -> Profile {
        Profile::parse(
            "kuten: inquiry\nrevision: 1\n\
             phases:\n  types: [Investigation]\n  commit_share: {low: 0.13, high: 0.26}\n\
             vocabulary:\n  verbs: [establish]\n  off_vocabulary_share: {low: 0.0, high: 0.0}\n\
             classes:\n  nodes_per_commit: {low: 0.50, high: 1.11}\n  median_node_lines: {low: 35, high: 62}\n",
        )
        .expect("the fixture profile parses")
    }

    fn current() -> Vintage {
        Vintage {
            has_phase_verb: true,
            vocabulary_is_closed: true,
            graph_present: true,
        }
    }

    /// A repository sitting on the middle of every band.
    fn conformant() -> Measurement {
        Measurement {
            commits: 100,
            phase_commits: 20,
            off_vocabulary_commits: 0,
            nodes: 80,
            median_node_lines: Some(48.0),
        }
    }

    #[test]
    fn a_conforming_repository_produces_a_finding_for_every_band() {
        let f = compare(&inquiry(), &conformant(), &current());
        assert_eq!(f.len(), 4, "one finding per declared band: {f:?}");
        assert!(f.iter().all(|f| f.verdict == Verdict::Conforming), "{f:?}");
        assert!(
            f.iter().all(|f| f.question.is_none()),
            "a conforming metric asks nothing"
        );
    }

    /// Both ends of a band are inside it. The cluster is quoted as a range over six
    /// repositories, so a repository at an end is one of the six.
    #[test]
    fn the_ends_of_a_band_are_inside_it() {
        let band = Band {
            low: 0.13,
            high: 0.26,
        };
        assert!(band.holds(0.13));
        assert!(band.holds(0.26));
        assert!(!band.holds(0.12));
        assert!(!band.holds(0.27));
    }

    #[test]
    fn a_corpus_that_stopped_running_phases_is_asked_about_it() {
        let mut m = conformant();
        m.phase_commits = 0;
        let f = compare(&inquiry(), &m, &current());
        let phases = f.iter().find(|f| f.metric == "phase-commit-share").unwrap();
        assert_eq!(phases.verdict, Verdict::Divergent);
        assert!(phases.question.is_some(), "divergence asks a question");
    }

    /// The whole lesson of A0's retraction, as an assertion.
    #[test]
    fn a_prelude_with_no_phase_verb_is_vintage_and_never_divergence() {
        let mut m = conformant();
        m.phase_commits = 0;
        let old = Vintage {
            has_phase_verb: false,
            vocabulary_is_closed: false,
            graph_present: true,
        };
        let f = compare(&inquiry(), &m, &old);
        let phases = f.iter().find(|f| f.metric == "phase-commit-share").unwrap();
        assert_eq!(
            phases.verdict,
            Verdict::Vintage,
            "a repository that could not have run a phase has not stopped running them"
        );
        assert!(phases.question.is_none());
    }

    /// The second half of the same lesson: 43% "violations" against a vocabulary the
    /// vendored prelude never closed is a property of the template.
    #[test]
    fn an_open_vendored_vocabulary_makes_off_vocabulary_meaningless() {
        let mut m = conformant();
        m.off_vocabulary_commits = 43;
        let old = Vintage {
            has_phase_verb: false,
            vocabulary_is_closed: false,
            graph_present: true,
        };
        let f = compare(&inquiry(), &m, &old);
        let vocab = f
            .iter()
            .find(|f| f.metric == "off-vocabulary-share")
            .unwrap();
        assert_eq!(vocab.verdict, Verdict::Vintage);
    }

    /// The vintage exemption is not a blanket one. Node shape is measurable at every
    /// vintage, so an old prelude buys no exemption from it.
    #[test]
    fn vintage_exempts_only_the_two_metrics_the_prelude_gates() {
        let mut m = conformant();
        m.nodes = 1100;
        let old = Vintage {
            has_phase_verb: false,
            vocabulary_is_closed: false,
            graph_present: true,
        };
        let f = compare(&inquiry(), &m, &old);
        let npc = f.iter().find(|f| f.metric == "nodes-per-commit").unwrap();
        assert_eq!(npc.verdict, Verdict::Divergent);
    }

    #[test]
    fn an_empty_repository_is_unmeasurable_rather_than_divergent() {
        let f = compare(&inquiry(), &Measurement::default(), &current());
        assert!(
            f.iter().all(|f| f.verdict == Verdict::Unmeasurable),
            "{f:?}"
        );
    }

    /// A slot the profile leaves empty produces no finding. That is what distinguishes
    /// *this practice makes no claim here* from *this repository was not measured*.
    #[test]
    fn an_unpopulated_slot_is_not_reported() {
        let p = Profile::parse("kuten: minimal\nrevision: 1\n").unwrap();
        assert!(compare(&p, &conformant(), &current()).is_empty());
    }

    #[test]
    fn the_phase_verb_is_read_as_a_table_row_and_not_as_the_word() {
        // Every vintage of GRAPH.md uses the word "phase" in prose. Only the vocabulary
        // table's own row says the verb exists.
        let prose = "A phase is a bounded unit of work. Phases settle onto the baseline.";
        assert!(!Vintage::read(prose).has_phase_verb);
        let table = "| Verb | When |\n| `phase` | A phase settled |\n";
        assert!(Vintage::read(table).has_phase_verb);
    }

    #[test]
    fn a_missing_vendored_graph_is_absent_rather_than_old() {
        let v = Vintage::absent();
        assert!(!v.graph_present);
        assert!(!v.has_phase_verb);
    }

    #[test]
    fn the_median_averages_the_two_middles_on_an_even_count() {
        assert_eq!(median(&[]), None);
        assert_eq!(median(&[40]), Some(40.0));
        assert_eq!(median(&[35, 45]), Some(40.0));
        assert_eq!(median(&[10, 35, 45]), Some(35.0));
    }

    #[test]
    fn a_declaration_carries_a_revision() {
        let d = Declaration::parse("kuten: inquiry\nrevision: 3\n").unwrap();
        assert_eq!(d.name, "inquiry");
        assert_eq!(d.revision, 3);
        // A record with no revision cannot be compared across one, so it does not parse.
        assert!(Declaration::parse("kuten: inquiry\n").is_err());
    }

    /// Upstream adding a slot must not make a vendored profile unreadable in a binary that
    /// predates it.
    #[test]
    fn an_unknown_slot_parses() {
        let p = Profile::parse("kuten: inquiry\nrevision: 1\nsomething_new: {a: 1}\n").unwrap();
        assert_eq!(p.name, "inquiry");
    }
}
