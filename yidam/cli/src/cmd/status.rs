use anyhow::Result;

use crate::git::{active_phase_count, genesis_date};
use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::{walk_corpus_instances, walk_md_files};

use super::has_open_claim;

pub fn status() -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let catalog = yidam_catalog_dir(&root);

    let instances = walk_corpus_instances(&corpus);
    let node_count = instances.len();

    let open_count = instances
        .iter()
        .filter(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
            let label = inst.label.unwrap_or_default();
            label.starts_with('?') || has_open_claim(&text)
        })
        .count();

    let catalog_entries = walk_md_files(&catalog).len();

    let index_path = root.join(".yidam").join("index");
    let index_freshness = if index_path.exists() {
        "present"
    } else {
        "not initialized"
    };

    let phases = active_phase_count(&root);
    let genesis = genesis_date(&root);

    let content = format!(
        "**{node_count} nodes** · {open_count} open · {catalog_entries} sources · \
         index {index_freshness} · {phases} active phase(s) · genesis {genesis}"
    );

    println!("{content}");
    update_file_regen(&root.join("README.md"), "yidam status", &content)
}

pub fn index_status() -> Result<()> {
    let root = repo_root()?;
    let index_path = root.join(".yidam").join("index");

    let content = if index_path.exists() {
        let meta_path = index_path.join("meta.json");
        if let Ok(meta) = std::fs::read_to_string(&meta_path) {
            format!("```\n{meta}\n```")
        } else {
            "Index present — no metadata file found.".to_string()
        }
    } else {
        "_Index not initialized. Run the Python SDK `sync_index` to build._".to_string()
    };

    println!("{content}");
    let corpus = yidam_corpus_dir(&root);
    update_file_regen(&corpus.join("README.md"), "yidam index-status", &content)?;
    update_file_regen(
        &root.join("crates").join("README.md"),
        "yidam index-status",
        &content,
    )
}
