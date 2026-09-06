//! What a contribution is scored on — the `rubric` slot's vocabulary, and the rows it produces.
//!
//! `yidam/tests/rubric.md` scores a **genesis commit**. It fires once in a repository's life,
//! and after it nothing answers *was this session's work any good* except the gates, which
//! answer only *did it break anything*. That is a floor and not a standard, and it matters
//! more for an agent than for a person: an agent arrives cold each session and has no
//! accumulated judgement to fall back on.
//!
//! # Rows, and no verdict
//!
//! Every criterion produces exactly one [`Row`], carrying a [`Reading`] and the evidence the
//! reading was drawn from. There is no overall number, no `conforming` flag and no band —
//! and each of those absences is a decision rather than an omission.
//!
//! **No band.** Every other populated slot in a kuten profile carries intervals measured over
//! eighteen derived corpora, and the profile's own header says *"not one of those four was
//! chosen"*. There is no such measurement for a rubric. A band here would be a number believed
//! because it is written down, which is the failure the whole layer exists to stop.
//!
//! **No overall verdict.** A single number over a range of commits names a person's session.
//! What is defensible is a reading per criterion with the nodes and commits it came from, so
//! that a reader can disagree with it by looking.
//!
//! **Nothing gates.** `cmd/kuten.rs` states the rule for this layer — *"Exits zero, always.
//! Divergence is not a defect, and a report that gated would make it one"* — and a score that
//! gated would be a gate decided by the kuten, which is exactly why the `thresholds` slot
//! ships empty.
//!
//! # Measured before shipped
//!
//! Each of the three criteria below was run over 10-commit windows across the derived corpora
//! and kept only because it discriminated. The three that were tried and dropped are in
//! [`CONSIDERED_AND_REJECTED`], with the measurement that dropped them — named in the model
//! rather than in a commit message, because a criterion nobody records rejecting is one
//! somebody proposes again.

use serde::Serialize;

/// How many evidence lines a row carries before it summarises the rest.
///
/// Evidence is the *denominator* a reading was drawn from, marked item by item — not only the
/// items that dragged it down. A reading of 1.00 and a reading of 0.00 both have something to
/// show, and a row that showed nothing when everything went right would make "perfect" and
/// "not computed" print the same.
const EVIDENCE_LIMIT: usize = 12;

/// A criterion a contribution is scored on.
///
/// **The set is closed here and declared per corpus.** A profile's `rubric.criteria` names
/// which of these this practice reads; a profile that names something else gets a row saying
/// so rather than silence, on the same forward-compatibility rule [`crate::kuten::Profile`]
/// parses unknown slots by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Criterion {
    /// The epistemic share **of the recognized subset**.
    Register,
    /// The share of surviving added nodes that something points at.
    Landing,
    /// How many of the surviving added nodes are open questions.
    Questions,
}

impl Criterion {
    /// Every criterion this binary implements.
    ///
    /// Also the neutral arm's selection: a repository holding no kuten is scored on all of
    /// them, and the report says the selection is the template's rather than that corpus's.
    pub const ALL: &'static [Criterion] = &[
        Criterion::Register,
        Criterion::Landing,
        Criterion::Questions,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Criterion::Register => "register",
            Criterion::Landing => "landing",
            Criterion::Questions => "questions",
        }
    }

    /// One line a reader of `AGENTS.md` can act on.
    pub fn gloss(self) -> &'static str {
        match self {
            Criterion::Register => {
                "of the commits whose verb the vocabulary recognizes, how many carry testimony \
                 rather than housekeeping"
            }
            Criterion::Landing => {
                "of the nodes this range added and left standing, how many something points at"
            }
            Criterion::Questions => {
                "how many of those nodes are open questions — a number, never a threshold"
            }
        }
    }

    pub fn from_id(id: &str) -> Option<Criterion> {
        Criterion::ALL.iter().copied().find(|c| c.id() == id)
    }

    /// The ids of every criterion, in declaration order.
    pub fn all_ids() -> Vec<String> {
        Criterion::ALL.iter().map(|c| c.id().to_string()).collect()
    }
}

/// A criterion that was measured and not built.
pub struct Rejected {
    pub id: &'static str,
    pub why: &'static str,
}

/// The criteria #286 names or invites, measured and dropped.
///
/// Recorded in the model and asserted against, rather than left in a commit message: each of
/// these is an obvious thing to reach for, and the measurement that says not to is the only
/// thing standing between the next reader and rebuilding it.
pub const CONSIDERED_AND_REJECTED: &[Rejected] = &[
    Rejected {
        id: "reachable",
        why: "out-degree — #286's \"did new nodes enter the graph reachable\". `orphan-out` is \
              Severity::Error, so a corpus holding an out-degree-zero node fails its own gate; \
              across sixteen corpora 0 of 2,736 nodes have none. Scoring it measures the gate \
              rather than the contribution. `orphan-in` is Info, ungated, and spans 16%–75% \
              across the same population, which is why `landing` reads in-degree instead",
    },
    Rejected {
        id: "opened",
        why: "the presence of an `open:` or `close:` commit. 2 of 72 measured windows contain \
              an `open:` at all, and a criterion 97% of ranges fail is not a criterion. What \
              survives of the idea is `questions`, read over the nodes rather than the verbs — \
              the same correction RFC-0028 §5 already made for question pressure",
    },
    Rejected {
        id: "epistemic-share",
        why: "the naive epistemic share over every authored commit. `classify_commit` is total \
              — Operational is the listed case and everything else falls through to Epistemic \
              — so this reads 1.00 for a corpus whose subjects are conventional commits with \
              no recognized verb at all. The two readings disagree by up to 0.90 over the same \
              range. `register` computes over the recognized subset for this reason",
    },
];

/// One commit in the range, reduced to what the rubric reads.
#[derive(Debug, Clone)]
pub struct CommitFact {
    /// Short hash, as a reader would quote it.
    pub short: String,
    /// The leading verb, or empty where the subject has no `verb: ` prefix.
    pub verb: String,
    /// Whether the closed vocabulary carries that verb.
    pub recognized: bool,
    /// Whether `classify_commit` files it as epistemic.
    ///
    /// Meaningful **only** where `recognized` is true; see [`CONSIDERED_AND_REJECTED`].
    pub epistemic: bool,
}

/// One node the range added and the tip still holds.
#[derive(Debug, Clone)]
pub struct NodeFact {
    /// Repository-relative path, as the gate names a node.
    pub rel: String,
    /// Whether anything points at it, through `orphan-in`'s own resolution.
    pub landed: bool,
    /// Whether it is an open question, through `claims::is_open_question`.
    pub open_question: bool,
}

/// What a range did, measured. **Data, so the scoring is a pure function of it.**
///
/// The reading and the git reconstruction are separated for the reason `kuten::compare` is
/// separated from `kuten::measure`: every shape a scored range can take is then assertable
/// without a repository being on disk in that shape.
#[derive(Debug, Clone, Default)]
pub struct Contribution {
    /// Authored, non-merge commits in the range.
    pub commits: Vec<CommitFact>,
    /// Nodes present at the tip and absent at the base.
    pub added: Vec<NodeFact>,
}

impl Contribution {
    fn recognized(&self) -> Vec<&CommitFact> {
        self.commits.iter().filter(|c| c.recognized).collect()
    }
}

/// What one criterion read.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Reading {
    /// A number, and how it is written for a reader.
    Measured { value: f64, shown: String },
    /// There was no denominator. **Never 0.00**, which is a different fact.
    Unmeasurable { why: String },
}

impl Reading {
    fn measured(value: f64, shown: String) -> Reading {
        Reading::Measured { value, shown }
    }

    fn unmeasurable(why: impl Into<String>) -> Reading {
        Reading::Unmeasurable { why: why.into() }
    }

    pub fn is_measured(&self) -> bool {
        matches!(self, Reading::Measured { .. })
    }

    /// What prints in the value column.
    pub fn shown(&self) -> &str {
        match self {
            Reading::Measured { shown, .. } => shown,
            Reading::Unmeasurable { .. } => "unmeasurable",
        }
    }
}

/// One criterion's whole answer.
#[derive(Debug, Clone, Serialize)]
pub struct Row {
    /// The criterion id, as the profile declares it.
    pub criterion: String,
    pub reading: Reading,
    /// What the reading was drawn from, item by item. Capped, and the cap says so.
    pub evidence: Vec<String>,
    /// What a reader has to know to read the number correctly.
    pub note: Option<String>,
}

fn percent(v: f64) -> String {
    format!("{:.0}%", v * 100.0)
}

/// The denominator's items, marked, and truncated with a line saying how many were dropped.
fn evidence<T>(items: &[T], mark: impl Fn(&T) -> String) -> Vec<String> {
    let mut out: Vec<String> = items.iter().take(EVIDENCE_LIMIT).map(mark).collect();
    if items.len() > EVIDENCE_LIMIT {
        out.push(format!("… and {} more", items.len() - EVIDENCE_LIMIT));
    }
    out
}

/// The precondition reported beside `register`, and deliberately not a row of its own.
///
/// The share of authored non-merge commits whose verb the closed vocabulary recognizes.
/// Measured over 72 windows it is bimodal **by repository** — 42 windows at 0.00 in corpora
/// that never adopted the vocabulary, 9 at 1.00 in corpora that did — so it separates
/// repositories rather than contributions, which is not what a contribution rubric is for.
/// It is reported all the same, because a `register` reading computed over three commits out
/// of forty means something different from one computed over forty.
fn legibility(c: &Contribution) -> String {
    let total = c.commits.len();
    if total == 0 {
        return "legibility — no authored non-merge commit in this range".to_string();
    }
    let recognized = c.recognized().len();
    format!(
        "legibility {recognized} of {total} ({}) — authored non-merge commits whose verb the \
         closed vocabulary carries. A precondition, not a score: it separates corpora that \
         adopted the vocabulary from ones that never did",
        percent(recognized as f64 / total as f64)
    )
}

fn register_row(c: &Contribution) -> Row {
    let recognized = c.recognized();
    let note = Some(legibility(c));
    // **Unmeasurable, and never 0.00.** `classify_commit` is total: Operational is the listed
    // case and everything else falls through to Epistemic. Over a range with no recognized
    // verb the naive share reads 1.00 and this reads nothing, and nothing is the honest
    // answer — there is no subset to take a register over.
    if recognized.is_empty() {
        return Row {
            criterion: Criterion::Register.id().to_string(),
            reading: Reading::unmeasurable(
                "no commit in this range carries a verb the closed vocabulary recognizes, so \
                 there is no subset to read a register over. A share computed over the whole \
                 range would be reading `classify_commit`'s fallthrough, not this work",
            ),
            evidence: Vec::new(),
            note,
        };
    }
    let epistemic = recognized.iter().filter(|f| f.epistemic).count();
    let value = epistemic as f64 / recognized.len() as f64;
    Row {
        criterion: Criterion::Register.id().to_string(),
        reading: Reading::measured(
            value,
            format!("{} of {} ({})", epistemic, recognized.len(), percent(value)),
        ),
        evidence: evidence(&recognized, |f| {
            format!(
                "{} `{}:` — {}",
                f.short,
                f.verb,
                if f.epistemic {
                    "testimony"
                } else {
                    "housekeeping"
                }
            )
        }),
        note,
    }
}

fn landing_row(c: &Contribution) -> Row {
    let id = Criterion::Landing.id().to_string();
    let note = Some(
        "in-degree read through `orphan-in`'s own resolution and its class exemption: a node \
         in a class the ontology declares no inbound edge for counts as landed, because \
         nothing is meant to point at it"
            .to_string(),
    );
    if c.added.is_empty() {
        return Row {
            criterion: id,
            reading: Reading::unmeasurable(
                "this range added no corpus node that survives at its tip, so there is nothing \
                 whose landing could be read",
            ),
            evidence: Vec::new(),
            note,
        };
    }
    let landed = c.added.iter().filter(|n| n.landed).count();
    let value = landed as f64 / c.added.len() as f64;
    Row {
        criterion: id,
        reading: Reading::measured(
            value,
            format!("{} of {} ({})", landed, c.added.len(), percent(value)),
        ),
        evidence: evidence(&c.added, |n| {
            format!(
                "{} — {}",
                n.rel,
                if n.landed {
                    "something points at it"
                } else {
                    "nothing links to this node"
                }
            )
        }),
        note,
    }
}

fn questions_row(c: &Contribution) -> Row {
    let id = Criterion::Questions.id().to_string();
    let note = Some(
        "a number, and never a threshold. Whether this corpus should be opening questions is \
         the `question_pressure` slot's to say; this says only what this range did"
            .to_string(),
    );
    if c.added.is_empty() {
        return Row {
            criterion: id,
            reading: Reading::unmeasurable(
                "this range added no corpus node that survives at its tip, so there is nothing \
                 to read a question share over",
            ),
            evidence: Vec::new(),
            note,
        };
    }
    let open = c.added.iter().filter(|n| n.open_question).count();
    let value = open as f64 / c.added.len() as f64;
    Row {
        criterion: id,
        reading: Reading::measured(
            value,
            format!("{} of {} ({})", open, c.added.len(), percent(value)),
        ),
        evidence: evidence(&c.added, |n| {
            format!(
                "{} — {}",
                n.rel,
                if n.open_question {
                    "an open question"
                } else {
                    "not a question"
                }
            )
        }),
        note,
    }
}

/// A criterion this binary has never heard of.
///
/// A row rather than a silence. A corpus vendored a profile at some revision and is running a
/// binary at another; a declared criterion that simply vanished from the report would be
/// indistinguishable from one that scored well.
fn unknown_row(id: &str) -> Row {
    Row {
        criterion: id.to_string(),
        reading: Reading::unmeasurable(format!(
            "`{id}` is declared by this corpus's rubric and this binary does not implement it. \
             The profile is newer than the binary reading it, or the id is misspelled"
        )),
        evidence: Vec::new(),
        note: None,
    }
}

/// Score a contribution against a declared set of criteria. **Pure, and total.**
///
/// Every declared criterion produces exactly one row, in declaration order, including the ones
/// that cannot be read — a criterion that vanished when it had no denominator could not be
/// told from one that was never declared.
pub fn score(criteria: &[String], c: &Contribution) -> Vec<Row> {
    criteria
        .iter()
        .map(|id| match Criterion::from_id(id) {
            Some(Criterion::Register) => register_row(c),
            Some(Criterion::Landing) => landing_row(c),
            Some(Criterion::Questions) => questions_row(c),
            None => unknown_row(id),
        })
        .collect()
}

/// The four questions a person answers and nothing scores.
///
/// Each is irreducible: it turns on whether an assertion is *warranted*, and there is no
/// corpus fact that decides it. They are emitted by `yidam score --brief` and read by nobody
/// else — a judged question with a computed number beside it would be read as the number.
pub const JUDGE_QUESTIONS: &[&str] = &[
    "Did the open questions go where the work actually found gaps — or where they were \
     cheapest to write?",
    "Do the new edges assert relationships the domain supports, or ones the file layout made \
     convenient?",
    "Is every promotion to `[verified]` warranted by the source it now cites, read at the \
     span it pins?",
    "Did the register match the work — did the commits that claim testimony carry any, and did \
     the ones filed as housekeeping change nothing anybody would want to cite?",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn commit(short: &str, verb: &str, recognized: bool, epistemic: bool) -> CommitFact {
        CommitFact {
            short: short.to_string(),
            verb: verb.to_string(),
            recognized,
            epistemic,
        }
    }

    fn node(rel: &str, landed: bool, open_question: bool) -> NodeFact {
        NodeFact {
            rel: rel.to_string(),
            landed,
            open_question,
        }
    }

    /// A range a corpus running the vocabulary would produce.
    fn contribution() -> Contribution {
        Contribution {
            commits: vec![
                commit("a1b2c3d4", "establish", true, true),
                commit("b2c3d4e5", "revise", true, true),
                commit("c3d4e5f6", "regen", true, false),
                commit("d4e5f6a7", "chore", false, true),
            ],
            added: vec![
                node("concept/tailwater.yml", true, false),
                node("concept/hydropeaking.yml", false, true),
            ],
        }
    }

    fn ids() -> Vec<String> {
        Criterion::all_ids()
    }

    /// One row, cloned out of the report — so a caller can score inline without keeping the
    /// whole vector alive for the assertion.
    fn row(rows: Vec<Row>, id: &str) -> Row {
        rows.iter()
            .find(|r| r.criterion == id)
            .unwrap_or_else(|| panic!("no `{id}` row in {rows:?}"))
            .clone()
    }

    /// Every declared criterion produces exactly one row, in declaration order.
    #[test]
    fn the_report_is_total_over_the_declared_criteria() {
        let rows = score(&ids(), &contribution());
        assert_eq!(
            rows.iter().map(|r| r.criterion.clone()).collect::<Vec<_>>(),
            ids()
        );
    }

    /// A corpus declaring a subset is scored on the subset, and on nothing else.
    #[test]
    fn a_declared_subset_is_the_whole_report() {
        let rows = score(&["landing".to_string()], &contribution());
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].criterion, "landing");
    }

    /// **The behavioural rule this whole criterion turns on.** `classify_commit` is total, so
    /// a naive epistemic share reads 1.00 over a range of conventional commits. This reads
    /// nothing, and says why.
    #[test]
    fn register_is_unmeasurable_and_never_zero_on_an_empty_recognized_subset() {
        let c = Contribution {
            commits: vec![
                commit("a1b2c3d4", "feat", false, true),
                commit("b2c3d4e5", "fix", false, true),
            ],
            ..Contribution::default()
        };
        let r = row(score(&ids(), &c), "register");
        assert!(
            !r.reading.is_measured(),
            "a zero denominator must not produce a number: {:?}",
            r.reading
        );
        match &r.reading {
            Reading::Unmeasurable { why } => assert!(why.contains("recognize"), "{why}"),
            other => panic!("{other:?}"),
        }
        // And the naive reading, which is the one this exists to refuse, would have been 1.00.
        assert_eq!(c.commits.iter().filter(|f| f.epistemic).count(), 2);
    }

    /// The share is over the recognized subset, not over the range.
    #[test]
    fn register_reads_the_recognized_subset_only() {
        let r = row(score(&ids(), &contribution()), "register");
        match &r.reading {
            // Two of the three recognized commits are epistemic. The unrecognized `chore:`
            // is outside the denominator entirely, though `classify_commit` calls it
            // epistemic.
            Reading::Measured { value, shown } => {
                assert!((value - 2.0 / 3.0).abs() < 1e-9, "{value}");
                assert!(shown.contains("2 of 3"), "{shown}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// Legibility is reported beside `register` and is never a row of its own.
    #[test]
    fn legibility_rides_along_and_is_not_scored() {
        let rows = score(&ids(), &contribution());
        assert!(
            !rows.iter().any(|r| r.criterion == "legibility"),
            "legibility is a precondition, not a criterion"
        );
        let register = row(rows, "register");
        let note = register
            .note
            .as_deref()
            .expect("register carries the precondition");
        assert!(note.contains("legibility 3 of 4 (75%)"), "{note}");
    }

    #[test]
    fn landing_reads_in_degree_over_the_surviving_added_nodes() {
        let r = row(score(&ids(), &contribution()), "landing");
        match &r.reading {
            Reading::Measured { value, shown } => {
                assert!((value - 0.5).abs() < 1e-9, "{value}");
                assert!(shown.contains("1 of 2"), "{shown}");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn questions_reports_a_count_and_a_share() {
        let r = row(score(&ids(), &contribution()), "questions");
        match &r.reading {
            Reading::Measured { value, shown } => {
                assert!((value - 0.5).abs() < 1e-9, "{value}");
                assert!(shown.contains("1 of 2"), "{shown}");
            }
            other => panic!("{other:?}"),
        }
    }

    /// A range that added nothing is unmeasurable on both node criteria, and says so
    /// separately for each — not with one shared sentence about an empty range.
    #[test]
    fn a_range_that_added_no_node_is_unmeasurable_rather_than_zero() {
        let c = Contribution {
            commits: contribution().commits,
            added: Vec::new(),
        };
        let rows = score(&ids(), &c);
        for id in ["landing", "questions"] {
            assert!(!row(rows.clone(), id).reading.is_measured(), "{id}");
        }
        assert!(row(rows, "register").reading.is_measured());
    }

    /// **The guard, as a property.** A row that read a number and showed nothing would be a
    /// claim with no way to check it.
    #[test]
    fn a_measured_row_always_carries_evidence() {
        let shapes = [
            contribution(),
            Contribution::default(),
            Contribution {
                commits: vec![commit("a1b2c3d4", "feat", false, true)],
                added: vec![node("a/b.yml", false, false)],
            },
            Contribution {
                commits: vec![commit("a1b2c3d4", "regen", true, false)],
                added: vec![node("a/b.yml", true, true)],
            },
        ];
        for c in shapes {
            for r in score(&ids(), &c) {
                if r.reading.is_measured() {
                    assert!(
                        !r.evidence.is_empty(),
                        "`{}` read {} and showed nothing",
                        r.criterion,
                        r.reading.shown()
                    );
                }
            }
        }
    }

    /// Evidence is capped, and the cap announces itself rather than truncating in silence.
    #[test]
    fn evidence_says_how_much_it_left_out() {
        let c = Contribution {
            added: (0..30)
                .map(|i| node(&format!("concept/n{i}.yml"), i % 2 == 0, false))
                .collect(),
            ..Contribution::default()
        };
        let r = row(score(&ids(), &c), "landing");
        assert_eq!(r.evidence.len(), EVIDENCE_LIMIT + 1);
        assert!(
            r.evidence.last().unwrap().contains("and 18 more"),
            "{:?}",
            r.evidence.last()
        );
    }

    /// A profile newer than the binary gets a row saying so. Silence would read as a pass.
    #[test]
    fn a_criterion_this_binary_does_not_implement_still_produces_a_row() {
        let rows = score(&["sourcing".to_string()], &contribution());
        assert_eq!(rows.len(), 1);
        match &rows[0].reading {
            Reading::Unmeasurable { why } => assert!(why.contains("does not implement"), "{why}"),
            other => panic!("{other:?}"),
        }
    }

    /// Nothing that was measured and rejected is quietly also a criterion.
    #[test]
    fn a_rejected_criterion_is_not_also_a_shipped_one() {
        for r in CONSIDERED_AND_REJECTED {
            assert!(
                Criterion::from_id(r.id).is_none(),
                "`{}` is recorded as rejected and is also implemented",
                r.id
            );
            assert!(!r.why.trim().is_empty(), "`{}` records no reason", r.id);
        }
        assert!(CONSIDERED_AND_REJECTED.len() >= 3);
    }

    #[test]
    fn every_criterion_round_trips_through_its_id() {
        for c in Criterion::ALL {
            assert_eq!(Criterion::from_id(c.id()), Some(*c));
            assert!(!c.gloss().trim().is_empty());
        }
        assert_eq!(Criterion::all_ids().len(), Criterion::ALL.len());
    }

    /// Four, and each is a question rather than a heading.
    #[test]
    fn the_judged_questions_are_questions() {
        assert_eq!(JUDGE_QUESTIONS.len(), 4);
        for q in JUDGE_QUESTIONS {
            assert!(q.trim_end().ends_with('?'), "{q}");
        }
    }
}
