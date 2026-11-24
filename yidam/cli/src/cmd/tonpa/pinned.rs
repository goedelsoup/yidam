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
    /// The name the dependency is published under (RFC-0064 §2.2), the field
    /// [`crate::deps::shadowed`] compares.
    corpus_id: Option<String>,
    instances: usize,
    vector_index_model: Option<String>,
}

/// The export's bytes at `sha`, read out of the object store like the corpus is.
///
/// Refused when the path is not a plain relative path inside the repository, and when the
/// tree at `sha` has no blob there, naming both.
fn read_export_at(sibling: &Path, sha: &str, export: &str) -> Result<Vec<u8>> {
    let path = Path::new(export);
    if path.as_os_str().is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_)))
    {
        bail!("export `{export}` is not a path inside the dependency's repository");
    }
    let out = Git::new(sibling)
        .args(["cat-file", "blob"])
        .rev(format!("{sha}:{export}"))
        .output()?;
    if !out.status.success() {
        bail!(
            "{} has no file {export} at {sha}\n  an export is read at the pinned commit, never \
             from the working tree",
            sibling.display()
        );
    }
    Ok(out.stdout)
}

/// The commit on `rev`'s history that last wrote `export`, which is where a pack computed
/// from the corpus lands (RFC-0063 §0.2). Literal pathspecs, so an export named with a `*`
/// means that file.
pub fn last_write(sibling: &Path, rev: &str, export: &str) -> Result<String> {
    Git::new(sibling)
        .env("GIT_LITERAL_PATHSPECS", "1")
        .args(["log", "-1", "--format=%H"])
        .rev(rev)
        .paths([export])
        .try_run()
        .filter(|s| !s.is_empty())
        .with_context(|| {
            format!(
                "no commit on {rev}'s history in {} writes {export}",
                sibling.display()
            )
        })
}

/// The tree id of `.yidam/corpus` at `commit`, or `None` when the commit has none.
fn corpus_tree(sibling: &Path, commit: &str) -> Option<String> {
    Git::new(sibling)
        .args(["rev-parse", "--verify", "--quiet"])
        .rev(format!("{commit}:{CORPUS}"))
        .try_run()
        .filter(|s| !s.is_empty())
}

/// Refuse when the corpus at `at` is not the corpus at `written`, the commit that last wrote
/// the export. Git trees only, so it holds for any export shape (RFC-0063 §1).
///
/// A commit with no corpus is refused too: two absent trees are equal, and an export computed
/// from no corpus has nothing at the pin to be checked against.
pub fn same_corpus(sibling: &Path, written: &str, at: &str, export: &str) -> Result<()> {
    let Some(here) = corpus_tree(sibling, at) else {
        bail!(
            "{} has no {CORPUS} at {at}\n  an export is checked against the corpus at its pin",
            sibling.display()
        );
    };
    if corpus_tree(sibling, written).as_deref() != Some(here.as_str()) {
        bail!(
            "the corpus moved after {export} was last written\n  written at: {written}\n  \
             corpus at:  {at}\n  recompute the export in {} and commit it",
            sibling.display()
        );
    }
    Ok(())
}

/// Write the sibling's corpus at `sha` to `<tonpa_dir>/<name>/`, and the lock entry for it.
///
/// `expected` is the entry the lock already holds, when it holds one for this commit. The
/// same commit cannot produce different content, so a mismatch means the lock was edited or
/// the digest's definition moved — either way not something to paper over by re-pinning.
/// Its export hash is compared only when it locked the export declared now.
///
/// `export`, when declared, is read at `sha` too, checked as new as the corpus there, and
/// unpacked to `export/<file name>` beside it (RFC-0063 §1). Every check runs before the
/// previous copy is touched.
///
/// The previous copy is removed first. A file the pin moved past would otherwise outlive it
/// and be read as part of the new one.
pub fn materialize(
    name: &str,
    rel: &str,
    sibling: &Path,
    sha: &str,
    export: Option<&str>,
    tonpa_dir: &Path,
    expected: Option<&LockedPackage>,
) -> Result<LockedPackage> {
    let entries = read_corpus_at(sibling, sha)?;
    let digest = crate::deps::tree_digest(&entries);
    if let Some(expected) = expected {
        if digest != expected.sha256 {
            bail!(
                "{name} at {sha} does not match tonpa.lock\n  expected: {}\n  got:      {digest}",
                expected.sha256
            );
        }
    }

    let export = match export {
        Some(path) => {
            let bytes = read_export_at(sibling, sha, path)?;
            same_corpus(sibling, &last_write(sibling, sha, path)?, sha, path)?;
            let hash = crate::deps::sha256_hex(&bytes);
            if let Some(want) = expected
                .filter(|e| e.export.as_deref() == Some(path))
                .and_then(|e| e.export_sha256.as_deref())
            {
                if hash != want {
                    bail!(
                        "{name}'s export {path} at {sha} does not match tonpa.lock\n  \
                         expected: {want}\n  got:      {hash}"
                    );
                }
            }
            let file = crate::deps::export_file(name, tonpa_dir, path)
                .with_context(|| format!("export `{path}` names no file"))?;
            Some((path, bytes, hash, file))
        }
        None => None,
    };

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
    let (genesis_hash, genesis, corpus_id) = identity_at(sibling, sha)?;
    let manifest = Manifest {
        bundle_version: "1",
        source: "path",
        path: rel,
        commit: sha,
        genesis: genesis.clone(),
        genesis_hash,
        corpus_id,
        instances,
        vector_index_model: None,
    };
    std::fs::write(
        dest.join("manifest.yml"),
        serde_yaml::to_string(&manifest).context("serialising manifest.yml")?,
    )?;
    if let Some((_, bytes, _, file)) = &export {
        if let Some(parent) = file.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(file, bytes).with_context(|| format!("writing {}", file.display()))?;
    }

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
        export: export.as_ref().map(|(path, ..)| path.to_string()),
        export_sha256: export.map(|(_, _, hash, _)| hash),
    })
}

/// The genesis hash, the genesis date and the corpus id the pinned commit carries.
///
/// A projection's mode and id are read out of the commit too, like the corpus is: a sibling
/// that was authored at the pin and projected since, or the reverse, is named as it was at the
/// pin. A projection has no genesis of its own (RFC-0064 §2.2), so both genesis fields are
/// `None` for one.
fn identity_at(
    sibling: &Path,
    sha: &str,
) -> Result<(Option<String>, Option<String>, Option<String>)> {
    let blob = |path: &str| {
        Git::new(sibling)
            .args(["cat-file", "blob"])
            .rev(format!("{sha}:{path}"))
            .try_run()
    };
    let mode = match blob(".yidam/config.toml") {
        Some(text) => crate::projection::parse_mode(&text).with_context(|| format!("at {sha}"))?,
        None => crate::projection::CorpusMode::Authored,
    };
    if let Some(by) = mode.generated_by() {
        let text = blob(crate::projection::CORPUS_TOML).with_context(|| {
            format!(
                "{} is a projection at {sha} and has no {} there",
                sibling.display(),
                crate::projection::CORPUS_TOML
            )
        })?;
        let id = crate::projection::parse_declared_id(&text, by)?;
        return Ok((None, None, Some(id)));
    }
    let (hash, date) = genesis_of(sibling, sha);
    let id = hash.as_deref().map(crate::s3vectors::request::corpus_id);
    Ok((hash, date, id))
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

        let locked = materialize("sib", "../sibling", &s, &first, None, &tonpa, None).unwrap();
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
        let again =
            materialize("sib", "../sibling", &s, &first, None, &tonpa, Some(&locked)).unwrap();
        assert_eq!(again.sha256, locked.sha256);

        // Moving the pin forward brings beta in.
        let moved = materialize("sib", "../sibling", &s, &second, None, &tonpa, None).unwrap();
        assert_ne!(moved.sha256, locked.sha256);
        assert!(tonpa.join("sib/corpus/concept/beta.yml").is_file());

        // …and moving it back takes beta out again, rather than leaving it behind.
        materialize("sib", "../sibling", &s, &first, None, &tonpa, None).unwrap();
        assert!(!tonpa.join("sib/corpus/concept/beta.yml").exists());
    }

    #[test]
    fn a_lock_that_disagrees_with_its_commit_is_refused() {
        let (tmp, s, first, _) = sibling();
        let tonpa = tmp.path().join("t");
        let mut wrong = materialize("sib", "../sibling", &s, &first, None, &tonpa, None).unwrap();
        wrong.sha256 = "deadbeef".into();
        let err = materialize("sib", "../sibling", &s, &first, None, &tonpa, Some(&wrong))
            .unwrap_err()
            .to_string();
        assert!(err.contains("does not match tonpa.lock"), "{err}");
    }

    /// A sibling whose pack lands one commit after the corpus it was computed from, as all six
    /// of RFC-0063 §0.2's do, and whose corpus then moves on. Returns (tempdir, sibling,
    /// corpus commit, compute commit, moved commit).
    fn with_export() -> (
        tempfile::TempDir,
        std::path::PathBuf,
        String,
        String,
        String,
    ) {
        let (tmp, s, first, _) = sibling();
        git(&s, &["reset", "-q", "--hard", &first]);
        std::fs::create_dir_all(s.join(".yidam/computed")).unwrap();
        std::fs::write(s.join(".yidam/computed/pack.json"), "{\"v\":1}\n").unwrap();
        git(&s, &["add", "-A"]);
        git(
            &s,
            &["commit", "-q", "--no-gpg-sign", "-m", "compute: pack"],
        );
        let compute = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        std::fs::write(s.join(".yidam/corpus/concept/gamma.yml"), "label: G\n").unwrap();
        git(&s, &["add", "-A"]);
        git(&s, &["commit", "-q", "--no-gpg-sign", "-m", "add gamma"]);
        let moved = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        (tmp, s, first, compute, moved)
    }

    const PACK: &str = ".yidam/computed/pack.json";

    /// Read at the pin, unpacked beside the corpus, locked, and verified by its hash.
    #[test]
    fn an_export_is_read_at_the_pin_unpacked_and_locked() {
        let (tmp, s, _, compute, _) = with_export();
        std::fs::write(s.join(PACK), "{\"v\":\"EDITED\"}\n").unwrap();
        let tonpa = tmp.path().join("t");
        let locked =
            materialize("sib", "../sibling", &s, &compute, Some(PACK), &tonpa, None).unwrap();
        let file = tonpa.join("sib/export/pack.json");
        assert_eq!(
            std::fs::read(&file).unwrap(),
            b"{\"v\":1}\n",
            "never the working tree"
        );
        assert_eq!(locked.export.as_deref(), Some(PACK));
        assert_eq!(
            locked.export_sha256.as_deref(),
            Some(crate::deps::sha256_hex(b"{\"v\":1}\n").as_str())
        );
        assert!(crate::deps::verify_installed("sib", &tonpa, &locked).unwrap());
        std::fs::write(&file, "{}\n").unwrap();
        assert!(!crate::deps::verify_installed("sib", &tonpa, &locked).unwrap());
        std::fs::remove_file(&file).unwrap();
        assert!(!crate::deps::verify_installed("sib", &tonpa, &locked).unwrap());

        let mut edited = locked.clone();
        edited.export_sha256 = Some("deadbeef".into());
        let err = materialize(
            "sib",
            "../sibling",
            &s,
            &compute,
            Some(PACK),
            &tonpa,
            Some(&edited),
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains("does not match tonpa.lock"), "{err}");
    }

    #[test]
    fn a_rev_whose_tree_lacks_the_export_is_refused_naming_both() {
        let (tmp, s, first, _, _) = with_export();
        let err = materialize(
            "sib",
            "../sibling",
            &s,
            &first,
            Some(PACK),
            &tmp.path().join("t"),
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains(PACK) && err.contains(&first), "{err}");
    }

    /// The corpus at `moved` is not the one the pack was computed from: refused, naming the
    /// commit that wrote the export and the commit asked for.
    #[test]
    fn an_export_older_than_the_corpus_at_rev_is_refused() {
        let (tmp, s, _, compute, moved) = with_export();
        let err = materialize(
            "sib",
            "../sibling",
            &s,
            &moved,
            Some(PACK),
            &tmp.path().join("t"),
            None,
        )
        .unwrap_err()
        .to_string();
        assert!(err.contains(&compute) && err.contains(&moved), "{err}");
        assert!(
            !tmp.path().join("t/sib").exists(),
            "refused before anything was written"
        );
    }

    /// `update`'s target, and why it refuses here: HEAD's corpus moved past the last export.
    #[test]
    fn the_last_write_is_found_on_the_history_asked_about() {
        let (_tmp, s, first, compute, moved) = with_export();
        assert_eq!(last_write(&s, &moved, PACK).unwrap(), compute);
        assert!(last_write(&s, &first, PACK).is_err());
        assert!(same_corpus(&s, &compute, &moved, PACK).is_err());
        assert!(
            same_corpus(&s, &compute, &first, PACK).is_ok(),
            "compute touched no corpus"
        );
    }

    /// Two commits with no corpus have equal (absent) trees, and that is not freshness.
    #[test]
    fn a_pin_with_no_corpus_is_refused() {
        let (_tmp, s, _, compute, _) = with_export();
        git(&s, &["rm", "-rq", CORPUS]);
        git(&s, &["commit", "-q", "--no-gpg-sign", "-m", "drop corpus"]);
        let bare = git_out(&s, &["rev-parse", "HEAD"]).trim().to_string();
        let err = same_corpus(&s, &bare, &bare, PACK).unwrap_err().to_string();
        assert!(err.contains("has no"), "{err}");
        assert!(same_corpus(&s, &compute, &compute, PACK).is_ok());
    }

    #[test]
    fn an_export_outside_the_repository_is_refused() {
        let (tmp, s, _, compute, _) = with_export();
        for bad in ["../x.json", "/etc/passwd", "", "./a/../b"] {
            let err = materialize(
                "sib",
                "../sibling",
                &s,
                &compute,
                Some(bad),
                &tmp.path().join("t"),
                None,
            )
            .unwrap_err()
            .to_string();
            assert!(err.contains("not a path inside"), "{bad}: {err}");
        }
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
