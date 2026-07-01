use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

// ── repo location ─────────────────────────────────────────────────────────────

fn repo_root() -> Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running git rev-parse")?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).context("git output utf8")?;
        Ok(PathBuf::from(s.trim()))
    } else {
        std::env::current_dir().context("current dir fallback")
    }
}

// ── REGEN marker helpers ──────────────────────────────────────────────────────

// Replaces content between <!-- REGEN: <command> ... --> and <!-- /REGEN -->.
// Returns the original string unchanged if either marker is not found.
fn update_regen(text: &str, command: &str, new_content: &str) -> String {
    let open_tag = format!("<!-- REGEN: {command}");
    let close_tag = "<!-- /REGEN -->";

    let Some(open_pos) = text.find(&open_tag) else {
        return text.to_string();
    };
    let after_open = open_pos + open_tag.len();
    let Some(arrow_rel) = text[after_open..].find("-->") else {
        return text.to_string();
    };
    let content_start = after_open + arrow_rel + 3; // right after "-->"
    let Some(close_rel) = text[content_start..].find(close_tag) else {
        return text.to_string();
    };
    let close_abs = content_start + close_rel;
    format!(
        "{}\n{}\n{}",
        &text[..content_start],
        new_content,
        &text[close_abs..]
    )
}

fn update_file_regen(path: &Path, command: &str, new_content: &str) -> Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let original =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    let updated = update_regen(&original, command, new_content);
    if updated != original {
        std::fs::write(path, &updated).with_context(|| format!("writing {}", path.display()))?;
        println!("  updated {}", path.display());
    }
    Ok(())
}

// ── markdown helpers ──────────────────────────────────────────────────────────

fn extract_title(text: &str) -> Option<String> {
    text.lines()
        .find(|l| l.starts_with("# "))
        .map(|l| l.trim_start_matches("# ").trim().to_string())
}

fn count_md_links(text: &str) -> usize {
    use pulldown_cmark::{Event, Parser, Tag};
    Parser::new(text)
        .filter(|e| {
            if let Event::Start(Tag::Link { dest_url, .. }) = e {
                !dest_url.starts_with("http")
            } else {
                false
            }
        })
        .count()
}

fn has_open_claim(text: &str) -> bool {
    text.contains("[open]")
}

// ── file walking ──────────────────────────────────────────────────────────────

fn walk_md_files(dir: &Path) -> Vec<PathBuf> {
    if !dir.exists() {
        return vec![];
    }
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_type().is_file()
                && e.path().extension().is_some_and(|x| x == "md")
                && e.file_name().to_str() != Some("README.md")
        })
        .map(|e| e.path().to_owned())
        .collect();
    files.sort();
    files
}

fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|s| s.lines().count())
        .unwrap_or(0)
}

// ── frontmatter ───────────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Default)]
struct Frontmatter {
    name: Option<String>,
    description: Option<String>,
}

fn parse_frontmatter(text: &str) -> Frontmatter {
    let body = text.trim_start();
    let Some(rest) = body.strip_prefix("---\n") else {
        return Frontmatter::default();
    };
    let Some(end) = rest.find("\n---") else {
        return Frontmatter::default();
    };
    serde_yaml::from_str(&rest[..end]).unwrap_or_default()
}

// ── TOML/JSON field extraction (no full parser needed) ────────────────────────

fn extract_toml_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field} = ");
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"').to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

fn extract_json_field(text: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\":");
    for line in text.lines() {
        if let Some(rest) = line.find(&pattern).map(|i| &line[i + pattern.len()..]) {
            let value = rest
                .trim()
                .trim_matches('"')
                .trim_end_matches(',')
                .trim()
                .to_string();
            if !value.is_empty() && value != "null" {
                return Some(value);
            }
        }
    }
    None
}

// ── git helpers ───────────────────────────────────────────────────────────────

fn genesis_date(root: &Path) -> String {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["log", "--reverse", "--format=%as", "--max-count=1"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "no commits".to_string())
}

fn active_phase_count(root: &Path) -> usize {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["branch", "--list", "ma/*"])
        .output()
        .ok();
    out.and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

// ── subcommands ───────────────────────────────────────────────────────────────

pub fn status() -> Result<()> {
    let root = repo_root()?;

    let nodes = walk_md_files(&root.join("corpus"));
    let node_count = nodes.len();

    let open_count = nodes
        .iter()
        .filter(|p| {
            let text = std::fs::read_to_string(p).unwrap_or_default();
            let title = extract_title(&text).unwrap_or_default();
            title.starts_with('?') || has_open_claim(&text)
        })
        .count();

    let catalog_entries = walk_md_files(&root.join("catalog")).len();

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

pub fn open_questions() -> Result<()> {
    let root = repo_root()?;
    let nodes = walk_md_files(&root.join("corpus"));

    let mut items = Vec::new();
    for path in &nodes {
        let text = std::fs::read_to_string(path).unwrap_or_default();
        let title = extract_title(&text).unwrap_or_default();
        if title.starts_with('?') || has_open_claim(&text) {
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            items.push(format!("- [{title}](corpus/{filename})"));
        }
    }

    let content = if items.is_empty() {
        "_No open questions._".to_string()
    } else {
        items.join("\n")
    };

    println!("{content}");
    update_file_regen(&root.join("README.md"), "yidam open-questions", &content)
}

pub fn corpus_index() -> Result<()> {
    let root = repo_root()?;
    let corpus_dir = root.join("corpus");
    let nodes = walk_md_files(&corpus_dir);

    let content = if nodes.is_empty() {
        "_No corpus nodes yet._".to_string()
    } else {
        let mut rows = vec![
            "| Node | Title | Lines | Links out |".to_string(),
            "|---|---|---|---|".to_string(),
        ];
        for path in &nodes {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let title = extract_title(&text).unwrap_or_else(|| "—".to_string());
            let lines = line_count(path);
            let links = count_md_links(&text);
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            rows.push(format!(
                "| [{filename}]({filename}) | {title} | {lines} | {links} |"
            ));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(
        &corpus_dir.join("README.md"),
        "yidam corpus-index",
        &content,
    )
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
    // Appears in both corpus/README.md and crates/README.md
    update_file_regen(
        &root.join("corpus").join("README.md"),
        "yidam index-status",
        &content,
    )?;
    update_file_regen(
        &root.join("crates").join("README.md"),
        "yidam index-status",
        &content,
    )
}

pub fn catalog_audit() -> Result<()> {
    let root = repo_root()?;
    let catalog_dir = root.join("catalog");
    let entries = walk_md_files(&catalog_dir);

    let content = if entries.is_empty() {
        "_No catalog entries yet._".to_string()
    } else {
        let corpus_nodes = walk_md_files(&root.join("corpus"));
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
            let citations = corpus_nodes
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
    update_file_regen(
        &catalog_dir.join("README.md"),
        "yidam catalog-audit",
        &content,
    )
}

pub fn agents_index() -> Result<()> {
    let root = repo_root()?;
    let agents_dir = root.join("agents");
    let agents = walk_md_files(&agents_dir);

    let content = if agents.is_empty() {
        "_No domain-specific agents yet._".to_string()
    } else {
        let mut rows = vec![
            "| Agent | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &agents {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let name = fm.name.unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            rows.push(format!("| [{name}]({filename}) | {desc} |"));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(
        &agents_dir.join("README.md"),
        "yidam agents-index",
        &content,
    )
}

pub fn skills_index() -> Result<()> {
    let root = repo_root()?;
    let skills_dir = root.join("skills");
    let skills = walk_md_files(&skills_dir);

    let content = if skills.is_empty() {
        "_No domain-specific skills yet._".to_string()
    } else {
        let mut rows = vec![
            "| Skill | Description |".to_string(),
            "|---|---|".to_string(),
        ];
        for path in &skills {
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let fm = parse_frontmatter(&text);
            let name = fm.name.unwrap_or_else(|| {
                path.file_stem()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string()
            });
            let desc = fm.description.unwrap_or_else(|| "—".to_string());
            let filename = path.file_name().unwrap_or_default().to_string_lossy();
            rows.push(format!("| [{name}]({filename}) | {desc} |"));
        }
        rows.join("\n")
    };

    println!("{content}");
    update_file_regen(
        &skills_dir.join("README.md"),
        "yidam skills-index",
        &content,
    )
}

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
            let desc = extract_toml_field(&text, "description").unwrap_or_else(|| "—".to_string());
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
                    && (name == "package.json" || name == "pyproject.toml" || name == "Cargo.toml")
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

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_regen_basic() {
        let input = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
_Run `yidam status` to populate._\n\
<!-- /REGEN -->\n";
        let expected = "\
## Status\n\
\n\
<!-- REGEN: yidam status\n\
Fields: node count, open questions.\n\
-->\n\
**12 nodes** · 3 open · index fresh\n\
<!-- /REGEN -->\n";
        assert_eq!(
            update_regen(input, "yidam status", "**12 nodes** · 3 open · index fresh"),
            expected
        );
    }

    #[test]
    fn update_regen_missing_marker_is_noop() {
        let input = "# No REGEN here\n";
        assert_eq!(update_regen(input, "yidam status", "new content"), input);
    }

    #[test]
    fn update_regen_idempotent() {
        let input = "<!-- REGEN: yidam status\n-->\ncontent\n<!-- /REGEN -->\n";
        let once = update_regen(input, "yidam status", "content");
        let twice = update_regen(&once, "yidam status", "content");
        assert_eq!(once, twice);
    }

    #[test]
    fn extract_title_finds_first_h1() {
        let text = "## Not this\n# This one\n# Not this either\n";
        assert_eq!(extract_title(text).as_deref(), Some("This one"));
    }

    #[test]
    fn count_md_links_skips_external() {
        let text = "[a](local.md) [b](https://example.com) [c](other.md)";
        assert_eq!(count_md_links(text), 2);
    }
}
