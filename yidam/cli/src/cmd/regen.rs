//! `yidam regen` — refresh every REGEN block in one pass.

use anyhow::{Context, Result};
use std::fmt::Write as _;

/// Every generator that writes a REGEN block.
///
/// **This is the list**, and it is one list on purpose. There used to be two: a `regen`
/// mise task that ran eight generators, and a derived-repo CI step that ran two of them
/// and then failed with *"Run 'mise run regen' and commit the result"*. Six generators'
/// blocks could go stale indefinitely without CI noticing, and the remedy CI prescribed
/// regenerated more than CI had checked — so following the instruction could produce a
/// diff the gate never asked for.
///
/// `status` and `open-questions` were in neither list, and both write blocks into the
/// root README: refreshed by no task, verified by no gate. `status` only became safe to
/// include once [`crate::git::phase_refs`] stopped counting local branches, because until
/// then its output differed between a fresh clone and a developer machine.
///
/// A generator whose target file does not exist is a no-op — `update_file_regen` returns
/// early — so this runs unchanged in a repository that has no `agents/` or `packages/`
/// yet. That is why the list is unconditional.
/// A named generator: the CLI subcommand's name, and the function behind it.
type Generator = (&'static str, fn() -> Result<()>);

const GENERATORS: &[Generator] = &[
    // Text, always. These write REGEN blocks into markdown; `--format json` is a
    // reporting mode and has nothing to regenerate.
    ("status", || super::status(crate::report::Format::Text)),
    ("open-questions", || {
        super::open_questions(crate::report::Format::Text)
    }),
    ("corpus-index", || {
        super::corpus_index(crate::report::Format::Text)
    }),
    ("index-status", || {
        super::index_status(crate::report::Format::Text)
    }),
    ("catalog-audit", || {
        super::catalog_audit(crate::report::Format::Text)
    }),
    ("agents-index", super::agents_index),
    ("skills-index", super::skills_index),
    ("crates-index", super::crates_index),
    ("packages-index", super::packages_index),
    ("bundle-status", super::bundle_status),
    // `vault-status` and `decisions-log` were generators that this list did not name (#831),
    // so `yidam regen` did not populate their blocks and `--check` did not report them stale.
    // In the corpus that reported it, the vault block held its "run this to populate"
    // placeholder from genesis through sixty-two phases — it went from empty to *wrong* the
    // day a vault was declared, and no gate had an opinion either way. `false` is
    // `vault_status`'s `direct`: see its own note on the one character between it and a
    // read-only command.
    ("vault-status", || super::vault_status(false)),
    ("decisions-log", super::decisions_log),
    // The declaration a corpus makes about its own practice, into the file an agent reads
    // at session start. Regenerated rather than hand-copied: a hand-copied declaration is
    // one re-vendor away from being silently wrong.
    ("kuten", super::kuten::block),
];

/// The names of every generator this command runs, in order.
///
/// Public because the guard on the other end of the chain lives in the binary: `help.rs`
/// declares which commands write, and `main.rs` is a separate compilation unit from this
/// library, so a `#[cfg(test)]` item here is invisible to it. The chain is the crate's own
/// `update_file_regen` call sites → [`GENERATORS`] → the `*` marker in `--help`, each link
/// asserted rather than described, because a generator can be missing from either end and
/// #831 found it missing from both.
pub fn generator_names() -> Vec<&'static str> {
    GENERATORS.iter().map(|(name, _)| *name).collect()
}

#[derive(serde::Serialize)]
pub struct RegenReport {
    /// Whether every REGEN block already holds what its generator produces.
    pub passed: bool,
    /// The blocks that do not. Empty when `passed`.
    pub stale: Vec<crate::regen::Stale>,
}

pub(crate) fn render_regen_check(r: &RegenReport) -> String {
    if r.passed {
        return "Every REGEN block is current.".to_string();
    }
    let mut out = format!("{} REGEN block(s) stale:\n", r.stale.len());
    for s in &r.stale {
        let _ = writeln!(out, "  {}  ({})", s.file, s.generator);
    }
    out.push_str("\nRun `yidam regen` and commit the result as a `regen:` commit.");
    out
}

/// Which REGEN blocks are not what their generators produce. **Writes nothing.**
///
/// Extracted so `yidam doctor` can ask the same question through the same [`GENERATORS`]
/// list. A second list there would be the third list this command exists to have
/// prevented — see the note above.
///
/// [`crate::regen::begin_check`] is process-global, so this is not reentrant. Nothing calls
/// it concurrently: both callers are a single command's single pass.
pub(crate) fn stale_blocks() -> Result<Vec<crate::regen::Stale>> {
    crate::regen::begin_check();
    for (name, run) in GENERATORS {
        let outcome = run().with_context(|| format!("running {name}"));
        if outcome.is_err() {
            // Leave check mode before propagating, or the next caller inherits a
            // half-finished check and a write path that silently records instead of writes.
            crate::regen::end_check();
        }
        outcome?;
    }
    Ok(crate::regen::end_check())
}

/// Refresh every REGEN block, or — with `check` — report which ones would change.
///
/// `--check` runs the same [`GENERATORS`] list, which is the whole point: a check that
/// walked its own list would be the third list this command exists to have prevented.
pub fn regen(check: bool, format: crate::report::Format) -> Result<()> {
    require_whole_history(&crate::paths::repo_root()?)?;
    if check {
        let stale = stale_blocks()?;
        return report_check(stale, format);
    }
    for (name, run) in GENERATORS {
        println!("── {name}");
        run().with_context(|| format!("running {name}"))?;
    }
    Ok(())
}

/// Refuse to run against a truncated history — in either mode.
///
/// **The block's contract has two halves and only one of them is about content.** A REGEN block
/// held by `--check` has to be a function of what every checkout of a commit has in common, and
/// most of what `status` reports is: the node count, the open questions, the claim counts all read
/// the corpus. `genesis` reads the repository's *first commit*, and a shallow clone does not have
/// it — nor does it say so. `rev-list --max-parents=0 HEAD` answers with the boundary commit,
/// which from the inside is indistinguishable from a root, so the generator writes a date the
/// clone invented. Measured: one commit of `allen-county-ohio` renders `genesis 2026-08-28` from a
/// full clone and `2026-09-07` from a `--depth 1` clone of the same commit.
///
/// The two ways out were making every generator depth-invariant, or making the command state what
/// it needs. Invariance is the wrong trade: it would cost `genesis` and the catalog's TTL ages,
/// both of which are history by definition and both of which readers want. So this is a
/// precondition, and it is checked in **both** modes for different reasons — `--check` would
/// otherwise report a diff nobody can commit their way out of, and the writing mode would commit
/// the invented date.
///
/// **Refusing, not skipping.** A report may say it found nothing; a gate may not. `doctor` is the
/// report, and it warns here instead — see [`crate::cmd::doctor`].
fn require_whole_history(root: &std::path::Path) -> Result<()> {
    if !crate::git::is_shallow(root) {
        return Ok(());
    }
    anyhow::bail!(
        "this is a shallow clone, and a REGEN block cannot be generated from one.\n\
         `genesis` reads the repository's first commit; a truncated history's oldest commit\n\
         is whatever was checked out, and git reports it as a root. The block would hold a\n\
         date this clone invented, and the gate would ask you to commit it.\n\n\
         Fetch the whole history first:\n\
         \x20 locally   git fetch --unshallow\n\
         \x20 in CI     actions/checkout with `fetch-depth: 0`"
    )
}

fn report_check(stale: Vec<crate::regen::Stale>, format: crate::report::Format) -> Result<()> {
    let report = RegenReport {
        passed: stale.is_empty(),
        stale,
    };
    let passed = report.passed;
    if format.is_json() {
        crate::report::emit(&crate::paths::repo_root()?, report)?;
    } else {
        println!("{}", render_regen_check(&report));
    }
    if !passed {
        std::process::exit(1);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every generator this crate has, discovered from its own source.
    ///
    /// Returns the distinct `<command>` each `update_file_regen` call site names, how many
    /// call sites were found, and how many of them named their generator with a literal. The
    /// last two are what stop a call whose command is computed from passing as a silently
    /// smaller set.
    ///
    /// **Discovered rather than listed**, and that is the whole point of it. The test this
    /// replaced compared [`GENERATORS`] against a second hardcoded array, so it agreed with
    /// itself for as long as both were edited together and reported nothing when they were
    /// not — which is how `vault-status` and `decisions-log` came to be generators that
    /// `yidam regen` did not run and `--check` did not check (#831). A guard whose subject is
    /// a list cannot see a hole in that list; this one's subject is the crate.
    ///
    /// Comment lines are dropped before the scan. Several doc comments here and in
    /// `lint/mod.rs` name `update_file_regen` and quote generator strings, and a guard that
    /// counts prose as code is satisfied by prose.
    fn generators_in_source() -> (std::collections::BTreeSet<String>, usize, usize) {
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut found = std::collections::BTreeSet::new();
        let mut call_sites = 0usize;
        let mut named = 0usize;
        for entry in walkdir::WalkDir::new(&src).into_iter().flatten() {
            if entry.path().extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let text = std::fs::read_to_string(entry.path()).expect("a source file reads");
            let code: String = text
                .lines()
                .filter(|l| !l.trim_start().starts_with("//"))
                .collect::<Vec<_>>()
                .join("\n");
            for (at, _) in code.match_indices("update_file_regen(") {
                // Two things that are not call sites. `fn ` is the definition — it takes
                // `command: &str` and names no generator. A leading quote is this scanner's
                // own needle, in this very file: a guard that matches its own source finds a
                // generator named the empty string and reports it missing.
                if code[..at].ends_with("fn ") || code[..at].ends_with('"') {
                    continue;
                }
                call_sites += 1;
                // A generous window: the widest call here spreads the path, the command and
                // the content over four lines.
                let window = &code[at..code.len().min(at + 400)];
                if let Some(start) = window.find("\"yidam ") {
                    let rest = &window[start + 1..];
                    if let Some(end) = rest.find('"') {
                        named += 1;
                        found.insert(rest[..end].trim_start_matches("yidam ").to_string());
                    }
                }
            }
        }
        (found, call_sites, named)
    }

    /// Every generator the crate has is one this command runs.
    ///
    /// A generator absent from [`GENERATORS`] has a REGEN block refreshed by nothing and
    /// checked by nothing — which is the defect this command exists to close, and which it
    /// had two instances of while a hardcoded pair of lists guarded the set.
    #[test]
    fn every_generator_in_the_crate_is_listed() {
        let (found, call_sites, named) = generators_in_source();
        // A floor on the scanner, not a count of the crate. It says the match still finds a
        // population; what the population *is* is the assertion below. A guard that reports
        // "nothing is missing" after matching nothing is the failure this whole test replaced.
        assert!(
            found.len() > 5,
            "the scan found only {} generator(s), so it is no longer reading call sites — the \
             shape it matches on has changed: {found:?}",
            found.len()
        );
        assert_eq!(
            named,
            call_sites,
            "{} of {call_sites} `update_file_regen` call sites do not name their generator \
             with a literal, so this scan cannot see which block they write. Name it, or \
             this guard is smaller than it reads.",
            call_sites - named
        );
        let listed = generator_names();
        let missing: Vec<&String> = found
            .iter()
            .filter(|g| !listed.contains(&g.as_str()))
            .collect();
        assert!(
            missing.is_empty(),
            "{missing:?} write REGEN blocks and are not in GENERATORS — `yidam regen` will \
             not populate them and `--check` will not report them stale"
        );
    }

    #[test]
    fn names_are_unique() {
        let names = generator_names();
        let mut seen: Vec<&str> = names.clone();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), names.len(), "duplicate generator in the list");
    }
}
