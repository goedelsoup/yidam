//! Prelude and scaffold links must resolve in the tree they are *installed* into.
//!
//! These files are written for where they land, not where they sit. `yidam/prelude/`
//! installs to `.yidam/.vendor/prelude/`; `sadhana/root/AGENTS.md` installs to the
//! repository root; `samudaya/` is deleted at genesis. So a relative link here is
//! checkable only against the layout bootstrap produces, and checking it against *this*
//! repository gets the answer backwards in both directions:
//!
//! | link | here | installed |
//! |---|---|---|
//! | `GRAPH.md` → `../../sangha/README.md` | absent | `.yidam/sangha/README.md` — correct |
//! | `directories.md` → `../../../samudaya/README.md` | present | `.yidam/samudaya/` — absent |
//!
//! One is right where it looks wrong; the other is wrong where it looks right. A lexical
//! rule ("never escape the prelude") flags both identically and was tried and discarded
//! for that reason. What works is modelling the destination.
//!
//! The mapping below is the one `bootstrap.md` prescribes, and
//! [`the_mapping_agrees_with_bootstrap`] holds the two together: bootstrap's own
//! `source → destination` table is parsed and every row must appear here. A test checking
//! a layout nobody installs would be worse than no test.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// `(source prefix, installed prefix)`. `None` means consumed at genesis — present here,
/// absent in every derived repository.
const MAPPING: &[(&str, Option<&str>)] = &[
    // The vendored prelude. One directory, deliberately: everything else under `yidam/`
    // is yidam's own machinery and does not survive the vendor step.
    ("yidam/prelude", Some(".yidam/.vendor/prelude")),
    // Root files. `sadhana/root/` is not a directory mirror — each file installs to a
    // specific path, overwriting yidam's own copy.
    ("sadhana/root/README.md", Some("README.md")),
    ("sadhana/root/AGENTS.md", Some("AGENTS.md")),
    ("sadhana/root/CLAUDE.md", Some(".claude/CLAUDE.md")),
    ("sadhana/root/mise.toml", Some("mise.toml")),
    ("sadhana/root/gitattributes", Some(".gitattributes")),
    (
        "sadhana/github/workflows/ci.yml",
        Some(".github/workflows/ci.yml"),
    ),
    // Directory mirrors.
    ("sadhana/sangha", Some(".yidam/sangha")),
    ("sadhana/catalog", Some(".yidam/catalog")),
    ("sadhana/corpus", Some(".yidam/corpus")),
    ("sadhana/skills", Some(".yidam/skills")),
    ("sadhana/crates", Some("crates")),
    ("sadhana/web", Some("web")),
    // Created on first use rather than at genesis, but they install here when they are.
    ("sadhana/agents", Some("agents")),
    ("sadhana/packages", Some("packages")),
    ("sadhana/docs", Some("docs")),
    // Consumed.
    ("sadhana/README.md", None),
    ("samudaya", None),
];

/// Paths a derived repository has that no template file becomes.
///
/// Written down rather than inferred: each is a real destination a link may point at, and
/// a link to one of them is correct even though nothing installs *to* it.
const ALWAYS_PRESENT: &[&str] = &[
    "LICENSE",
    ".gitignore",
    ".gitattributes",
    "mise.yidam.toml",
    ".yidam.toml",
    ".yidam",
    ".yidam/decisions",
    ".yidam/embeddings",
    ".yidam/index",
    ".yidam/private-paths",
];

fn installed_path(rel: &str) -> Option<Option<String>> {
    for (src, dst) in MAPPING {
        if rel == *src {
            return Some(dst.map(str::to_string));
        }
        let prefix = format!("{src}/");
        if let Some(tail) = rel.strip_prefix(&prefix) {
            return Some(dst.map(|d| format!("{d}/{tail}")));
        }
    }
    None
}

/// Every path a fresh derived repository holds, files and directories alike.
fn installed_tree(root: &Path) -> BTreeSet<String> {
    let mut tree: BTreeSet<String> = ALWAYS_PRESENT.iter().map(|s| s.to_string()).collect();
    for (src, _) in MAPPING {
        for entry in WalkDir::new(root.join(src))
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let s = entry.path().to_string_lossy();
            if s.contains("/node_modules/")
                || s.contains("/.venv/")
                || s.contains("/target/")
                || s.contains("/__pycache__/")
                || s.contains("/.pytest_cache/")
            {
                continue;
            }
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let Some(Some(dest)) = installed_path(&rel.to_string_lossy()) else {
                continue;
            };
            // Every ancestor is a directory that exists.
            let mut acc = PathBuf::new();
            for part in Path::new(&dest).components() {
                acc.push(part);
                tree.insert(acc.to_string_lossy().to_string());
            }
        }
    }
    tree
}

/// Replace the contents of `` `…` `` spans with spaces, preserving byte offsets.
fn blank_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(c);
        } else if in_code {
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Markdown link targets worth resolving, skipping fenced and inline code.
fn targets(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut fence: Option<String> = None;
    for (n, raw) in text.lines().enumerate() {
        let trimmed = raw.trim_start();
        if let Some(open) = &fence {
            if trimmed.starts_with(open.as_str()) {
                fence = None;
            }
            continue;
        }
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fence = Some(trimmed.chars().take(3).collect());
            continue;
        }
        // Blank inline code so `[label](path)` shown AS an example is not read as a link.
        // Four of this test's first six findings were this, and every one of them was a
        // document explaining what an edge looks like.
        let line = blank_code_spans(raw);
        let mut j = 0;
        while let Some(open) = line[j..].find("](") {
            let at = j + open + 2;
            let Some(close) = line[at..].find(')') else {
                break;
            };
            let t = line[at..at + close].trim();
            j = at + close + 1;
            let t = t.split_whitespace().next().unwrap_or(t);
            let t = t.split('#').next().unwrap_or(t);
            if t.is_empty()
                || t.starts_with('#')
                || t.contains("://")
                || t.starts_with("mailto:")
                || t.starts_with('/')
            {
                continue;
            }
            out.push((n + 1, t.to_string()));
        }
    }
    out
}

/// Resolve `target` from `from_dir` lexically. `None` if it climbs above the root.
fn resolve(from_dir: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if from_dir.is_empty() {
        vec![]
    } else {
        from_dir.split('/').collect()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

#[test]
fn every_template_link_resolves_in_the_installed_layout() {
    let root = repo_root();
    let tree = installed_tree(&root);
    assert!(
        tree.contains(".yidam/.vendor/prelude/GRAPH.md"),
        "the installed tree is not being built — mapping or walk is wrong"
    );

    let mut bad = Vec::new();
    let mut checked = 0usize;

    for (src, dst) in MAPPING {
        if dst.is_none() {
            continue; // consumed; its links go nowhere because it goes nowhere
        }
        for entry in WalkDir::new(root.join(src))
            .into_iter()
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if path.extension().is_none_or(|x| x != "md") {
                continue;
            }
            let s = path.to_string_lossy();
            if s.contains("/node_modules/") || s.contains("/.venv/") || s.contains("/target/") {
                continue;
            }
            let Ok(rel) = path.strip_prefix(&root) else {
                continue;
            };
            let Some(Some(dest)) = installed_path(&rel.to_string_lossy()) else {
                continue;
            };
            let dest_dir = Path::new(&dest)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            let Ok(text) = std::fs::read_to_string(path) else {
                continue;
            };
            for (line, target) in targets(&text) {
                checked += 1;
                match resolve(&dest_dir, &target) {
                    Some(r) if tree.contains(&r) => {}
                    Some(r) => bad.push(format!(
                        "  {}:{} — `{}`\n      installs to {}\n      resolves to {} — absent",
                        rel.display(),
                        line,
                        target,
                        dest,
                        r
                    )),
                    None => bad.push(format!(
                        "  {}:{} — `{}` climbs above the repository root from {}",
                        rel.display(),
                        line,
                        target,
                        dest
                    )),
                }
            }
        }
    }

    assert!(checked > 0, "no relative links found — the scan is broken");
    assert!(
        bad.is_empty(),
        "{} link(s) do not resolve in the layout bootstrap installs:\n{}\n\n\
         These files are written for where they LAND, not where they sit. Count `../` from \
         the installed path, or use an absolute https:// URL to reach outside the derived \
         repository entirely.",
        bad.len(),
        bad.join("\n")
    );
}

/// Bootstrap's own `source → destination` table must be a subset of [`MAPPING`].
///
/// The mapping is only worth having if it is the one that gets installed. If bootstrap
/// grows a row and this does not, the test starts checking a layout nobody produces —
/// which is the failure mode the issue behind this test named specifically.
#[test]
fn the_mapping_agrees_with_bootstrap() {
    let root = repo_root();
    let text = std::fs::read_to_string(root.join("yidam/prelude/skills/bootstrap.md")).unwrap();

    let mut rows = 0;
    for line in text.lines() {
        let Some((lhs, rhs)) = line.split_once('→') else {
            continue;
        };
        let src = lhs.trim();
        if !src.starts_with("sadhana/") {
            continue;
        }
        // `dest (overwrites yidam's)` — the parenthetical is commentary.
        let dst = rhs.trim().split_whitespace().next().unwrap_or("").trim();
        if dst.is_empty() {
            continue;
        }
        rows += 1;
        let found = MAPPING
            .iter()
            .any(|(s, d)| *s == src && d.map(|d| d == dst).unwrap_or(false));
        assert!(
            found,
            "bootstrap.md maps `{src}` → `{dst}`, and MAPPING in this test does not. \
             Update the table here, or the layout being checked is not the one installed."
        );
    }
    assert!(
        rows >= 6,
        "found only {rows} `→` rows in bootstrap.md — the parse is broken, not the doc"
    );
}

#[test]
fn resolve_is_lexical_and_bounded() {
    assert_eq!(
        resolve(".yidam/.vendor/prelude", "GRAPH.md").unwrap(),
        ".yidam/.vendor/prelude/GRAPH.md"
    );
    // The case a same-tree checker gets backwards.
    assert_eq!(
        resolve(".yidam/.vendor/prelude", "../../sangha/README.md").unwrap(),
        ".yidam/sangha/README.md"
    );
    assert_eq!(resolve("", "AGENTS.md").unwrap(), "AGENTS.md");
    assert_eq!(
        resolve(".yidam/sangha", "../.vendor/prelude/CONSTITUTION.md").unwrap(),
        ".yidam/.vendor/prelude/CONSTITUTION.md"
    );
    // Above the root is not a path.
    assert!(resolve("", "../outside.md").is_none());
}
