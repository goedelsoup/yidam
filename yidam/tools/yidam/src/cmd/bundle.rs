use anyhow::Result;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::io::Write;
use tar::Builder;

use crate::git::{genesis_date, genesis_message, head_commit_short};
use crate::paths::{repo_root, yidam_corpus_dir, yidam_decisions_dir, yidam_skills_dir};
use crate::walk::{walk_corpus_instances, walk_decision_files, walk_md_files, walk_ont_files};

use super::corpus::{render_corpus_index, render_graph_check};
use super::decisions::render_decisions_log;
use super::registry::render_skills_index;

fn add_bytes<W: Write>(tar: &mut Builder<W>, path: &str, data: &[u8]) -> Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append_data(&mut header, path, data)?;
    Ok(())
}

/// Produce `.yidam/bundle.yiz` — a gzipped tar archive with layout:
///
///   manifest.yml          machine-readable provenance + counts
///   corpus/               raw .ont.yml schema files + instance .yml files
///   skills/               raw skill .md files
///   decisions/            raw decision .yml files
///   index/corpus.md       rendered instance table
///   index/graph.md        graph integrity check report
///   index/decisions.md    rendered decisions table
///   index/skills.md       rendered skills table
pub fn bundle() -> Result<()> {
    let root = repo_root()?;
    let corpus_dir = yidam_corpus_dir(&root);
    let decisions_dir = yidam_decisions_dir(&root);
    let skills_dir = yidam_skills_dir(&root);

    let ont_files = walk_ont_files(&corpus_dir);
    let instances = walk_corpus_instances(&corpus_dir);
    let skill_files = walk_md_files(&skills_dir);
    let decision_files = walk_decision_files(&decisions_dir);

    let commit = head_commit_short(&root);
    let genesis = genesis_date(&root);
    let genesis_msg = genesis_message(&root);
    let generated_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let gz = GzEncoder::new(Vec::new(), Compression::default());
    let mut tar = Builder::new(gz);

    // manifest.yml
    let domain = genesis_msg
        .lines()
        .next()
        .unwrap_or("")
        .trim_start_matches("chore: genesis — ")
        .to_string();
    let manifest = format!(
        "bundle_version: \"1\"\n\
         commit: \"{commit}\"\n\
         genesis: \"{genesis}\"\n\
         generated_at: {generated_at}\n\
         domain: \"{domain}\"\n\
         classes: {}\n\
         instances: {}\n\
         skills: {}\n\
         decisions: {}\n\
         index:\n\
         \x20 - corpus.md\n\
         \x20 - graph.md\n\
         \x20 - decisions.md\n\
         \x20 - skills.md\n",
        ont_files.len(),
        instances.len(),
        skill_files.len(),
        decision_files.len(),
    );
    add_bytes(&mut tar, "manifest.yml", manifest.as_bytes())?;

    // corpus/ — schema files at root, instance files inside class subdirs
    for path in &ont_files {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        add_bytes(&mut tar, &format!("corpus/{filename}"), &std::fs::read(path)?)?;
    }
    for path in &instances {
        let class = path
            .parent()
            .and_then(|d| d.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        add_bytes(
            &mut tar,
            &format!("corpus/{class}/{filename}"),
            &std::fs::read(path)?,
        )?;
    }

    // skills/
    for path in &skill_files {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        add_bytes(
            &mut tar,
            &format!("skills/{filename}"),
            &std::fs::read(path)?,
        )?;
    }

    // decisions/
    for path in &decision_files {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown");
        add_bytes(
            &mut tar,
            &format!("decisions/{filename}"),
            &std::fs::read(path)?,
        )?;
    }

    // index/ — rendered views
    let corpus_index = render_corpus_index(&root, &corpus_dir);
    add_bytes(&mut tar, "index/corpus.md", corpus_index.as_bytes())?;

    let (graph_report, _) = render_graph_check(&root, &corpus_dir);
    add_bytes(&mut tar, "index/graph.md", graph_report.as_bytes())?;

    let decisions_index = render_decisions_log(&decisions_dir);
    add_bytes(&mut tar, "index/decisions.md", decisions_index.as_bytes())?;

    let skills_index = render_skills_index(&skills_dir);
    add_bytes(&mut tar, "index/skills.md", skills_index.as_bytes())?;

    let gz = tar.into_inner()?;
    let bytes = gz.finish()?;

    let output = root.join(".yidam").join("bundle.yiz");
    std::fs::write(&output, &bytes)?;

    println!(
        "Bundle written: {} classes, {} instances, {} skills, {} decisions, {} bytes → {}",
        ont_files.len(),
        instances.len(),
        skill_files.len(),
        decision_files.len(),
        bytes.len(),
        output.display()
    );
    Ok(())
}
