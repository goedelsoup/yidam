use anyhow::Result;
use std::path::Path;

use crate::parse::Decision;
use crate::paths::{repo_root, yidam_decisions_dir};
use crate::regen::update_file_regen;
use crate::walk::walk_decision_files;

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
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let d: Decision = serde_yaml::from_str(&text).unwrap_or_default();
        let id = d.id.unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        let summary = d.summary.unwrap_or_else(|| "—".to_string());
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
