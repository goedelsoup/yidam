//! `yidam count` — a figure in a sentence, computed rather than remembered.
//!
//! A number that states a fact about the corpus is published twice: in the graph, where it is
//! computed, and in a sentence, where somebody typed it. The typed copy drifts, and three
//! derived repositories independently built a gate against it. None of those gates could reach
//! a number in the *middle* of a sentence, because a REGEN block was line-shaped — which
//! RFC-0043's first half fixed, and which this half makes worth having.
//!
//! Two forms, and the bare one is the generator:
//!
//! - `yidam count <query>` answers the query with a number. Reads.
//! - `yidam count` refreshes every `<!-- REGEN: yidam count <query> -->` block in the tracked
//!   tree. Writes, which is why `--help` marks it, and is what `yidam regen` runs.

use anyhow::{bail, Context, Result};
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use crate::cmd::query;

/// The literal every `count` block's command begins with, argument excluded.
///
/// One spelling, read by two things that would otherwise each carry their own: the block scan
/// below, and [`crate::cmd::regen::PARAMETERISED`], which tells `unclaimed_in` that a command
/// of this shape names a generator that exists.
///
/// The trailing space is load-bearing. Without it `yidam counterexamples` reads as a `count`
/// block whose query is `erexamples` — the prefix-versus-equality confusion this RFC's first
/// half spent itself removing from `update_regen`.
pub const COMMAND_PREFIX: &str = "yidam count ";

/// The generator's name, as [`crate::cmd::regen::GENERATORS`] and `--help` spell it.
pub const NAME: &str = "count";

/// What `yidam count <query>` answers.
#[derive(serde::Serialize)]
pub struct CountReport {
    /// The query, echoed. No `kind` discriminator beside it: not one of the hundred-odd
    /// fields `report.schema.json` declares is a shape tag, and a lone one here would be a
    /// convention of exactly one report.
    pub query: String,
    /// How many nodes satisfied the query's final step, or null when it was rejected.
    ///
    /// Null rather than `0`, for the reason the refusal below gives: a query that could not
    /// run has not counted zero of anything, and a consumer that could not tell the two apart
    /// would read a diagnosis as a measurement.
    pub matched: Option<usize>,
    /// Present and null when the query ran, so a consumer testing the key does not have to
    /// distinguish "accepted" from "a binary too old to say".
    pub rejected: Option<query::check::Rejection>,
}

/// Run one query: how many nodes satisfied its final step, or why it could not run.
///
/// `Result` rather than the report's two `Option`s, because exactly one of the two is always
/// present and a pair of `Option`s cannot say so. [`block`] reads the answer per block, and a
/// shape admitting *neither* would leave it with a fourth case it can only assert its way out
/// of — which is a panic path in a command whose whole job is to not publish a number it
/// cannot stand behind.
///
/// `limit: 0` asks for no projection at all. [`query::QueryReport::matched`] is the full count
/// rather than the projected page — the traversal is what produces it, and `--limit` bounds
/// only what is shown — so nothing is lost by projecting nothing, and the rows are not built.
fn answer(root: &Path, text: &str) -> std::result::Result<usize, query::check::Rejection> {
    let report = query::run(
        root,
        text,
        &query::Options {
            limit: 0,
            ..Default::default()
        },
    );
    match report.rejected {
        Some(rejection) => Err(rejection),
        None => Ok(report.matched),
    }
}

/// The same answer, in the shape `--format json` publishes.
fn run_one(root: &Path, text: &str) -> CountReport {
    let (matched, rejected) = match answer(root, text) {
        Ok(n) => (Some(n), None),
        Err(rejection) => (None, Some(rejection)),
    };
    CountReport {
        query: text.to_string(),
        matched,
        rejected,
    }
}

/// `yidam count [query]` — the number, or the refresh.
///
/// `--format` describes the answer to a query. The refresh form writes blocks and reports what
/// it updated, as every generator does.
pub fn count(
    root: Option<&Path>,
    query: Option<&str>,
    format: crate::report::Format,
) -> Result<()> {
    let Some(text) = query else {
        return block(root);
    };
    let root = crate::paths::resolve_root(root)?;
    let report = run_one(&root, text);
    let passed = report.rejected.is_none();
    crate::report::gate(&root, format, report, passed, |r| match &r.rejected {
        // Printed, not propagated — the shape `query` states at length. A rejection travelling
        // as an `Error` would land on stderr above an empty stdout, which a `--format json`
        // consumer cannot tell from a crash.
        Some(rejection) => println!("{}: {}", rejection.code, rejection.message),
        None => println!("{}", r.matched.unwrap_or(0)),
    })
}

/// One block this generator answers for.
struct Block {
    /// Repository-relative, as `tracked::list` gives it.
    file: String,
    /// The command's argument, **verbatim** — `command[COMMAND_PREFIX.len()..]`, untrimmed.
    ///
    /// Untrimmed because the write below rebuilds the command from it, and a rebuild that
    /// normalised whitespace would name a command no block carries. Since #1094 `update_regen`
    /// matches a command by equality, so a near miss is not a near miss: it is a silent no-op,
    /// and `--check` would then report a block `regen` had just declined to write.
    /// `the_rebuilt_command_is_the_one_the_block_carries` is the assertion.
    argument: String,
    /// The query to run — the argument with its edges trimmed.
    query: String,
}

impl Block {
    /// The command as the document spells it, built from [`COMMAND_PREFIX`].
    ///
    /// `#[cfg(test)]` because it is an oracle, not the writer. [`block`] rebuilds the command
    /// with an inline `format!` instead, so that `generators_in_source` finds a literal at the
    /// call site and can attribute it — and the test below is what holds the two spellings of
    /// the same prefix to each other.
    #[cfg(test)]
    fn command(&self) -> String {
        format!("{COMMAND_PREFIX}{}", self.argument)
    }
}

/// Every `count` block in the tracked markdown set.
///
/// **The tracked set, and markdown within it** — [`crate::cmd::tracked::list`], for the reason
/// [`crate::cmd::regen::unclaimed_blocks`] gives at length. This generator discovers its own
/// blocks rather than writing at a path it knows, which is what makes it the only one whose
/// command is not a constant: the *document* names the query.
fn blocks(root: &Path) -> Result<Vec<Block>> {
    let mut found = Vec::new();
    for rel in super::tracked::list(root)? {
        if !rel.ends_with(".md") {
            continue;
        }
        let path = root.join(&rel);
        // Tracked and not in the worktree — a deleted file staged but not committed. There is
        // nothing to read and nothing to say about it.
        if !path.exists() {
            continue;
        }
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {rel}"))?;
        found.extend(
            in_document(&text)
                .into_iter()
                .map(|(argument, query)| Block {
                    file: rel.clone(),
                    argument,
                    query,
                }),
        );
    }
    Ok(found)
}

/// The `(argument, query)` of every `count` block in one document.
///
/// Split out from [`blocks`] so the scan is testable without a git repository — the walk is
/// `tracked::list`'s contract and is tested there.
///
/// **Masked first.** A document explaining the marker syntax shows blocks it does not own, and
/// [`crate::markdown::mask_code`] is this repository's one answer to *shown versus said*. Both
/// `docs/rfcs/0043-inline-regen-and-count.md` and `docs/upgrading.md` show a `count` block, and
/// regenerating a shown example would rewrite prose *about* the tool into output *of* it.
/// `unclaimed_in` masks for the same reason and would otherwise disagree with this scan about
/// which blocks exist.
///
/// A bare `<!-- REGEN: yidam count -->` is not one of these. The scanner trims the command, so
/// such a block's command is `yidam count` — it does not carry [`COMMAND_PREFIX`] at all, and
/// naming no query it is `unclaimed`'s to report rather than this generator's to guess at.
fn in_document(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for marker in yidam_core::markers::parse_markers(&crate::markdown::mask_code(text)) {
        let yidam_core::markers::Marker::Regen { command, .. } = marker else {
            continue;
        };
        let Some(argument) = command.strip_prefix(COMMAND_PREFIX) else {
            continue;
        };
        found.push((argument.to_string(), argument.trim().to_string()));
    }
    found
}

/// Refresh every `count` block in the repository.
///
/// The generator behind [`crate::cmd::regen::GENERATORS`]' `count` entry, so `yidam regen`
/// writes these blocks and `yidam regen --check` reports them stale through the same single
/// write point as the other fourteen.
///
/// **A query that does not typecheck refuses its block and fails the command.** It does not
/// write a `0`: a query that cannot run has not counted zero of anything, and a gate whose
/// failure mode is to publish a plausible number is worse than no gate. The block keeps
/// whatever it holds, and this says which block and why. Every block is visited before the
/// failure, so one bad query does not hide the next.
pub fn block(root: Option<&Path>) -> Result<()> {
    let resolved = crate::paths::resolve_root(root)?;
    let blocks = blocks(&resolved)?;
    if blocks.is_empty() {
        return Ok(());
    }

    // Each distinct query once, however many blocks publish it. Two sentences citing the same
    // figure are the case this exists for — *"44 districts … of those 44 districts"* — and
    // running the traversal twice for them would make the generator's cost a function of how
    // often a corpus repeats itself.
    let mut answers: BTreeMap<&str, std::result::Result<usize, query::check::Rejection>> =
        BTreeMap::new();
    for b in &blocks {
        answers
            .entry(b.query.as_str())
            .or_insert_with(|| answer(&resolved, &b.query));
    }

    // `(block, rejection)`, so the message below reads the diagnosis it was handed rather than
    // looking it up again and having to assert that it is there.
    let mut refused: Vec<(&Block, &query::check::Rejection)> = Vec::new();
    for b in &blocks {
        let n = match &answers[b.query.as_str()] {
            Ok(n) => *n,
            Err(rejection) => {
                refused.push((b, rejection));
                continue;
            }
        };
        // The command is rebuilt here rather than carried, so that the literal a reader — and
        // `generators_in_source` — needs is at the call site. A generator whose command is
        // computed is one that guard cannot otherwise attribute to a name; spelling the prefix
        // keeps this site inside the scan rather than exempt from it.
        crate::regen::update_file_regen(
            &resolved.join(&b.file),
            &format!("yidam count {}", b.argument),
            &n.to_string(),
        )?;
    }

    if refused.is_empty() {
        return Ok(());
    }
    let mut detail = String::new();
    for (b, rejection) in &refused {
        let _ = writeln!(
            detail,
            "  {}  ({})\n    {}: {}",
            b.file, b.query, rejection.code, rejection.message
        );
    }
    bail!(
        "{} count block(s) name a query that does not typecheck:\n{}\n\n\
         Those blocks keep whatever they hold — a query that cannot run has not counted zero \
         of anything. Correct the query, or delete the block.",
        refused.len(),
        detail.trim_end()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Since #1094 `update_regen` matches a command by equality, so the command this rebuilds
    /// has to be the one the scan read — byte for byte, whitespace included. A rebuild that
    /// normalised would make the write a silent no-op, and `--check` would then report the
    /// block the generator had just declined to write.
    #[test]
    fn the_rebuilt_command_is_the_one_the_block_carries() {
        // Two spaces after the command: nothing anybody writes on purpose, and exactly what a
        // normalising rebuild would quietly repair into a command no block has.
        let text = "a <!-- REGEN: yidam count  district -->1<!-- /REGEN --> b\n";
        let scanned: Vec<String> = yidam_core::markers::scan_markers(text)
            .regen
            .into_iter()
            .map(|s| s.command)
            .collect();
        assert_eq!(scanned, ["yidam count  district"]);

        let found = in_document(text);
        assert_eq!(found, [(" district".to_string(), "district".to_string())]);
        let b = Block {
            file: "x.md".into(),
            argument: found[0].0.clone(),
            query: found[0].1.clone(),
        };
        assert_eq!(b.command(), scanned[0]);
        // The scan reads `COMMAND_PREFIX`; the write spells the same prefix inline, because
        // the guard needs a literal there. Two spellings of one string, held to each other.
        assert_eq!(format!("yidam count {}", b.argument), scanned[0]);
        assert_eq!(format!("yidam count {}", ""), COMMAND_PREFIX);
    }

    #[test]
    fn a_block_shown_in_a_fence_or_a_span_is_not_a_block() {
        let fenced = "```md\n<!-- REGEN: yidam count district -->44<!-- /REGEN -->\n```\n";
        assert!(in_document(fenced).is_empty(), "{fenced}");
        let spanned =
            "See `<!-- REGEN: yidam count district -->44<!-- /REGEN -->` for the shape.\n";
        assert!(in_document(spanned).is_empty(), "{spanned}");
    }

    /// The trailing space in [`COMMAND_PREFIX`] is what stops a longer command reading as this
    /// one. `update_regen` learned the same lesson in #1094.
    #[test]
    fn a_command_that_merely_begins_with_count_is_not_one() {
        let text = "<!-- REGEN: yidam counterexamples -->\n1\n<!-- /REGEN -->\n";
        assert!(in_document(text).is_empty(), "{text}");
    }

    /// A `count` block naming no query is not this generator's, in either marker form.
    ///
    /// The scan trims the command, so trailing spaces cannot make one: both of these carry the
    /// command `yidam count`, which lacks the prefix. That is what leaves them to `unclaimed`,
    /// and why [`crate::cmd::regen::PARAMETERISED`] has to keep the bare name out of the set
    /// `unclaimed_in` matches whole — otherwise registering this generator would turn a block
    /// nothing writes into a block nothing reports.
    #[test]
    fn a_block_with_no_query_is_left_to_unclaimed() {
        for text in [
            "<!-- REGEN: yidam count -->\n1\n<!-- /REGEN -->\n",
            "<!-- REGEN: yidam count   -->\n1\n<!-- /REGEN -->\n",
            "x <!-- REGEN: yidam count -->1<!-- /REGEN --> y\n",
        ] {
            assert!(in_document(text).is_empty(), "{text}");
        }
    }

    #[test]
    fn an_inline_block_and_a_block_form_one_are_both_found() {
        let text = "Ohio has <!-- REGEN: yidam count district -->43<!-- /REGEN --> districts.\n\
                    \n\
                    <!-- REGEN: yidam count person -->\n2\n<!-- /REGEN -->\n";
        assert_eq!(
            in_document(text),
            [
                ("district".to_string(), "district".to_string()),
                ("person".to_string(), "person".to_string()),
            ]
        );
    }
}
