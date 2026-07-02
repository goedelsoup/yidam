use anyhow::{Context, Result};
use std::collections::HashSet;
use std::path::Path;

use crate::parse::{CorpusInstance, CorpusLink};
use crate::paths::repo_root;

pub fn diff_corpus(range: &str) -> Result<()> {
    let root = repo_root()?;
    let (before_ref, after_ref) = parse_range(range);

    // Normalise to an explicit two-dot range so git diff is unambiguous
    let git_range = format!("{before_ref}..{after_ref}");

    let out = std::process::Command::new("git")
        .current_dir(&root)
        .args(["diff", "--name-status", &git_range, "--", ".yidam/corpus/"])
        .output()
        .context("running git diff --name-status")?;

    if !out.status.success() {
        anyhow::bail!(
            "git diff failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    let diff_text = String::from_utf8(out.stdout).context("git diff output is not UTF-8")?;

    let mut node_lines: Vec<String> = Vec::new();
    let mut edge_lines: Vec<String> = Vec::new();

    for line in diff_text.lines() {
        let Some((status, raw_path)) = line.split_once('\t') else {
            continue;
        };
        let path = Path::new(raw_path);

        // Skip ontology schema files; only handle instance .yml files
        if raw_path.ends_with(".ont.yml") || !raw_path.ends_with(".yml") {
            continue;
        }
        if !raw_path.contains(".yidam/corpus/") {
            continue;
        }

        // Display path strips the leading `.yidam/` for brevity
        let display = raw_path.strip_prefix(".yidam/").unwrap_or(raw_path);

        match status {
            "A" => {
                let new_yaml = git_show(&root, &after_ref, path).unwrap_or_default();
                let inst = parse_inst(&new_yaml);
                let label = inst_label(&inst, path);
                node_lines.push(format!("+ node  {display:<55} [added]"));
                for link in inst.links.unwrap_or_default() {
                    edge_lines.push(edge_line('+', &label, &link, "added"));
                }
            }
            "D" => {
                let old_yaml = git_show(&root, &before_ref, path).unwrap_or_default();
                let inst = parse_inst(&old_yaml);
                let label = inst_label(&inst, path);
                node_lines.push(format!("- node  {display:<55} [removed]"));
                for link in inst.links.unwrap_or_default() {
                    edge_lines.push(edge_line('-', &label, &link, "removed"));
                }
            }
            "M" => {
                let old_yaml = git_show(&root, &before_ref, path).unwrap_or_default();
                let new_yaml = git_show(&root, &after_ref, path).unwrap_or_default();
                let old_inst = parse_inst(&old_yaml);
                let new_inst = parse_inst(&new_yaml);
                let label = inst_label(&new_inst, path);

                let claim_delta = count_claim_changes(&old_yaml, &new_yaml);
                let note = if claim_delta == 0 {
                    "modified".to_string()
                } else if claim_delta == 1 {
                    "modified — 1 claim changed".to_string()
                } else {
                    format!("modified — {claim_delta} claims changed")
                };
                node_lines.push(format!("~ node  {display:<55} [{note}]"));

                let old_links: HashSet<String> = old_inst
                    .links
                    .unwrap_or_default()
                    .iter()
                    .map(link_key)
                    .collect();
                let new_links_vec = new_inst.links.unwrap_or_default();
                let new_link_keys: HashSet<String> = new_links_vec.iter().map(link_key).collect();

                for link in &new_links_vec {
                    if !old_links.contains(&link_key(link)) {
                        edge_lines.push(edge_line('+', &label, link, "added"));
                    }
                }
                // Re-parse old links to report removals
                let old_inst2 = parse_inst(&old_yaml);
                for link in old_inst2.links.unwrap_or_default() {
                    if !new_link_keys.contains(&link_key(&link)) {
                        edge_lines.push(edge_line('-', &label, &link, "removed"));
                    }
                }
            }
            _ => {}
        }
    }

    if node_lines.is_empty() && edge_lines.is_empty() {
        println!("No corpus instance changes in {range}.");
        return Ok(());
    }

    for line in &node_lines {
        println!("{line}");
    }
    if !edge_lines.is_empty() {
        if !node_lines.is_empty() {
            println!();
        }
        for line in &edge_lines {
            println!("{line}");
        }
    }

    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn parse_range(range: &str) -> (String, String) {
    if let Some(idx) = range.find("..") {
        let before = range[..idx].to_string();
        let after = range[idx + 2..].to_string();
        (
            before,
            if after.is_empty() {
                "HEAD".to_string()
            } else {
                after
            },
        )
    } else {
        (range.to_string(), "HEAD".to_string())
    }
}

fn git_show(root: &Path, git_ref: &str, path: &Path) -> Option<String> {
    let spec = format!("{git_ref}:{}", path.display());
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["show", &spec])
        .output()
        .ok()?;
    if out.status.success() {
        String::from_utf8(out.stdout).ok()
    } else {
        None
    }
}

fn parse_inst(yaml: &str) -> CorpusInstance {
    serde_yaml::from_str(yaml).unwrap_or_default()
}

fn inst_label(inst: &CorpusInstance, path: &Path) -> String {
    inst.label.clone().unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("?")
            .to_string()
    })
}

fn link_key(link: &CorpusLink) -> String {
    format!(
        "{}|{}",
        link.target.as_deref().unwrap_or(""),
        link.relationship.as_deref().unwrap_or("")
    )
}

fn target_display(target: &str) -> &str {
    // "../concept/formation.yml" → "formation"
    // Falls back to the full target string if parsing fails
    Path::new(target)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(target)
}

fn edge_line(sigil: char, source: &str, link: &CorpusLink, tag: &str) -> String {
    let target = link.target.as_deref().unwrap_or("?");
    let rel = link.relationship.as_deref().unwrap_or("link");
    let dst = target_display(target);
    format!("{sigil} edge  {source} →[{rel}]→ {dst:<40}  [{tag}]")
}

fn count_claim_changes(old: &str, new: &str) -> usize {
    // Count claim-tagged lines that appeared or disappeared
    fn extract(text: &str) -> HashSet<&str> {
        text.lines()
            .map(|l| l.trim())
            .filter(|l| {
                l.contains("[verified]") || l.contains("[inference]") || l.contains("[open]")
            })
            .collect()
    }
    let old_claims = extract(old);
    let new_claims = extract(new);
    old_claims.symmetric_difference(&new_claims).count()
}
