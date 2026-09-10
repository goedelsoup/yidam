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

// ── manifest.yml ──────────────────────────────────────────────────────────────

/// The subset of an unpacked bundle's `manifest.yml` that anything here reads.
///
/// Here rather than in `cmd/tonpa/install.rs`, where it used to live, because the readers
/// are on both sides of the `tonpa` feature gate: `install` decodes a manifest it has just
/// extracted, `lint`'s external-citation checks decode one on disk, and `doctor` asks
/// whether an unpacked corpus is the one a path dependency shadows — and only the first of
/// those can fetch anything. There were two decoders of this file before this struct, one
/// per side, and they already disagreed about which fields exist.
///
/// Every field is `Option`, and that is the format's versioning policy rather than caution:
/// adding a field to `manifest.yml` is non-breaking, so a reader compiled today meets
/// bundles written before any given field existed. Absence is a legitimate answer here and
/// never an error.
#[derive(Debug, Default, serde::Deserialize)]
pub struct BundleManifest {
    /// Short SHA of the HEAD commit when the bundle was produced. Its *length* is chosen by
    /// git from the producing repository's object count, so it is not comparable across
    /// repositories — RFC-0019 §2 is about exactly that.
    pub commit: Option<String>,
    /// ISO date (YYYY-MM-DD) of the genesis (first) commit.
    pub genesis: Option<String>,
    /// Full SHA of the genesis commit — the one value in the manifest that says *which
    /// corpus this is*. `None` for a bundle produced before the field existed, and for a
    /// tree with no root commit.
    pub genesis_hash: Option<String>,
    /// Name of the fastembed model used to build the vector index, if present.
    pub vector_index_model: Option<String>,
    /// Number of corpus instance files included in the bundle.
    /// Decoded for forward compatibility; not consumed by any command yet.
    #[allow(dead_code)]
    pub instances: Option<u64>,
}

/// Decode a `manifest.yml` body. A manifest that does not parse decodes as all-absent
/// rather than failing: a bundle whose manifest is unreadable is still a directory of
/// corpus files, and every caller has an answer for a field it did not get.
pub fn parse_manifest(yaml: &str) -> BundleManifest {
    serde_yaml::from_str(yaml).unwrap_or_default()
}

/// Read `<dir>/manifest.yml`. `None` when there is no manifest there at all, which is what
/// distinguishes "not an unpacked bundle" from "an unpacked bundle that says little".
pub fn read_manifest(dir: &Path) -> Option<BundleManifest> {
    std::fs::read_to_string(dir.join("manifest.yml"))
        .ok()
        .map(|text| parse_manifest(&text))
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

// ── one name, two corpora ─────────────────────────────────────────────────────

/// A path dependency and an unpacked bundle that claim one name and are not the same corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowedDependency {
    pub name: String,
    /// Genesis hash of the sibling repository the path dependency points at.
    pub path_genesis: String,
    /// Genesis hash from the unpacked bundle's `manifest.yml`.
    pub fetched_genesis: String,
}

/// Names under which two *different* corpora are installed, one shadowing the other.
///
/// [`resolved`] drops the unpacked copy when a path dependency claims its name, and does so
/// silently — deliberately, because the overwhelmingly common case is that they are the same
/// corpus in two forms: someone fetched a dependency and then pointed at a local checkout of
/// it to edit. Reporting every shadow would report that, constantly, for no defect.
///
/// **The genesis digest is what separates the two cases**, and it is the whole reason
/// RFC-0032 §4.5 asked the manifest for one. Same hash: one corpus read two ways, and
/// preferring the checkout is right. Different hash: two corpora claim one name, the reader
/// is silently reading one of them, and `tonpa.lock` pins the other — a wrong answer rather
/// than a missing one, which nothing could say before the manifest carried the field.
///
/// Silent when either side's hash is unknown: a sibling that is not a git repository, or a
/// bundle produced before `genesis_hash` existed. **Unknown is not different.** Guessing
/// either way would either invent a conflict or hide one, and the honest answer to "are these
/// the same corpus" with one identity missing is that it cannot be told.
///
/// Mirrors [`resolved`]'s own condition rather than restating it: a path dependency whose
/// corpus directory is not there does not shadow anything, because `resolved` skips it and
/// the unpacked copy is what gets read.
pub fn shadowed(root: &Path) -> Vec<ShadowedDependency> {
    let config = load_config(&crate::paths::tonpa_config_path(root));
    let tonpa_dir = crate::paths::tonpa_dir(root);
    let mut out: Vec<ShadowedDependency> = Vec::new();

    for (name, dep) in &config.dependencies {
        let Some(rel) = &dep.path else { continue };
        let sibling = root.join(rel);
        if !crate::paths::yidam_corpus_dir(&sibling).is_dir() {
            continue;
        }
        let unpacked = tonpa_dir.join(name);
        let Some(manifest) = read_manifest(&unpacked) else {
            continue;
        };
        let (Some(path_genesis), Some(fetched_genesis)) =
            (crate::git::genesis_hash(&sibling), manifest.genesis_hash)
        else {
            continue;
        };
        if path_genesis != fetched_genesis {
            out.push(ShadowedDependency {
                name: name.clone(),
                path_genesis,
                fetched_genesis,
            });
        }
    }

    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // ── one manifest shape ────────────────────────────────────────────────────

    #[test]
    fn a_manifest_carrying_the_genesis_hash_decodes_it() {
        let m = parse_manifest(
            "bundle_version: \"1\"\ncommit: \"abc1234\"\ngenesis: \"2026-01-01\"\n\
             genesis_hash: \"da4eeb36530f1111222233334444555566667777\"\ninstances: 8\n",
        );
        assert_eq!(
            m.genesis_hash.as_deref(),
            Some("da4eeb36530f1111222233334444555566667777")
        );
        assert_eq!(m.commit.as_deref(), Some("abc1234"));
    }

    /// The versioning policy in one assertion: every bundle written before `genesis_hash`
    /// existed still decodes, and the field it does not carry reads as absent. A reader that
    /// errored here would make an additive field a breaking one.
    #[test]
    fn a_bundle_from_before_the_field_still_decodes_and_says_absent() {
        let m = parse_manifest(
            "bundle_version: \"1\"\ncommit: \"abc1234\"\ngenesis: \"2026-01-01\"\n\
             generated_at: 0\ndomain: \"d\"\nvector_index_model: null\n",
        );
        assert_eq!(m.genesis_hash, None);
        assert_eq!(m.commit.as_deref(), Some("abc1234"));
    }

    #[test]
    fn an_explicit_null_is_absent_and_not_a_parse_failure() {
        let m = parse_manifest("commit: \"abc1234\"\ngenesis_hash: null\n");
        assert_eq!(m.genesis_hash, None);
        assert_eq!(
            m.commit.as_deref(),
            Some("abc1234"),
            "a null in one field must not lose the others"
        );
    }

    /// A directory that is not an unpacked bundle and one whose manifest says little are
    /// different answers, and `shadowed` needs them to be: the first shadows nothing.
    #[test]
    fn a_directory_with_no_manifest_reads_as_no_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(read_manifest(tmp.path()).is_none());
        std::fs::write(tmp.path().join("manifest.yml"), "not: a manifest\n").unwrap();
        assert!(read_manifest(tmp.path()).is_some());
    }

    // ── one name, two corpora ─────────────────────────────────────────────────

    fn git(dir: &Path, args: &[&str]) {
        let ok = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap()
            .success();
        assert!(ok, "git {args:?} failed");
    }

    /// A corpus with one commit, so it has a genesis hash of its own.
    fn corpus(dir: &Path) -> String {
        std::fs::create_dir_all(crate::paths::yidam_corpus_dir(dir)).unwrap();
        std::fs::write(
            crate::paths::yidam_corpus_dir(dir).join("a.ont.yml"),
            "x: 1\n",
        )
        .unwrap();
        git(dir, &["init", "-q", "-b", "main"]);
        git(dir, &["config", "user.email", "t@t.com"]);
        git(dir, &["config", "user.name", "T"]);
        git(dir, &["config", "commit.gpgsign", "false"]);
        git(dir, &["add", "-A"]);
        git(
            dir,
            &["commit", "-q", "--no-gpg-sign", "-m", "genesis: a corpus"],
        );
        crate::git::genesis_hash(dir).expect("the fixture repository has a root commit")
    }

    /// `root` declares `dep` as a path dependency pointing at `sibling`, and has an unpacked
    /// bundle of the same name whose manifest carries `unpacked_hash`.
    fn shadow_fixture(unpacked_hash: Option<&str>) -> (tempfile::TempDir, String) {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("root");
        let sibling = tmp.path().join("sibling");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&sibling).unwrap();
        let sibling_hash = corpus(&sibling);

        std::fs::create_dir_all(root.join(".yidam")).unwrap();
        std::fs::write(
            crate::paths::tonpa_config_path(&root),
            "[dependencies.dep]\npath = \"../sibling\"\n",
        )
        .unwrap();

        let unpacked = crate::paths::tonpa_dir(&root).join("dep");
        std::fs::create_dir_all(&unpacked).unwrap();
        let hash_line = match unpacked_hash {
            Some(h) => format!("genesis_hash: \"{h}\"\n"),
            None => String::new(),
        };
        std::fs::write(
            unpacked.join("manifest.yml"),
            format!("bundle_version: \"1\"\ncommit: \"abc1234\"\n{hash_line}"),
        )
        .unwrap();
        (tmp, sibling_hash)
    }

    fn root_of(tmp: &tempfile::TempDir) -> std::path::PathBuf {
        tmp.path().join("root")
    }

    /// The defect the digest exists to name. Nothing before this could say it: the two
    /// corpora agree on their declared name, and `genesis` is a *date*, which two corpora
    /// created on one day also agree on.
    #[test]
    fn two_different_corpora_under_one_name_are_reported() {
        let (tmp, sibling_hash) = shadow_fixture(Some("0000111122223333444455556666777788889999"));
        let found = shadowed(&root_of(&tmp));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].name, "dep");
        assert_eq!(found[0].path_genesis, sibling_hash);
        assert_eq!(
            found[0].fetched_genesis,
            "0000111122223333444455556666777788889999"
        );
    }

    /// The common case, and the reason `resolved` is silent: someone fetched a dependency
    /// and then pointed at a checkout of the same corpus to edit it. Reporting this would
    /// report the normal development loop as a defect.
    #[test]
    fn one_corpus_in_two_forms_is_not_a_shadow() {
        let (tmp, sibling_hash) = shadow_fixture(None);
        let unpacked = crate::paths::tonpa_dir(&root_of(&tmp)).join("dep");
        std::fs::write(
            unpacked.join("manifest.yml"),
            format!("bundle_version: \"1\"\ngenesis_hash: \"{sibling_hash}\"\n"),
        )
        .unwrap();
        assert!(shadowed(&root_of(&tmp)).is_empty());
    }

    /// Unknown is not different. A bundle produced before the field existed cannot be
    /// compared, and inventing a conflict there would make every pre-existing installation
    /// report one.
    #[test]
    fn a_bundle_with_no_digest_cannot_be_compared_and_is_not_reported() {
        let (tmp, _) = shadow_fixture(None);
        assert!(shadowed(&root_of(&tmp)).is_empty());
    }

    /// The other half of the same rule: a sibling that is not a git repository has no
    /// identity to compare either.
    #[test]
    fn a_sibling_that_is_not_a_repository_cannot_be_compared() {
        let (tmp, _) = shadow_fixture(Some("0000111122223333444455556666777788889999"));
        std::fs::remove_dir_all(tmp.path().join("sibling/.git")).unwrap();
        assert!(shadowed(&root_of(&tmp)).is_empty());
    }

    /// Mirrors `resolved`'s own condition. A path dependency whose corpus directory is not
    /// there is skipped by the reader, so the unpacked copy is what gets read and nothing is
    /// being shadowed — reporting a conflict here would name a corpus nobody reads.
    #[test]
    fn a_path_dependency_that_does_not_resolve_shadows_nothing() {
        let (tmp, _) = shadow_fixture(Some("0000111122223333444455556666777788889999"));
        let root = root_of(&tmp);
        assert_eq!(shadowed(&root).len(), 1, "the fixture must start reported");
        std::fs::remove_dir_all(crate::paths::yidam_corpus_dir(&tmp.path().join("sibling")))
            .unwrap();
        assert!(shadowed(&root).is_empty());
        assert!(
            !resolved(&root)
                .iter()
                .any(|d| d.kind == DependencyKind::Path),
            "the reader must agree it is not reading the checkout"
        );
    }

    #[test]
    fn a_name_with_no_unpacked_bundle_is_not_a_shadow() {
        let (tmp, _) = shadow_fixture(Some("0000111122223333444455556666777788889999"));
        let root = root_of(&tmp);
        std::fs::remove_dir_all(crate::paths::tonpa_dir(&root).join("dep")).unwrap();
        assert!(shadowed(&root).is_empty());
    }

    #[test]
    fn a_repository_declaring_nothing_reports_nothing() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(shadowed(tmp.path()).is_empty());
    }
}
