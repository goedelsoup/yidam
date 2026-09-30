use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

/// A REGEN block whose committed content is not what its generator produces.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Stale {
    /// The file holding the block, as its generator opened it.
    ///
    /// Which means it carries whatever `--root` was spelled as: `README.md` run from inside
    /// the repository, `../other/README.md` from outside it. [`Unclaimed::file`] is always
    /// repository-relative, and says why.
    pub file: String,
    /// The generator whose block it is — the `<command>` in `<!-- REGEN: <command> -->`.
    pub generator: String,
}

/// A REGEN block whose command names no generator this binary has.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Unclaimed {
    /// Repository-relative path of the file holding the block.
    ///
    /// Always relative, which [`Stale::file`] is not. That one is the path its generator was
    /// handed; this one comes from `git ls-files --full-name`, which answers about the
    /// repository rather than about the directory the command was run from. The envelope's
    /// `root` names what it is relative to.
    pub file: String,
    /// The `<name>` in `<!-- REGEN: yidam <name> -->`, with the `yidam ` stripped.
    pub generator: String,
}

/// The `yidam ` commands in `text` that name no generator in `known`.
///
/// **Nothing else asks this question, and that is the defect.** Every other part of the
/// contract is *pushed*: a generator names a file and a command, [`update_regen`] finds that
/// command's open tag or returns the text unchanged, and [`record`] logs the block when the
/// two disagree. A block whose command no generator carries is never anybody's tag, so it is
/// never found, never recorded, and never written — it keeps its genesis placeholder through
/// every `yidam regen` and every gate. Measured on the reporting fixture at `3111beb`: a
/// `<!-- REGEN: yidam statsu -->` block survived `yidam regen` unchanged and
/// `yidam regen --check` printed *"Every REGEN block is current."* So this is the one thing
/// in the contract that has to be *pulled* — read off the document rather than off the list.
///
/// **Only the `yidam ` prefix, because the marker namespace is not yidam's.** The name after
/// `<!-- REGEN:` is whatever program writes the block, and a derived repository's own binary
/// writes its own: `ohio-education-funding` carries fourteen blocks under thirteen distinct
/// `edfund-connect <name>` commands — `repository-overview`, `claim-totals`,
/// `connector-registry` and ten more — none of which is a yidam generator and every one of
/// which is refreshed by a command yidam has never heard of. Judging those would report a
/// working repository as broken, with no remedy yidam could print that would be true.
/// `yidam` answers for its own prefix and for nothing else, which is also why the prefix is
/// load-bearing enough to be worth stating: every `update_file_regen` call site in this crate
/// passes `"yidam <name>"`.
///
/// **Masked first.** A document explaining the marker syntax shows blocks it does not own,
/// and [`crate::markdown::mask_code`] is this repository's one answer to *shown versus said*.
/// [`crate::cmd::lint::checks::malformed_regen_block`] is a `Warn` rather than an `Error`
/// precisely because it does not mask and so cannot tell the two apart; this is a gate, so it
/// masks.
///
/// Measured across the 22 repositories on disk carrying REGEN blocks, 2026-09-27: 235 blocks
/// open with `<!-- REGEN: yidam ` at the start of a line, of which 20 sit inside a code
/// fence and 215 are scanned. The 20 are one document — `yidam/prelude/sdks/README.md`,
/// which explains the marker format and is vendored into every derived repository — and it
/// shows `corpus-index`, a real generator, so masking changes no verdict today. It is here
/// for the day somebody documents the syntax with a name that is not.
///
/// The same run says the gate reddens nothing on adoption: **zero** of those 215 blocks names
/// a generator that does not exist.
///
/// [`yidam_core::markers::parse_markers`] and not a reader of its own. A second scanner would
/// be a second answer to *where does a block begin*, which is the argument
/// `malformed_regen_block` already makes one module over.
///
/// **Two ways to be claimed, because since RFC-0043 there are two shapes of command.**
/// `known` holds the generators whose command is their name and nothing else, matched whole.
/// `parameterised` holds the ones whose command carries an argument — `count` — matched as a
/// name followed by a **space** and something. The separator is the whole of the rule: without
/// it `yidam counterexamples` would be claimed by `count`, and `yidam status-quo` by `status`,
/// which is the prefix-versus-equality confusion #1094 removed from `update_regen`.
///
/// A parameterised generator's **bare** name is claimed by neither, deliberately. `count`
/// exists, and no run of it can ever write `<!-- REGEN: yidam count -->`, because that block
/// names no query. Reporting it is this gate's entire purpose; the caller keeps the bare name
/// out of `known` — see `regen::claimable` — so that registering a parameterised generator
/// cannot turn a reported block into a silent one.
pub fn unclaimed_in(text: &str, known: &[&str], parameterised: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for marker in yidam_core::markers::parse_markers(&crate::markdown::mask_code(text)) {
        let yidam_core::markers::Marker::Regen { command, .. } = marker else {
            continue;
        };
        let Some(name) = command.strip_prefix("yidam ") else {
            continue;
        };
        let claimed = known.contains(&name)
            || parameterised.iter().any(|p| {
                name.strip_prefix(p)
                    .is_some_and(|rest| rest.starts_with(' '))
            });
        // Deduped the way `record` dedupes: two blocks in one file naming the same missing
        // generator are one thing to fix.
        if claimed || found.iter().any(|f| f == name) {
            continue;
        }
        found.push(name.to_string());
    }
    found
}

/// Check mode, and what it found.
///
/// A process-global rather than a parameter, and that is a deliberate trade. Every
/// generator is a `fn() -> Result<()>` invoked from one list; threading a mode through ten
/// of them would put an argument on ten public commands so that one of them could be asked
/// a different question. `update_file_regen` is the single write point in this crate, and
/// the flag is read there and nowhere else.
///
/// `None` is the normal mode: write. `Some(_)` is check mode: record and write nothing.
static CHECK: Mutex<Option<Vec<Stale>>> = Mutex::new(None);

/// Enter check mode. Returns what the generators found when [`end_check`] is called.
pub fn begin_check() {
    *CHECK.lock().expect("regen check lock") = Some(Vec::new());
}

pub fn end_check() -> Vec<Stale> {
    CHECK
        .lock()
        .expect("regen check lock")
        .take()
        .unwrap_or_default()
}

/// Whether a generator should print its content.
///
/// In check mode it must not: the generators print their block to stdout, and thirty lines
/// of corpus index in front of a JSON report is not a JSON report.
pub fn checking() -> bool {
    CHECK.lock().expect("regen check lock").is_some()
}

/// Write mode with nothing said: the generators write their blocks and print neither the
/// content nor the *updated* line.
///
/// For a command that refreshes the blocks on the way to its own report — `phase settle`
/// (#1066) — whose `--format json` has to stay one JSON document. Same trade as [`CHECK`]
/// and for the same reason: the alternative is a parameter on every generator so that one
/// caller can ask them to be quiet.
static QUIET: AtomicBool = AtomicBool::new(false);

/// Run the generators silently until [`end_quiet`].
pub fn begin_quiet() {
    QUIET.store(true, Ordering::SeqCst);
}

pub fn end_quiet() {
    QUIET.store(false, Ordering::SeqCst);
}

fn quiet() -> bool {
    QUIET.load(Ordering::SeqCst)
}

/// Print a generator's rendered block, unless we are only checking — or refreshing quietly.
///
/// Every generator calls this instead of `println!` for exactly one reason: `--check` has to
/// be able to emit a report on the same stream.
pub fn emit(content: &str) {
    if !checking() && !quiet() {
        println!("{content}");
    }
}

fn record(path: &Path, command: &str) {
    let mut guard = CHECK.lock().expect("regen check lock");
    let Some(found) = guard.as_mut() else { return };
    let file = repo_relative(path);
    let generator = command
        .strip_prefix("yidam ")
        .unwrap_or(command)
        .to_string();
    let stale = Stale { file, generator };
    if !found.contains(&stale) {
        found.push(stale);
    }
}

/// Best-effort repository-relative path, for a report a person reads.
fn repo_relative(path: &Path) -> String {
    let absolute: PathBuf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    crate::paths::repo_root()
        .ok()
        .and_then(|root| absolute.strip_prefix(&root).ok().map(|p| p.to_path_buf()))
        .unwrap_or(absolute)
        .to_string_lossy()
        .replace('\\', "/")
}

// The one implementation, and it is not here.
//
// This module carried a fourth copy of `update_regen` — the SDKs have three, kept in step by
// the parity fixtures — and it disagreed with them. Given empty content it wrote a blank line
// between the markers; `yidam_core`'s collapses it, which is what
// `parity/fixtures/update_regen/empty-new-content.toml` requires of all three and what
// `graph.dfy`'s `ClearingASectionLeavesNoBlankLine` proves of the model.
//
// Nothing compared them. The parity surface grades the three SDKs against each other, and
// this copy is not an SDK, so it sat outside the comparison whose entire purpose is that
// there are exactly three answers. Its three unit tests never passed an empty string.
//
// The CLI already depends on `yidam-core` for `ontology`, `git` and `corpus`. This removes an
// implementation rather than adding a dependency.
pub use yidam_core::markers::update_regen;

pub fn update_file_regen(path: &Path, command: &str, new_content: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let updated = update_regen(&original, command, new_content);
    if updated == original {
        return Ok(());
    }
    // The single write point, which is what makes `--check` a flag rather than a second
    // implementation of the same ten generators.
    if checking() {
        record(path, command);
        return Ok(());
    }
    std::fs::write(path, &updated).with_context(|| format!("writing {}", path.display()))?;
    if !quiet() {
        println!("  updated {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_regen_basic() {
        let input = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
_Run `yidam status` to populate._\n\
<!-- /REGEN -->\n";
        let expected = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
**12 nodes** · 3 open · index fresh\n\
<!-- /REGEN -->\n";
        assert_eq!(
            update_regen(input, "yidam status", "**12 nodes** · 3 open · index fresh"),
            expected
        );
    }

    #[test]
    fn missing_marker_is_noop() {
        let input = "# No REGEN here\n";
        assert_eq!(update_regen(input, "yidam status", "new content"), input);
    }

    /// Clearing a section leaves the two markers on consecutive lines.
    ///
    /// The case that was never covered here, and the reason a fourth implementation could
    /// disagree with the contract for as long as it existed: the three tests above pass an
    /// empty string to nothing.
    ///
    /// Read out of the parity fixture rather than restated. A fourth copy of the expected
    /// output is exactly what this change removes, and writing one here would put it back a
    /// file over.
    #[test]
    fn clearing_a_section_matches_the_parity_fixture() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../prelude/sdks/parity/fixtures/update_regen/empty-new-content.toml");
        let raw = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
        // `toml::from_str`, not `raw.parse()`. toml 1.1 repointed `FromStr for Value` at
        // its *value* parser (`ValueDeserializer::parse`) — 0.8's went to `from_str`, the
        // document parser — so `.parse()` on a whole fixture now fails at the first key.
        // `FromStr for Table` is still the document parser, which is why the one other
        // `.parse()` in this repository (`task_layer.rs`) was unaffected.
        let fx: toml::Value = toml::from_str(&raw).expect("the fixture parses as TOML");

        let new_content = fx["input"]["new_content"].as_str().expect("new_content");
        assert!(
            new_content.is_empty(),
            "this test is about the empty case; the fixture now passes {new_content:?}"
        );
        assert_eq!(
            update_regen(
                fx["input"]["content"].as_str().expect("content"),
                fx["input"]["command"].as_str().expect("command"),
                new_content,
            ),
            fx["expected"]["content"]
                .as_str()
                .expect("expected.content"),
            "the CLI writes a different document than the three SDKs are held to"
        );
    }

    /// The generator names this crate ships, as the scanner's callers pass them.
    const KNOWN: &[&str] = &["status", "corpus-index", "vault-status"];

    /// The generators whose command carries an argument, as `regen::claimable` splits them.
    ///
    /// `count` is in this list and **not** in [`KNOWN`], which is the partition the caller
    /// makes and the reason `a_parameterised_generator_needs_its_argument` has an arm for the
    /// bare name.
    const PARAMETERISED: &[&str] = &["count"];

    fn block(command: &str) -> String {
        format!("<!-- REGEN: {command}\n-->\n_placeholder_\n<!-- /REGEN -->\n")
    }

    /// A name outside the list is reported; a name inside it is not.
    ///
    /// Both arms, because a scanner that has stopped matching REGEN open tags at all passes
    /// the second one on its own.
    #[test]
    fn a_name_no_generator_carries_is_reported() {
        assert_eq!(
            unclaimed_in(&block("yidam statsu"), KNOWN, PARAMETERISED),
            ["statsu"]
        );
        assert!(unclaimed_in(&block("yidam status"), KNOWN, PARAMETERISED).is_empty());
    }

    /// `update_regen` matches its open tag by **prefix**, so a name that merely extends a
    /// real one is the worse half of this defect: `yidam status-quo` sitting above the real
    /// block is found by `status`'s own write and filled with `status`'s content. Either way
    /// nobody declared it.
    #[test]
    fn a_name_that_extends_a_real_one_is_not_that_generator() {
        assert_eq!(
            unclaimed_in(&block("yidam status-quo"), KNOWN, PARAMETERISED),
            ["status-quo"]
        );
    }

    /// Another program's blocks are not yidam's to judge.
    ///
    /// `ohio-education-funding` writes twelve of these with its own binary. Reporting them
    /// would call a working repository broken, and no remedy yidam could print would be true.
    #[test]
    fn a_block_belonging_to_another_program_is_left_alone() {
        assert!(
            unclaimed_in(&block("edfund-connect claim-totals"), KNOWN, PARAMETERISED).is_empty()
        );
        assert!(unclaimed_in(&block("edfund-connect status"), KNOWN, PARAMETERISED).is_empty());
    }

    /// A block a document *shows* is not a block the repository *has*.
    #[test]
    fn a_shown_block_is_not_a_said_one() {
        let fenced = format!(
            "Like so:\n\n```markdown\n{}```\n",
            block("yidam demo-index")
        );
        assert!(
            unclaimed_in(&fenced, KNOWN, PARAMETERISED).is_empty(),
            "{fenced}"
        );

        let spanned = "The `<!-- REGEN: yidam demo-index -->` marker is filled by nothing.\n";
        assert!(unclaimed_in(spanned, KNOWN, PARAMETERISED).is_empty());
    }

    /// One thing to fix is reported once, and two are reported twice.
    ///
    /// The second half is the one that matters: a dedupe keyed on the file rather than the
    /// name would collapse two different mistakes into one line.
    #[test]
    fn repeats_collapse_and_distinct_names_do_not() {
        let text = block("yidam statsu") + &block("yidam statsu") + &block("yidam corpus-idx");
        assert_eq!(
            unclaimed_in(&text, KNOWN, PARAMETERISED),
            ["statsu", "corpus-idx"]
        );
    }

    /// A malformed block is still a block nothing writes.
    ///
    /// `malformed-regen-block` reports the damage; the name is this scanner's business, and
    /// a block that is both damaged and misnamed must not fall between them.
    #[test]
    fn a_malformed_block_is_still_scanned_for_its_name() {
        let no_close = "<!-- REGEN: yidam statsu\n-->\n_placeholder_\n";
        assert_eq!(unclaimed_in(no_close, KNOWN, PARAMETERISED), ["statsu"]);
    }

    /// A parameterised generator claims its name **with** an argument and not without one.
    ///
    /// Four arms, and each is a different way for the rule to be wrong. Matching the bare
    /// name would silence a block no run can ever write. Not matching the argument form would
    /// report every `count` block in the corpus. Matching without the separator would give
    /// `count` a block that says `counterexamples`. And an argument form whose *name* is not
    /// parameterised is still nobody's.
    #[test]
    fn a_parameterised_generator_needs_its_argument() {
        // No query: a real generator, and no run of it writes this block.
        assert_eq!(
            unclaimed_in(&block("yidam count"), KNOWN, PARAMETERISED),
            ["count"]
        );
        // A query: claimed.
        assert!(unclaimed_in(&block("yidam count district"), KNOWN, PARAMETERISED).is_empty());
        assert!(unclaimed_in(
            &block("yidam count district[party=R] -has-> person"),
            KNOWN,
            PARAMETERISED
        )
        .is_empty());
        // The separator is the rule: no space, no claim.
        assert_eq!(
            unclaimed_in(&block("yidam counterexamples"), KNOWN, PARAMETERISED),
            ["counterexamples"]
        );
        // An argument on a name that takes none is still unclaimed, under its whole command.
        assert_eq!(
            unclaimed_in(&block("yidam status please"), KNOWN, PARAMETERISED),
            ["status please"]
        );
    }

    /// An empty `parameterised` changes nothing.
    ///
    /// The regression arm for every caller that has no parameterised generator — and the one
    /// that fails if the prefix test is ever written so that an empty prefix matches, which
    /// would claim every block in every document.
    #[test]
    fn no_parameterised_generators_is_the_old_behaviour() {
        assert_eq!(unclaimed_in(&block("yidam count"), KNOWN, &[]), ["count"]);
        assert_eq!(
            unclaimed_in(&block("yidam count district"), KNOWN, &[]),
            ["count district"]
        );
        assert!(unclaimed_in(&block("yidam status"), KNOWN, &[]).is_empty());
    }

    #[test]
    fn idempotent() {
        let input = "<!-- REGEN: yidam status\n-->\ncontent\n<!-- /REGEN -->\n";
        let once = update_regen(input, "yidam status", "content");
        let twice = update_regen(&once, "yidam status", "content");
        assert_eq!(once, twice);
    }
}
