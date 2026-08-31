//! The backend seam, and the one backend that needs no network.
//!
//! # Why the trait is synchronous
//!
//! The S3 backend will be async underneath — `reqwest` is. It is still reached through a
//! synchronous trait, and the runtime is the backend's own business.
//!
//! The alternative is an async trait, which would put `tokio` in the signature of every
//! caller and therefore in the ungated half of this module. That half exists precisely so a
//! light build can hash, cache, verify and read a `file://` vault with no network stack
//! compiled in at all; an async seam would spend that for nothing, because every one of these
//! operations is a single command-line invocation rather than something in a loop.
//!
//! # Why `file://` is not a test double
//!
//! It is a shipped backend. A mounted archive, an NFS share, a synced directory — these are
//! real places to keep artifacts, and a corpus that uses one needs no credentials and no
//! transport. That it also happens to make every behavioural test hermetic is a consequence,
//! not the purpose, and it is the reason the tests here exercise production code rather than
//! a mock that can drift from it.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use super::cas::ContentHash;
use super::config::VaultConfig;

/// A place bytes can be kept, addressed by content.
///
/// Deliberately three verbs. A store is not asked to list itself, and nothing in RFC-0023
/// ever enumerates one: every artifact a repository cares about is named by a committed
/// record, so the questions are always about a digest somebody already has.
pub trait Store {
    /// Where this store is, for a message. Never a credential.
    fn describe(&self) -> String;

    /// Whether the store already holds these bytes.
    fn has(&self, hash: &ContentHash) -> Result<bool>;

    /// Copy the artifact out of the store to `dest`.
    ///
    /// The implementation writes `dest` only on success; a caller finding a file there can
    /// rely on it being complete.
    fn get(&self, hash: &ContentHash, dest: &Path) -> Result<()>;

    /// Copy `src` into the store under `hash`.
    ///
    /// Idempotent: putting bytes the store already holds is a no-op rather than a rewrite.
    fn put(&self, hash: &ContentHash, src: &Path) -> Result<()>;

    /// What a `PUT` of this artifact would send, for `push --dry-run`.
    ///
    /// `None` where there is nothing to explain — a `file://` store copies a file and has no
    /// request to show. For S3 this is the canonical request, which is the *only* artifact of
    /// a signing bug a person can actually inspect: a server reports a mismatch as a bad
    /// signature and never says which byte it disagreed about.
    fn explain_put(&self, _hash: &ContentHash) -> Option<String> {
        None
    }
}

/// Open the store a vault declares.
///
/// Takes the vault's *name* as well as its config, because credentials are resolved per
/// vault — `YIDAM_VAULT_<NAME>_ACCESS_KEY_ID`, with the ambient `AWS_*` honoured only for
/// `default`. See [`super::creds`] for why that asymmetry is deliberate.
pub fn open(vault: &str, cfg: &VaultConfig) -> Result<Box<dyn Store>> {
    let url = cfg.url.trim();
    if let Some(rest) = url.strip_prefix("file://") {
        return Ok(Box::new(FileStore::new(file_url_path(rest, url)?)));
    }
    if url.starts_with("s3://") {
        #[cfg(feature = "vault-s3")]
        return Ok(Box::new(super::s3::S3Store::new(vault, cfg)?));
        #[cfg(not(feature = "vault-s3"))]
        bail!(
            "vault url {url:?} names an S3 store, and this build has no S3 transport.\n  \
             `vault-s3` is in the default feature set, so this is a build compiled without \
             it — `yidam --version` lists what is compiled in. A `file:///…` url works in \
             every build."
        );
    }
    bail!(
        "vault url {url:?} has no scheme this build understands.\n  \
         Supported: `file:///path/to/directory`. `s3://bucket/prefix` is specified in \
         RFC-0023 and not yet built."
    )
}

/// The filesystem path a `file://` url names.
///
/// Hand-parsed rather than pulled from a URL crate: this module is ungated so that a light
/// build can read a vault, and `url` arrives through `reqwest`, which is not. The subset that
/// matters is small and the failures are worth naming individually — a `file://` url whose
/// path is silently wrong points a whole vault at the wrong directory.
fn file_url_path(rest: &str, whole: &str) -> Result<PathBuf> {
    // `file:///path` leaves `/path`; `file://localhost/path` leaves `localhost/path`.
    let path = match rest.strip_prefix("localhost/") {
        Some(p) => format!("/{p}"),
        None => rest.to_string(),
    };
    if !path.starts_with('/') {
        bail!(
            "vault url {whole:?} is not an absolute path.\n  \
             A `file://` url takes three slashes and an absolute path — \
             `file:///mnt/archive/yidam`. What follows two slashes is a host, and a vault \
             cannot live on one."
        );
    }
    Ok(PathBuf::from(percent_decode(&path)?))
}

/// Decode `%XX` escapes, and only those.
///
/// A path with a space in it is spelled `%20` in a url and there is no reason to refuse one.
/// An invalid escape is an error rather than a literal: `%2` at the end of a path is a typo,
/// and treating it as three characters would build a directory name nobody meant.
fn percent_decode(s: &str) -> Result<String> {
    if !s.contains('%') {
        return Ok(s.to_string());
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok());
            match hex {
                Some(b) => {
                    out.push(b);
                    i += 3;
                }
                None => bail!(
                    "vault url has an invalid percent-escape at byte {i}: {:?}",
                    &s[i..s.len().min(i + 3)]
                ),
            }
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).context("vault url decodes to invalid UTF-8")
}

/// A store that is a directory.
pub struct FileStore {
    root: PathBuf,
}

impl FileStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        FileStore { root: root.into() }
    }

    fn path_of(&self, hash: &ContentHash) -> PathBuf {
        self.root
            .join("sha256")
            .join(hash.shard())
            .join(hash.as_str())
    }
}

impl Store for FileStore {
    fn describe(&self) -> String {
        format!("file://{}", self.root.display())
    }

    fn has(&self, hash: &ContentHash) -> Result<bool> {
        Ok(self.path_of(hash).is_file())
    }

    fn get(&self, hash: &ContentHash, dest: &Path) -> Result<()> {
        let src = self.path_of(hash);
        if !src.is_file() {
            bail!("{} does not hold {hash}", self.describe());
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        // Through a temporary beside the destination, for the reason `Cache::put_file` gives:
        // a reader must never find a partial artifact under a name that asserts its contents.
        let tmp = temp_beside(dest, hash);
        std::fs::copy(&src, &tmp)
            .with_context(|| format!("copying {} to {}", src.display(), tmp.display()))?;
        std::fs::rename(&tmp, dest)
            .with_context(|| format!("moving {} into place at {}", tmp.display(), dest.display()))
    }

    fn put(&self, hash: &ContentHash, src: &Path) -> Result<()> {
        let dest = self.path_of(hash);
        if dest.is_file() {
            return Ok(());
        }
        let dir = dest.parent().expect("store paths always have a parent");
        std::fs::create_dir_all(dir)
            .with_context(|| format!("creating {} in the vault", dir.display()))?;
        let tmp = temp_beside(&dest, hash);
        std::fs::copy(src, &tmp)
            .with_context(|| format!("copying {} into {}", src.display(), self.describe()))?;
        std::fs::rename(&tmp, &dest)
            .with_context(|| format!("moving {} into place at {}", tmp.display(), dest.display()))
    }
}

fn temp_beside(dest: &Path, hash: &ContentHash) -> PathBuf {
    let dir = dest.parent().unwrap_or(Path::new("."));
    dir.join(format!(".{}.{}.part", hash.as_str(), std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn cfg(url: &str) -> VaultConfig {
        VaultConfig {
            url: url.to_string(),
            audience: Some("test".into()),
            holds: None,
            region: None,
            endpoint: None,
            path_style: None,
        }
    }

    fn open_err(url: &str) -> String {
        open("default", &cfg(url))
            .map(|_| ())
            .expect_err("expected this url to be refused")
            .to_string()
    }

    fn file_with(dir: &Path, name: &str, bytes: &[u8]) -> PathBuf {
        let p = dir.join(name);
        std::fs::File::create(&p).unwrap().write_all(bytes).unwrap();
        p
    }

    #[test]
    fn a_file_url_resolves_to_its_directory() {
        for (url, want) in [
            ("file:///mnt/archive/yidam", "/mnt/archive/yidam"),
            ("file://localhost/mnt/archive", "/mnt/archive"),
            ("file:///mnt/My%20Archive", "/mnt/My Archive"),
        ] {
            let store = open("default", &cfg(url)).unwrap();
            assert_eq!(store.describe(), format!("file://{want}"), "for {url}");
        }
    }

    /// `file://mnt/archive` is a *host* called `mnt`, not a relative path. Silently treating
    /// it as one would point a vault at a directory nobody named.
    #[test]
    fn a_file_url_with_a_host_is_refused_rather_than_guessed_at() {
        let err = open_err("file://mnt/archive");
        assert!(err.contains("absolute"), "{err}");
        assert!(err.contains("file:///"), "carries the fix: {err}");
    }

    #[test]
    fn an_invalid_percent_escape_is_an_error_rather_than_a_literal() {
        assert!(open_err("file:///mnt/bad%2").contains("percent-escape"));
        assert!(open_err("file:///mnt/bad%zz").contains("percent-escape"));
    }

    /// An `s3://` url now resolves to a real store in a build carrying `vault-s3`, so what is
    /// left to distinguish is a *scheme nobody implements* — which must not be reported as
    /// though a feature were missing.
    #[test]
    fn an_unknown_scheme_is_refused_as_unknown() {
        let err = open_err("gopher://nope");
        assert!(err.contains("no scheme this build understands"), "{err}");
        assert!(err.contains("file:///"), "offers what does work: {err}");
    }

    /// An `s3://` url gets past scheme resolution. It may still fail on credentials, which is
    /// a different diagnosis and belongs to `creds`; what matters here is that it is no
    /// longer refused for the scheme.
    #[test]
    fn an_s3_url_is_no_longer_refused_for_its_scheme() {
        let err = open_err("s3://bucket/prefix");
        assert!(
            !err.contains("no scheme this build understands"),
            "resolved by scheme, not refused: {err}"
        );
        assert!(err.contains("credentials"), "{err}");
    }

    #[test]
    fn a_file_store_round_trips_an_artifact_by_its_digest() {
        let tmp = TempDir::new().unwrap();
        let store = FileStore::new(tmp.path().join("vault"));
        let src = file_with(tmp.path(), "doc.pdf", b"the document");
        let hash = ContentHash::of_file(&src).unwrap();

        assert!(!store.has(&hash).unwrap());
        store.put(&hash, &src).unwrap();
        assert!(store.has(&hash).unwrap());

        let out = tmp.path().join("fetched/doc.pdf");
        store.get(&hash, &out).unwrap();
        assert_eq!(std::fs::read(&out).unwrap(), b"the document");
        assert_eq!(ContentHash::of_file(&out).unwrap(), hash);
    }

    #[test]
    fn putting_what_the_store_already_holds_is_a_no_op() {
        let tmp = TempDir::new().unwrap();
        let store = FileStore::new(tmp.path().join("vault"));
        let src = file_with(tmp.path(), "a", b"bytes");
        let hash = ContentHash::of_file(&src).unwrap();

        store.put(&hash, &src).unwrap();
        let at = store.path_of(&hash);
        let mtime = std::fs::metadata(&at).unwrap().modified().unwrap();
        store.put(&hash, &src).unwrap();
        assert_eq!(std::fs::metadata(&at).unwrap().modified().unwrap(), mtime);
    }

    #[test]
    fn getting_what_the_store_does_not_hold_says_so_and_writes_nothing() {
        let tmp = TempDir::new().unwrap();
        let store = FileStore::new(tmp.path().join("vault"));
        let hash = ContentHash::of_bytes(b"never stored");
        let out = tmp.path().join("out");

        let err = store.get(&hash, &out).unwrap_err().to_string();
        assert!(err.contains(hash.as_str()), "{err}");
        assert!(!out.exists(), "a failed get must leave nothing behind");
    }

    /// The store and the cache use the same layout, so bytes are findable by the same key
    /// wherever they sit. A divergence here would be invisible until a pull.
    #[test]
    fn the_store_lays_artifacts_out_the_way_the_cache_does() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("v");
        let store = FileStore::new(&root);
        let cache = super::super::Cache::at(&root);
        let hash = ContentHash::of_bytes(b"anything");
        assert_eq!(store.path_of(&hash), cache.path_of(&hash));
    }
}
