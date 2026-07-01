use anyhow::Result;
use std::path::PathBuf;

use crate::parse::parse_frontmatter;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::walk_md_files;

pub fn catalog_audit() -> Result<()> {
    let root = repo_root()?;
    let catalog = yidam_catalog_dir(&root);
    let entries = walk_md_files(&catalog);

    let content = if entries.is_empty() {
        "_No catalog entries yet._".to_string()
    } else {
        let corpus = yidam_corpus_dir(&root);
        let corpus_files: Vec<PathBuf> = if corpus.exists() {
            walkdir::WalkDir::new(&corpus)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| {
                    e.file_type().is_file()
                        && e.path()
                            .extension()
                            .is_some_and(|x| x == "md" || x == "yml")
                })
                .map(|e| e.path().to_owned())
                .collect()
        } else {
            vec![]
        };

        let mut rows = vec![
            "| Entry | Description | Citations |".to_string(),
            "|---|---|---|".to_string(),
        ];
        for path in &entries {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            let slug = path.file_stem().unwrap_or_default().to_string_lossy();
            let citations = corpus_files
                .iter()
                .filter(|np| {
                    std::fs::read_to_string(np)
                        .unwrap_or_default()
                        .contains(slug.as_ref())
                })
                .count();
            rows.push(format!(
                "| [{filename}]({filename}) | {desc} | {citations} |"
            ));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(&catalog.join("README.md"), "yidam catalog-audit", &content)
}
