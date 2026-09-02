use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// A REGEN block whose committed content is not what its generator produces.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Stale {
    /// Repository-relative path of the file holding the block.
    pub file: String,
    /// The generator whose block it is — the `<command>` in `<!-- REGEN: <command> -->`.
    pub generator: String,
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

/// Print a generator's rendered block, unless we are only checking.
///
/// Every generator calls this instead of `println!` for exactly one reason: `--check` has to
/// be able to emit a report on the same stream.
pub fn emit(content: &str) {
    if !checking() {
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
    println!("  updated {}", path.display());
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
        let fx: toml::Value = raw.parse().expect("the fixture parses as TOML");

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

    #[test]
    fn idempotent() {
        let input = "<!-- REGEN: yidam status\n-->\ncontent\n<!-- /REGEN -->\n";
        let once = update_regen(input, "yidam status", "content");
        let twice = update_regen(&once, "yidam status", "content");
        assert_eq!(once, twice);
    }
}
