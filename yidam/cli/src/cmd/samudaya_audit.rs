use anyhow::Result;
use std::collections::HashMap;

use crate::parse::parse_samudaya_seed;
use crate::paths::{repo_root, samudaya_dir};

const VALID_KINDS: &[&str] = &["axiom", "hint", "constraint", "augmentation"];

fn extract_body(text: &str) -> &str {
    let body = text.trim_start();
    let Some(rest) = body.strip_prefix("---\n") else {
        return text;
    };
    let Some(end) = rest.find("\n---") else {
        return text;
    };
    &rest[end + 4..]
}

fn has_title(text: &str) -> bool {
    extract_body(text)
        .lines()
        .any(|l| l.starts_with("# ") && l.len() > 2)
}

pub fn samudaya_audit() -> Result<()> {
    let root = repo_root()?;
    let samudaya = samudaya_dir(&root);

    if !samudaya.exists() {
        println!("samudaya/ not found at {}.", samudaya.display());
        println!(
            "This repository has no samudaya seeds (or they were already consumed at genesis)."
        );
        return Ok(());
    }

    // Walk samudaya/ for .md files, skipping examples/ and README.md
    let mut seed_files: Vec<_> = walkdir::WalkDir::new(&samudaya)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_str() != Some("README.md")
                && !e.path().components().any(|c| c.as_os_str() == "examples")
        })
        .map(|e| e.path().to_owned())
        .collect();
    seed_files.sort();

    if seed_files.is_empty() {
        println!("samudaya/ contains no seed files (only examples/ templates).");
        println!("A derived repo will inherit 0 seeds.");
        return Ok(());
    }

    let mut issues: Vec<String> = Vec::new();
    let mut kind_counts: HashMap<String, usize> = HashMap::new();
    let mut constitutional_count = 0usize;

    for path in &seed_files {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let seed = parse_samudaya_seed(&text);
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let kind = match &seed.kind {
            None => {
                issues.push(format!("{rel}: missing 'kind:' field in frontmatter"));
                "_unknown".to_string()
            }
            Some(k) => {
                if !VALID_KINDS.contains(&k.as_str()) {
                    issues.push(format!(
                        "{rel}: unknown kind '{k}' — expected one of: {}",
                        VALID_KINDS.join(", ")
                    ));
                }
                *kind_counts.entry(k.clone()).or_insert(0) += 1;
                k.clone()
            }
        };

        if !has_title(&text) {
            issues.push(format!("{rel}: missing title (H1 heading in body)"));
        }

        if kind == "augmentation" {
            match seed.constitutional {
                None => issues.push(format!(
                    "{rel}: augmentation missing 'constitutional: true|false' — \
                     set true to commit into CONSTITUTION.md permanently, false otherwise"
                )),
                Some(true) => {
                    constitutional_count += 1;
                    println!(
                        "[review] {rel}: constitutional augmentation — \
                         will be committed permanently into the derived repo's CONSTITUTION.md"
                    );
                    println!(
                        "         Verify it does not contradict Articles I–VI \
                         (prelude/CONSTITUTION.md)"
                    );
                }
                Some(false) => {}
            }
        }
    }

    // Print issues
    for issue in &issues {
        eprintln!("[warn]  {issue}");
    }

    // Summary
    let n = seed_files.len();
    let m = kind_counts.len();

    println!();
    println!("Seeds found: {n} (across {m} kind(s))");
    let mut kinds: Vec<_> = kind_counts.iter().collect();
    kinds.sort_by(|a, b| a.0.cmp(b.0));
    for (kind, count) in &kinds {
        println!("  {kind}: {count}");
    }
    if constitutional_count > 0 {
        println!(
            "  ({constitutional_count} constitutional augmentation(s) flagged for review above)"
        );
    }
    println!();
    println!("A derived repo will inherit {n} seeds covering {m} kind(s).");

    if !issues.is_empty() {
        let ni = issues.len();
        eprintln!();
        eprintln!("{ni} issue(s) found in samudaya/ seeds.");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_title_detects_h1() {
        assert!(has_title("---\nkind: axiom\n---\n# My Title\nBody text.\n"));
        assert!(!has_title("---\nkind: axiom\n---\nNo heading here.\n"));
        assert!(!has_title("---\nkind: axiom\n---\n## Only h2\n"));
    }

    #[test]
    fn extract_body_strips_frontmatter() {
        let text = "---\nkind: axiom\n---\n# Title\nBody.\n";
        let body = extract_body(text);
        assert!(body.contains("# Title"));
        assert!(!body.contains("kind:"));
    }

    #[test]
    fn parse_seed_constitutional_field() {
        let text = "---\nkind: augmentation\nconstitutional: true\n---\n# Aug\nContent.\n";
        let seed = parse_samudaya_seed(text);
        assert_eq!(seed.kind.as_deref(), Some("augmentation"));
        assert_eq!(seed.constitutional, Some(true));
    }
}
