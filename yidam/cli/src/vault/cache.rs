//! The local cache — machine-wide, content-addressed, and deliberately vault-blind.
//!
//! # Why it is not under `.yidam/`
//!
//! Every other directory this CLI writes belongs to one repository. This one does not, and
//! the reason is the addressing: two corpora that cite the same paper computed the same
//! name for it, so a per-repository cache would store it twice and verify it twice to
//! establish the same fact. The cache is keyed by content, so sharing it across every
//! repository on a machine is safe by construction rather than by convention.
//!
//! It follows the XDG base-directory spec — `$XDG_CACHE_HOME`, else `~/.cache` — because
//! that is where a machine-wide derived artifact belongs on the platforms this runs on, and
//! because it is what a user's existing cache-clearing habits already cover. Losing it
//! costs a re-fetch and nothing else, which is the property RFC-0023 is built to guarantee.
//!
//! # Why it is not partitioned by vault
//!
//! This is the one design decision here worth arguing, because the opposite is intuitive:
//! if artifacts are isolated into separate stores, surely the cache should mirror that.
//!
//! It should not. Bytes are bytes — the same digest in two stores is the same file, and
//! duplicating it locally would buy nothing but disk. Isolation in RFC-0023 is a property
//! of *where bytes may be sent*, which the committed record answers, not of where they
//! happen to sit while being read. Keeping that straight is what stops a cache hit from
//! ever being mistaken for permission: [`Cache::contains`] answers "do I have these bytes",
//! and no caller may read it as "may I upload these bytes".

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use super::cas::ContentHash;

/// Where the cache lives, and the artifacts in it.
#[derive(Debug, Clone)]
pub struct Cache {
    root: PathBuf,
}

/// What [`Cache::verify`] found for one entry.
#[derive(Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The bytes hash to the name they are filed under.
    Intact,
    /// They do not. The name is the digest that was expected; this is what was found.
    Corrupt { found: ContentHash },
    /// Nothing is filed under that name.
    Absent,
}

impl Cache {
    /// A cache rooted at an explicit directory.
    ///
    /// Separate from [`Cache::resolve`] so tests never touch a developer's real cache — and
    /// so the environment logic is testable without setting process-wide variables, which
    /// two tests running in the same process cannot do independently.
    pub fn at(root: impl Into<PathBuf>) -> Self {
        Cache { root: root.into() }
    }

    /// The cache this machine uses, from the environment.
    ///
    /// `lookup` is passed rather than read for the reason above; production callers hand it
    /// [`std::env::var`].
    ///
    /// `YIDAM_VAULT_CACHE` wins outright. It exists for the case a shared cache is the wrong
    /// answer — a build agent with a scratch disk, or an operator who wants the bytes on an
    /// encrypted volume — and for tests. Nothing here validates that the path is writable:
    /// a cache that cannot be written reports that at the moment it is written, naming the
    /// path, which is more useful than a check at startup that has to guess what will be
    /// attempted.
    pub fn resolve(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        if let Some(explicit) = lookup("YIDAM_VAULT_CACHE").filter(|s| !s.trim().is_empty()) {
            return Ok(Cache::at(PathBuf::from(explicit)));
        }
        let base = match lookup("XDG_CACHE_HOME").filter(|s| !s.trim().is_empty()) {
            Some(x) => PathBuf::from(x),
            None => {
                let home = lookup("HOME").filter(|s| !s.trim().is_empty()).context(
                    "cannot locate a cache directory: neither YIDAM_VAULT_CACHE, \
                         XDG_CACHE_HOME nor HOME is set. Set YIDAM_VAULT_CACHE to say where \
                         vault artifacts should live.",
                )?;
                PathBuf::from(home).join(".cache")
            }
        };
        Ok(Cache::at(base.join("yidam").join("vault")))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where an artifact with this digest sits, whether or not it is there.
    pub fn path_of(&self, hash: &ContentHash) -> PathBuf {
        self.root
            .join("sha256")
            .join(hash.shard())
            .join(hash.as_str())
    }

    /// Whether these bytes are already here.
    ///
    /// **This is not a permission check.** It answers only whether a fetch would be a no-op.
    /// See the module header: a cache hit says nothing about where the bytes may be sent.
    pub fn contains(&self, hash: &ContentHash) -> bool {
        self.path_of(hash).is_file()
    }

    /// File `src` under its digest, and return where it landed.
    ///
    /// The digest is supplied rather than recomputed, because every caller already has one:
    /// it either just hashed the file or is placing bytes a store handed over against the
    /// name that was asked for. Recomputing here would hide the difference between those two
    /// situations, and the second is the one where a mismatch matters.
    ///
    /// **Written through a temporary in the cache directory, then renamed.** Not a system
    /// temp file: `rename` is only atomic within a filesystem, and `/tmp` is routinely a
    /// different one — so a system temp file would silently degrade to copy-then-truncate
    /// and put a half-written artifact under a name that asserts its contents. The temp
    /// carries the process id so two `yidam` processes caching the same artifact cannot
    /// write the same scratch path.
    ///
    /// An artifact already present is left exactly as it is. Content addressing makes that
    /// safe — the bytes under a name are the bytes that name asserts, or `verify` has
    /// something to say — and it makes a re-`put` free rather than a rewrite.
    pub fn put_file(&self, src: &Path, hash: &ContentHash) -> Result<PathBuf> {
        let dest = self.path_of(hash);
        if dest.is_file() {
            return Ok(dest);
        }
        let dir = dest.parent().expect("cache paths always have a parent");
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating cache directory {}", dir.display()))?;

        let tmp = dir.join(format!(".{}.{}.part", hash.as_str(), std::process::id()));
        std::fs::copy(src, &tmp).with_context(|| {
            format!(
                "copying {} into the cache at {}",
                src.display(),
                tmp.display()
            )
        })?;
        std::fs::rename(&tmp, &dest).with_context(|| {
            // Leave the temp behind on failure rather than cleaning up: it is evidence, and
            // `gc` knows how to sweep `.part` files.
            format!("moving {} into place at {}", tmp.display(), dest.display())
        })?;
        Ok(dest)
    }

    /// Re-hash one cached artifact and say whether it is still what its name claims.
    pub fn verify(&self, hash: &ContentHash) -> Result<Verdict> {
        let path = self.path_of(hash);
        if !path.is_file() {
            return Ok(Verdict::Absent);
        }
        let found = ContentHash::of_file(&path)?;
        Ok(if &found == hash {
            Verdict::Intact
        } else {
            Verdict::Corrupt { found }
        })
    }

    /// Every artifact the cache holds, in a stable order.
    ///
    /// Sorted, because this feeds a report and a listing whose order moves between machines
    /// is one no golden can pin. Entries that are not well-formed digests are skipped rather
    /// than reported: the `.part` files above are exactly that, and so is anything else a
    /// person dropped in the directory, and neither is an artifact this cache is claiming.
    pub fn entries(&self) -> Result<Vec<ContentHash>> {
        let algo = self.root.join("sha256");
        if !algo.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        for shard in read_dir_sorted(&algo)? {
            if !shard.is_dir() {
                continue;
            }
            for entry in read_dir_sorted(&shard)? {
                if !entry.is_file() {
                    continue;
                }
                let Some(name) = entry.file_name().and_then(|n| n.to_str()) else {
                    continue;
                };
                if let Ok(h) = ContentHash::parse(name) {
                    out.push(h);
                }
            }
        }
        out.sort();
        Ok(out)
    }
}

fn read_dir_sorted(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("reading {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    paths.sort();
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Write;
    use tempfile::TempDir;

    fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        move |k: &str| map.get(k).cloned()
    }

    fn file_with(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::File::create(&p).unwrap().write_all(bytes).unwrap();
        p
    }

    #[test]
    fn an_explicit_cache_directory_outranks_everything() {
        let c = Cache::resolve(env(&[
            ("YIDAM_VAULT_CACHE", "/scratch/vault"),
            ("XDG_CACHE_HOME", "/xdg"),
            ("HOME", "/home/x"),
        ]))
        .unwrap();
        assert_eq!(c.root(), Path::new("/scratch/vault"));
    }

    #[test]
    fn xdg_wins_over_home_and_home_is_the_fallback() {
        let c = Cache::resolve(env(&[("XDG_CACHE_HOME", "/xdg"), ("HOME", "/home/x")])).unwrap();
        assert_eq!(c.root(), Path::new("/xdg/yidam/vault"));

        let c = Cache::resolve(env(&[("HOME", "/home/x")])).unwrap();
        assert_eq!(c.root(), Path::new("/home/x/.cache/yidam/vault"));
    }

    /// An empty variable is not a location. Treating `XDG_CACHE_HOME=""` as a directory
    /// roots the cache at the filesystem root, which is the kind of thing that is noticed
    /// only after it has happened.
    #[test]
    fn an_empty_variable_is_ignored_rather_than_used_as_a_path() {
        let c = Cache::resolve(env(&[
            ("YIDAM_VAULT_CACHE", "  "),
            ("XDG_CACHE_HOME", ""),
            ("HOME", "/home/x"),
        ]))
        .unwrap();
        assert_eq!(c.root(), Path::new("/home/x/.cache/yidam/vault"));
    }

    #[test]
    fn with_nothing_in_the_environment_it_says_what_to_set() {
        let err = Cache::resolve(env(&[])).unwrap_err().to_string();
        assert!(err.contains("YIDAM_VAULT_CACHE"), "{err}");
    }

    #[test]
    fn putting_a_file_files_it_under_its_digest_and_finds_it_again() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("cache"));
        let src = file_with(tmp.path(), "doc.pdf", b"hello vault");
        let hash = ContentHash::of_file(&src).unwrap();

        assert!(!cache.contains(&hash));
        let landed = cache.put_file(&src, &hash).unwrap();
        assert!(cache.contains(&hash));
        assert_eq!(landed, cache.path_of(&hash));
        assert!(landed.ends_with(format!("sha256/{}/{hash}", hash.shard())));
        assert_eq!(std::fs::read(&landed).unwrap(), b"hello vault");
    }

    /// A second `put` of bytes already held must not rewrite them, and must not fail.
    #[test]
    fn putting_an_artifact_already_held_is_a_no_op() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("cache"));
        let src = file_with(tmp.path(), "a", b"same bytes");
        let hash = ContentHash::of_file(&src).unwrap();

        let first = cache.put_file(&src, &hash).unwrap();
        let mtime = std::fs::metadata(&first).unwrap().modified().unwrap();
        let again = cache.put_file(&src, &hash).unwrap();
        assert_eq!(first, again);
        assert_eq!(
            std::fs::metadata(&again).unwrap().modified().unwrap(),
            mtime
        );
    }

    /// No `.part` file may survive a successful put — one left behind would be counted by a
    /// later `gc` as garbage it has to reason about, and by a person as a failed write.
    #[test]
    fn a_successful_put_leaves_no_temporary_behind() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("cache"));
        let src = file_with(tmp.path(), "a", b"bytes");
        let hash = ContentHash::of_file(&src).unwrap();
        cache.put_file(&src, &hash).unwrap();

        let dir = cache.path_of(&hash).parent().unwrap().to_path_buf();
        let leftovers: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .filter(|n| n.ends_with(".part"))
            .collect();
        assert!(leftovers.is_empty(), "left behind: {leftovers:?}");
    }

    #[test]
    fn verify_distinguishes_intact_absent_and_corrupt() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("cache"));
        let src = file_with(tmp.path(), "a", b"original");
        let hash = ContentHash::of_file(&src).unwrap();

        assert_eq!(cache.verify(&hash).unwrap(), Verdict::Absent);
        cache.put_file(&src, &hash).unwrap();
        assert_eq!(cache.verify(&hash).unwrap(), Verdict::Intact);

        // Rot the bytes under a name that asserts them. This is the case the whole design
        // exists to make detectable, so it is asserted on rather than assumed.
        std::fs::write(cache.path_of(&hash), b"tampered").unwrap();
        match cache.verify(&hash).unwrap() {
            Verdict::Corrupt { found } => {
                assert_eq!(found, ContentHash::of_bytes(b"tampered"));
                assert_ne!(found, hash);
            }
            other => panic!("expected corruption, got {other:?}"),
        }
    }

    #[test]
    fn entries_are_sorted_and_skip_what_is_not_an_artifact() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("cache"));
        let mut expected = Vec::new();
        for body in [&b"one"[..], b"two", b"three"] {
            let src = file_with(tmp.path(), "x", body);
            let h = ContentHash::of_file(&src).unwrap();
            cache.put_file(&src, &h).unwrap();
            expected.push(h);
        }
        expected.sort();

        // A stray temporary and a stray directory are not artifacts and must not be listed.
        let shard = cache.path_of(&expected[0]).parent().unwrap().to_path_buf();
        std::fs::write(shard.join(".deadbeef.9.part"), b"partial").unwrap();
        std::fs::create_dir_all(shard.join("not-a-file")).unwrap();

        assert_eq!(cache.entries().unwrap(), expected);
    }

    #[test]
    fn a_cache_that_does_not_exist_yet_holds_nothing_rather_than_failing() {
        let tmp = TempDir::new().unwrap();
        let cache = Cache::at(tmp.path().join("never-created"));
        assert_eq!(cache.entries().unwrap(), Vec::new());
    }
}
