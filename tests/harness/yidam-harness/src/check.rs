use anyhow::Result;
use pulldown_cmark::{Event, Parser, Tag};
use std::path::Path;
use walkdir::WalkDir;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckReport {
    pub results: Vec<CheckResult>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CheckResult {
    pub id: String,
    pub description: String,
    pub passed: bool,
    pub detail: Option<String>,
}

impl CheckReport {
    pub fn passed(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }
    pub fn total(&self) -> usize {
        self.results.len()
    }
    pub fn any_failed(&self) -> bool {
        self.results.iter().any(|r| !r.passed)
    }
    pub fn print(&self) {
        for r in &self.results {
            let mark = if r.passed { "✓" } else { "✗" };
            println!("[{mark}] {} — {}", r.id, r.description);
            if let Some(d) = &r.detail {
                println!("    {d}");
            }
        }
        println!("{}/{} passed", self.passed(), self.total());
    }
}

/// Run all structural checks against a result directory.
pub fn run_all(result_dir: &Path) -> Result<CheckReport> {
    let corpus_dir = result_dir.join("corpus");
    let corpus_nodes = collect_md_files(&corpus_dir);

    let s1 = check_s1(&corpus_nodes);
    let s2 = check_s2(&corpus_nodes, result_dir);
    let s3 = check_s3(&corpus_nodes, result_dir);
    let s4 = check_s4(result_dir);
    let s5 = check_s5(result_dir);
    let s6 = check_s6(result_dir);
    let s7 = check_s7(&corpus_nodes, result_dir);

    Ok(CheckReport {
        results: vec![s1, s2, s3, s4, s5, s6, s7],
    })
}

fn collect_md_files(dir: &Path) -> Vec<std::path::PathBuf> {
    if !dir.exists() {
        return vec![];
    }
    WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "md"))
        .map(|e| e.path().to_owned())
        .collect()
}

fn outgoing_links(path: &Path) -> Vec<String> {
    let Ok(content) = std::fs::read_to_string(path) else {
        return vec![];
    };
    let mut links = Vec::new();
    let parser = Parser::new(&content);
    for event in parser {
        if let Event::Start(Tag::Link { dest_url, .. }) = event {
            let url = dest_url.to_string();
            // Only count relative links (edges to other nodes, not external URLs)
            if !url.starts_with("http") {
                links.push(url);
            }
        }
    }
    links
}

fn line_count(path: &Path) -> usize {
    std::fs::read_to_string(path)
        .map(|c| c.lines().count())
        .unwrap_or(0)
}

fn check_s1(nodes: &[std::path::PathBuf]) -> CheckResult {
    CheckResult {
        id: "S1".into(),
        description: "corpus/ exists and contains ≥2 .md files".into(),
        passed: nodes.len() >= 2,
        detail: if nodes.len() < 2 {
            Some(format!("found {} node(s)", nodes.len()))
        } else {
            None
        },
    }
}

fn check_s2(nodes: &[std::path::PathBuf], _root: &Path) -> CheckResult {
    let without_links: Vec<_> = nodes
        .iter()
        .filter(|p| outgoing_links(p).is_empty())
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    CheckResult {
        id: "S2".into(),
        description: "each corpus node has ≥1 outgoing markdown link".into(),
        passed: without_links.is_empty(),
        detail: if without_links.is_empty() {
            None
        } else {
            Some(format!("no links: {}", without_links.join(", ")))
        },
    }
}

fn check_s3(nodes: &[std::path::PathBuf], _root: &Path) -> CheckResult {
    // Orphan: zero outgoing AND zero incoming links
    // Build incoming link map
    let mut incoming: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for node in nodes {
        for link in outgoing_links(node) {
            *incoming.entry(link).or_insert(0) += 1;
        }
    }
    let orphans: Vec<_> = nodes
        .iter()
        .filter(|p| {
            let out = outgoing_links(p);
            let name = p
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            out.is_empty() && incoming.get(&name).copied().unwrap_or(0) == 0
        })
        .map(|p| {
            p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    CheckResult {
        id: "S3".into(),
        description: "no orphan corpus nodes (zero in AND zero out links)".into(),
        passed: orphans.is_empty(),
        detail: if orphans.is_empty() {
            None
        } else {
            Some(format!("orphans: {}", orphans.join(", ")))
        },
    }
}

fn check_s4(root: &Path) -> CheckResult {
    let commit_log = root.join("commit.log");
    let count = std::fs::read_to_string(&commit_log)
        .map(|s| s.lines().filter(|l| l.starts_with("commit ")).count())
        .unwrap_or(0);
    CheckResult {
        id: "S4".into(),
        description: "exactly 1 git commit exists (the genesis commit)".into(),
        passed: count == 1,
        detail: if count != 1 {
            Some(format!("found {count} commit(s)"))
        } else {
            None
        },
    }
}

fn check_s5(root: &Path) -> CheckResult {
    let commit_log = std::fs::read_to_string(root.join("commit.log")).unwrap_or_default();
    // Message is everything after the first blank line following "commit ..." header lines
    let lines: Vec<&str> = commit_log.lines().collect();
    let msg_lines = lines.iter().skip_while(|l| !l.is_empty()).skip(1).count();
    CheckResult {
        id: "S5".into(),
        description: "genesis commit message is ≥3 lines".into(),
        passed: msg_lines >= 3,
        detail: if msg_lines < 3 {
            Some(format!("message has {msg_lines} line(s)"))
        } else {
            None
        },
    }
}

fn check_s6(root: &Path) -> CheckResult {
    let dirs = [
        ("agents", root.join("agents").exists()),
        ("catalog", root.join("catalog").exists()),
        ("skills", root.join("skills").exists()),
    ];
    let missing: Vec<&str> = dirs.iter().filter(|(_, ok)| !ok).map(|(n, _)| *n).collect();
    CheckResult {
        id: "S6".into(),
        description: "agents/, catalog/, and skills/ stub directories exist".into(),
        passed: missing.is_empty(),
        detail: if missing.is_empty() {
            None
        } else {
            Some(format!("missing: {}", missing.join(", ")))
        },
    }
}

fn check_s7(nodes: &[std::path::PathBuf], _root: &Path) -> CheckResult {
    let oversize: Vec<_> = nodes
        .iter()
        .filter(|p| line_count(p) > 40)
        .map(|p| {
            format!(
                "{} ({}L)",
                p.file_name().unwrap_or_default().to_string_lossy(),
                line_count(p)
            )
        })
        .collect();
    CheckResult {
        id: "S7".into(),
        description: "no corpus node exceeds 40 lines".into(),
        passed: oversize.is_empty(),
        detail: if oversize.is_empty() {
            None
        } else {
            Some(oversize.join(", "))
        },
    }
}
