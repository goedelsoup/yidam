use anyhow::Result;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::parse::CorpusInstance;
use crate::paths::{repo_root, yidam_catalog_dir, yidam_corpus_dir};
use crate::walk::{walk_corpus_instances, walk_md_files, walk_ont_files};

struct Issue {
    rel: String,
    kind: &'static str,
    message: String,
    suggestion: &'static str,
}

struct ParsedInstance {
    path: PathBuf,
    inst: CorpusInstance,
}

pub fn lint(warn_only: bool, suggest: bool) -> Result<()> {
    let root = repo_root()?;
    let corpus_dir = yidam_corpus_dir(&root);
    let catalog_dir = yidam_catalog_dir(&root);

    let instance_paths = walk_corpus_instances(&corpus_dir);
    let ont_files = walk_ont_files(&corpus_dir);

    let defined_classes: HashSet<String> = ont_files
        .iter()
        .filter_map(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .map(|s| s.to_string())
        })
        .collect();

    // Parse all instances once
    let parsed: Vec<ParsedInstance> = instance_paths
        .iter()
        .map(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
            ParsedInstance {
                path: p.clone(),
                inst,
            }
        })
        .collect();

    // Build set of all link targets (absolute paths) to detect orphan-in
    let mut all_targets: HashSet<PathBuf> = HashSet::new();
    for p in &parsed {
        let dir = p.path.parent().unwrap_or(p.path.as_path());
        for link in p.inst.links.as_deref().unwrap_or(&[]) {
            if let Some(target) = &link.target {
                all_targets.insert(dir.join(target));
            }
        }
    }

    let mut issues: Vec<Issue> = Vec::new();

    for p in &parsed {
        let rel = p
            .path
            .strip_prefix(&root)
            .unwrap_or(&p.path)
            .to_string_lossy()
            .to_string();

        if p.inst.class.is_none() {
            issues.push(Issue {
                rel: rel.clone(),
                kind: "missing-class",
                message: "missing 'class:' field".to_string(),
                suggestion: "Add 'class: <name>' matching an existing .ont.yml schema file",
            });
        }

        if let Some(class) = &p.inst.class {
            if !defined_classes.is_empty() && !defined_classes.contains(class) {
                issues.push(Issue {
                    rel: rel.clone(),
                    kind: "unknown-class",
                    message: format!("unknown class '{class}': no matching {class}.ont.yml"),
                    suggestion:
                        "Create the schema file or update 'class:' to match an existing one",
                });
            }
        }

        if p.inst.label.is_none() {
            issues.push(Issue {
                rel: rel.clone(),
                kind: "missing-label",
                message: "missing 'label:' field".to_string(),
                suggestion: "Add 'label: <short name>' describing this instance",
            });
        }

        if p.inst.description.is_none() {
            issues.push(Issue {
                rel: rel.clone(),
                kind: "no-description",
                message: "missing 'description:' field".to_string(),
                suggestion: "Add 'description: <1–2 sentences>' explaining this concept",
            });
        }

        let links = p.inst.links.as_deref().unwrap_or(&[]);

        if links.is_empty() {
            issues.push(Issue {
                rel: rel.clone(),
                kind: "orphan-out",
                message: "no outgoing links — isolated node".to_string(),
                suggestion: "Add at least one 'links:' entry connecting this node to the graph",
            });
        } else {
            let dir = p.path.parent().unwrap_or(p.path.as_path());
            for link in links {
                match &link.target {
                    None => issues.push(Issue {
                        rel: rel.clone(),
                        kind: "broken-link",
                        message: "link entry missing 'target:' field".to_string(),
                        suggestion: "Add 'target: <relative/path.yml>' to the link entry",
                    }),
                    Some(target) => {
                        if !dir.join(target).exists() {
                            issues.push(Issue {
                                rel: rel.clone(),
                                kind: "broken-link",
                                message: format!("broken link target: {target}"),
                                suggestion: "Verify the target path exists or update the link",
                            });
                        }
                    }
                }
            }
        }

        // Orphan-in: nothing else points to this node
        if !all_targets.contains(&p.path) {
            issues.push(Issue {
                rel: rel.clone(),
                kind: "orphan-in",
                message: "no incoming links — nothing points to this node".to_string(),
                suggestion: "Link to this node from at least one other corpus instance",
            });
        }
    }

    // Unused catalog entries
    let catalog_entries = walk_md_files(&catalog_dir);
    let corpus_texts: Vec<String> = instance_paths
        .iter()
        .map(|p| std::fs::read_to_string(p).unwrap_or_default())
        .collect();

    for entry_path in &catalog_entries {
        let slug = entry_path.file_stem().unwrap_or_default().to_string_lossy();
        let citations = corpus_texts
            .iter()
            .filter(|t| t.contains(slug.as_ref()))
            .count();
        if citations == 0 {
            issues.push(Issue {
                rel: entry_path
                    .strip_prefix(&root)
                    .unwrap_or(entry_path)
                    .to_string_lossy()
                    .to_string(),
                kind: "unused-catalog",
                message: format!("catalog entry '{slug}' has zero citations in the corpus"),
                suggestion: "Cite this source in at least one corpus instance, or remove the entry",
            });
        }
    }

    if issues.is_empty() {
        println!("lint: {} instance(s) checked — all clean.", parsed.len());
        return Ok(());
    }

    for issue in &issues {
        eprintln!("[{}] {}: {}", issue.kind, issue.rel, issue.message);
        if suggest {
            eprintln!("       → {}", issue.suggestion);
        }
    }

    let n = issues.len();
    if warn_only || suggest {
        eprintln!("lint: {n} issue(s) (reported, not failing)");
        Ok(())
    } else {
        anyhow::bail!("lint: {n} issue(s) found")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_corpus(root: &Path) -> PathBuf {
        let corpus = root.join(".yidam").join("corpus");
        let class_dir = corpus.join("reach");
        fs::create_dir_all(&class_dir).unwrap();
        fs::write(corpus.join("reach.ont.yml"), "class: reach\n").unwrap();
        corpus
    }

    #[test]
    fn collect_issues_missing_description() {
        let tmp = TempDir::new().unwrap();
        let corpus = make_corpus(tmp.path());
        let class_dir = corpus.join("reach");
        fs::write(
            class_dir.join("alpha.yml"),
            "class: reach\nlabel: Alpha\nlinks:\n  - target: beta.yml\n    relationship: refines\n",
        )
        .unwrap();
        fs::write(
            class_dir.join("beta.yml"),
            "class: reach\nlabel: Beta\ndescription: A beta concept.\nlinks:\n  - target: alpha.yml\n    relationship: refines\n",
        ).unwrap();

        let instances = walk_corpus_instances(&corpus);
        let parsed: Vec<ParsedInstance> = instances
            .iter()
            .map(|p| {
                let text = fs::read_to_string(p).unwrap_or_default();
                let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
                ParsedInstance {
                    path: p.clone(),
                    inst,
                }
            })
            .collect();

        let alpha = parsed
            .iter()
            .find(|p| p.path.ends_with("alpha.yml"))
            .unwrap();
        assert!(
            alpha.inst.description.is_none(),
            "alpha should have no description"
        );
        let beta = parsed
            .iter()
            .find(|p| p.path.ends_with("beta.yml"))
            .unwrap();
        assert!(
            beta.inst.description.is_some(),
            "beta should have a description"
        );
    }

    #[test]
    fn orphan_in_detection() {
        let tmp = TempDir::new().unwrap();
        let corpus = make_corpus(tmp.path());
        let class_dir = corpus.join("reach");
        // isolated: no other node links to it
        fs::write(
            class_dir.join("isolated.yml"),
            "class: reach\nlabel: Isolated\ndescription: Alone.\nlinks:\n  - target: connected.yml\n    relationship: refines\n",
        ).unwrap();
        fs::write(
            class_dir.join("connected.yml"),
            "class: reach\nlabel: Connected\ndescription: Referenced.\nlinks:\n  - target: isolated.yml\n    relationship: refines\n",
        ).unwrap();

        let instances = walk_corpus_instances(&corpus);
        let parsed: Vec<ParsedInstance> = instances
            .iter()
            .map(|p| {
                let text = fs::read_to_string(p).unwrap_or_default();
                let inst: CorpusInstance = serde_yaml::from_str(&text).unwrap_or_default();
                ParsedInstance {
                    path: p.clone(),
                    inst,
                }
            })
            .collect();

        let mut all_targets: HashSet<PathBuf> = HashSet::new();
        for p in &parsed {
            let dir = p.path.parent().unwrap_or(p.path.as_path());
            for link in p.inst.links.as_deref().unwrap_or(&[]) {
                if let Some(t) = &link.target {
                    all_targets.insert(dir.join(t));
                }
            }
        }

        // Both nodes are targeted by the other, so neither is orphan-in
        for p in &parsed {
            assert!(
                all_targets.contains(&p.path),
                "{} should have an incoming link",
                p.path.display()
            );
        }
    }
}
