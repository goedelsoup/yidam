//! `yidam score <range>` — read a contribution against the criteria this corpus declared.
//!
//! The thin half. [`crate::score`] holds the model and the scoring, which is a pure function
//! of a [`Contribution`]; everything here reads a repository to build one, refuses where a
//! score would not be comparable, and renders.
//!
//! # Where the criteria come from
//!
//! From the vendored kuten's `rubric` slot when the repository holds one, and from
//! [`crate::score::Criterion::ALL`] when it does not. **The second is the default arm, not a
//! fallback**: nothing retrofits a kuten into an existing corpus, `migrate` has no path for
//! it, and 0 of 18 derived corpora hold one. The neutral arm runs the *same* criteria and
//! says the selection is the template's rather than this corpus's — which is the whole
//! difference, and the reason it is said out loud rather than left to be inferred from a
//! missing line.
//!
//! # It writes nothing, and it exits zero however it reads
//!
//! `cmd/kuten.rs`'s rule, one level in: *"Exits zero, always. Divergence is not a defect, and
//! a report that gated would make it one."* A contribution that read 0.00 on every criterion
//! exits zero, because a kuten binds nobody and a score that gated would be a gate decided by
//! one.
//!
//! **The one non-zero exit is a refusal to answer, which is a different thing from a bad
//! reading.** A range that spans a kuten revision has no single set of criteria to be read
//! against, and the two halves are not comparable. The harness states the precedent for
//! exactly this shape — it *"rejects cross-version diffs with an explicit error rather than
//! silently producing misleading output"* — and #662 is what happens to a refusal that exits
//! quietly: the harness's own instance of it has been inert for a protocol version and
//! nothing noticed, because nothing tested the version-to-version case. So this one bails,
//! and an integration test drives a range across a revision change and asserts that it does.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Result};

use crate::cmd::lint::checks::class_of;
use crate::cmd::query::at::{self, Blobs};
use crate::cmd::query::Graph;
use crate::kuten;
use crate::report::Format;
use crate::score::{self, CommitFact, Contribution, NodeFact, Row};

/// Why the criteria are what they are — a closed set, so a consumer branches on it rather
/// than on prose.
///
/// `held` alone collapsed three different situations into one sentence, and two of them were
/// false about the repository (#695). Only one is derivable from the rest of the payload:
/// telling *the range predates the adoption* from *this corpus declares none* needs the
/// working tree's record, which nothing else here reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Source {
    /// The range's tip declares a kuten and its profile is vendored. The criteria are the
    /// corpus's own.
    Kuten,
    /// Neither the range's tip nor the working tree declares one. The supported state, and
    /// what every derived corpus was in until the layer shipped.
    NoKuten,
    /// The tip declares none and the working tree does. The criteria are correctly the
    /// template's — the range is older than the decision — and the repository holds one.
    ///
    /// The dominant case in the field: both repositories that adopted did so in the last two
    /// commits of 1,300-commit histories, so nearly every range a maintainer would score is
    /// this one.
    PredatesAdoption,
    /// The tip declares one and no profile is vendored for it. The criteria fall back to the
    /// template's, which `check` and `doctor` both report and this used to do silently.
    UnreadableProfile,
}

/// The whole answer `yidam score` gives.
#[derive(Debug, serde::Serialize)]
pub struct Report {
    /// The range as the caller wrote it.
    pub range: String,
    /// The commit the range starts after.
    pub base: String,
    /// The commit it ends at.
    pub tip: String,
    /// Whether the criteria are this corpus's or the template's.
    ///
    /// `true` exactly when [`Self::source`] is [`Source::Kuten`]. Kept beside it because it is
    /// the question most consumers are asking, and because it is in the frozen required set.
    pub held: bool,
    /// Which of the four situations the range is in.
    pub source: Source,
    /// The kuten the criteria came from, where one was held.
    pub kuten: Option<String>,
    /// The revision of the profile the criteria came from.
    ///
    /// **Null whenever `held` is false**, including when the range's tip names a revision this
    /// repository cannot read. The record used to carry `held: false` beside `revision: 1`,
    /// which says two contradictory things about the same range; what the tip declared now
    /// lives in [`Self::declared`], where it does not have to agree with a criteria source
    /// that does not exist.
    pub revision: Option<u32>,
    /// What the decision record at the range's **tip** names, whatever the criteria became.
    ///
    /// Present in exactly two states: alongside the criteria in [`Source::Kuten`], and alone
    /// in [`Source::UnreadableProfile`], which is the disagreement stated rather than resolved.
    pub declared: Option<Declared>,
    /// The kuten the **working tree** declares, where that is not what the range was scored
    /// against.
    ///
    /// The one fact in this report that is about now rather than about the range, and it is
    /// here for the one distinction the range cannot make on its own: a tip that predates an
    /// adoption and a corpus that has adopted nothing are the same commit-shaped absence.
    pub holds_now: Option<String>,
    /// Whether the vendored profile the criteria came from is at another revision.
    ///
    /// `kuten check`'s field, for its reason: a comparison across revisions is annotated
    /// rather than silently made. Here it says the criteria were read from a profile the
    /// range's own record does not name.
    pub revision_skew: bool,
    /// Authored, non-merge commits in the range.
    pub commits: usize,
    /// Nodes the range added that the tip still holds.
    pub added_nodes: usize,
    /// One per declared criterion, in declaration order.
    pub rows: Vec<Row>,
    /// The judged questions, when `--brief` asked for them. Scored by nothing.
    pub questions: Vec<&'static str>,
}

/// A decision record's contents, as the range's tip carries them.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Declared {
    pub name: String,
    pub revision: u32,
}

/// The payload, nested under one key — `kuten check`'s shape, for its reason.
#[derive(serde::Serialize)]
struct Payload<'a> {
    score: &'a Report,
}

// ── the refusal ───────────────────────────────────────────────────────────────

/// The kuten a commit's tree declares, as `(name, revision)`.
///
/// **Readable today, and that is why the refusal can be tested.** The decision record is a
/// committed file, so `git show <commit>:<path>` answers about any revision in range without
/// touching the working tree — the same property `at.rs` holds for the corpus. A missing
/// record is `None`, which is a real state and not an error: it is the state of every derived
/// corpus measured.
///
/// The commit is a resolved sha by the time it reaches here, so there is no argument for a
/// caller to smuggle an option through.
fn declared_at(root: &Path, commit: &str) -> Option<(String, u32)> {
    let out = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{commit}:{}", kuten::DECISION_PATH)])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let d = kuten::Declaration::parse(&text).ok()?;
    Some((d.name, d.revision))
}

fn describe(declared: &Option<(String, u32)>) -> String {
    match declared {
        Some((name, revision)) => format!("`{name}` at revision {revision}"),
        None => "no kuten at all".to_string(),
    }
}

/// Refuse a range whose two ends declare different practices. **Before any comparison.**
///
/// RFC-0028 §2: *"`score` refuses a range spanning one."* A score is a reading against
/// declared criteria, and two revisions are two sets of criteria — a number spanning them is
/// not one number about one thing, and averaging them silently is the failure the harness's
/// cross-version rule was written against.
fn refuse_if_spanning(root: &Path, base: &at::Revision, tip: &at::Revision) -> Result<()> {
    let (before, after) = (
        declared_at(root, &base.commit),
        declared_at(root, &tip.commit),
    );
    if before == after {
        return Ok(());
    }
    bail!(
        "this range spans a change of kuten, so there is no one set of criteria to score it \
         against.\n\n  at {}  {}\n  at {}  {}\n\nA score is comparable only within a revision. \
         Score the two halves separately, either side of the commit that changed {}.",
        crate::cmd::query::short(&base.commit),
        describe(&before),
        crate::cmd::query::short(&tip.commit),
        describe(&after),
        kuten::DECISION_PATH
    );
}

// ── reading a repository ──────────────────────────────────────────────────────

/// What the range did, measured.
///
/// The commit half goes through `lint --commits`' own reader and its own merge predicate, so
/// this and the vocabulary check cannot disagree about what a verb is or which merge subjects
/// git wrote. The node half reconstructs the corpus at both ends through `at.rs`, which reads
/// git objects and never the working tree — so an uncommitted edit can neither add a node to
/// a contribution nor take one away.
fn measure(
    root: &Path,
    range: &str,
    base: &at::Revision,
    tip: &at::Revision,
) -> Result<Contribution> {
    let subjects = crate::cmd::lint::commits::read_subjects(root, Some(range));
    let commits: Vec<CommitFact> = subjects
        .iter()
        .filter(|s| !crate::cmd::lint::commits::is_merge(&s.text, s.parents))
        .map(|s| CommitFact {
            short: crate::cmd::query::short(&s.hash).to_string(),
            verb: s.verb.clone(),
            recognized: yidam_core::git::is_recognized_verb(&s.verb),
            // The canonical classifier, not a local copy of the operational list. It is
            // meaningful only over the recognized subset, and `score` uses it only there.
            epistemic: yidam_core::git::classify_commit("", &s.text).kind
                == yidam_core::git::CommitKind::Epistemic,
        })
        .collect();

    let mut blobs = Blobs::default();
    let before = Graph::at_with(root, &base.commit, &mut blobs)?;
    let held: HashSet<&str> = before.nodes.iter().map(|n| n.rel.as_str()).collect();
    let after = Graph::at_with(root, &tip.commit, &mut blobs)?;

    // `orphan-in`'s own answer, run over the tip's whole graph: a node's in-degree is a fact
    // about the corpus that holds it, not about the diff that added it, and the class
    // exemption is only computable from the whole ontology. A second in-degree here would be
    // a second answer to a question the gate already settles.
    let orphaned: HashSet<String> =
        crate::cmd::lint::checks::orphan_in(&after.nodes, &after.classes)
            .violations
            .into_iter()
            .map(|v| v.node)
            .collect();

    // From the ontology at the tip rather than a second walk of the working tree — `pack`'s
    // reason, and here also a correctness one: a range ending at a past commit must be read
    // against that commit's classes.
    let fields = crate::claims::ClaimFields::from_declarations(after.classes.iter().map(|c| {
        let claim_fields = c
            .properties
            .iter()
            .filter(|p| p.r#type == crate::claims::CLAIM_PROPERTY_TYPE)
            .map(|p| p.name.clone())
            .collect();
        (c.name.clone(), claim_fields)
    }));

    let added = after
        .nodes
        .iter()
        .filter(|n| !held.contains(n.rel.as_str()))
        .map(|n| NodeFact {
            rel: n.rel.clone(),
            landed: !orphaned.contains(&n.rel),
            // The open-question predicate, and not a fourth spelling of it.
            open_question: crate::claims::is_open_question(
                n.inst.label.as_deref().unwrap_or_default(),
                &n.text,
                fields.for_class(&class_of(n)),
            ),
        })
        .collect();

    Ok(Contribution { commits, added })
}

// ── rendering ─────────────────────────────────────────────────────────────────

/// Who chose the criteria, in the words the difference actually turns on.
///
/// Four states, four sentences. The criteria selection was already right in every one of
/// them; what was wrong was a single sentence claiming, in the present tense about the
/// repository, something that was only ever true of the range (#695).
fn source_line(r: &Report) -> String {
    match r.source {
        Source::Kuten => match (&r.kuten, r.revision) {
            (Some(name), Some(revision)) => format!(
                "Criteria: the `{name}` kuten's rubric, at revision {revision} — this corpus's \
                 own selection."
            ),
            // Unreachable by construction, and stated rather than unwrapped: `Kuten` is set
            // from the same profile that fills both fields.
            _ => "Criteria: this corpus's own kuten.".to_string(),
        },
        Source::NoKuten => "Criteria: the template's own. This repository holds no kuten, so the \
                            selection below is not this corpus's — it is what yidam would ask of \
                            any corpus."
            .to_string(),
        Source::PredatesAdoption => format!(
            "Criteria: the template's own. This range predates the {}kuten this repository now \
             holds, so the criteria are the ones that applied then.",
            match &r.holds_now {
                Some(name) => format!("`{name}` "),
                None => String::new(),
            }
        ),
        Source::UnreadableProfile => format!(
            "Criteria: the template's own. This range declares {}, and no profile is vendored \
             for it — so the selection below is not this corpus's.",
            match &r.declared {
                Some(d) => format!("`{}` at revision {}", d.name, d.revision),
                None => "a kuten".to_string(),
            }
        ),
    }
}

pub(crate) fn render(r: &Report) -> String {
    let mut out = format!(
        "Score for `{}` — {} commit(s), {} node(s) added and still standing.\n\n{}\n\n",
        r.range,
        r.commits,
        r.added_nodes,
        source_line(r)
    );
    // The same sentence `kuten check` and `doctor` give for the same state. Three surfaces,
    // one answer: this was the only one of them that fell back in silence (#695).
    if r.source == Source::UnreadableProfile {
        let name = r.declared.as_ref().map(|d| d.name.as_str()).unwrap_or("it");
        let _ = write!(
            out,
            "⚠ `{name}` is declared and no profile is vendored for it. Re-vendor the prelude, \
             or record a superseding decision.\n\n"
        );
    }
    if r.revision_skew {
        out.push_str(
            "⚠ The vendored profile the criteria came from is at another revision than the \
             record this range names. Re-vendor, or record a superseding decision.\n\n",
        );
    }
    for row in &r.rows {
        let _ = writeln!(out, "  {:<12} {}", row.criterion, row.reading.shown());
        if let crate::score::Reading::Unmeasurable { why } = &row.reading {
            let _ = writeln!(out, "      — {why}");
        }
        if let Some(note) = &row.note {
            let _ = writeln!(out, "      — {note}");
        }
        for line in &row.evidence {
            let _ = writeln!(out, "      · {line}");
        }
        out.push('\n');
    }
    if !r.questions.is_empty() {
        out.push_str("For a person to answer. Nothing below is scored:\n");
        for q in &r.questions {
            let _ = writeln!(out, "  · {q}");
        }
        out.push('\n');
    }
    out.push_str(
        "No row here is a verdict and there is no overall number: a single score over a range \
         of commits names somebody's session. A kuten binds nobody, and this exits zero \
         however it reads.",
    );
    out
}

/// **Writes nothing, and exits zero however it reads.** See the module header for the one
/// case that does not answer at all.
pub fn score(range: &str, format: Format, brief: bool) -> Result<()> {
    let root = crate::paths::repo_root()?;
    let (before, after) = crate::cmd::diff::parse_range(range);
    let base = at::resolve(&root, &before)?;
    let tip = at::resolve(&root, &after)?;
    refuse_if_spanning(&root, &base, &tip)?;

    // **The range's own declaration, read at its tip** — not the working tree's. A range
    // ending before a `decide:` commit is scored against what the corpus had then, which is
    // the whole point of the refusal above; taking the criteria from whatever is checked out
    // now would make the refusal a formality it could walk straight past.
    //
    // The *profile* still comes from the working tree's vendored directory, because nothing
    // reconstructs one at a past ref: `at.rs::tree()` is pathspec'd to `.yidam/corpus`, so
    // `.yidam/.vendor` is not in any reconstruction. Where the two disagree the report says
    // so rather than quietly using either.
    let declaration = declared_at(&root, &tip.commit);
    let profile = match &declaration {
        Some((name, _)) => kuten::read_profile(&root, name)?,
        None => None,
    };
    let criteria = kuten::Profile::criteria(profile.as_ref());

    // Read only to name the state, never to choose criteria: a range is scored against what
    // its own tip declared, and the refusal above is what makes that meaningful. Taking the
    // criteria from here would be the walk-past the refusal exists to prevent.
    let holds_now = kuten::read_declaration(&root)?.map(|d| d.name);
    let source = match (&declaration, &profile) {
        (Some(_), Some(_)) => Source::Kuten,
        (Some(_), None) => Source::UnreadableProfile,
        (None, _) if holds_now.is_some() => Source::PredatesAdoption,
        (None, _) => Source::NoKuten,
    };

    let git_range = format!("{before}..{after}");
    let contribution = measure(&root, &git_range, &base, &tip)?;
    let report = Report {
        range: range.to_string(),
        base: base.commit.clone(),
        tip: tip.commit.clone(),
        held: source == Source::Kuten,
        source,
        kuten: profile.as_ref().map(|p| p.name.clone()),
        // The criteria's revision, and null where there are no corpus criteria — see the
        // field. `revision_skew` below is what says the record named a different one.
        revision: declaration
            .as_ref()
            .map(|(_, r)| *r)
            .filter(|_| profile.is_some()),
        declared: declaration.as_ref().map(|(name, revision)| Declared {
            name: name.clone(),
            revision: *revision,
        }),
        holds_now: holds_now.filter(|_| source != Source::Kuten),
        revision_skew: match (&declaration, &profile) {
            (Some((_, declared)), Some(p)) => *declared != p.revision,
            _ => false,
        },
        commits: contribution.commits.len(),
        added_nodes: contribution.added.len(),
        rows: score::score(&criteria, &contribution),
        questions: if brief {
            score::JUDGE_QUESTIONS.to_vec()
        } else {
            Vec::new()
        },
    };

    if format.is_json() {
        crate::report::emit(&root, Payload { score: &report })?;
    } else {
        println!("{}", render(&report));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::Criterion;

    fn report(kuten: Option<(&str, u32)>, rows: Vec<Row>) -> Report {
        let source = match kuten {
            Some(_) => Source::Kuten,
            None => Source::NoKuten,
        };
        in_state(source, kuten, None, rows)
    }

    /// A report in any of the four states, built the way `score` builds one.
    ///
    /// `declared` is what the range's tip named; it is the criteria's source in the held arm
    /// and the unreadable profile's name in the arm that has no criteria of its own.
    fn in_state(
        source: Source,
        declared: Option<(&str, u32)>,
        holds_now: Option<&str>,
        rows: Vec<Row>,
    ) -> Report {
        let held = source == Source::Kuten;
        Report {
            range: "HEAD~5..HEAD".to_string(),
            base: "a".repeat(40),
            tip: "b".repeat(40),
            held,
            source,
            kuten: declared.filter(|_| held).map(|(n, _)| n.to_string()),
            revision: declared.filter(|_| held).map(|(_, r)| r),
            declared: declared.map(|(name, revision)| Declared {
                name: name.to_string(),
                revision,
            }),
            holds_now: holds_now.map(str::to_string),
            revision_skew: false,
            commits: 5,
            added_nodes: 2,
            rows,
            questions: Vec::new(),
        }
    }

    fn rows() -> Vec<Row> {
        score::score(
            &Criterion::all_ids(),
            &Contribution {
                commits: vec![CommitFact {
                    short: "a1b2c3d4".into(),
                    verb: "establish".into(),
                    recognized: true,
                    epistemic: true,
                }],
                added: vec![NodeFact {
                    rel: "concept/tailwater.yml".into(),
                    landed: true,
                    open_question: false,
                }],
            },
        )
    }

    /// **The arm every repository is in.** It must read as a state rather than as an absence,
    /// and it must say whose selection the criteria are.
    #[test]
    fn the_no_kuten_arm_names_whose_criteria_these_are() {
        let text = render(&report(None, rows()));
        assert!(text.contains("holds no kuten"), "{text}");
        assert!(text.contains("not this corpus's"), "{text}");
        for c in Criterion::ALL {
            assert!(text.contains(c.id()), "`{}` missing from {text}", c.id());
        }
    }

    /// The held arm names the practice and its revision, and runs the same criteria.
    #[test]
    fn the_held_arm_names_the_practice_and_the_revision() {
        let text = render(&report(Some(("inquiry", 1)), rows()));
        assert!(text.contains("`inquiry`"), "{text}");
        assert!(text.contains("revision 1"), "{text}");
        for c in Criterion::ALL {
            assert!(text.contains(c.id()), "`{}` missing from {text}", c.id());
        }
    }

    /// **The case the field is in.** Both repositories that adopted did so in the last two
    /// commits of 1,300-commit histories, so nearly every range a maintainer scores predates
    /// the adoption — and every one of them used to be told the repository holds no kuten.
    #[test]
    fn a_range_predating_the_adoption_does_not_say_the_repository_holds_none() {
        let text = render(&in_state(
            Source::PredatesAdoption,
            None,
            Some("inquiry"),
            rows(),
        ));
        assert!(
            !text.contains("holds no kuten"),
            "the repository holds one — {text}"
        );
        assert!(text.contains("predates the `inquiry` kuten"), "{text}");
        // The criteria selection was never the defect: the template's are correct here.
        assert!(text.contains("the template's own"), "{text}");
    }

    /// Three surfaces, one answer. `check` says the profile cannot be read and `doctor` warns;
    /// this fell back to the template's criteria in silence.
    #[test]
    fn an_unreadable_profile_warns_the_way_check_and_doctor_do() {
        let text = render(&in_state(
            Source::UnreadableProfile,
            Some(("inquiry", 1)),
            None,
            rows(),
        ));
        assert!(
            text.contains("`inquiry` is declared and no profile is vendored for it"),
            "the wording `doctor` uses — {text}"
        );
        assert!(text.contains("⚠"), "it is a warning, not a remark — {text}");
        assert!(!text.contains("holds no kuten"), "{text}");
    }

    /// The record may not say two contradictory things about one range. `held: false` beside
    /// `revision: 1` was the shipped shape, and it validated against the report schema.
    #[test]
    fn a_report_that_holds_nothing_names_no_criteria_revision() {
        for r in [
            in_state(Source::NoKuten, None, None, rows()),
            in_state(Source::PredatesAdoption, None, Some("inquiry"), rows()),
            in_state(
                Source::UnreadableProfile,
                Some(("inquiry", 1)),
                None,
                rows(),
            ),
            in_state(Source::Kuten, Some(("inquiry", 1)), None, rows()),
        ] {
            let json = serde_json::to_value(&r).expect("serializes");
            assert_eq!(
                r.held,
                !json["revision"].is_null(),
                "held and revision disagree in {:?}: {json}",
                r.source
            );
            assert_eq!(
                r.held,
                !json["kuten"].is_null(),
                "held and kuten disagree in {:?}: {json}",
                r.source
            );
            assert_eq!(r.held, r.source == Source::Kuten, "{:?}", r.source);
        }
    }

    /// What the tip declared survives the criteria not coming from it — otherwise the
    /// unreadable-profile arm reports a fallback and never says what it fell back *from*.
    #[test]
    fn the_declaration_is_reported_even_where_it_could_not_be_read() {
        let r = in_state(
            Source::UnreadableProfile,
            Some(("inquiry", 1)),
            None,
            rows(),
        );
        let json = serde_json::to_value(&r).expect("serializes");
        assert_eq!(json["declared"]["name"], "inquiry");
        assert_eq!(json["declared"]["revision"], 1);
        assert_eq!(json["source"], "unreadable-profile");
    }

    /// No verdict, in the report and in the words. A score that read as a pass or a failure
    /// would be a gate with a softer name.
    #[test]
    fn the_report_carries_no_verdict_and_reads_as_none() {
        let text = render(&report(Some(("inquiry", 1)), rows()));
        assert!(text.contains("no overall number"), "{text}");
        for word in ["fail", "pass", "conform"] {
            assert!(
                !text.to_lowercase().contains(word),
                "a reading must not read as a verdict (`{word}`): {text}"
            );
        }
    }

    /// The judged questions appear only when asked for, and are marked as unscored.
    #[test]
    fn the_judged_questions_are_opt_in_and_scored_by_nothing() {
        let quiet = render(&report(None, rows()));
        assert!(!quiet.contains("Nothing below is scored"), "{quiet}");

        let mut asked = report(None, rows());
        asked.questions = score::JUDGE_QUESTIONS.to_vec();
        let text = render(&asked);
        assert!(text.contains("Nothing below is scored"), "{text}");
        for q in score::JUDGE_QUESTIONS {
            assert!(text.contains(q), "{q}");
        }
    }

    /// Both halves of a refusal are named, and so is what to do about it.
    #[test]
    fn a_spanning_range_is_described_from_both_ends() {
        assert_eq!(
            describe(&Some(("inquiry".into(), 2))),
            "`inquiry` at revision 2"
        );
        assert_eq!(describe(&None), "no kuten at all");
    }
}
