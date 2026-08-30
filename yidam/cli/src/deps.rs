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

// ── tonpa.lock ────────────────────────────────────────────────────────────────
//
// Here rather than in `cmd/tonpa/config.rs`, for the reason that module already gives about
// `Dependency` and `TonpaConfig`: they lived beside the fetching commands while only the
// fetching commands read them, and moved when a read layer needed them too. `doctor` is now
// that read layer for the lock — it answers "did the corpora arrive" without being able to
// fetch anything — and two definitions of "is this bundle the one we pinned" is exactly the
// drift this repository keeps finding in other people's code.
//
// It also has to be reachable from a build without the `tonpa` feature. `cmd::tonpa` is
// gated on it; `sha2` is not optional, and neither is reading a file, so nothing here needs
// the gate. A binary that cannot fetch a corpus can still say whether one is missing.

#[derive(Debug, serde::Deserialize, serde::Serialize, Default)]
pub struct LockFile {
    // TOML [[package]] array-of-tables
    #[serde(default, rename = "package")]
    pub packages: Vec<LockedPackage>,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct LockedPackage {
    pub name: String,
    pub url: String,
    pub sha256: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genesis: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dims: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nodes: Option<u64>,
}

pub fn load_lock(path: &Path) -> anyhow::Result<LockFile> {
    use anyhow::Context;
    if !path.exists() {
        return Ok(LockFile::default());
    }
    let text =
        std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}

pub fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(data);
    hex::encode(h.finalize())
}

/// Is the bundle unpacked at `<tonpa_dir>/<name>/` the one `tonpa.lock` pins?
///
/// `false` for both "not there" and "there but different", because the caller that fetches
/// treats them the same — it re-fetches at the pinned hash either way. A caller that cannot
/// fetch has to tell them apart itself; the bundle path is where it looks.
pub fn verify_installed(
    name: &str,
    tonpa_dir: &Path,
    locked: &LockedPackage,
) -> anyhow::Result<bool> {
    let bundle_path = tonpa_dir.join(name).join("bundle.yiz");
    if !bundle_path.exists() {
        return Ok(false);
    }
    let data = std::fs::read(&bundle_path)?;
    Ok(sha256_hex(&data) == locked.sha256)
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
