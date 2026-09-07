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

/// The `AGENTS.md` declaration block, re-exported from `cmd` where it is written.
///
/// A populated slot has exactly two places it can reach a reader — this block and
/// [`compare`] — and the guard that holds every populated slot to a consumer has to be able
/// to ask both. `cmd` is a private module, so without this the guard could ask only half the
/// question, and half of that guard is precisely what lets a slot with no consumer ship.
pub use crate::cmd::kuten::render_block;

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

/// The direction of the arrow between corpus and object — RFC-0028 §6.
///
/// `authored` is the default and the only value `inquiry` proposes: the corpus is written in
/// git, `GRAPH.md`'s premise holds, and every history-derived surface applies. `projected`
/// says the arrow runs object → corpus — the corpus is regenerated from the object by the
/// repository's own tooling, and `git log` is the audit trail of the project rather than of
/// the corpus. Projection is a **declared state**, not misuse: the largest repository in A0's
/// population reached it deliberately (#582), and a model with no word for it forces every
/// repository that reaches the same conclusion to re-derive it.
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    #[default]
    Authored,
    Projected,
}

impl Direction {
    pub fn name(self) -> &'static str {
        match self {
            Self::Authored => "authored",
            Self::Projected => "projected",
        }
    }

    /// The state in the words a reader of `doctor` or `AGENTS.md` needs.
    pub fn describe(self) -> &'static str {
        match self {
            Self::Authored => "the corpus is authored in git",
            Self::Projected => "the corpus is projected from its object",
        }
    }
}

/// The artifact outside the corpus, and the direction of the arrow between them.
///
/// # There is no `paths` field, and that is settled
///
/// RFC-0028 §4 describes the slot as naming the object's paths. It cannot, and the reason is
/// structural rather than a matter of taste. A kuten is an **upstream-authored profile**
/// vendored unchanged: `inquiry` is one profile serving six repositories with six different
/// object shapes, and [`Declaration`] — the only thing a corpus writes — is `{kuten,
/// revision}`. There is no channel by which a corpus supplies paths to it. Paths are a fact
/// about a repository, not about a practice.
///
/// So the live register lives in `[object] paths` in `.yidam/config.toml`, on the precedent
/// RFC-0028 §9 already argues for the clocks: **the kuten proposes values, never holds live
/// ones.** What the kuten declares here is the one thing that *is* a property of the
/// practice — which way the arrow runs.
#[derive(Debug, Clone, Deserialize)]
pub struct Object {
    #[serde(default)]
    pub direction: Direction,
}

/// Which of a repository's two registers a path belongs to — RFC-0028 §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Register {
    /// The corpus. Everything the declared vocabulary governs, and the default: a path no
    /// declaration claims is corpus, because a repository that has said nothing about an
    /// object has exactly one register.
    Corpus,
    /// The artifact outside the corpus, as `[object] paths` names it.
    Object,
}

/// Which registers one commit's paths fall in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Touch {
    CorpusOnly,
    ObjectOnly,
    Both,
    /// The commit lists no paths at all.
    ///
    /// **Governed by the corpus register, and that is the decision rather than a fallthrough.**
    /// `git log --name-only` prints no names for a merge commit, so an authored merge — the
    /// `adopt: the baseline after electoral-purpose` form the vocabulary asks for — arrives
    /// here with an empty path list. Absence of evidence is not a declaration of jurisdiction:
    /// reading it as `ObjectOnly` would silence the verb check on exactly the commits where
    /// two inquiry threads join, which is where it earns its keep.
    None,
}

/// The two registers a repository has.
///
/// # Why the paths are read from the corpus and not from the kuten
///
/// See [`Object`]. The kuten declares the *direction* of the arrow, which is a property of the
/// practice; the paths are a property of this repository, and they live in `[object] paths` in
/// `.yidam/config.toml`.
///
/// # The corpus register is the default, and nothing enumerates it
///
/// There is no corpus glob list. A repository that declares no object has one register and
/// every path is in it, which is what [`Registers::corpus_only`] means and why it reproduces
/// today's behaviour exactly. Declaring the corpus's own paths instead would make the
/// undeclared case ambiguous — a new top-level directory would silently leave the corpus's
/// jurisdiction — and it is the object that is the exception, in three of eighteen measured
/// repositories.
#[derive(Debug, Clone, Default)]
pub struct Registers {
    /// Globs naming the object register. Empty means the repository has one register.
    object: Vec<String>,
}

impl Registers {
    /// Every repository that declares no object.
    ///
    /// Every path is corpus, so [`Registers::touch`] answers `CorpusOnly` for any commit with
    /// paths and `None` for one without — and `lint --commits` reports exactly what it
    /// reported before this existed.
    pub fn corpus_only() -> Self {
        Self { object: Vec::new() }
    }

    /// `[object] paths` from `.yidam/config.toml`.
    ///
    /// A repository with no config file, or one whose `[object]` section is absent or lists
    /// no paths, gets [`Registers::corpus_only`]. So does one whose config fails to parse:
    /// this is a report, and a malformed config is `lint`'s own finding to make, not a reason
    /// for the verb check to change what it governs.
    pub fn of_repo(root: &Path) -> Self {
        let paths = crate::config::load_yidam_config(root)
            .map(|c| c.object.paths)
            .unwrap_or_default();
        Self::of_globs(paths)
    }

    /// The registers a given set of object globs describes.
    pub fn of_globs(object: Vec<String>) -> Self {
        Self {
            object: object
                .into_iter()
                .map(|g| g.trim().trim_end_matches('/').to_string())
                .filter(|g| !g.is_empty())
                .collect(),
        }
    }

    /// Whether any object path is declared at all.
    pub fn declares_object(&self) -> bool {
        !self.object.is_empty()
    }

    /// Which register one repository-relative path falls in.
    pub fn register_of(&self, path: &str) -> Register {
        let path = path.trim_start_matches("./");
        if self.object.iter().any(|g| glob_covers(g, path)) {
            Register::Object
        } else {
            Register::Corpus
        }
    }

    /// Which registers a commit's paths fall in.
    pub fn touch(&self, paths: &[String]) -> Touch {
        let mut corpus = false;
        let mut object = false;
        for p in paths {
            match self.register_of(p) {
                Register::Corpus => corpus = true,
                Register::Object => object = true,
            }
        }
        match (corpus, object) {
            (true, true) => Touch::Both,
            (true, false) => Touch::CorpusOnly,
            (false, true) => Touch::ObjectOnly,
            (false, false) => Touch::None,
        }
    }
}

/// Whether `pattern` claims `path` — matching the path itself or any directory above it.
///
/// The ancestor rule is what makes `paths = ["web"]` mean the directory rather than a file
/// called `web`, which is the form anyone writing this by hand will reach for first. `web/**`
/// says the same thing explicitly and both work.
fn glob_covers(pattern: &str, path: &str) -> bool {
    let pat: Vec<&str> = pattern.split('/').collect();
    let segs: Vec<&str> = path.split('/').collect();
    (1..=segs.len()).any(|n| glob_match(&pat, &segs[..n]))
}

/// Segment-wise glob match. `**` spans any number of segments, `*` any run within one.
fn glob_match(pat: &[&str], segs: &[&str]) -> bool {
    match pat.first() {
        None => segs.is_empty(),
        Some(&"**") => (0..=segs.len()).any(|skip| glob_match(&pat[1..], &segs[skip..])),
        Some(p) => match segs.first() {
            Some(s) if segment_match(p, s) => glob_match(&pat[1..], &segs[1..]),
            _ => false,
        },
    }
}

/// One segment against one pattern segment, where `*` matches any run of characters.
fn segment_match(pat: &str, seg: &str) -> bool {
    let parts: Vec<&str> = pat.split('*').collect();
    if parts.len() == 1 {
        return pat == seg;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !seg.starts_with(first) || !seg.ends_with(last) || seg.len() < first.len() + last.len() {
        return false;
    }
    let mut rest = &seg[first.len()..seg.len() - last.len()];
    for mid in &parts[1..parts.len() - 1] {
        match rest.find(mid) {
            Some(i) => rest = &rest[i + mid.len()..],
            None => return false,
        }
    }
    true
}

/// What kind of question this corpus should be opening.
///
/// `Coverage` is reserved and unimplemented: it needs a class to declare what its instances
/// span, which is #578 and is unscheduled. It parses, and it never diverges.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PressureKind {
    Epistemic,
    Coverage,
}

impl PressureKind {
    /// Every kind the model names.
    ///
    /// The layer document carries the same set as a table, and a guard asserts the two agree
    /// — so a kind that exists in one place and not the other cannot ship.
    pub const ALL: &'static [Self] = &[Self::Epistemic, Self::Coverage];

    pub fn name(self) -> &'static str {
        match self {
            Self::Epistemic => "epistemic",
            Self::Coverage => "coverage",
        }
    }
}

/// The criteria a contribution is scored by — RFC-0028 §1, and #286.
///
/// # Criteria only. There is no band here, and that is a decision
///
/// Every other populated slot carries intervals measured over eighteen derived corpora, and
/// the profile's own header says *"not one of those four was chosen"*. There is no equivalent
/// measurement for a rubric: what was measured is that each criterion **discriminates** across
/// ranges, not what a good reading of one is. A band here would be a number believed because
/// it is written down, which is the failure this whole layer exists to name.
///
/// So the slot says *which* criteria this practice reads, and [`crate::score`] says what each
/// one computes. `yidam score` reports a row per criterion and no verdict.
#[derive(Debug, Clone, Deserialize)]
pub struct Rubric {
    pub criteria: Vec<String>,
}

/// What kind of question this corpus should be opening — RFC-0028 §5.
///
/// **It creates pressure toward a kind of question; it does not author one.** That is
/// RFC-0020's licence read exactly: opening a question asserts nothing the work did not
/// already assert, which is why `propose` may draft `open:` and may not draft `establish:`.
/// The slot reaches a report and nothing else.
#[derive(Debug, Clone, Deserialize)]
pub struct QuestionPressure {
    pub kind: PressureKind,
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
    #[serde(default)]
    pub object: Option<Object>,
    #[serde(default)]
    pub question_pressure: Option<QuestionPressure>,
    #[serde(default)]
    pub rubric: Option<Rubric>,
}

impl Profile {
    /// The criteria a contribution held to this profile is scored on.
    ///
    /// **[`crate::score::Criterion::ALL`] where the profile declares none**, which is the
    /// state every repository is in: nothing retrofits a kuten into an existing corpus, and 0
    /// of 18 derived corpora hold one. The neutral arm runs the same criteria — what differs
    /// is whose selection it is, and the report says which.
    pub fn criteria(profile: Option<&Self>) -> Vec<String> {
        profile
            .and_then(|p| p.rubric.as_ref())
            .map(|r| r.criteria.clone())
            .unwrap_or_else(crate::score::Criterion::all_ids)
    }
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

/// Every run of whitespace — newlines included — as a single space, so a phrase can be
/// matched without knowing where the paragraph it sits in was wrapped.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
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
    /// which appears in this document's prose dozens of times in every vintage. A GFM table
    /// row is one line by construction, so that match cannot be broken by reflowing the
    /// document; it would be broken by a formatter that drops the leading pipe, or by a
    /// column inserted before `Verb`, and neither has ever happened here.
    ///
    /// The closed list is matched on the sentence that declares it closed, but against the
    /// document with its whitespace collapsed rather than line by line. Matching the raw text
    /// was the shipped behaviour and it read `false` against this template's own prelude and
    /// all eighteen derived corpora: the sentence had been rewrapped so that `This` ended one
    /// line and `list is closed:` began the next, and `contains` sees the newline. The
    /// sentence is prose and will be rewrapped again, so nothing that treats a line break as
    /// significant can hold. Collapsing first makes the match indifferent to where the
    /// paragraph happens to break.
    pub fn read(graph_md: &str) -> Self {
        let has_phase_verb = graph_md
            .lines()
            .any(|l| l.trim_start().starts_with("| `phase`"));
        let vocabulary_is_closed = collapse_whitespace(graph_md).contains("This list is closed");
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
    ///
    /// **Every authored commit, and deliberately not register-scoped** — unlike
    /// `lint --commits`, which since A3 declines to report a commit touching only the object
    /// register. The asymmetry is a decision and was measured before it was taken.
    ///
    /// *It costs nothing today.* Under [`Registers::corpus_only`] — the state of all six
    /// repositories that defined this profile, none of which holds a `.yidam/config.toml` at
    /// all — **zero** commits change register in any of them, or in either object-coupled
    /// repository. Scoping and not scoping give the same number for every corpus measured.
    ///
    /// *It would cost something later.* This number is read against a band. Scoping it would
    /// make a band-checked quantity settable from `.yidam/config.toml`: a corpus could move
    /// its own `off_vocabulary_share` toward `0.00–0.00` by widening `[object] paths`,
    /// without writing a commit. The counterfactual measures the lever — declaring every
    /// top-level path but `.yidam/` as the object takes matt-huffman from 0.1671 to 0.1291
    /// and ohio-education-funding from 0.4930 to 0.3248. A conformance reading a corpus can
    /// dial is not a measurement, and RFC-0024's rule against a gate loosened quietly is the
    /// same argument one level out.
    ///
    /// So `lint --commits` reports what a reader is asked to act on, scoped to the register
    /// the vocabulary governs; this measures what the repository did, whole. A repository
    /// declaring `[object] paths` will see the two differ, and that is the intended reading
    /// rather than a defect.
    pub off_vocabulary_commits: usize,
    /// Instance nodes in the corpus.
    pub nodes: usize,
    /// Median instance node length, in lines. `None` when there are no nodes.
    pub median_node_lines: Option<f64>,
    /// Instance nodes the corpus currently holds open as questions.
    ///
    /// **Corpus state, and deliberately not `open:` commits.** RFC-0028 §5's example rule
    /// counts commits; measured over the six repositories that defined this profile, two of
    /// them — bitlocker and hermetic-ch — have **zero** `open:` commits while holding 27 and
    /// 15 open-tagged corpus files. A rule that reports divergence against two of its own
    /// defining members is what §9 calls a wrong extraction.
    ///
    /// Counted with [`crate::claims::is_open_question`], which is the predicate
    /// `yidam open-questions`, `due` and the MCP server already share. A second notion of
    /// what an open question is would be a second answer to a settled question.
    #[serde(default)]
    pub open_questions: usize,
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
            Self::Conforming => "ok",
            Self::Divergent => "diverges",
            Self::Vintage => "vintage",
            Self::Unmeasurable => "unmeasured",
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

/// The question-pressure finding — RFC-0028 §5.
///
/// **A second construction path into [`Finding`], and deliberately not a second finding
/// type.** [`Metric::read`] answers one question — *is this number inside that interval* —
/// and question pressure is not that shape: what the kuten declares is a **kind**, and what
/// the repository shows is a count of open questions. `Finding` is already band-agnostic
/// (`declared` and `measured` are both `String`), so the consumer keying on `metric` and
/// `verdict` sees one report and not two, and no new [`Verdict`] variant is needed.
fn question_pressure_finding(pressure: &QuestionPressure, m: &Measurement) -> Finding {
    let finding = |verdict, measured: String, question| Finding {
        slot: "question_pressure",
        metric: "question-pressure",
        verdict,
        declared: pressure.kind.name().to_string(),
        measured,
        question,
    };

    // Reserved, and it never diverges. A corpus can only be pressed to open *coverage*
    // questions once a class can declare what its instances span, and that is #578 —
    // unscheduled, on the record. Naming the state is the whole of this epic's interface to
    // it; reporting a corpus as divergent against an unimplemented rule would be worse than
    // reporting nothing.
    if pressure.kind == PressureKind::Coverage {
        return finding(
            Verdict::Unmeasurable,
            "reserved and unimplemented — a class cannot yet declare what its instances span \
             (#578)"
                .to_string(),
            None,
        );
    }

    // No history is not a corpus that has stopped asking. It is a corpus with nothing to
    // read, and that is `Unmeasurable` here exactly as it is for every band.
    if m.commits == 0 {
        return finding(
            Verdict::Unmeasurable,
            "nothing to measure".to_string(),
            None,
        );
    }

    if m.open_questions > 0 {
        return finding(
            Verdict::Conforming,
            format!("{} open", m.open_questions),
            None,
        );
    }

    finding(
        Verdict::Divergent,
        "none open".to_string(),
        Some(format!(
            "this corpus holds no open question across {} commits, against a declared pressure \
             toward {} ones. What is this corpus currently unsure of — and is the answer that \
             its questions are settled, or that they are being asked somewhere the corpus \
             cannot see?",
            m.commits,
            pressure.kind.name()
        )),
    )
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
///
/// One finding is not band-shaped: `question_pressure` declares a **kind**, and it is built
/// by [`question_pressure_finding`] rather than by [`Metric::read`]. It carries the same four
/// verdicts as every other finding, and adds none.
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

    if let Some(pressure) = &profile.question_pressure {
        out.push(question_pressure_finding(pressure, m));
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

/// Every kuten profile vendored in this repository, by directory name, sorted.
///
/// **Discovered, never listed.** The vendor step copies the whole `yidam/prelude/` tree
/// (`cp -R`), so a profile added upstream arrives in a derived repository without anything
/// here being told about it. A command offering a hardcoded set would go on offering
/// yesterday's, and the one thing a corpus adopting a practice must be able to see is what it
/// actually holds.
pub fn vendored_profiles(root: &Path) -> Vec<String> {
    let dir = root.join(VENDORED_DIR);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out: Vec<String> = entries
        .filter_map(Result::ok)
        .filter(|e| e.path().join("kuten.yml").is_file())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    out.sort();
    out
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
///
/// The corpus half walks the instances once and asks two questions of each: how long it is,
/// and whether it is an open question. The second goes through
/// [`crate::claims::is_open_question`] — **the** open-question predicate, already shared by
/// `yidam open-questions`, `due`, `lint --history` and the MCP server, and frozen in
/// `sdks/parity/mcp/tools.json`. A count of open questions computed any other way here would
/// be a fifth answer to a question that has exactly one.
pub fn measure(root: &Path) -> Measurement {
    let subjects = crate::cmd::lint::commits::read_subjects(root, None);
    let authored: Vec<&crate::cmd::lint::commits::Subject> = subjects
        .iter()
        .filter(|s| !crate::cmd::lint::commits::is_merge(&s.text, s.parents))
        .collect();

    let corpus = crate::paths::yidam_corpus_dir(root);
    let fields = crate::claims::ClaimFields::load(&corpus);
    let mut lines: Vec<usize> = Vec::new();
    let mut open_questions = 0usize;
    for path in crate::walk::walk_corpus_instances(&corpus) {
        lines.push(crate::walk::line_count(&path));
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let inst: crate::parse::CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let label = inst.label.unwrap_or_default();
        let class = inst.class.unwrap_or_default();
        if crate::claims::is_open_question(&label, &text, fields.for_class(&class)) {
            open_questions += 1;
        }
    }
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
        open_questions,
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
             phases:\n  types: [Investigation]\n  commit_share: {low: 0.12, high: 0.27}\n\
             vocabulary:\n  verbs: [establish]\n  off_vocabulary_share: {low: 0.0, high: 0.02}\n\
             classes:\n  nodes_per_commit: {low: 0.50, high: 1.12}\n  median_node_lines: {low: 35, high: 62}\n",
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

    /// A profile that also declares the two kind-shaped slots A3 populates.
    fn inquiry_with_pressure(kind: &str) -> Profile {
        Profile::parse(&format!(
            "kuten: inquiry\nrevision: 1\n\
             object:\n  direction: authored\n\
             question_pressure:\n  kind: {kind}\n"
        ))
        .expect("the fixture profile parses")
    }

    /// A repository sitting on the middle of every band.
    fn conformant() -> Measurement {
        Measurement {
            commits: 100,
            phase_commits: 20,
            off_vocabulary_commits: 0,
            nodes: 80,
            median_node_lines: Some(48.0),
            open_questions: 6,
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

    // ── the two kind-shaped slots ─────────────────────────────────────────────

    /// `authored` is the default, so a profile that names the slot without naming a
    /// direction is not thereby a projection.
    #[test]
    fn the_object_direction_defaults_to_authored() {
        let p = Profile::parse("kuten: x\nrevision: 1\nobject: {}\n").unwrap();
        assert_eq!(
            p.object.expect("the object slot").direction,
            Direction::Authored
        );
        let p = Profile::parse("kuten: x\nrevision: 1\nobject:\n  direction: projected\n").unwrap();
        assert_eq!(
            p.object.expect("the object slot").direction,
            Direction::Projected
        );
    }

    /// The object slot produces no finding: a direction is a declared state, not a
    /// measurement, and there is nothing in a repository to read it against.
    #[test]
    fn the_object_slot_declares_a_state_and_reports_no_divergence() {
        let p = Profile::parse("kuten: x\nrevision: 1\nobject:\n  direction: projected\n").unwrap();
        assert!(compare(&p, &conformant(), &current()).is_empty());
    }

    #[test]
    fn a_corpus_holding_open_questions_conforms_to_epistemic_pressure() {
        let f = compare(
            &inquiry_with_pressure("epistemic"),
            &conformant(),
            &current(),
        );
        assert_eq!(f.len(), 1, "one slot declared, one finding: {f:?}");
        assert_eq!(f[0].metric, "question-pressure");
        assert_eq!(f[0].verdict, Verdict::Conforming);
        assert_eq!(f[0].measured, "6 open");
        assert!(f[0].question.is_none(), "a conforming metric asks nothing");
    }

    /// The divergence RFC-0028 §5 names, and it is a question rather than a defect.
    #[test]
    fn a_corpus_that_has_opened_nothing_is_asked_about_it() {
        let mut m = conformant();
        m.open_questions = 0;
        let f = compare(&inquiry_with_pressure("epistemic"), &m, &current());
        assert_eq!(f[0].verdict, Verdict::Divergent);
        assert_eq!(f[0].measured, "none open");
        let q = f[0]
            .question
            .as_deref()
            .expect("divergence asks a question");
        assert!(q.contains("100 commits"), "{q}");
    }

    /// A repository with no history has not stopped asking; it has nothing to read.
    #[test]
    fn question_pressure_over_an_empty_history_is_unmeasurable() {
        let f = compare(
            &inquiry_with_pressure("epistemic"),
            &Measurement::default(),
            &current(),
        );
        assert_eq!(f[0].verdict, Verdict::Unmeasurable);
        assert!(f[0].question.is_none());
    }

    /// **G3.** The reserved kind parses, and it never diverges — whatever the repository
    /// shows. #578 is unscheduled, and a corpus must not be reported as failing a rule
    /// nothing has implemented.
    #[test]
    fn the_reserved_coverage_kind_parses_and_never_diverges() {
        let profile = inquiry_with_pressure("coverage");
        assert_eq!(
            profile.question_pressure.as_ref().expect("the slot").kind,
            PressureKind::Coverage
        );
        let shapes = [
            Measurement::default(),
            conformant(),
            Measurement {
                open_questions: 0,
                ..conformant()
            },
            Measurement {
                commits: 1,
                open_questions: 0,
                ..Measurement::default()
            },
        ];
        for m in shapes {
            let f = compare(&profile, &m, &current());
            assert_eq!(f.len(), 1, "{m:?}");
            assert_eq!(
                f[0].verdict,
                Verdict::Unmeasurable,
                "coverage is reserved and unimplemented; {m:?} must not read as divergence"
            );
            assert!(f[0].measured.contains("#578"), "{:?}", f[0]);
            assert!(f[0].question.is_none());
        }
    }

    /// Adding a kind-shaped finding must not have added a verdict. Four is the set every
    /// consumer — the JSON schema included — is written against.
    #[test]
    fn the_kind_shaped_finding_adds_no_verdict() {
        let mut seen = std::collections::BTreeSet::new();
        for kind in ["epistemic", "coverage"] {
            for m in [
                Measurement::default(),
                conformant(),
                Measurement {
                    open_questions: 0,
                    ..conformant()
                },
            ] {
                for f in compare(&inquiry_with_pressure(kind), &m, &current()) {
                    seen.insert(f.verdict.tag());
                }
            }
        }
        assert!(
            seen.is_subset(
                &["ok", "diverges", "vintage", "unmeasured"]
                    .into_iter()
                    .collect()
            ),
            "{seen:?}"
        );
    }

    #[test]
    fn every_pressure_kind_the_model_names_parses_under_its_own_name() {
        for kind in PressureKind::ALL {
            let p = Profile::parse(&format!(
                "kuten: x\nrevision: 1\nquestion_pressure:\n  kind: {}\n",
                kind.name()
            ))
            .unwrap_or_else(|e| panic!("`{}` does not parse ({e})", kind.name()));
            assert_eq!(p.question_pressure.expect("the slot").kind, *kind);
        }
    }

    /// Upstream adding a slot must not make a vendored profile unreadable in a binary that
    /// predates it.
    #[test]
    fn an_unknown_slot_parses() {
        let p = Profile::parse("kuten: inquiry\nrevision: 1\nsomething_new: {a: 1}\n").unwrap();
        assert_eq!(p.name, "inquiry");
    }

    // -- registers ------------------------------------------------------------------

    /// **G9's other half.** A repository that declares no object has one register, and every
    /// path is in it — including the ones that look most like an artifact.
    #[test]
    fn with_no_declaration_every_path_is_corpus() {
        let r = Registers::corpus_only();
        for p in [
            ".yidam/corpus/a.md",
            "web/index.html",
            "crates/x/src/lib.rs",
            "package.json",
            "README.md",
        ] {
            assert_eq!(r.register_of(p), Register::Corpus, "{p}");
        }
        assert!(!r.declares_object());
        assert_eq!(r.touch(&["web/index.html".into()]), Touch::CorpusOnly);
    }

    #[test]
    fn a_declared_glob_claims_what_it_names_and_everything_beneath_it() {
        let r = Registers::of_globs(vec![
            "web/**".into(),
            "crates".into(),
            "package.json".into(),
        ]);
        for p in [
            "web/index.html",
            "web/src/app/main.tsx",
            "crates/x/src/lib.rs",
            "package.json",
        ] {
            assert_eq!(r.register_of(p), Register::Object, "{p}");
        }
        for p in [".yidam/corpus/a.md", "README.md", "docs/web.md"] {
            assert_eq!(r.register_of(p), Register::Corpus, "{p}");
        }
    }

    #[test]
    fn a_star_stays_inside_one_segment_and_a_double_star_does_not() {
        let one = Registers::of_globs(vec!["src/*.rs".into()]);
        assert_eq!(one.register_of("src/main.rs"), Register::Object);
        assert_eq!(one.register_of("src/cmd/main.rs"), Register::Corpus);

        let many = Registers::of_globs(vec!["src/**/*.rs".into()]);
        assert_eq!(many.register_of("src/main.rs"), Register::Object);
        assert_eq!(many.register_of("src/cmd/lint/main.rs"), Register::Object);
        assert_eq!(many.register_of("src/main.py"), Register::Corpus);
    }

    #[test]
    fn every_touch_the_split_can_produce() {
        let r = Registers::of_globs(vec!["web/**".into()]);
        assert_eq!(r.touch(&["web/a.tsx".into()]), Touch::ObjectOnly);
        assert_eq!(r.touch(&[".yidam/corpus/a.md".into()]), Touch::CorpusOnly);
        assert_eq!(
            r.touch(&["web/a.tsx".into(), ".yidam/corpus/a.md".into()]),
            Touch::Both
        );
        assert_eq!(r.touch(&[]), Touch::None);
    }

    /// A blank or slash-suffixed entry is not a glob that claims the repository root.
    #[test]
    fn a_trailing_slash_and_an_empty_entry_are_normalised_away() {
        let r = Registers::of_globs(vec!["web/".into(), "  ".into(), "".into()]);
        assert!(r.declares_object());
        assert_eq!(r.register_of("web/a.tsx"), Register::Object);
        assert_eq!(r.register_of("README.md"), Register::Corpus);
    }

    /// A repository with no `.yidam/config.toml` at all — which is every one of the six that
    /// defined this profile — gets one register.
    #[test]
    fn a_repository_with_no_config_declares_no_object() {
        let dir = tempfile::tempdir().expect("tempdir");
        assert!(!Registers::of_repo(dir.path()).declares_object());
    }

    #[test]
    fn the_paths_are_read_from_the_corpus_config() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".yidam")).expect("mkdir");
        std::fs::write(
            dir.path().join(".yidam/config.toml"),
            "[object]\npaths = [\"web/**\"]\n",
        )
        .expect("write");
        let r = Registers::of_repo(dir.path());
        assert!(r.declares_object());
        assert_eq!(r.register_of("web/a.tsx"), Register::Object);
    }
}
