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
    pub held: bool,
    /// The kuten the criteria came from, where one was held.
    pub kuten: Option<String>,
    /// The revision the decision record names **at the tip of the range**.
    pub revision: Option<u32>,
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
fn source_line(r: &Report) -> String {
    match (&r.kuten, r.revision) {
        (Some(name), Some(revision)) => format!(
            "Criteria: the `{name}` kuten's rubric, at revision {revision} — this corpus's own \
             selection."
        ),
        _ => "Criteria: the template's own. This repository holds no kuten, so the selection \
              below is not this corpus's — it is what yidam would ask of any corpus."
            .to_string(),
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

    let git_range = format!("{before}..{after}");
    let contribution = measure(&root, &git_range, &base, &tip)?;
    let report = Report {
        range: range.to_string(),
        base: base.commit.clone(),
        tip: tip.commit.clone(),
        held: profile.is_some(),
        kuten: profile.as_ref().map(|p| p.name.clone()),
        revision: declaration.as_ref().map(|(_, r)| *r),
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
        Report {
            range: "HEAD~5..HEAD".to_string(),
            base: "a".repeat(40),
            tip: "b".repeat(40),
            held: kuten.is_some(),
            kuten: kuten.map(|(n, _)| n.to_string()),
            revision: kuten.map(|(_, r)| r),
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
