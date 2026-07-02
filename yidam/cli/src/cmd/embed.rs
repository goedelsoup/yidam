use anyhow::Result;
use serde::Serialize;

use crate::git::head_commit_short;
use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_corpus_dir, yidam_embeddings_dir};
use crate::walk::walk_corpus_instances;

#[derive(Serialize)]
pub struct EmbedRecord {
    pub path: String,
    pub class: String,
    pub label: String,
    pub text: String,
    pub commit: String,
}

/// Compose the embedding target text for a corpus instance.
/// Combines label, description, and relationship hints into a single string.
fn compose_text(label: &str, description: &str, links: &[crate::parse::CorpusLink]) -> String {
    let relationships: Vec<String> = links
        .iter()
        .filter_map(|l| l.target.as_deref())
        .filter(|t| !t.ends_with(".ont.yml"))
        .filter_map(|t| {
            std::path::Path::new(t)
                .file_stem()
                .map(|n| n.to_string_lossy().replace('-', " "))
        })
        .collect();

    let mut parts: Vec<String> = Vec::new();
    if !label.is_empty() {
        parts.push(label.to_string());
    }
    if !description.is_empty() {
        parts.push(description.to_string());
    }
    if !relationships.is_empty() {
        parts.push(format!("Related: {}.", relationships.join(", ")));
    }
    parts.join(" ")
}

pub fn embed() -> Result<()> {
    let root = repo_root()?;
    let corpus_dir = yidam_corpus_dir(&root);
    let embeddings_dir = yidam_embeddings_dir(&root);
    let commit = head_commit_short(&root);

    let instances = walk_corpus_instances(&corpus_dir);
    if instances.is_empty() {
        println!("No corpus instances found in {}.", corpus_dir.display());
        return Ok(());
    }

    std::fs::create_dir_all(&embeddings_dir)?;

    let mut count = 0;
    for path in &instances {
        let yaml = std::fs::read_to_string(path)?;
        let inst: CorpusInstance = match serde_yaml::from_str(&yaml) {
            Ok(v) => v,
            Err(e) => {
                let rel = path.strip_prefix(&root).unwrap_or(path);
                eprintln!("[warn] skipping {}: {e}", rel.display());
                continue;
            }
        };

        let class = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string();

        let label = inst.label.as_deref().unwrap_or("").to_string();
        let description = inst.description.as_deref().unwrap_or("").to_string();
        let links = inst.links.as_deref().unwrap_or(&[]);
        let text = compose_text(&label, &description, links);

        let rel_path = path.strip_prefix(&root).unwrap_or(path);
        let record = EmbedRecord {
            path: rel_path.to_string_lossy().to_string(),
            class: class.clone(),
            label,
            text,
            commit: commit.clone(),
        };

        let out_dir = embeddings_dir.join(&class);
        std::fs::create_dir_all(&out_dir)?;
        let json_name = path
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
            + ".json";
        std::fs::write(
            out_dir.join(&json_name),
            serde_json::to_string_pretty(&record)?,
        )?;
        count += 1;
    }

    println!("Extracted {count} embedding record(s) → {}", embeddings_dir.display());
    Ok(())
}
