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
use std::fmt::Write as _;

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
    /// Adopt a vendored kuten, writing `.yidam/decisions/kuten.yml`
    ///
    /// The retrofit path for a corpus that already exists. The bootstrap writes this record
    /// at genesis and nothing else did, so a repository that re-vendored a prelude carrying
    /// the layer held the profile and could not declare it.
    Adopt {
        /// The profile to adopt, by its directory name under the vendored `kuten/`
        name: String,
    },
}

/// `yidam kuten`, with or without a subcommand.
///
/// No subcommand writes the `AGENTS.md` block, which is what puts `kuten` in the generator
/// list beside the other ten. `check` reads; `adopt` writes the decision record.
pub fn run(sub: Option<KutenCommand>) -> Result<()> {
    match sub {
        None => block(),
        Some(KutenCommand::Check { format }) => check(format),
        Some(KutenCommand::Adopt { name }) => adopt(&name),
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

    // A colon, not a full stop. Every profile's `gloss:` is a sentence fragment in the
    // profile's own words and opens lower-case — the shipped one is "the corpus grows through
    // sustained inquiry — questions opened, and settled" — so a full stop in front of it
    // renders as a typo in the one document an agent reads at session start.
    let mut out = format!(
        "**This corpus's practice is `{}`, at revision {}:** {}\n",
        profile.name,
        declaration.revision,
        stopped(&profile.gloss)
    );
    if declaration.revision != profile.revision {
        let _ = write!(
            out,
            "\n> The vendored profile is at revision {}, and the decision record names {}. \
             Re-vendor, or record a superseding decision.\n",
            profile.revision, declaration.revision
        );
    }
    out.push('\n');
    if let Some(phases) = &profile.phases {
        let _ = writeln!(
            out,
            "- **Phases** — {}. Between {} of commits settle one.",
            phases.types.join(", "),
            share_band(phases.commit_share)
        );
    }
    // No **Shape** line: the `classes` bands were retired at revision 2 (#692), because both
    // measured how old a repository is. An agent reading this at session start was being told
    // its corpus's stage as though it were its practice.
    if let Some(vocabulary) = &profile.vocabulary {
        let _ = writeln!(
            out,
            "- **Vocabulary** — {} verbs, and between {} of commits outside them.",
            vocabulary.verbs.len(),
            share_band(vocabulary.off_vocabulary_share)
        );
    }
    if let Some(object) = &profile.object {
        // The direction, and the consequence a reader needs in the same breath. An
        // `authored` corpus is what every history-derived surface already assumes; a
        // `projected` one is the state that made those surfaces answer nothing, and saying
        // only the word would leave the reader to re-derive what it means (RFC-0028 §6).
        let _ = writeln!(
            out,
            "- **Object** — {}, so its history is {}.",
            object.direction.describe(),
            match object.direction {
                kuten::Direction::Authored => "the record",
                kuten::Direction::Projected =>
                    "the project's and not the corpus's: `replay`, `--at`, `log --epistemic` \
                     and the residence clocks do not apply",
            }
        );
    }
    if let Some(pressure) = &profile.question_pressure {
        let _ = writeln!(
            out,
            "- **Questions** — this practice presses toward {} ones. It creates the pressure \
             and authors nothing.",
            pressure.kind.name()
        );
    }
    if let Some(rubric) = &profile.rubric {
        // **The block, and not `check`.** The criteria are read by `yidam score <range>`,
        // which is not a corpus-wide report and has no place in one: `check` measures a
        // repository's whole history against declared bands, and a contribution score is
        // about a range somebody chose. What the block owes the reader is *which* criteria
        // the next session will be read against — before the session, where an agent meets
        // it — so the slot's reader is `block`.
        let _ = writeln!(
            out,
            "- **Contribution** — a session's work is read against {}. `yidam score <range>` \
             reports one row each, with the evidence, and no overall number.",
            list(&criteria_glosses(&rubric.criteria))
        );
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

/// A profile's `gloss:` with a terminal stop, added only if it wrote none.
///
/// One answer, because both surfaces that quote a gloss — the `AGENTS.md` block and the
/// decision record `adopt` writes — join it to a sentence of their own, and a gloss is
/// written as a fragment. Two copies of "add a period unless there is one" is how the two
/// documents come to punctuate the same sentence differently.
fn stopped(gloss: &str) -> String {
    let gloss = gloss.trim();
    if gloss.is_empty() || gloss.ends_with(['.', '!', '?']) {
        gloss.to_string()
    } else {
        format!("{gloss}.")
    }
}

/// Write the `AGENTS.md` REGEN block. The generator `yidam regen` runs.
pub fn block() -> Result<()> {
    let root = crate::paths::repo_root()?;
    let content = block_content(&root)?;
    crate::regen::emit(&content);
    write_block(&root, &content)
}

/// The block's text for this repository, held or unheld.
fn block_content(root: &std::path::Path) -> Result<String> {
    let declaration = kuten::read_declaration(root)?;
    let profile = match &declaration {
        Some(d) => kuten::read_profile(root, &d.name)?,
        None => None,
    };
    Ok(render_block(declaration.as_ref(), profile.as_ref()))
}

/// Put the text in the file, without echoing it.
///
/// Split from [`block`] because `adopt` writes this block too, and a generator's job is to
/// print what it generated while `adopt`'s is to say what it did. Running the whole generator
/// there would print the block's full text in the middle of a four-line report.
fn write_block(root: &std::path::Path, content: &str) -> Result<()> {
    crate::regen::update_file_regen(&root.join("AGENTS.md"), "yidam kuten", content)
}

// ── adopting one ──────────────────────────────────────────────────────────────

/// The `AGENTS.md` section carrying the kuten REGEN block.
///
/// **A second copy of `sadhana/root/AGENTS.md`'s section, held equal by a test.** The
/// scaffold is consumed and deleted at genesis, so a repository that already exists cannot be
/// handed the marker the way a new one is — and a marker is not something to ask a person to
/// hand-write, since `update_file_regen` silently does nothing when it is absent or malformed.
/// The copy is the cost of that; [`the_inserted_section_is_the_scaffold_s`] is what keeps the
/// two from drifting.
const AGENTS_SECTION: &str = "\
## What this corpus's practice is aimed at

<!-- REGEN: yidam kuten
Regenerated by: `yidam kuten`
Fields: the kuten this corpus adopted, its revision, and the bands it declares — phase
        types and phase-commit share, nodes per commit and median node length, the
        vocabulary size and the off-vocabulary share it expects.
Reads: `.yidam/decisions/kuten.yml` and the profile it names under
       `.yidam/.vendor/prelude/kuten/`. Holding no kuten is a supported state and is
       reported as one.
-->
_Run `yidam kuten` to populate._
<!-- /REGEN -->

The declaration narrows the loop and may not widen the model, and **it binds nobody**:
divergence from it is a question for a person, not a defect. `yidam kuten check` reports
where this repository's history and its declaration disagree, writes nothing, and exits zero.
Change it by a `decide:` commit carrying a superseding decision record — never by editing the
vendored profile, which is discarded on the next re-vendor.
";

/// The marker `update_file_regen` looks for. Absent, it writes nothing and says nothing.
const AGENTS_MARKER: &str = "<!-- REGEN: yidam kuten";

/// The decision record `adopt` writes, from a profile it read.
///
/// # What this may write, and what it may not
///
/// Four fields are facts about the act: which profile, at which revision, and the profile's
/// own `gloss:` quoted back. The revision is **copied from the vendored profile**, which is
/// the whole reason this is a command. `bootstrap.md` states the rule as *"that profile's own
/// `revision:`, copied — not typed from memory"*, and a retrofit performed by hand is exactly
/// the transcription the revision model exists to survive.
///
/// It does not write `rationale:`. Why *this* corpus adopts *this* practice is the adopter's
/// reasoning and nobody else's, and a placeholder shipped unfilled reads as an answer. The
/// command says so on the way out instead of inventing one.
fn record(name: &str, revision: u32, gloss: &str) -> String {
    let mut out = String::new();
    out.push_str("id: kuten\n");
    let _ = writeln!(
        out,
        "summary: the `{name}` kuten, adopted at revision {revision}"
    );
    let _ = writeln!(out, "kuten: {name}");
    let _ = writeln!(out, "revision: {revision}");
    out.push_str("decision: |\n");
    let gloss = stopped(gloss);
    if gloss.is_empty() {
        let _ = writeln!(out, "  This corpus's practice is `{name}`.");
    } else {
        let _ = writeln!(out, "  This corpus's practice is `{name}`: {gloss}");
    }
    out.push_str("\n  Adopted with `yidam kuten adopt` against the profile vendored at\n");
    let _ = writeln!(
        out,
        "  `{}/{name}/kuten.yml`, at revision {revision}.",
        kuten::VENDORED_DIR
    );
    out.push_str("  The revision is copied from that profile, and every consumer reads the\n");
    out.push_str("  kuten at the vintage this repository holds rather than at upstream's\n");
    out.push_str("  current one.\n");
    out
}

/// What `AGENTS.md` was in, when `adopt` went looking for somewhere to put the declaration.
///
/// Three states, and two of them used to be one `Ok(None)` — which is how a repository
/// adopted a kuten into a repository with no document to declare it in, and every surface
/// reported that as fine (#694). Skipping the write is still right; saying nothing is not.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Section {
    /// The file existed without the marker, and now carries it.
    Added,
    /// The marker was already there, so [`write_block`] fills it and there is nothing to say.
    AlreadyThere,
    /// There is no `AGENTS.md` at all. Nothing in the loop carries the declaration, and
    /// nothing will until a person makes somewhere for it to go.
    NoFile,
}

/// Give `AGENTS.md` the kuten REGEN block if it has none. Returns what it found.
///
/// Appended rather than placed: the scaffold puts the section near the top of a file it also
/// wrote, and this is editing a document somebody else has been keeping. Appending cannot
/// land inside another generator's block or split a section, and where it sits matters less
/// than that it is there — `yidam regen` fills it wherever it is.
///
/// A repository with no `AGENTS.md` at all gets none written. That file is the derived
/// repository's own, and conjuring one from a command that was asked to record a decision
/// would be reaching well past what was asked.
fn ensure_agents_section(root: &std::path::Path) -> Result<Section> {
    let path = root.join("AGENTS.md");
    if !path.exists() {
        return Ok(Section::NoFile);
    }
    let text = std::fs::read_to_string(&path)?;
    if text.contains(AGENTS_MARKER) {
        return Ok(Section::AlreadyThere);
    }
    let mut updated = text;
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated.push('\n');
    updated.push_str(AGENTS_SECTION);
    std::fs::write(&path, updated)?;
    Ok(Section::Added)
}

/// `yidam kuten adopt <name>` — declare a vendored profile as this corpus's practice.
///
/// Every refusal exits non-zero and names the repair. A refusal is not an adoption, and one
/// that exits quietly is a corpus believing it declared something it did not.
pub fn adopt(name: &str) -> Result<()> {
    let root = crate::paths::repo_root()?;

    let vendored = kuten::vendored_profiles(&root);
    if vendored.is_empty() {
        anyhow::bail!(
            "no kuten profile is vendored at {}/. Re-vendor the prelude first — \
             `mise run yidam-vendor-update` — and the layer arrives with it.",
            kuten::VENDORED_DIR
        );
    }
    let Some(profile) = kuten::read_profile(&root, name)? else {
        anyhow::bail!(
            "no profile named `{name}` is vendored here. This repository holds: {}",
            vendored.join(", ")
        );
    };

    // A kuten may change after genesis, and #572's scope decision 4 says how: a `decide:`
    // commit carrying a superseding record, so `replay` marks the discontinuity and `score`
    // refuses a range spanning it. Overwriting the record in place would erase the very
    // discontinuity those two exist to report, so this refuses rather than offering a flag.
    let path = root.join(kuten::DECISION_PATH);
    if path.exists() {
        let held = kuten::read_declaration(&root)?;
        let naming = held
            .map(|d| format!("`{}` at revision {}", d.name, d.revision))
            .unwrap_or_else(|| "a record this command cannot read".to_string());
        anyhow::bail!(
            "{} already holds {naming}. Changing a kuten is a `decide:` commit carrying a \
             superseding record, not an overwrite — `replay` marks the discontinuity and \
             `score` refuses a range spanning it.",
            kuten::DECISION_PATH
        );
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, record(name, profile.revision, &profile.gloss))?;

    println!("wrote {}", kuten::DECISION_PATH);
    println!("  kuten: {name}");
    println!(
        "  revision: {} — copied from the vendored profile",
        profile.revision
    );

    // A declaration the agent never reads is this epic's own diagnosed failure aimed at its
    // centrepiece. The scaffold puts this section in a new repository's AGENTS.md and is then
    // deleted, so a corpus that already existed has no marker — and `update_file_regen` on a
    // file with no marker writes nothing and reports nothing.
    //
    // And the absent-file arm says so. It used to be the same silence as *the marker is
    // already there*, so a repository that adopted with no `AGENTS.md` got a four-line report
    // identical to a repository that had one — and nothing it could run afterwards would tell
    // it either (#694). The write is still declined; only the silence is.
    match ensure_agents_section(&root)? {
        Section::Added => println!("added the kuten section to AGENTS.md"),
        Section::AlreadyThere => {}
        Section::NoFile => {
            println!();
            println!("There is no AGENTS.md here, so nothing an agent reads carries this");
            println!("declaration — which is the failure this layer exists to prevent.");
            println!();
            println!("Create one holding this section, and `yidam kuten` fills it from then on:");
            println!();
            for line in AGENTS_SECTION.lines() {
                println!("    {line}");
            }
            println!();
            println!("`yidam doctor` reports this until a file carries the marker.");
        }
    }
    // And fill it, rather than leaving the repository one step from done. An adopted kuten
    // whose block still reads "Run `yidam kuten` to populate" is a repository that has
    // declared a practice its agent cannot see, and `regen --check` reports it stale — which
    // makes adoption something that breaks the gate until a second command is remembered.
    write_block(&root, &block_content(&root)?)?;

    println!();
    println!("The record carries no `rationale:`. Why this corpus adopts this practice is");
    println!("yours to write, and a placeholder would read as an answer.");
    println!();
    println!("`yidam kuten check` now reads this corpus against what it just declared.");
    Ok(())
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
        let _ = write!(
            out,
            "\n⚠ The vendored profile is at revision {}. A comparison across revisions is \
             annotated rather than made: re-vendor, or record a superseding decision.\n",
            r.vendored_revision.unwrap_or_default()
        );
    }
    let _ = write!(
        out,
        "\n{} commit(s), {} node(s) measured.\n\n",
        r.measurement.commits, r.measurement.nodes
    );
    for f in &r.findings {
        let _ = writeln!(
            out,
            "  [{}] {:<22} declared {:<12} measured {}",
            f.verdict.tag(),
            f.metric,
            f.declared,
            f.measured
        );
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
        let _ = writeln!(out, "  · {q}");
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

    /// **The section `adopt` inserts is the scaffold's, character for character.**
    ///
    /// Two copies exist because they must: the scaffold is deleted at genesis, so a
    /// repository that already exists can only be handed the marker by the binary. This is
    /// what stops them drifting — and drift here is silent in the worst way, because
    /// `update_file_regen` writes nothing at all when the marker it looks for is absent or
    /// malformed, so a mangled copy would leave the block permanently unfilled and say
    /// nothing about it.
    #[test]
    fn the_inserted_section_is_the_scaffolds() {
        let path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../sadhana/root/AGENTS.md");
        let scaffold = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
        assert!(
            scaffold.contains(AGENTS_SECTION),
            "`sadhana/root/AGENTS.md` no longer contains the section `kuten adopt` inserts. \
             A new repository would get one wording and a retrofitted one another, and the \
             two are the same document."
        );
        assert!(
            AGENTS_SECTION.contains(AGENTS_MARKER),
            "the inserted section carries no `{AGENTS_MARKER}` marker, so `yidam kuten` \
             would write nothing into it — silently, which is how it fails"
        );
    }

    /// The three states `adopt` can find `AGENTS.md` in are three answers, and two of them
    /// used to be the same silence (#694).
    ///
    /// The absent-file arm is the one that mattered: it is the state one of the two day-one
    /// adopters was left in, and its report was character-for-character the report of a
    /// repository whose section was already there.
    #[test]
    fn the_three_states_agents_md_can_be_in_are_distinguishable() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        assert_eq!(ensure_agents_section(root).unwrap(), Section::NoFile);

        std::fs::write(root.join("AGENTS.md"), "# Agents\n\nSome guidance.\n").unwrap();
        assert_eq!(ensure_agents_section(root).unwrap(), Section::Added);
        let text = std::fs::read_to_string(root.join("AGENTS.md")).unwrap();
        assert!(text.contains(AGENTS_MARKER), "{text}");
        assert!(
            text.starts_with("# Agents"),
            "appended, not replaced: {text}"
        );

        // Idempotent, and for the reason the marker exists: a second append would give the
        // file two blocks and `update_file_regen` would fill whichever it met first.
        assert_eq!(ensure_agents_section(root).unwrap(), Section::AlreadyThere);
        assert_eq!(
            std::fs::read_to_string(root.join("AGENTS.md")).unwrap(),
            text
        );
    }

    /// A gloss is a fragment, and both surfaces that quote one join it to a sentence.
    #[test]
    fn a_gloss_is_stopped_once_and_never_twice() {
        assert_eq!(stopped("questions opened"), "questions opened.");
        assert_eq!(stopped("questions opened."), "questions opened.");
        assert_eq!(stopped("  spaced  "), "spaced.");
        assert_eq!(stopped(""), "");
    }

    /// The record carries the two fields a tool reads, and no rationale.
    ///
    /// The revision is asserted against a profile declaring something other than 1, because a
    /// `record` that ignored its argument and wrote the shipped profile's revision would pass
    /// against 1 forever — which is the transcription error this command exists to remove.
    #[test]
    fn the_record_copies_the_revision_and_invents_no_rationale() {
        let text = record("inquiry", 7, "the corpus grows through sustained inquiry");
        assert!(text.contains("kuten: inquiry\n"), "{text}");
        assert!(text.contains("revision: 7\n"), "{text}");
        assert!(
            text.contains("`inquiry`: the corpus grows through sustained inquiry."),
            "the gloss is quoted, joined with a colon and stopped once: {text}"
        );
        assert!(
            !text.contains("rationale"),
            "why this corpus adopts this practice is the adopter's, and a placeholder reads \
             as an answer: {text}"
        );
        let parsed = Declaration::parse(&text).expect("the record parses as a declaration");
        assert_eq!(parsed.name, "inquiry");
        assert_eq!(parsed.revision, 7);
    }

    fn profile() -> Profile {
        Profile::parse(
            "kuten: inquiry\nrevision: 1\ngloss: questions opened, and settled\n\
             phases:\n  types: [Investigation, Extraction]\n  commit_share: {low: 0.12, high: 0.27}\n\
             vocabulary:\n  verbs: [establish, open]\n  off_vocabulary_share: {low: 0.0, high: 0.02}\n\
             classes:\n  nodes_per_commit: {low: 0.50, high: 1.12}\n  median_node_lines: {low: 35, high: 62}\n\
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
            suffixed_commits: 0,
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
