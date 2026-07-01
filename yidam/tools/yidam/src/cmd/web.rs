use anyhow::Result;

use crate::paths::repo_root;
use crate::regen::update_file_regen;

pub fn bundle_status() -> Result<()> {
    let root = repo_root()?;
    let web_dir = root.join("web");
    let bundle_dir = web_dir.join("bundle");

    let content = if bundle_dir.exists() {
        let feeds: Vec<_> = std::fs::read_dir(&bundle_dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
            .collect();
        if feeds.is_empty() {
            "Bundle directory present — no feed files found.".to_string()
        } else {
            let names: Vec<_> = feeds
                .iter()
                .filter_map(|e| e.file_name().into_string().ok())
                .map(|n| format!("`{n}`"))
                .collect();
            format!("{} feed(s): {}", feeds.len(), names.join(", "))
        }
    } else {
        "_Web bundle not initialized._".to_string()
    };

    println!("{content}");
    update_file_regen(&web_dir.join("README.md"), "yidam bundle-status", &content)
}
