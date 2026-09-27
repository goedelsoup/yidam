//! `yidam regen` — refresh every REGEN block in one pass.

use anyhow::{Context, Result};
use std::fmt::Write as _;

/// A named generator: the CLI subcommand's name, and the function behind it.
///
/// Every generator takes the corpus rather than resolving one, because `regen --root` has to
/// mean the same thing as running each of these commands with `--root` (#918). A generator
/// that resolved its own root would refresh *this* repository's blocks while the reader had
/// named another — and the fourteen would not even agree with each other about which.
type Generator = (&'static str, fn(Option<&std::path::Path>) -> Result<()>);

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
const GENERATORS: &[Generator] = &[
    // Text, always. These write REGEN blocks into markdown; `--format json` is a
    // reporting mode and has nothing to regenerate.
    ("status", |root| {
        super::status(root, crate::report::Format::Text)
    }),
    ("open-questions", |root| {
        super::open_questions(root, crate::report::Format::Text)
    }),
    ("corpus-index", |root| {
        super::corpus_index(root, crate::report::Format::Text)
    }),
    ("index-status", |root| {
        super::index_status(root, crate::report::Format::Text)
    }),
    ("catalog-audit", |root| {
        super::catalog_audit(root, crate::report::Format::Text)
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
    ("vault-status", |root| super::vault_status(root, false)),
    ("decisions-log", super::decisions_log),
    // The declaration a corpus makes about its own practice, into the file an agent reads
    // at session start. Regenerated rather than hand-copied: a hand-copied declaration is
    // one re-vendor away from being silently wrong.
    ("kuten", super::kuten::block),
    // What this corpus has practiced, into `PRACTICE.md` — a document, regenerated rather
    // than authored (#287). A repository keeping no `PRACTICE.md` has opted out, and the
    // generator is its own no-op there, before `update_file_regen`'s.
    ("practice", super::practice::block),
    // The one generator that is not handed its file. The other fourteen write a block at a
    // path they know; this one reads the tracked markdown set for blocks whose command is
    // `yidam count <query>` and answers each with a number (RFC-0043). A repository with no
    // such block is its own no-op, which is every repository the day this lands.
    (super::count::NAME, super::count::block),
];

/// The generators whose command carries an argument, and what the argument is called.
///
/// One entry, and the shape is general because the reason is. Every other generator's command
/// is its name and nothing else, so `unclaimed_in` can ask whether a block's command *is* a
/// generator. `count`'s is `count <query>`: the bare name writes nothing, and the name plus an
/// argument writes the block. Both halves matter.
///
/// Without the split, registering `count` would make `<!-- REGEN: yidam count -->` — a block
/// naming no query, which no generator can ever write — go from correctly reported by #1062's
/// gate to silently claimed by a generator that will never visit it. That is the exact defect
/// that gate exists to close, so the name is kept out of the whole-match set and matched only
/// with its separator.
///
/// The label is for the reader of a failed `--check`: [`render_regen_check`] prints the
/// generators as `count <query>`, so a bare block's remedy is visible in the list rather than
/// described in a sentence.
const PARAMETERISED: &[(&str, &str)] = &[(super::count::NAME, "<query>")];

/// The generator names `unclaimed_in` matches whole, and the prefixes it matches with an
/// argument. See [`PARAMETERISED`].
fn claimable() -> (Vec<&'static str>, Vec<&'static str>) {
    let prefixes: Vec<&'static str> = PARAMETERISED.iter().map(|(name, _)| *name).collect();
    let whole = generator_names()
        .into_iter()
        .filter(|name| !prefixes.contains(name))
        .collect();
    (whole, prefixes)
}

/// How each generator is spelled in the remedy a failed `--check` prints.
///
/// `count <query>` rather than `count`, because a reader whose block says `yidam count` has
/// been told it names no generator this binary can write for, and the list is where they find
/// out what is missing.
fn generator_usages() -> Vec<String> {
    generator_names()
        .into_iter()
        .map(
            |name| match PARAMETERISED.iter().find(|(n, _)| *n == name) {
                Some((_, argument)) => format!("{name} {argument}"),
                None => name.to_string(),
            },
        )
        .collect()
}

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
    /// The blocks no generator answers for. Empty when `passed`.
    ///
    /// A second list rather than more rows in `stale`, because the two verdicts have
    /// different remedies and `stale`'s is printed as an instruction. `yidam regen` fixes a
    /// stale block; it cannot fix a block it has no generator for, and a gate whose
    /// prescribed remedy does not clear it is the unsatisfiable gate
    /// [`require_whole_history`] refuses for the same reason.
    pub unclaimed: Vec<crate::regen::Unclaimed>,
}

pub(crate) fn render_regen_check(r: &RegenReport) -> String {
    if r.passed {
        return "Every REGEN block is current.".to_string();
    }
    let mut out = String::new();
    if !r.stale.is_empty() {
        let _ = writeln!(out, "{} REGEN block(s) stale:", r.stale.len());
        for s in &r.stale {
            let _ = writeln!(out, "  {}  ({})", s.file, s.generator);
        }
        out.push_str("\nRun `yidam regen` and commit the result as a `regen:` commit.\n");
    }
    if !r.unclaimed.is_empty() {
        if !r.stale.is_empty() {
            out.push('\n');
        }
        let _ = writeln!(
            out,
            "{} REGEN block(s) name a command no generator writes:",
            r.unclaimed.len()
        );
        for u in &r.unclaimed {
            let _ = writeln!(out, "  {}  (yidam {})", u.file, u.generator);
        }
        // "a command no generator writes" rather than "a generator that does not exist",
        // because since RFC-0043 those are two things. `yidam statsu` names nothing; `yidam
        // count` names a generator that exists and cannot write a block with no query in it.
        // The old wording contradicted itself on the second — it said `count` did not exist,
        // above a list containing `count`.
        let _ = writeln!(
            out,
            "\n`yidam regen` writes no block for those, so they keep whatever they hold. \
             Correct the command or delete the block.\n\
             The generators are: {}",
            generator_usages().join(", ")
        );
    }
    out.trim_end().to_string()
}

/// Which REGEN blocks are not what their generators produce. **Writes nothing.**
///
/// Extracted so `yidam doctor` can ask the same question through the same [`GENERATORS`]
/// list. A second list there would be the third list this command exists to have
/// prevented — see the note above.
///
/// [`crate::regen::begin_check`] is process-global, so this is not reentrant. Nothing calls
/// it concurrently: both callers are a single command's single pass.
pub(crate) fn stale_blocks(root: Option<&std::path::Path>) -> Result<Vec<crate::regen::Stale>> {
    crate::regen::begin_check();
    for (name, run) in GENERATORS {
        let outcome = run(root).with_context(|| format!("running {name}"));
        if outcome.is_err() {
            // Leave check mode before propagating, or the next caller inherits a
            // half-finished check and a write path that silently records instead of writes.
            crate::regen::end_check();
        }
        outcome?;
    }
    Ok(crate::regen::end_check())
}

/// Which REGEN blocks name a command no generator writes. **Writes nothing.**
///
/// The other half of [`stale_blocks`], and it runs the other way round. That one asks each
/// generator what its block should hold; this one asks each document what it claims a
/// generator for. Neither question finds the other's answer: a block nothing is pushed into
/// is a block nothing records, which is how `<!-- REGEN: yidam statsu -->` survived
/// `yidam regen` and was called current by `yidam regen --check` (#1062).
///
/// **The tracked set, and markdown within it.** `git ls-files` rather than a walk, for the
/// reason [`crate::cmd::tracked`] gives at length: a list of what to skip grows one entry per
/// accident, and the entry is always written after the accident. It also lands the walk on
/// the right population — this gate answers about the commit, so an uncommitted document is
/// not yet a block this repository has. The prose walk `lint` uses would have been the near
/// miss: it reads `.yidam/`, `docs/` and the root `README.md`, and five generators write
/// outside all three (`crates/`, `packages/`, `web/`, `AGENTS.md`, `PRACTICE.md`).
///
/// [`crate::cmd::tracked::list`] and not [`crate::cmd::tracked::paths`]: the latter refuses
/// an empty tracked set, because a *copy* from one is a copy of nothing. A read from one is
/// a read of nothing, which is a fine thing to report. Borrowing the refusal made this gate
/// tell an initialised-but-uncommitted derived repository to run somewhere else.
pub(crate) fn unclaimed_blocks(root: &std::path::Path) -> Result<Vec<crate::regen::Unclaimed>> {
    let (known, parameterised) = claimable();
    let mut found = Vec::new();
    for rel in super::tracked::list(root)? {
        if !rel.ends_with(".md") {
            continue;
        }
        let path = root.join(&rel);
        // Tracked and not in the worktree — a deleted file staged but not committed. There
        // is nothing to read and nothing to say about it.
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
        found.extend(
            crate::regen::unclaimed_in(&text, &known, &parameterised)
                .into_iter()
                .map(|generator| crate::regen::Unclaimed {
                    file: rel.clone(),
                    generator,
                }),
        );
    }
    Ok(found)
}

/// Refresh every REGEN block, or — with `check` — report which ones would change.
///
/// `--check` runs the same [`GENERATORS`] list, which is the whole point: a check that
/// walked its own list would be the third list this command exists to have prevented.
pub fn regen(
    root: Option<&std::path::Path>,
    check: bool,
    format: crate::report::Format,
) -> Result<()> {
    let resolved = crate::paths::resolve_root(root)?;
    require_whole_history(&resolved)?;
    if check {
        let stale = stale_blocks(root)?;
        let unclaimed = unclaimed_blocks(&resolved)?;
        return report_check(&resolved, stale, unclaimed, format);
    }
    for (name, run) in GENERATORS {
        println!("── {name}");
        run(root).with_context(|| format!("running {name}"))?;
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

fn report_check(
    root: &std::path::Path,
    stale: Vec<crate::regen::Stale>,
    unclaimed: Vec<crate::regen::Unclaimed>,
    format: crate::report::Format,
) -> Result<()> {
    let report = RegenReport {
        passed: stale.is_empty() && unclaimed.is_empty(),
        stale,
        unclaimed,
    };
    let passed = report.passed;
    // The root is the caller's, not `repo_root()`'s. It was the latter until #918, which is
    // the one place `--root` could be accepted and the envelope still name the directory the
    // process happened to start in — `root_flag.rs` measured it. The blocks were always read
    // from the right corpus; only the envelope disagreed, which is exactly the class of
    // defect a report nobody reads by hand keeps.
    crate::report::gate(root, format, report, passed, |r| {
        println!("{}", render_regen_check(r))
    })
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
    /// Comment lines are dropped before the scan. Several doc comments here and under
    /// `lint/` name `update_file_regen` and quote generator strings, and a guard that
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
                        found.insert(generator_named_by(&rest[..end]).to_string());
                    }
                }
            }
        }
        (found, call_sites, named)
    }

    /// The generator a `"yidam …"` literal names.
    ///
    /// Fourteen call sites spell their command whole — `"yidam status"` — and the name is
    /// what follows the prefix. `count`'s cannot: its command carries a query the *document*
    /// wrote, so the literal at the call site is a `format!` template, `"yidam count {}"`.
    ///
    /// The amendment RFC-0043 specifies is to read a brace-bearing literal as a **prefix**:
    /// take what precedes the first `{` and require it, below, to be a listed generator. That
    /// keeps the site inside this scan rather than exempt from it, which is the whole of what
    /// the `named == call_sites` clause is for. A site naming neither — no literal, or a
    /// template whose prefix is not a generator — still fails.
    fn generator_named_by(literal: &str) -> &str {
        let name = literal.trim_start_matches("yidam ");
        match name.find('{') {
            Some(brace) => name[..brace].trim_end(),
            None => name,
        }
    }

    /// The prefix rule reads a template as its generator, and reads nothing else differently.
    ///
    /// Written against the helper rather than against the scan, because the scan's answer is
    /// the union over the crate: a bug that returned the empty string for every literal would
    /// still leave `found` non-empty and `missing` empty as long as one site happened to
    /// parse. This is the case-by-case version, including the two that must still fail.
    #[test]
    fn a_template_literal_names_its_generator() {
        assert_eq!(generator_named_by("yidam status"), "status");
        assert_eq!(generator_named_by("yidam open-questions"), "open-questions");
        assert_eq!(generator_named_by("yidam count {}"), "count");
        // Nothing before the brace is nothing named, and the assertion below rejects it.
        assert_eq!(generator_named_by("yidam {}"), "");
        assert!(!generator_names().contains(&""));
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

    /// Every parameterised generator is a generator.
    ///
    /// [`PARAMETERISED`] subtracts from the set `unclaimed_in` matches whole, so an entry
    /// naming nothing would remove nothing and quietly do so — and one naming a generator
    /// that was later renamed would leave the old name matched-with-an-argument forever,
    /// which is a block claimed by nothing.
    #[test]
    fn every_parameterised_generator_is_one() {
        let listed = generator_names();
        for (name, argument) in PARAMETERISED {
            assert!(
                listed.contains(name),
                "`{name}` is parameterised and is not a generator: {listed:?}"
            );
            assert!(!argument.is_empty(), "`{name}`'s argument has no name");
        }
    }

    /// The partition is a partition: every generator is in exactly one half.
    #[test]
    fn the_two_halves_cover_the_generators_once_each() {
        let (whole, prefixes) = claimable();
        let mut both: Vec<&str> = whole.iter().chain(prefixes.iter()).copied().collect();
        both.sort_unstable();
        let mut listed = generator_names();
        listed.sort_unstable();
        assert_eq!(both, listed);
        assert!(
            !whole.contains(&super::super::count::NAME),
            "`count` is matched whole, so a block naming it with no query is claimed by a \
             generator that cannot write one"
        );
        assert!(prefixes.contains(&super::super::count::NAME));
    }

    /// The remedy names the argument a parameterised generator needs.
    #[test]
    fn the_usage_list_shows_a_parameterised_generator_its_argument() {
        let usages = generator_usages();
        assert!(usages.contains(&"status".to_string()), "{usages:?}");
        assert!(usages.contains(&"count <query>".to_string()), "{usages:?}");
        assert_eq!(usages.len(), generator_names().len());
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
