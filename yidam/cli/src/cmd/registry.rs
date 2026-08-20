use anyhow::Result;
use std::path::Path;

use crate::parse::parse_frontmatter;
use crate::paths::{repo_root, yidam_skills_dir};
use crate::regen::update_file_regen;
use crate::walk::walk_md_files;

pub(crate) fn render_skills_index(skills_dir: &Path) -> String {
    let skills = walk_md_files(skills_dir);
    if skills.is_empty() {
        return "_No domain-specific skills yet._".to_string();
    }
    let mut rows = vec![
        "| Skill | Description |".to_string(),
        "|---|---|".to_string(),
    ];
    for path in &skills {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let fm = parse_frontmatter(&text);
        let name = fm.name.unwrap_or_else(|| {
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
        let desc = fm.description.unwrap_or_else(|| "—".to_string());
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        rows.push(format!("| [{name}]({filename}) | {desc} |"));
    }
    rows.join("\n")
}

pub fn agents_index() -> Result<()> {
    let root = repo_root()?;
    let agents_dir = root.join("agents");
    let agents = walk_md_files(&agents_dir);

    let content = if agents.is_empty() {
        "_No domain-specific agents yet._".to_string()
    } else {
        let mut rows = vec![
            "| Agent | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &agents {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let name = fm.name.unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            rows.push(format!("| [{name}]({filename}) | {desc} |"));
        }
        rows.join("\n")
    };

    crate::regen::emit(&content);
    update_file_regen(
        &agents_dir.join("README.md"),
        "yidam agents-index",
        &content,
    )
}

pub fn skills_index() -> Result<()> {
    let root = repo_root()?;
    let skills_dir = yidam_skills_dir(&root);
    let content = render_skills_index(&skills_dir);
    crate::regen::emit(&content);
    update_file_regen(
        &skills_dir.join("README.md"),
        "yidam skills-index",
        &content,
    )
}
