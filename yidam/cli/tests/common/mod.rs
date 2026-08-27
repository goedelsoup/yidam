//! The layout bootstrap installs, and how to build one.
//!
//! Shared by [`installed_layout_links`], which checks that every relative link resolves in
//! this layout, and [`derived_repo_smoke`], which materializes it and runs its gate. One
//! definition, because a test checking a layout nobody installs is worse than no test — and
//! two copies of the mapping is the shortest path to exactly that.

#![allow(dead_code)] // each test binary uses a different part of this

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One template path and where bootstrap puts it.
pub struct Install {
    /// Path in this repository, a file or a directory prefix.
    pub src: &'static str,
    /// Installed path in a derived repository. `None` means consumed at genesis — present
    /// here, absent in every derived repository.
    pub dst: Option<&'static str>,
    /// The bootstrap answer this row is contingent on, if any. `None` is unconditional:
    /// every derived repository has it.
    pub when: Option<&'static str>,
}

/// Governance mode `sadhana/sangha/` requires. The default is `single-elector`, which
/// installs no sangha at all.
pub const COLLECTIVE: &str = "governance: collective";

pub const MAPPING: &[Install] = &[
    // The vendored prelude. One directory, deliberately: everything else under `yidam/`
    // is yidam's own machinery and does not survive the vendor step.
    row("yidam/prelude", Some(".yidam/.vendor/prelude")),
    // Root files. `sadhana/root/` is not a directory mirror — each file installs to a
    // specific path, overwriting yidam's own copy.
    row("sadhana/root/README.md", Some("README.md")),
    row("sadhana/root/AGENTS.md", Some("AGENTS.md")),
    row("sadhana/root/CLAUDE.md", Some(".claude/CLAUDE.md")),
    row("sadhana/root/mise.toml", Some("mise.toml")),
    row("sadhana/root/gitattributes", Some(".gitattributes")),
    row("sadhana/root/gitignore", Some(".gitignore")),
    row(
        "sadhana/github/workflows/ci.yml",
        Some(".github/workflows/ci.yml"),
    ),
    // Overwrites yidam's own release.yml, which publishes the CLI's binaries on a `cli/v*`
    // tag — from a repository that has no CLI. Left in place it is a workflow waiting to run
    // against the wrong thing.
    row(
        "sadhana/github/workflows/release.yml",
        Some(".github/workflows/release.yml"),
    ),
    // Directory mirrors.
    Install {
        src: "sadhana/sangha",
        dst: Some(".yidam/sangha"),
        when: Some(COLLECTIVE),
    },
    row("sadhana/catalog", Some(".yidam/catalog")),
    row("sadhana/corpus", Some(".yidam/corpus")),
    row("sadhana/skills", Some(".yidam/skills")),
    row("sadhana/crates", Some("crates")),
    row("sadhana/web", Some("web")),
    // Created on first use rather than at genesis, but they install here when they are.
    row("sadhana/agents", Some("agents")),
    row("sadhana/packages", Some("packages")),
    row("sadhana/docs", Some("docs")),
    // Consumed.
    row("sadhana/README.md", None),
    row("samudaya", None),
];

/// An unconditional row — everything but the sangha.
const fn row(src: &'static str, dst: Option<&'static str>) -> Install {
    Install {
        src,
        dst,
        when: None,
    }
}

/// Files a derived repository keeps from the template root, unchanged.
///
/// The bootstrap skill names these explicitly: "Keep `LICENSE` and `mise.yidam.toml`".
/// `.gitattributes` and `.gitignore` are *not* among them — both arrive through [`MAPPING`],
/// overwritten in step 3 from `sadhana/root/`. `.gitignore` was on this list for as long as
/// bootstrap called it generic, which shipped yidam's own — including an ignore for a path
/// under `yidam/tests/` that the vendor step deletes.
pub const KEPT_AT_ROOT: &[&str] = &["LICENSE", "mise.yidam.toml"];

/// Directories bootstrap creates empty.
pub const CREATED_EMPTY: &[&str] = &[".yidam/decisions", ".yidam/embeddings", ".yidam/index"];

/// Paths a derived repository holds regardless of what any [`MAPPING`] row produces.
///
/// Mostly things no template file becomes — the provenance pin, the directories bootstrap
/// creates empty. `.gitattributes` and `.gitignore` are the exceptions and are redundant
/// here: both also arrive through [`MAPPING`], and the tree is a set, so naming them twice
/// costs nothing and asserts they are present however they got there.
pub const ALWAYS_PRESENT: &[&str] = &[
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

/// The row covering `rel`, and where `rel` itself lands.
pub fn install_of(rel: &str) -> Option<(&'static Install, Option<String>)> {
    for e in MAPPING {
        if rel == e.src {
            return Some((e, e.dst.map(str::to_string)));
        }
        let prefix = format!("{}/", e.src);
        if let Some(tail) = rel.strip_prefix(&prefix) {
            return Some((e, e.dst.map(|d| format!("{d}/{tail}"))));
        }
    }
    None
}

/// Files git tracks under `prefix`, repo-relative.
///
/// Tracked rather than walked, because the production vendor step copies out of a
/// `git clone` and therefore never sees `target/`, `__pycache__`, `.pytest_cache` or
/// `.DS_Store`. A directory walk of a working tree picks up ~38,000 files against the 495 a
/// derived repository actually receives, and would have this test measuring the maintainer's
/// build output.
///
/// `ls-files` and not `archive HEAD`, so an uncommitted edit is covered too — the point is
/// to test the tree about to be committed, not the one already was.
pub fn tracked_under(root: &Path, prefix: &str) -> Vec<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z", "--", prefix])
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed for {prefix}");
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Build the repository bootstrap produces, in `target`.
///
/// `conditions` is what the bootstrap dialogue answered yes to; an empty set is
/// single-elector, the documented default and the case the skill takes when the user is
/// unsure. Returns the number of files written.
pub fn materialize(root: &Path, target: &Path, conditions: &BTreeSet<&str>) -> usize {
    let mut written = 0;
    for e in MAPPING {
        if e.dst.is_none() {
            continue; // consumed at genesis
        }
        if e.when.is_some_and(|w| !conditions.contains(w)) {
            continue; // the answer this row needed was not given
        }
        for tracked in tracked_under(root, e.src) {
            let dest = match install_of(&tracked) {
                Some((_, Some(d))) => d,
                _ => continue,
            };
            let to = target.join(&dest);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(&tracked), &to)
                .unwrap_or_else(|e| panic!("copy {tracked} -> {dest}: {e}"));
            written += 1;
        }
    }
    for keep in KEPT_AT_ROOT {
        std::fs::copy(root.join(keep), target.join(keep)).unwrap_or_else(|e| panic!("{keep}: {e}"));
        written += 1;
    }
    for dir in CREATED_EMPTY {
        std::fs::create_dir_all(target.join(dir)).unwrap();
    }
    written
}
