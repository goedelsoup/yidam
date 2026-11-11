//! A path dependency pinned to a commit (#1466): a sibling corpus read **at a sha**, out of
//! its object store, and never out of its working tree.
//!
//! The two existing forms each give up something a repository with local siblings and no
//! releases needs. A fetched dependency is pinned but needs a published `bundle.yiz`; a path
//! dependency needs nothing published but is unpinned by design, because a working tree that
//! changes under you has no moment to record. A commit is such a moment, and git already
//! holds it.
//!
//! **Materialised, not read through.** The corpus at the pin is written to
//! `.yidam/tonpa/<name>/` — the directory a fetched bundle unpacks to, with a `manifest.yml`
//! naming the commit — so every reader of an installed dependency reads this one without
//! learning a third form: `query`, `lint`'s citation checks (which take the pin from the
//! manifest), `check-diff`'s vocabulary, `serve`. A reader that ran `git show <sha>:…` itself
//! would have to exist once per reader, and each would need the sibling on disk at read time.

use anyhow::{bail, Context, Result};
use std::path::{Component, Path};

use super::config::LockedPackage;
use crate::git::Git;

/// The corpus directory, as git names it inside a commit.
const CORPUS: &str = ".yidam/corpus";

/// Resolve `rev` in `sibling` to the full SHA of a commit.
///
/// Stored resolved, never as given. An abbreviation is a pin only until the sibling grows
/// enough objects to make it ambiguous, and a branch or tag name is not a pin at all.
pub fn resolve(sibling: &Path, rev: &str) -> Result<String> {
    if !sibling.is_dir() {
        bail!("no repository at {}", sibling.display());
    }
    Git::new(sibling)
        .args(["rev-parse", "--verify", "--quiet"])
        .rev(format!("{rev}^{{commit}}"))
        .try_run()
        .filter(|s| !s.is_empty())
        .with_context(|| format!("`{rev}` is not a commit in {}", sibling.display()))
}

/// Every file under `.yidam/corpus/` at `sha`, relative to the corpus directory, with its
/// bytes.
///
/// `ls-tree` then one `cat-file --batch`, rather than `git archive`: an archive honours the
/// sibling's `export-ignore` and `export-subst` attributes, so it can omit a file or rewrite
/// one, and a pin that silently differs from the commit it names is the defect this exists
/// to remove. Blobs only — a symlink (mode `120000`) is a path, not corpus content, and a
/// submodule has no blob here to read.
pub fn read_corpus_at(sibling: &Path, sha: &str) -> Result<Vec<(String, Vec<u8>)>> {
    let listing = Git::new(sibling)
        .args(["ls-tree", "-r", "-z", "--full-tree"])
        .rev(sha)
        .paths([CORPUS])
        .output()?;
    if !listing.status.success() {
        bail!(
            "git ls-tree {sha} in {} failed: {}",
            sibling.display(),
            String::from_utf8_lossy(&listing.stderr).trim()
        );
    }

    let mut files: Vec<(String, String)> = Vec::new(); // (rel, oid)
    for record in listing.stdout.split(|b| *b == 0).filter(|r| !r.is_empty()) {
        let record = String::from_utf8_lossy(record);
        // <mode> SP <type> SP <oid> TAB <path>
        let Some((meta, path)) = record.split_once('\t') else {
            bail!("unreadable ls-tree record: {record}");
        };
        let mut meta = meta.split(' ');
        let (Some(mode), Some(kind), Some(oid)) = (meta.next(), meta.next(), meta.next()) else {
            bail!("unreadable ls-tree record: {record}");
        };
        if kind != "blob" || mode == "120000" {
            continue;
        }
        let Some(rel) = path.strip_prefix(CORPUS).and_then(|r| r.strip_prefix('/')) else {
            continue;
        };
        files.push((rel.to_string(), oid.to_string()));
    }
    if files.is_empty() {
        bail!(
            "{} has no {CORPUS}/ at {sha} — a path dependency points at a yidam repository",
            sibling.display()
        );
    }

    let request: String = files.iter().map(|(_, oid)| format!("{oid}\n")).collect();
    let out = Git::new(sibling)
        .args(["cat-file", "--batch"])
        .stdin(request.into_bytes())
        .output()?;
    if !out.status.success() {
        bail!(
            "git cat-file in {} failed: {}",
            sibling.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    let blobs = parse_batch(&out.stdout, files.len())?;
    Ok(files.into_iter().map(|(rel, _)| rel).zip(blobs).collect())
}

/// The `cat-file --batch` response: `<oid> <type> <size>\n<size bytes>\n` per object, in the
/// order requested. Every object must be there; a missing one is an error rather than a
/// shorter corpus.
fn parse_batch(mut out: &[u8], expected: usize) -> Result<Vec<Vec<u8>>> {
    let mut blobs = Vec::with_capacity(expected);
    for _ in 0..expected {
        let Some(nl) = out.iter().position(|b| *b == b'\n') else {
            bail!("cat-file --batch ended early");
        };
        let header = String::from_utf8_lossy(&out[..nl]).into_owned();
        out = &out[nl + 1..];
        let size: usize = header
            .split(' ')
            .nth(2)
            .and_then(|s| s.parse().ok())
            .with_context(|| format!("cat-file --batch: {header}"))?;
        if out.len() < size + 1 {
            bail!("cat-file --batch ended inside an object: {header}");
        }
        blobs.push(out[..size].to_vec());
        out = &out[size + 1..];
    }
    Ok(blobs)
}

/// What `manifest.yml` records for a pinned path, in the field names a bundle uses so that
/// [`crate::deps::BundleManifest`] reads it unchanged. `source` and `path` are the additions,
/// and the manifest format already tolerates fields a reader does not know.
#[derive(serde::Serialize)]
struct Manifest<'a> {
    bundle_version: &'a str,
    source: &'a str,
    path: &'a str,
    commit: &'a str,
    genesis: Option<String>,
    genesis_hash: Option<String>,
    instances: usize,
    vector_index_model: Option<String>,
}

/// Write the sibling's corpus at `sha` to `<tonpa_dir>/<name>/`, and the lock entry for it.
///
/// `expected` is the digest the lock already holds, when it holds one for this commit. The
/// same commit cannot produce different content, so a mismatch means the lock was edited or
/// the digest's definition moved — either way not something to paper over by re-pinning.
///
/// The previous copy is removed first. A file the pin moved past would otherwise outlive it
/// and be read as part of the new one.
pub fn materialize(
    name: &str,
    rel: &str,
    sibling: &Path,
    sha: &str,
    tonpa_dir: &Path,
    expected: Option<&str>,
) -> Result<LockedPackage> {
    let entries = read_corpus_at(sibling, sha)?;
    let digest = crate::deps::tree_digest(&entries);
    if let Some(expected) = expected {
        if digest != expected {
            bail!(
                "{name} at {sha} does not match tonpa.lock\n  expected: {expected}\n  got:      {digest}"
            );
        }
    }

    let dest = tonpa_dir.join(name);
    if dest.exists() {
        std::fs::remove_dir_all(&dest).with_context(|| format!("clearing {}", dest.display()))?;
    }
    let corpus = dest.join("corpus");
    for (file, bytes) in &entries {
        let rel_path = Path::new(file);
        if !rel_path
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            bail!("{name} at {sha} has a corpus path outside itself: {file}");
        }
        let target = corpus.join(rel_path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, bytes).with_context(|| format!("writing {}", target.display()))?;
    }

    let instances = entries.iter().filter(|(f, _)| f.contains('/')).count();
    let (genesis_hash, genesis) = genesis_of(sibling, sha);
    let manifest = Manifest {
        bundle_version: "1",
        source: "path",
        path: rel,
        commit: sha,
        genesis: genesis.clone(),
        genesis_hash,
        instances,
        vector_index_model: None,
    };
    std::fs::write(
        dest.join("manifest.yml"),
        serde_yaml::to_string(&manifest).context("serialising manifest.yml")?,
    )?;

    Ok(LockedPackage {
        name: name.to_string(),
        url: rel.to_string(),
        sha256: digest,
        commit: Some(sha.to_string()),
        genesis,
        model: None,
        dims: None,
        nodes: Some(entries.len() as u64),
        rev: Some(sha.to_string()),
    })
}

/// The root commit `sha` descends from, and its date — the identity a bundle's manifest
/// carries. Read from the pinned commit's history rather than the sibling's HEAD, which may
/// be on another branch entirely. `None` in a shallow clone, for the reason
/// [`crate::git::genesis_hash`] gives.
fn genesis_of(sibling: &Path, sha: &str) -> (Option<String>, Option<String>) {
    if crate::git::is_shallow(sibling) {
        return (None, None);
    }
    let hash = Git::new(sibling)
        .args(["rev-list", "--max-parents=0"])
        .rev(sha)
        .lines()
        .and_then(|l| l.into_iter().next())
        .filter(|s| !s.is_empty());
    let date = hash.as_ref().and_then(|h| {
        Git::new(sibling)
            .args(["log", "-1", "--format=%as"])
            .rev(h)
            .try_run()
            .filter(|s| !s.is_empty())
    });
    (hash, date)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{git, git_out};

    /// A sibling corpus with two commits: `alpha` at the first, `beta` added at the second.
    /// Returns (tempdir, sibling, first sha, second sha).
    fn sibling() -> (tempfile::TempDir, std::path::PathBuf, String, String) {
        let tmp = tempfile::TempDir::new().unwrap();
        let s = tmp.path().join("sibling");
        let corpus = s.join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("concept")).unwrap();
        std::fs::write(corpus.join("concept.ont.yml"), "name: Concept\n").unwrap();
        std::fs::write(corpus.join("concept/alpha.yml"), "label: Alpha\n").unwrap();
        crate::git::fixture::init(&s);
        git(&s, &["add", "-A"]);
        git(
            &s,
            &["commit", "-q", "--no-gpg-sign", "-m", "genesis: sibling"],
        );
        let first = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(corpus.join("concept/beta.yml"), "label: Beta\n").unwrap();
        git(&s, &["add", "-A"]);
        git(&s, &["commit", "-q", "--no-gpg-sign", "-m", "add beta"]);
        let second = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        (tmp, s, first, second)
    }

    #[test]
    fn an_abbreviation_resolves_to_the_full_commit() {
        let (_tmp, s, first, _) = sibling();
        assert_eq!(resolve(&s, &first[..8]).unwrap(), first);
        assert!(resolve(&s, "0000000000000000000000000000000000000000").is_err());
    }

    /// The property the pin exists for. An uncommitted edit and a later commit in the sibling
    /// are both invisible to a read at the first commit.
    #[test]
    fn the_corpus_is_read_at_the_commit_and_not_from_the_working_tree() {
        let (_tmp, s, first, _) = sibling();
        std::fs::write(s.join(".yidam/corpus/concept/alpha.yml"), "label: EDITED\n").unwrap();
        let files: std::collections::BTreeMap<_, _> =
            read_corpus_at(&s, &first).unwrap().into_iter().collect();
        assert_eq!(
            files.keys().cloned().collect::<Vec<_>>(),
            vec![
                "concept.ont.yml".to_string(),
                "concept/alpha.yml".to_string()
            ],
            "beta arrived at the second commit"
        );
        assert_eq!(files["concept/alpha.yml"], b"label: Alpha\n");
    }

    /// `git archive` would honour this attribute and drop the file; the pin must not.
    #[test]
    fn an_export_ignore_attribute_does_not_drop_a_file() {
        let (_tmp, s, _, _) = sibling();
        std::fs::write(s.join(".gitattributes"), ".yidam/corpus/** export-ignore\n").unwrap();
        git(&s, &["add", "-A"]);
        git(&s, &["commit", "-q", "--no-gpg-sign", "-m", "attrs"]);
        let head = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        assert_eq!(read_corpus_at(&s, &head).unwrap().len(), 3);
    }

    #[test]
    fn a_commit_with_no_corpus_is_refused() {
        let tmp = tempfile::TempDir::new().unwrap();
        let s = tmp.path().join("s");
        std::fs::create_dir_all(&s).unwrap();
        std::fs::write(s.join("README"), "x\n").unwrap();
        crate::git::fixture::init(&s);
        git(&s, &["add", "-A"]);
        git(
            &s,
            &["commit", "-q", "--no-gpg-sign", "-m", "genesis: empty"],
        );
        let head = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        let err = read_corpus_at(&s, &head).unwrap_err().to_string();
        assert!(err.contains("has no .yidam/corpus/"), "{err}");
    }

    /// Materialised where a fetched bundle unpacks, verified by the digest it was locked
    /// with, and found by the read layer as an installed, pinned dependency.
    #[test]
    fn a_materialised_pin_verifies_and_moving_it_replaces_the_files() {
        let (tmp, s, first, second) = sibling();
        let tonpa = tmp.path().join("consumer/.yidam/tonpa");

        let locked = materialize("sib", "../sibling", &s, &first, &tonpa, None).unwrap();
        assert_eq!(locked.rev.as_deref(), Some(first.as_str()));
        assert_eq!(locked.commit.as_deref(), Some(first.as_str()));
        assert!(crate::deps::verify_installed("sib", &tonpa, &locked).unwrap());
        let m = crate::deps::read_manifest(&tonpa.join("sib")).unwrap();
        assert_eq!(m.commit.as_deref(), Some(first.as_str()));
        assert!(m.genesis_hash.is_some());

        // A hand edit to the installed copy is caught.
        std::fs::write(tonpa.join("sib/corpus/concept/alpha.yml"), "label: X\n").unwrap();
        assert!(!crate::deps::verify_installed("sib", &tonpa, &locked).unwrap());

        // Re-materialising at the same commit against the lock restores it.
        let again = materialize(
            "sib",
            "../sibling",
            &s,
            &first,
            &tonpa,
            Some(&locked.sha256),
        )
        .unwrap();
        assert_eq!(again.sha256, locked.sha256);

        // Moving the pin forward brings beta in.
        let moved = materialize("sib", "../sibling", &s, &second, &tonpa, None).unwrap();
        assert_ne!(moved.sha256, locked.sha256);
        assert!(tonpa.join("sib/corpus/concept/beta.yml").is_file());

        // …and moving it back takes beta out again, rather than leaving it behind.
        materialize("sib", "../sibling", &s, &first, &tonpa, None).unwrap();
        assert!(!tonpa.join("sib/corpus/concept/beta.yml").exists());
    }

    #[test]
    fn a_lock_that_disagrees_with_its_commit_is_refused() {
        let (tmp, s, first, _) = sibling();
        let tonpa = tmp.path().join("t");
        let err = materialize("sib", "../sibling", &s, &first, &tonpa, Some("deadbeef"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not match tonpa.lock"), "{err}");
    }

    #[test]
    fn a_batch_response_missing_an_object_is_an_error() {
        assert!(parse_batch(b"abc blob 3\nxyz\n", 2).is_err());
        assert_eq!(
            parse_batch(b"abc blob 3\nxyz\ndef blob 0\n\n", 2).unwrap(),
            vec![b"xyz".to_vec(), Vec::new()]
        );
    }
}
