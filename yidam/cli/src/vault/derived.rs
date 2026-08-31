//! The artifacts this repository *produces* — the index, its embeddings, the bundle.
//!
//! # Why these need a channel at all
//!
//! A catalog artifact is something somebody fetched, and the record of it is written by hand.
//! These three are computed, and the computation is the problem:
//!
//! > `.yidam/index/` is built only by a binary compiled `--features index` — protoc 31 plus an
//! > ONNX runtime. The release workflow builds the light default, and nothing keeps the index
//! > in git.
//!
//! So the index exists on whichever machine could build it and nowhere else. That is the gap
//! this closes: the vault carries it, and a lock file says which bytes and which store.
//!
//! # An index is a directory, and a vault stores one blob
//!
//! `.yidam/index/` is three files that only mean anything together — a `corpus.arrow` from one
//! build beside a `meta.json` from another is a corrupt index and nothing would notice. So a
//! directory is packed into a single archive and hashed **as one object**. Partial arrival
//! stops being expressible rather than being checked for.
//!
//! The archive is deterministic — sorted entries, zeroed mtimes, fixed mode — so packing the
//! same index twice produces the same digest. Not required for correctness, since the lock
//! records what was actually pushed; required for the thing a person expects, which is that
//! pushing an unchanged index does nothing.
//!
//! # Derived artifacts are opt-in on `push`
//!
//! `yidam vault push` sends what the *catalog* names and nothing else. An index is hundreds of
//! megabytes and pushing one should be something a person asked for, not something that
//! happens because they typed a command whose usual job is a handful of PDFs.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::cas::ContentHash;

/// One artifact this repository computes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Derived {
    Index,
    Embeddings,
    Bundle,
}

impl Derived {
    pub const ALL: [Derived; 3] = [Derived::Index, Derived::Embeddings, Derived::Bundle];

    /// The routing vocabulary name — what a vault's `holds` lists.
    pub fn kind(self) -> &'static str {
        match self {
            Derived::Index => super::config::INDEX_KIND,
            Derived::Embeddings => super::config::EMBEDDINGS_KIND,
            Derived::Bundle => super::config::BUNDLE_KIND,
        }
    }

    pub fn parse(s: &str) -> Option<Derived> {
        Derived::ALL.into_iter().find(|d| d.kind() == s)
    }

    /// Where it sits in a working tree.
    pub fn path(self, root: &Path) -> PathBuf {
        match self {
            Derived::Index => crate::paths::yidam_index_dir(root),
            Derived::Embeddings => crate::paths::yidam_embeddings_dir(root),
            Derived::Bundle => root.join(".yidam").join("bundle.yiz"),
        }
    }

    /// Whether it is a directory, and therefore has to be packed to become one object.
    pub fn is_dir(self) -> bool {
        !matches!(self, Derived::Bundle)
    }

    /// What builds it, for a message telling somebody why there is nothing to push.
    pub fn built_by(self) -> &'static str {
        match self {
            Derived::Index => "yidam index-build",
            Derived::Embeddings => "yidam embed",
            Derived::Bundle => "yidam bundle",
        }
    }

    pub fn present(self, root: &Path) -> bool {
        let p = self.path(root);
        if self.is_dir() {
            p.is_dir() && std::fs::read_dir(&p).is_ok_and(|mut d| d.next().is_some())
        } else {
            p.is_file()
        }
    }
}

/// Write the single blob that represents this artifact.
///
/// A file is copied verbatim — a `.yiz` is already one object and wrapping it in a second
/// archive would double the storage to describe nothing. A directory is packed.
pub fn pack_to(root: &Path, d: Derived, dest: &Path) -> Result<()> {
    let src = d.path(root);
    if !d.present(root) {
        bail!(
            "there is no {} at {} to push.\n  Build one with `{}`.",
            d.kind(),
            src.display(),
            d.built_by()
        );
    }
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !d.is_dir() {
        std::fs::copy(&src, dest)
            .with_context(|| format!("copying {} to {}", src.display(), dest.display()))?;
        return Ok(());
    }
    let out =
        std::fs::File::create(dest).with_context(|| format!("creating {}", dest.display()))?;
    let gz = flate2::write::GzEncoder::new(out, flate2::Compression::default());
    let mut tar = tar::Builder::new(gz);
    for rel in entries(&src)? {
        let full = src.join(&rel);
        let data = std::fs::read(&full).with_context(|| format!("reading {}", full.display()))?;
        // Every field a filesystem would vary is pinned. `new_gnu` zeroes the header, and the
        // three that are not zero by accident — size, mode, checksum — are set explicitly.
        // An mtime left alone is the one that would silently make two identical indexes hash
        // differently on two machines.
        let mut header = tar::Header::new_gnu();
        header.set_size(data.len() as u64);
        header.set_mode(0o644);
        header.set_mtime(0);
        header.set_uid(0);
        header.set_gid(0);
        header.set_cksum();
        tar.append_data(&mut header, &rel, data.as_slice())
            .with_context(|| format!("adding {} to the archive", rel.display()))?;
    }
    tar.into_inner()
        .context("finishing the archive")?
        .finish()
        .context("finishing compression")?
        .flush()
        .context("flushing the archive")
}

/// Every file under `dir`, relative and sorted.
///
/// Sorted because a directory walk is in whatever order the filesystem feels like, and two
/// machines that disagree about it would produce two archives of the same index.
fn entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(at) = stack.pop() {
        for e in std::fs::read_dir(&at).with_context(|| format!("reading {}", at.display()))? {
            let path = e?.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.is_file() {
                out.push(path.strip_prefix(dir).unwrap_or(&path).to_path_buf());
            }
        }
    }
    out.sort();
    Ok(out)
}

/// Put a fetched blob back where it came from.
///
/// **The existing artifact is replaced only once the new one is whole.** An index is unpacked
/// beside its destination and swapped in; a failure part-way leaves the old index exactly as
/// it was, rather than leaving half of a new one in the place a reader trusts.
pub fn unpack_from(root: &Path, d: Derived, blob: &Path) -> Result<()> {
    let dest = d.path(root);
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if !d.is_dir() {
        let staged = with_suffix(&dest, "incoming");
        std::fs::copy(blob, &staged)?;
        std::fs::rename(&staged, &dest)
            .with_context(|| format!("moving {} into place", dest.display()))?;
        return Ok(());
    }

    let staged = with_suffix(&dest, "incoming");
    let _ = std::fs::remove_dir_all(&staged);
    std::fs::create_dir_all(&staged)?;
    let f = std::fs::File::open(blob).with_context(|| format!("opening {}", blob.display()))?;
    tar::Archive::new(flate2::read::GzDecoder::new(f))
        .unpack(&staged)
        .with_context(|| format!("unpacking the {} archive", d.kind()))?;

    let replaced = with_suffix(&dest, "replaced");
    let _ = std::fs::remove_dir_all(&replaced);
    let had_one = dest.exists();
    if had_one {
        std::fs::rename(&dest, &replaced)
            .with_context(|| format!("moving the existing {} aside", d.kind()))?;
    }
    match std::fs::rename(&staged, &dest) {
        Ok(()) => {
            let _ = std::fs::remove_dir_all(&replaced);
            Ok(())
        }
        Err(e) => {
            // Put the old one back. Failing with no index where there had been a working one
            // would be a worse outcome than the failure being reported.
            if had_one {
                let _ = std::fs::rename(&replaced, &dest);
            }
            Err(e).with_context(|| format!("moving the new {} into place", d.kind()))
        }
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_os_string();
    s.push(format!(".{suffix}"));
    PathBuf::from(s)
}

/// What was pushed, and where it went.
///
/// Committed, at `.yidam/index.lock`. The name is the artifact this exists for; embeddings and
/// the bundle ride along because they are the index's inputs and its container, and three lock
/// files describing one pipeline would be three things to keep in step.
#[derive(Debug, Default, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DerivedLock {
    /// Bumped when a reader must refuse rather than guess. New *fields* are additive and
    /// optional, for the reason `tonpa.lock` learned: a consumer reading a just-added field
    /// crashes against the released CLI, and the version guards only the other direction.
    #[serde(default = "one")]
    pub format_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub index: Option<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embeddings: Option<Entry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bundle: Option<Entry>,
}

fn one() -> u32 {
    1
}

/// The version this build writes, and the highest it can read.
pub const LOCK_FORMAT_VERSION: u32 = 1;

/// One artifact's record.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Entry {
    pub sha256: String,
    pub bytes: u64,
    /// **Which store it actually went to**, not which store the config would route it to
    /// today. A pull that re-derived the destination from `holds` would follow an edit made
    /// after the push and look in the wrong place — which is a mutable ref by another name,
    /// and the thing this whole design exists to avoid.
    pub vault: String,
}

impl DerivedLock {
    pub fn get(&self, d: Derived) -> Option<&Entry> {
        match d {
            Derived::Index => self.index.as_ref(),
            Derived::Embeddings => self.embeddings.as_ref(),
            Derived::Bundle => self.bundle.as_ref(),
        }
    }

    pub fn set(&mut self, d: Derived, e: Entry) {
        let slot = match d {
            Derived::Index => &mut self.index,
            Derived::Embeddings => &mut self.embeddings,
            Derived::Bundle => &mut self.bundle,
        };
        *slot = Some(e);
    }
}

pub fn lock_path(root: &Path) -> PathBuf {
    root.join(".yidam").join("index.lock")
}

pub fn load_lock(root: &Path) -> Result<DerivedLock> {
    let path = lock_path(root);
    if !path.exists() {
        return Ok(DerivedLock {
            format_version: LOCK_FORMAT_VERSION,
            ..Default::default()
        });
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    let lock: DerivedLock =
        toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
    if lock.format_version > LOCK_FORMAT_VERSION {
        bail!(
            "{} is format_version {} and this build reads {}.\n  \
             It was written by a newer yidam. Upgrade rather than guess: a lock file that \
             is read leniently is a lock file that stops locking.",
            path.display(),
            lock.format_version,
            LOCK_FORMAT_VERSION
        );
    }
    Ok(lock)
}

pub fn save_lock(root: &Path, lock: &DerivedLock) -> Result<()> {
    let path = lock_path(root);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let text = toml::to_string_pretty(lock).context("serializing the lock file")?;
    std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))
}

/// The digest of a packed artifact, without keeping it in memory.
pub fn hash_file(path: &Path) -> Result<(ContentHash, u64)> {
    let hash = ContentHash::of_file(path)?;
    let bytes = std::fs::metadata(path)?.len();
    Ok((hash, bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn index_at(root: &Path, arrow: &[u8]) {
        let d = crate::paths::yidam_index_dir(root);
        std::fs::create_dir_all(&d).unwrap();
        std::fs::write(d.join("corpus.arrow"), arrow).unwrap();
        std::fs::write(d.join("meta.json"), br#"{"indexed_commit":"abc"}"#).unwrap();
    }

    /// **The property the whole channel rests on.** Two machines packing the same index must
    /// produce the same digest, or "already pushed" can never be answered and every push
    /// uploads again and rewrites the lock.
    #[test]
    fn packing_the_same_index_twice_produces_the_same_bytes() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        index_at(a.path(), b"vectors");
        index_at(b.path(), b"vectors");

        let (pa, pb) = (a.path().join("a.tgz"), b.path().join("b.tgz"));
        pack_to(a.path(), Derived::Index, &pa).unwrap();
        // A different mtime on disk is exactly what a second machine has.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        pack_to(b.path(), Derived::Index, &pb).unwrap();

        assert_eq!(
            ContentHash::of_file(&pa).unwrap(),
            ContentHash::of_file(&pb).unwrap(),
            "the archive must not carry a timestamp"
        );
    }

    #[test]
    fn a_different_index_hashes_differently() {
        let a = TempDir::new().unwrap();
        let b = TempDir::new().unwrap();
        index_at(a.path(), b"vectors");
        index_at(b.path(), b"other vectors");
        let (pa, pb) = (a.path().join("a.tgz"), b.path().join("b.tgz"));
        pack_to(a.path(), Derived::Index, &pa).unwrap();
        pack_to(b.path(), Derived::Index, &pb).unwrap();
        assert_ne!(
            ContentHash::of_file(&pa).unwrap(),
            ContentHash::of_file(&pb).unwrap()
        );
    }

    #[test]
    fn an_index_round_trips_through_the_archive() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        index_at(src.path(), b"the vectors");
        let blob = src.path().join("packed");
        pack_to(src.path(), Derived::Index, &blob).unwrap();

        unpack_from(dst.path(), Derived::Index, &blob).unwrap();
        let out = crate::paths::yidam_index_dir(dst.path());
        assert_eq!(
            std::fs::read(out.join("corpus.arrow")).unwrap(),
            b"the vectors"
        );
        assert!(out.join("meta.json").is_file());
        // Nothing is left beside it.
        assert!(!with_suffix(&out, "incoming").exists());
        assert!(!with_suffix(&out, "replaced").exists());
    }

    /// A pull replaces what is there, and leaves nothing of the old one behind.
    #[test]
    fn unpacking_over_an_existing_index_replaces_it_entirely() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        index_at(src.path(), b"new");
        // The destination has an older index carrying a file the new one does not.
        index_at(dst.path(), b"old");
        std::fs::write(
            crate::paths::yidam_index_dir(dst.path()).join("stale.bin"),
            b"x",
        )
        .unwrap();

        let blob = src.path().join("packed");
        pack_to(src.path(), Derived::Index, &blob).unwrap();
        unpack_from(dst.path(), Derived::Index, &blob).unwrap();

        let out = crate::paths::yidam_index_dir(dst.path());
        assert_eq!(std::fs::read(out.join("corpus.arrow")).unwrap(), b"new");
        assert!(
            !out.join("stale.bin").exists(),
            "a replaced index must not keep the old one's files"
        );
    }

    /// A `.yiz` is already one object. Wrapping it in a second archive would double the
    /// storage to describe nothing.
    #[test]
    fn a_bundle_is_stored_verbatim_rather_than_re_archived() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        let body = b"a bundle, already compressed".to_vec();
        std::fs::write(tmp.path().join(".yidam/bundle.yiz"), &body).unwrap();

        let blob = tmp.path().join("packed");
        pack_to(tmp.path(), Derived::Bundle, &blob).unwrap();
        assert_eq!(std::fs::read(&blob).unwrap(), body);
        assert_eq!(
            ContentHash::of_file(&blob).unwrap(),
            ContentHash::of_bytes(&body)
        );
    }

    #[test]
    fn pushing_what_was_never_built_says_what_builds_it() {
        let tmp = TempDir::new().unwrap();
        let err = pack_to(tmp.path(), Derived::Index, &tmp.path().join("out"))
            .unwrap_err()
            .to_string();
        assert!(err.contains("yidam index-build"), "{err}");
    }

    #[test]
    fn every_derived_kind_is_one_the_routing_vocabulary_knows() {
        for d in Derived::ALL {
            assert!(
                super::super::config::ARTIFACT_KINDS.contains(&d.kind()),
                "{} is not routable",
                d.kind()
            );
            assert_eq!(Derived::parse(d.kind()), Some(d));
        }
        assert_eq!(Derived::parse("catalog"), None);
    }

    #[test]
    fn a_lock_round_trips_and_a_newer_one_is_refused() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        assert_eq!(load_lock(tmp.path()).unwrap().index, None);

        let mut lock = load_lock(tmp.path()).unwrap();
        lock.set(
            Derived::Index,
            Entry {
                sha256: "a".repeat(64),
                bytes: 12,
                vault: "default".into(),
            },
        );
        save_lock(tmp.path(), &lock).unwrap();
        let back = load_lock(tmp.path()).unwrap();
        assert_eq!(back, lock);
        assert_eq!(back.get(Derived::Index).unwrap().vault, "default");

        std::fs::write(lock_path(tmp.path()), "format_version = 99\n").unwrap();
        let err = load_lock(tmp.path()).unwrap_err().to_string();
        assert!(err.contains("newer yidam"), "{err}");
    }

    /// A lock written by an older build has no `format_version` at all, and must read as
    /// version 1 rather than as a parse error.
    #[test]
    fn a_lock_without_a_version_reads_as_the_first_one() {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam")).unwrap();
        std::fs::write(
            lock_path(tmp.path()),
            format!(
                "[index]\nsha256 = \"{}\"\nbytes = 4\nvault = \"default\"\n",
                "b".repeat(64)
            ),
        )
        .unwrap();
        let lock = load_lock(tmp.path()).unwrap();
        assert_eq!(lock.format_version, 1);
        assert_eq!(lock.index.unwrap().bytes, 4);
    }
}
