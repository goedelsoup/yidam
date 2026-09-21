use anyhow::Result;
use std::path::Path;

use crate::parse::Decision;
use crate::paths::{repo_root, yidam_decisions_dir};
use crate::regen::update_file_regen;
use crate::walk::walk_decision_files;

/// How a decision record is identified: its `id:`, or its file stem when it declares none.
///
/// Both forms are accepted because both are what a reader would write. The fallback is the
/// log's own, and shared rather than restated — a second rule for naming one record is a
/// second answer to *which record is this*.
fn decision_id(path: &Path, d: &Decision) -> String {
    d.id.clone().unwrap_or_else(|| {
        path.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    })
}

fn read_decision(path: &Path) -> Decision {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    serde_yaml::from_str(&text).unwrap_or_default()
}

/// The record a corpus named, as a repo-relative path, or `None` when it holds no such one.
///
/// Used where a configuration key points at a decision. A pointer that resolves to nothing
/// is the interesting case, and it is why this returns an `Option` rather than a bool: the
/// caller reports the path it found, so a reader can go and read the argument.
pub(crate) fn find_decision(decisions_dir: &Path, id: &str) -> Option<String> {
    walk_decision_files(decisions_dir)
        .into_iter()
        .find(|p| decision_id(p, &read_decision(p)) == id)
        .map(|p| {
            format!(
                ".yidam/decisions/{}",
                p.file_name().unwrap_or_default().to_string_lossy()
            )
        })
}

pub(crate) fn render_decisions_log(decisions_dir: &Path) -> String {
    let decision_files = walk_decision_files(decisions_dir);
    if decision_files.is_empty() {
        return "_No decisions recorded yet._".to_string();
    }
    let mut rows = vec![
        "| Decision | Summary |".to_string(),
        "|---|---|".to_string(),
    ];
    for path in &decision_files {
        let d = read_decision(path);
        let id = decision_id(path, &d);
        let summary = d.summary.clone().unwrap_or_else(|| "—".to_string());
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        rows.push(format!("| [{id}]({filename}) | {summary} |"));
    }
    rows.join("\n")
}

pub fn decisions_log() -> Result<()> {
    let root = repo_root()?;
    let decisions_dir = yidam_decisions_dir(&root);
    let content = render_decisions_log(&decisions_dir);
    crate::regen::emit(&content);
    update_file_regen(
        &decisions_dir.join("README.md"),
        "yidam decisions-log",
        &content,
    )
}
