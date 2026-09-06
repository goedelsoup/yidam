//! `yidam kuten` — write the declaration into `AGENTS.md`, and read the corpus against it.
//!
//! RFC-0028 §9 specifies both halves. `yidam kuten` writes the `AGENTS.md` REGEN block;
//! `yidam kuten check` reads the vendored declaration and the history and reports divergence.
//!
//! # Why the declaration needs a REGEN block at all
//!
//! The four surfaces this layer grows are CLI reports a person runs. What an agent reads at
//! session start is the derived repository's `AGENTS.md`, and it had no slot for what the
//! work is *for*. A declaration nothing in the loop reads is this epic's own diagnosed
//! failure aimed at its centrepiece — so the declaration lands where the reader already is.
//!
//! **Regenerated, never hand-copied.** A hand-copied declaration is one re-vendor away from
//! being silently wrong, which is exactly the vintage error A0 exists to warn against.
//!
//! # `check` authors nothing and refuses nothing
//!
//! It writes no file, drafts no commit, and exits zero however far a corpus has drifted.
//! Divergence is a question for a person. Anything that refuses arrives through the policy
//! layer, where it is visible as an override.

use anyhow::Result;
use clap::Subcommand;

use crate::kuten::{self, Report};
use crate::report::Format;

#[derive(Debug, Subcommand)]
pub enum KutenCommand {
    /// Report where this corpus's history diverges from the kuten it declared
    ///
    /// Read-only, and it exits zero. Divergence from a kuten is a question for a person, not
    /// a defect — `due`'s argument, one level out. A metric the repository's *vendored*
    /// prelude could not have produced is reported as vintage and never as divergence.
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// `yidam kuten`, with or without a subcommand.
///
/// No subcommand writes the `AGENTS.md` block, which is what puts `kuten` in the generator
/// list beside the other ten. The subcommand reads.
pub fn run(sub: Option<KutenCommand>) -> Result<()> {
    match sub {
        None => block(),
        Some(KutenCommand::Check { format }) => check(format),
    }
}

// ── the AGENTS.md block ───────────────────────────────────────────────────────

/// Render the block from a declaration, or from the absence of one.
///
/// Takes the two documents rather than a root so the rendering is testable without a
/// repository — and so the no-kuten arm, which is every repository today, is exercised by
/// the same function that renders the held one.
pub fn render_block(
    declaration: Option<&kuten::Declaration>,
    profile: Option<&kuten::Profile>,
) -> String {
    let (Some(declaration), Some(profile)) = (declaration, profile) else {
        return "_This repository holds no kuten._ That is a supported state: the loop runs on \
                the template's own defaults, and nothing here declares what the work is aimed \
                at. `yidam kuten check` reports it and exits zero."
            .to_string();
    };

    let mut out = format!(
        "**This corpus's practice is `{}`, at revision {}.** {}\n",
        profile.name, declaration.revision, profile.gloss
    );
    if declaration.revision != profile.revision {
        out.push_str(&format!(
            "\n> The vendored profile is at revision {}, and the decision record names {}. \
             Re-vendor, or record a superseding decision.\n",
            profile.revision, declaration.revision
        ));
    }
    out.push('\n');
    if let Some(phases) = &profile.phases {
        out.push_str(&format!(
            "- **Phases** — {}. Between {} of commits settle one.\n",
            phases.types.join(", "),
            share_band(phases.commit_share)
        ));
    }
    if let Some(classes) = &profile.classes {
        out.push_str(&format!(
            "- **Shape** — {} nodes per commit, and a median node of {:.0}–{:.0} lines.\n",
            classes.nodes_per_commit.describe(),
            classes.median_node_lines.low,
            classes.median_node_lines.high
        ));
    }
    if let Some(vocabulary) = &profile.vocabulary {
        out.push_str(&format!(
            "- **Vocabulary** — {} verbs, and {} of commits outside them.\n",
            vocabulary.verbs.len(),
            share_band(vocabulary.off_vocabulary_share)
        ));
    }
    if let Some(object) = &profile.object {
        // The direction, and the consequence a reader needs in the same breath. An
        // `authored` corpus is what every history-derived surface already assumes; a
        // `projected` one is the state that made those surfaces answer nothing, and saying
        // only the word would leave the reader to re-derive what it means (RFC-0028 §6).
        out.push_str(&format!(
            "- **Object** — {}, so its history is {}.\n",
            object.direction.describe(),
            match object.direction {
                kuten::Direction::Authored => "the record",
                kuten::Direction::Projected =>
                    "the project's and not the corpus's: `replay`, `--at`, `log --epistemic` \
                     and the residence clocks do not apply",
            }
        ));
    }
    if let Some(pressure) = &profile.question_pressure {
        out.push_str(&format!(
            "- **Questions** — this practice presses toward {} ones. It creates the pressure \
             and authors nothing.\n",
            pressure.kind.name()
        ));
    }
    if let Some(rubric) = &profile.rubric {
        // **The block, and not `check`.** The criteria are read by `yidam score <range>`,
        // which is not a corpus-wide report and has no place in one: `check` measures a
        // repository's whole history against declared bands, and a contribution score is
        // about a range somebody chose. What the block owes the reader is *which* criteria
        // the next session will be read against — before the session, where an agent meets
        // it — so the slot's reader is `block`.
        out.push_str(&format!(
            "- **Contribution** — a session's work is read against {}. `yidam score <range>` \
             reports one row each, with the evidence, and no overall number.\n",
            list(&criteria_glosses(&rubric.criteria))
        ));
    }
    out.push_str(
        "\nIt narrows the loop and may not widen the model, and it binds nobody: divergence \
         from it is a question for a person, not a defect. Ask `yidam kuten check`.",
    );
    out
}

/// Each declared criterion as `` `id` (what it reads) ``.
///
/// The gloss comes from [`crate::score::Criterion`] rather than from the profile: a corpus
/// declares *which* criteria it is read against, and what each one computes is the binary's
/// answer, not a per-corpus one. A criterion this binary does not implement is named as such
/// rather than dropped — the same reading `score` gives it.
fn criteria_glosses(criteria: &[String]) -> Vec<String> {
    criteria
        .iter()
        .map(|id| match crate::score::Criterion::from_id(id) {
            Some(c) => format!("`{id}` ({})", c.gloss()),
            None => format!("`{id}` (declared here and not implemented by this binary)"),
        })
        .collect()
}

/// `a`, `a and b`, `a, b and c` — an English list, because this renders into prose.
fn list(items: &[String]) -> String {
    match items {
        [] => "no criteria at all".to_string(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

fn share_band(b: kuten::Band) -> String {
    format!("{:.0}% and {:.0}%", b.low * 100.0, b.high * 100.0)
}

/// Write the `AGENTS.md` REGEN block. The generator `yidam regen` runs.
pub fn block() -> Result<()> {
    let root = crate::paths::repo_root()?;
    let declaration = kuten::read_declaration(&root)?;
    let profile = match &declaration {
        Some(d) => kuten::read_profile(&root, &d.name)?,
        None => None,
    };
    let content = render_block(declaration.as_ref(), profile.as_ref());
    crate::regen::emit(&content);
    crate::regen::update_file_regen(&root.join("AGENTS.md"), "yidam kuten", &content)
}

// ── the check ─────────────────────────────────────────────────────────────────

/// The payload, nested under one key.
///
/// One top-level field rather than eight, so the shared envelope's namespace does not grow a
/// `held` and a `conforming` that mean nothing to the other twenty reports.
#[derive(serde::Serialize)]
struct Payload<'a> {
    kuten: &'a Report,
}

pub(crate) fn render_check(r: &Report) -> String {
    if !r.held {
        return "No kuten declared.\n\nThis repository runs on the template's defaults and \
                declares nothing about what its work is aimed at. That is a supported state, \
                and it is not a finding."
            .to_string();
    }
    let name = r.name.as_deref().unwrap_or("?");
    if let Some(why) = &r.unresolved {
        return format!("Kuten `{name}` is declared and cannot be read.\n\n{why}");
    }

    let mut out = format!(
        "Kuten `{name}`, revision {}.\n",
        r.declared_revision.unwrap_or_default()
    );
    if r.revision_skew {
        out.push_str(&format!(
            "\n⚠ The vendored profile is at revision {}. A comparison across revisions is \
             annotated rather than made: re-vendor, or record a superseding decision.\n",
            r.vendored_revision.unwrap_or_default()
        ));
    }
    out.push_str(&format!(
        "\n{} commit(s), {} node(s) measured.\n\n",
        r.measurement.commits, r.measurement.nodes
    ));
    for f in &r.findings {
        out.push_str(&format!(
            "  [{}] {:<22} declared {:<12} measured {}\n",
            f.verdict.tag(),
            f.metric,
            f.declared,
            f.measured
        ));
    }
    let questions: Vec<&String> = r
        .findings
        .iter()
        .filter_map(|f| f.question.as_ref())
        .collect();
    if questions.is_empty() {
        out.push_str("\nNothing diverges.");
        return out;
    }
    out.push_str("\nQuestions for a person — none of these is a defect:\n");
    for q in questions {
        out.push_str(&format!("  · {q}\n"));
    }
    out.push_str(
        "\nA kuten binds nobody. Answer the question, revise the practice, or record a \
         superseding decision.",
    );
    out
}

/// **Exits zero, always.** Divergence is not a defect, and a report that gated would make it
/// one — which is the whole of the argument `due` already makes about being owed.
pub fn check(format: Format) -> Result<()> {
    let root = crate::paths::repo_root()?;
    let report = kuten::check(&root)?;
    if format.is_json() {
        crate::report::emit(&root, Payload { kuten: &report })?;
    } else {
        println!("{}", render_check(&report));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kuten::{Declaration, Measurement, Profile, Verdict, Vintage};

    fn profile() -> Profile {
        Profile::parse(
            "kuten: inquiry\nrevision: 1\ngloss: questions opened, and settled\n\
             phases:\n  types: [Investigation, Extraction]\n  commit_share: {low: 0.13, high: 0.26}\n\
             vocabulary:\n  verbs: [establish, open]\n  off_vocabulary_share: {low: 0.0, high: 0.0}\n\
             classes:\n  nodes_per_commit: {low: 0.50, high: 1.11}\n  median_node_lines: {low: 35, high: 62}\n\
             object:\n  direction: authored\n\
             question_pressure:\n  kind: epistemic\n\
             rubric:\n  criteria: [register, landing, questions]\n",
        )
        .unwrap()
    }

    fn declaration(revision: u32) -> Declaration {
        Declaration::parse(&format!("kuten: inquiry\nrevision: {revision}\n")).unwrap()
    }

    #[test]
    fn the_block_names_the_practice_and_the_revision() {
        let text = render_block(Some(&declaration(1)), Some(&profile()));
        assert!(text.contains("`inquiry`"), "{text}");
        assert!(text.contains("revision 1"), "{text}");
        assert!(text.contains("Investigation"), "{text}");
        assert!(text.contains("binds nobody"), "{text}");
    }

    /// The two slots A3 populates reach the document the agent actually reads. A declaration
    /// nothing in the loop reads is this epic's own diagnosed failure aimed at its centre.
    #[test]
    fn the_block_names_the_direction_and_the_pressure() {
        let text = render_block(Some(&declaration(1)), Some(&profile()));
        assert!(text.contains("authored in git"), "{text}");
        assert!(text.contains("epistemic"), "{text}");

        let projected =
            Profile::parse("kuten: mirror\nrevision: 1\nobject:\n  direction: projected\n")
                .unwrap();
        let text = render_block(Some(&declaration(1)), Some(&projected));
        assert!(text.contains("projected from its object"), "{text}");
        assert!(
            text.contains("do not apply"),
            "a projected corpus's reader is told which surfaces stop answering: {text}"
        );
    }

    /// **The `rubric` slot's only reader.** The criteria have to reach the document an agent
    /// meets at session start, and they have to arrive glossed: a bare list of three words is
    /// a declaration nobody can act on, which is a surface with no consumer wearing one.
    #[test]
    fn the_block_names_each_criterion_and_what_it_reads() {
        let text = render_block(Some(&declaration(1)), Some(&profile()));
        for c in crate::score::Criterion::ALL {
            assert!(text.contains(c.id()), "`{}` missing from {text}", c.id());
            assert!(
                text.contains(c.gloss()),
                "`{}` arrives unglossed: {text}",
                c.id()
            );
        }
        assert!(text.contains("yidam score"), "{text}");
        assert!(
            text.contains("no overall number"),
            "the block must not read as a score somebody passes: {text}"
        );
    }

    /// A criterion the profile declares and this binary does not implement is named as such.
    /// Dropping it would make a newer profile's declaration silently invisible.
    #[test]
    fn a_criterion_the_binary_does_not_implement_is_named_in_the_block() {
        let newer =
            Profile::parse("kuten: inquiry\nrevision: 1\nrubric:\n  criteria: [sourcing]\n")
                .unwrap();
        let text = render_block(Some(&declaration(1)), Some(&newer));
        assert!(text.contains("`sourcing`"), "{text}");
        assert!(text.contains("not implemented by this binary"), "{text}");
    }

    /// The arm every repository is in today, and it must read as a state rather than a fault.
    #[test]
    fn the_block_reports_no_kuten_as_a_supported_state() {
        let text = render_block(None, None);
        assert!(text.contains("supported state"), "{text}");
    }

    /// A re-vendor that moved the profile under a decision record is the confound A0's own
    /// correction was about. The block says so where the agent reads it.
    #[test]
    fn the_block_annotates_a_revision_it_cannot_compare_across() {
        let text = render_block(Some(&declaration(2)), Some(&profile()));
        assert!(text.contains("superseding decision"), "{text}");
    }

    #[test]
    fn the_check_prints_a_question_and_never_a_defect() {
        let m = Measurement {
            commits: 200,
            phase_commits: 0,
            off_vocabulary_commits: 0,
            nodes: 160,
            median_node_lines: Some(48.0),
            open_questions: 12,
        };
        let vintage = Vintage::read("| `phase` | settled |\nThis list is closed");
        let findings = crate::kuten::compare(&profile(), &m, &vintage);
        let report = Report {
            held: true,
            name: Some("inquiry".into()),
            declared_revision: Some(1),
            vendored_revision: Some(1),
            revision_skew: false,
            unresolved: None,
            vintage,
            measurement: m,
            findings,
            conforming: false,
        };
        let text = render_check(&report);
        assert!(text.contains("Questions for a person"), "{text}");
        assert!(text.contains("binds nobody"), "{text}");
        assert!(
            !text.to_lowercase().contains("fail"),
            "divergence must not read as a failure: {text}"
        );
    }

    #[test]
    fn an_unheld_check_says_so_without_findings() {
        let text = render_check(&Report::unheld(Measurement::default(), Vintage::absent()));
        assert!(text.contains("No kuten declared"), "{text}");
        assert!(text.contains("supported state"), "{text}");
    }

    /// Every verdict tag is distinct, or two states print the same and a reader cannot tell
    /// a vintage artifact from a divergence — the one distinction this command exists for.
    #[test]
    fn the_verdict_tags_are_distinct() {
        let tags = [
            Verdict::Conforming.tag(),
            Verdict::Divergent.tag(),
            Verdict::Vintage.tag(),
            Verdict::Unmeasurable.tag(),
        ];
        let mut sorted = tags.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), tags.len());
    }
}
