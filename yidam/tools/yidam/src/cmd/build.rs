use anyhow::Result;
use std::path::PathBuf;

use crate::parse::{extract_json_field, extract_toml_field};
use crate::paths::repo_root;
use crate::regen::update_file_regen;

pub fn crates_index() -> Result<()> {
    let root = repo_root()?;
    let crates_dir = root.join("crates");

    let mut crate_tomls: Vec<PathBuf> = if crates_dir.exists() {
        walkdir::WalkDir::new(&crates_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name() == "Cargo.toml" && e.depth() > 0)
            .map(|e| e.path().to_owned())
            .collect()
    } else {
        vec![]
    };
    crate_tomls.sort();

    let content = if crate_tomls.is_empty() {
        "_No crates yet._".to_string()
    } else {
        let mut rows = vec![
            "| Crate | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &crate_tomls {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let name = extract_toml_field(&text, "name").unwrap_or_else(|| "—".to_string());
            let desc =
                extract_toml_field(&text, "description").unwrap_or_else(|| "—".to_string());
            let dir = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            rows.push(format!("| [{name}]({dir}/) | {desc} |"));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(
        &crates_dir.join("README.md"),
        "yidam crates-index",
        &content,
    )
}

pub fn packages_index() -> Result<()> {
    let root = repo_root()?;
    let packages_dir = root.join("packages");

    let mut manifests: Vec<PathBuf> = if packages_dir.exists() {
        walkdir::WalkDir::new(&packages_dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy();
                e.depth() > 0
                    && (name == "package.json"
                        || name == "pyproject.toml"
                        || name == "Cargo.toml")
            })
            .map(|e| e.path().to_owned())
            .collect()
    } else {
        vec![]
    };
    manifests.sort();

    let content = if manifests.is_empty() {
        "_No packages yet._".to_string()
    } else {
        let mut rows = vec![
            "| Package | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &manifests {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let name = extract_toml_field(&text, "name")
                .or_else(|| extract_json_field(&text, "name"))
                .unwrap_or_else(|| "—".to_string());
            let desc = extract_toml_field(&text, "description")
                .or_else(|| extract_json_field(&text, "description"))
                .unwrap_or_else(|| "—".to_string());
            let dir = path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| name.clone());
            rows.push(format!("| [{name}]({dir}/) | {desc} |"));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(
        &packages_dir.join("README.md"),
        "yidam packages-index",
        &content,
    )
}
