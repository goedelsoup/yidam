use anyhow::Result;
use std::path::Path;

use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_corpus_dir};
use crate::regen::update_file_regen;
use crate::walk::{line_count, walk_corpus_instances, walk_ont_files};

use super::has_open_claim;

pub(crate) fn render_corpus_index(root: &Path, corpus: &Path) -> String {
    let instances = walk_corpus_instances(corpus);
    if instances.is_empty() {
        return "_No corpus instances yet._".to_string();
    }
    // `Claims` is verified / inference / open. It tells a reader how much of a node is
    // measured against how much is supposed, without opening the file.
    let mut rows = vec![
        "| Instance | Class | Label | Links out | Claims | Lines |".to_string(),
        "|---|---|---|---|---|---|".to_string(),
    ];
    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let claims = crate::claims::count_in_source(&text).cell();
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let class = inst.class.unwrap_or_else(|| "—".to_string());
        let label = inst.label.unwrap_or_else(|| "—".to_string());
        let links = inst.links.unwrap_or_default().len();
        let lines = line_count(path);
        let rel = path.strip_prefix(root).unwrap_or(path);
        let filename = path.file_name().unwrap_or_default().to_string_lossy();
        rows.push(format!(
            "| [{filename}]({}) | {class} | {label} | {links} | {claims} | {lines} |",
            rel.display()
        ));
    }
    rows.join("\n")
}

pub(crate) fn render_open_questions(root: &Path, corpus: &Path) -> String {
    let instances = walk_corpus_instances(corpus);
    let mut items = Vec::new();
    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let label = inst.label.clone().unwrap_or_default();
        if label.starts_with('?') || has_open_claim(&text) {
            let rel = path.strip_prefix(root).unwrap_or(path);
            items.push(format!("- [{label}]({})", rel.display()));
        }
    }
    if items.is_empty() {
        "_No open questions._".to_string()
    } else {
        items.join("\n")
    }
}

/// Returns (report_text, issue_count). issue_count > 0 means the graph has problems.
pub(crate) fn render_graph_check(root: &Path, corpus: &Path) -> (String, usize) {
    let instances = walk_corpus_instances(corpus);
    let ont_files = walk_ont_files(corpus);

    if instances.is_empty() && ont_files.is_empty() {
        return (
            format!("No corpus content found in {}.", corpus.display()),
            0,
        );
    }

    let defined_classes: std::collections::HashSet<String> = ont_files
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(|s| s.to_string())
        })
        .collect();

    let mut issues: Vec<(std::path::PathBuf, Vec<String>)> = Vec::new();

    for path in &instances {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
        let mut node_issues = Vec::new();

        match &inst.class {
            None => node_issues.push("missing 'class:' field".to_string()),
            Some(class) if !defined_classes.is_empty() && !defined_classes.contains(class) => {
                node_issues.push(format!(
                    "unknown class '{class}': no matching {class}.ont.yml"
                ));
            }
            _ => {}
        }
        if inst.label.is_none() {
            node_issues.push("missing 'label:' field".to_string());
        }

        let links = inst.links.unwrap_or_default();
        if links.is_empty() {
            node_issues.push("orphan node: no outgoing links".to_string());
        } else {
            let dir = path.parent().unwrap_or(path);
            for link in &links {
                match &link.target {
                    None => node_issues.push("link entry missing 'target:' field".to_string()),
                    Some(target) => {
                        let resolved = dir.join(target);
                        if !resolved.exists() {
                            node_issues.push(format!("broken link: {target}"));
                        }
                    }
                }
            }
        }

        if !node_issues.is_empty() {
            issues.push((path.clone(), node_issues));
        }
    }

    let classes_with_instances: std::collections::HashSet<String> = instances
        .iter()
        .filter_map(|p| {
            p.parent()
                .and_then(|d| d.file_name())
                .and_then(|n| n.to_str())
                .map(|s| s.to_string())
        })
        .collect();
    let mut empty_classes: Vec<&str> = defined_classes
        .iter()
        .filter(|c| !classes_with_instances.contains(*c))
        .map(|c| c.as_str())
        .collect();

    let total = instances.len();
    let clean = total - issues.len();
    let issue_count = issues.len();

    let mut out = String::new();
    if issues.is_empty() {
        out.push_str(&format!(
            "Checked {total} instances across {} classes — all clean.",
            defined_classes.len()
        ));
    } else {
        out.push_str(&format!(
            "Checked {total} instances across {} classes — {clean} clean, {issue_count} with issues:\n",
            defined_classes.len()
        ));
        for (path, node_issues) in &issues {
            let rel = path.strip_prefix(root).unwrap_or(path);
            out.push_str(&format!("\n  {}", rel.display()));
            for issue in node_issues {
                out.push_str(&format!("\n    - {issue}"));
            }
        }
    }

    if !empty_classes.is_empty() {
        empty_classes.sort();
        out.push_str(&format!(
            "\n\nClasses with schema but no instances: {}",
            empty_classes.join(", ")
        ));
    }

    (out, issue_count)
}

pub fn corpus_index() -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let content = render_corpus_index(&root, &corpus);
    println!("{content}");
    update_file_regen(&corpus.join("README.md"), "yidam corpus-index", &content)
}

pub fn open_questions() -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let content = render_open_questions(&root, &corpus);
    println!("{content}");
    update_file_regen(&root.join("README.md"), "yidam open-questions", &content)
}

pub fn graph_check() -> Result<()> {
    let root = repo_root()?;
    let corpus = yidam_corpus_dir(&root);
    let (report, issue_count) = render_graph_check(&root, &corpus);
    // Also need to check for empty classes to print that section
    println!("{report}");
    if issue_count > 0 {
        anyhow::bail!("{issue_count} instance(s) have issues")
    }
    Ok(())
}
