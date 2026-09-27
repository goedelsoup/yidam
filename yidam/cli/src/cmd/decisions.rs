use anyhow::Result;
use std::path::Path;

use crate::corpus::load_decisions;
use crate::paths::yidam_decisions_dir;
use crate::regen::update_file_regen;

/// The records under `decisions_dir`, read through the corpus model.
///
/// Read there rather than here since #1056: this module's own reader was
/// `serde_yaml::from_str(&text).unwrap_or_default()`, which is the one construction #676 spent
/// itself removing — it turns a record nobody can read into a record that declares nothing, and
/// the log below then renders it with its file stem and an em dash. Going through
/// [`crate::corpus::DecisionRecord`] keeps the parse outcome attached to the record, so
/// `malformed-yaml` has a file to report and this log has one reader rather than a second one.
///
/// The directory is taken rather than a root because both callers hold one already, and `due`
/// resolves it from a path of its own.
fn records(decisions_dir: &Path) -> Vec<crate::corpus::DecisionRecord> {
    // The parent of `.yidam/decisions/`, which is what a repo-relative `rel` is relative to.
    // Only `malformed-yaml` reads it, and it opens the corpus rather than coming through here.
    let root = decisions_dir
        .parent()
        .and_then(Path::parent)
        .unwrap_or(decisions_dir);
    load_decisions(root, decisions_dir)
}

/// The record a corpus named, as a repo-relative path, or `None` when it holds no such one.
///
/// Used where a configuration key points at a decision. A pointer that resolves to nothing
/// is the interesting case, and it is why this returns an `Option` rather than a bool: the
/// caller reports the path it found, so a reader can go and read the argument.
pub(crate) fn find_decision(decisions_dir: &Path, id: &str) -> Option<String> {
    records(decisions_dir)
        .into_iter()
        .find(|d| d.id() == id)
        .map(|d| format!(".yidam/decisions/{}", d.filename()))
}

pub(crate) fn render_decisions_log(decisions_dir: &Path) -> String {
    let decisions = records(decisions_dir);
    if decisions.is_empty() {
        return "_No decisions recorded yet._".to_string();
    }
    let mut rows = vec![
        "| Decision | Summary |".to_string(),
        "|---|---|".to_string(),
    ];
    for d in &decisions {
        let summary = d
            .decision
            .summary
            .clone()
            .unwrap_or_else(|| "—".to_string());
        rows.push(format!("| [{}]({}) | {summary} |", d.id(), d.filename()));
    }
    rows.join("\n")
}

pub fn decisions_log(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let decisions_dir = yidam_decisions_dir(&root);
    let content = render_decisions_log(&decisions_dir);
    crate::regen::emit(&content);
    update_file_regen(
        &decisions_dir.join("README.md"),
        "yidam decisions-log",
        &content,
    )
}
