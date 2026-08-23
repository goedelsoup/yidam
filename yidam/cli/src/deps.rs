//! What this repository depends on, and where those corpora actually live.
//!
//! Two kinds of dependency, and the difference is where the corpus is read from rather than
//! anything about what it is:
//!
//! - **Fetched** — a `.yiz` bundle downloaded, hashed, locked, and unpacked into
//!   `.yidam/tonpa/<name>/`. Reproducible, pinned, and stale by construction: it is whatever
//!   was published, until someone updates it.
//! - **Path** — a sibling repository read where it sits. Not fetched, not hashed, not
//!   locked, because hashing a working tree that changes under you records nothing. This is
//!   the only form that supports a development loop: an edit in the producer is visible in
//!   the consumer without cutting a release.
//!
//! This module is **not** behind the `tonpa` feature. That feature buys the network —
//! resolving a source, fetching an archive, writing a lock. Knowing what a repository
//! depends on, and reading it, needs none of that, and a derived repository installs the
//! light build.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

// ── tonpa.toml ────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct TonpaConfig {
    pub package: Option<PackageMeta>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, Dependency>,
    pub index: Option<IndexConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PackageMeta {
    pub name: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct Dependency {
    pub url: Option<String>,
    pub github: Option<String>,
    pub tag: Option<String>,
    pub path: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct IndexConfig {
    #[serde(default = "default_true")]
    pub merge_imported_index: bool,
}

fn default_true() -> bool {
    true
}

/// Read `.yidam/tonpa.toml`, or an empty config when there is none.
///
/// A repository with no dependencies and a repository that has never declared any are the
/// same answer here, deliberately: every caller wants "what does this depend on", and
/// neither is an error.
pub fn load_config(path: &Path) -> TonpaConfig {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str(&s).ok())
        .unwrap_or_default()
}

// ── where a dependency's corpus is ────────────────────────────────────────────

/// How a dependency arrived, which is what decides whether it can be pinned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    /// Unpacked from a fetched bundle under `.yidam/tonpa/<name>/`.
    Fetched,
    /// Read in place from a sibling repository.
    Path,
}

/// A dependency resolved to the directory its corpus is actually read from.
#[derive(Debug, Clone)]
pub struct ResolvedDependency {
    pub name: String,
    /// The directory holding `<class>/<name>.yml` instance files.
    pub corpus_dir: PathBuf,
    pub kind: DependencyKind,
}

/// Every dependency whose corpus can be read right now, sorted by name.
///
/// Path dependencies come from `tonpa.toml`; fetched ones from what is unpacked on disk.
/// The two are deliberately sourced differently: a fetched dependency is present because it
/// was installed, and a declared-but-uninstalled one is `tonpa status`'s question, not this
/// one. A path dependency has nothing to install, so the declaration is all there is.
///
/// A path that does not resolve is skipped rather than reported here — a sibling checkout
/// that is not on this machine is a normal state for a repository someone else cloned, and
/// this function answers "what can be read", not "what is wrong".
pub fn resolved(root: &Path) -> Vec<ResolvedDependency> {
    let mut out: Vec<ResolvedDependency> = Vec::new();

    // Path dependencies, declared in tonpa.toml and read where they sit.
    let config = load_config(&crate::paths::tonpa_config_path(root));
    for (name, dep) in &config.dependencies {
        let Some(rel) = &dep.path else { continue };
        // Relative to the repository root, which is what a person writing `../sibling` in
        // `.yidam/tonpa.toml` means — not relative to the config file two levels down.
        let corpus_dir = crate::paths::yidam_corpus_dir(&root.join(rel));
        if corpus_dir.is_dir() {
            out.push(ResolvedDependency {
                name: name.clone(),
                corpus_dir,
                kind: DependencyKind::Path,
            });
        }
    }

    // Fetched dependencies, unpacked under .yidam/tonpa/<name>/.
    let dir = crate::paths::tonpa_dir(root);
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut fetched: Vec<ResolvedDependency> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir() && e.path().join("manifest.yml").is_file())
            .filter_map(|e| {
                let name = e.file_name().to_str()?.to_string();
                Some(ResolvedDependency {
                    name,
                    corpus_dir: e.path().join("corpus"),
                    kind: DependencyKind::Fetched,
                })
            })
            .collect();
        // A name declared as a path dependency wins over an unpacked directory of the same
        // name. The path form is the one someone is actively editing; silently preferring a
        // stale unpacked copy would make an edit appear to have no effect, which is the one
        // failure a development loop must not have.
        fetched.retain(|f| !out.iter().any(|p| p.name == f.name));
        out.extend(fetched);
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}
