//! Content addressing — the one identity every artifact in a vault has.
//!
//! # Why the hash is the name
//!
//! RFC-0023's whole design rests on one constraint: *a vault stores bytes; git stores the
//! record of them.* That only works if the record can name bytes unambiguously, which means
//! the name has to be a function of the bytes and nothing else. A path can be moved, a key
//! can be reused, a filename lies the moment somebody edits the file underneath it. A digest
//! cannot.
//!
//! Everything else in the design follows from that and is not an independent decision: push
//! is idempotent because writing the same bytes twice writes the same key; deduplication is
//! free because two repositories that fetched the same paper computed the same name for it;
//! and garbage collection is exactly computable because the set of live names is the set the
//! working tree mentions.
//!
//! # SHA-256, and why the algorithm is in the key
//!
//! `sha2` is already a base dependency — `tonpa` hashes bundles with it
//! ([`crate::deps::sha256_hex`]) — so this adds nothing to the light build.
//!
//! The key spells `sha256/` explicitly rather than treating the digest as opaque. That costs
//! seven bytes per key and buys the only migration path a content-addressed store ever gets:
//! a second algorithm becomes a second prefix beside the first, both readable, with no
//! ambiguity about which function produced a given name. A store whose keys do not say what
//! they are has to be drained to change hash.

use std::fmt;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};

/// Bytes read per `read` while hashing a file.
///
/// 64 KiB rather than the whole file: a vault is expected to hold a vector index, and those
/// run to hundreds of megabytes. Reading one into memory to name it would make the memory
/// cost of *identifying* an artifact proportional to its size — which is the same defect
/// RFC-0023 avoids on upload by streaming the body, and it would be odd to avoid it there
/// and not here.
const CHUNK: usize = 64 * 1024;

/// The SHA-256 of an artifact's bytes, as 64 lowercase hex characters.
///
/// A newtype rather than a `String`, because the two are used in the same places and only
/// one of them is a content address. Every route into this type validates, so a `Hash` in
/// hand is a well-formed digest and no caller has to re-check one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ContentHash(String);

impl ContentHash {
    /// Parse a digest a record or a command line supplied.
    ///
    /// **Lowercase only, and that is a decision rather than an oversight.** Hex is
    /// case-insensitive and accepting either spelling would mean one artifact has two names,
    /// which in a content-addressed store means two keys, two cache entries, and a `verify`
    /// that can disagree with itself. Uppercase input is rejected with the lowercase form in
    /// the message, so the fix is a copy and paste rather than a puzzle.
    pub fn parse(s: &str) -> Result<Self> {
        let t = s.trim();
        if t.len() != 64 {
            bail!(
                "not a sha256: expected 64 hex characters, got {} in {t:?}",
                t.len()
            );
        }
        if let Some(bad) = t.chars().find(|c| !matches!(c, '0'..='9' | 'a'..='f')) {
            if bad.is_ascii_uppercase() {
                bail!(
                    "not a sha256: {t:?} is uppercase; vault keys are lowercase hex — use {}",
                    t.to_ascii_lowercase()
                );
            }
            bail!("not a sha256: {t:?} contains {bad:?}, which is not lowercase hex");
        }
        Ok(ContentHash(t.to_string()))
    }

    /// The digest of bytes already in memory.
    pub fn of_bytes(bytes: &[u8]) -> Self {
        let mut h = Sha256::new();
        h.update(bytes);
        ContentHash(hex::encode(h.finalize()))
    }

    /// The digest of a file, read in chunks.
    ///
    /// This is also the number a signed upload needs before it sends anything, which is why
    /// RFC-0023 can promise an S3 `PUT` that streams from disk and still sets a real
    /// `x-amz-content-sha256`: the payload hash is already known, because naming the
    /// artifact required computing it.
    pub fn of_file(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("opening {} to hash", path.display()))?;
        let mut reader = BufReader::with_capacity(CHUNK, file);
        let mut hasher = Sha256::new();
        let mut buf = vec![0u8; CHUNK];
        loop {
            let n = reader
                .read(&mut buf)
                .with_context(|| format!("reading {}", path.display()))?;
            if n == 0 {
                break;
            }
            hasher.update(&buf[..n]);
        }
        Ok(ContentHash(hex::encode(hasher.finalize())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The two-character directory a digest is filed under.
    ///
    /// Fanning out by the first byte keeps any one directory to roughly 1/256th of the
    /// store. That matters for the local cache, which is machine-wide and shared across
    /// every repository on the box, and it is free for an object store, where the prefix is
    /// a naming convention rather than a directory.
    pub fn shard(&self) -> &str {
        &self.0[..2]
    }

    /// The key this artifact has in a store rooted at `prefix`.
    ///
    /// `prefix` may be empty, for a store whose root is the bucket itself.
    pub fn key(&self, prefix: &str) -> String {
        let p = prefix.trim_matches('/');
        if p.is_empty() {
            format!("sha256/{}/{}", self.shard(), self.0)
        } else {
            format!("{p}/sha256/{}/{}", self.shard(), self.0)
        }
    }
}

impl fmt::Display for ContentHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// The published SHA-256 of the empty input and of `abc`. Pinned against the standard
    /// rather than against this implementation's own output, so the test fails if the
    /// algorithm is ever swapped for something that merely round-trips with itself.
    const EMPTY: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
    const ABC: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";

    #[test]
    fn digests_match_the_published_vectors() {
        assert_eq!(ContentHash::of_bytes(b"").as_str(), EMPTY);
        assert_eq!(ContentHash::of_bytes(b"abc").as_str(), ABC);
    }

    /// The chunked file path and the in-memory path must agree, including across a buffer
    /// boundary — a hasher fed one chunk at a time is where an off-by-one hides, and it
    /// would produce a name that is stable, wrong, and indistinguishable from a right one.
    #[test]
    fn hashing_a_file_agrees_with_hashing_its_bytes_across_chunk_boundaries() {
        let tmp = TempDir::new().unwrap();
        for size in [0usize, 1, CHUNK - 1, CHUNK, CHUNK + 1, CHUNK * 2 + 7] {
            let bytes: Vec<u8> = (0..size).map(|i| (i % 251) as u8).collect();
            let p = tmp.path().join(format!("f{size}"));
            File::create(&p).unwrap().write_all(&bytes).unwrap();
            assert_eq!(
                ContentHash::of_file(&p).unwrap(),
                ContentHash::of_bytes(&bytes),
                "file and byte hashing disagree at {size} bytes"
            );
        }
    }

    #[test]
    fn a_key_names_its_algorithm_and_shards_by_the_first_byte() {
        let h = ContentHash::parse(ABC).unwrap();
        assert_eq!(h.shard(), "ba");
        assert_eq!(h.key("yidam"), format!("yidam/sha256/ba/{ABC}"));
        // A store rooted at the bucket has no prefix segment, and must not grow an empty one.
        assert_eq!(h.key(""), format!("sha256/ba/{ABC}"));
        assert_eq!(h.key("/yidam/"), format!("yidam/sha256/ba/{ABC}"));
    }

    /// One artifact must not have two names. Hex is case-insensitive and a store is not.
    #[test]
    fn an_uppercase_digest_is_refused_and_the_message_carries_the_fix() {
        let err = ContentHash::parse(&ABC.to_ascii_uppercase())
            .expect_err("uppercase hex is a second name for one artifact")
            .to_string();
        assert!(err.contains(ABC), "the message should carry the fix: {err}");
    }

    #[test]
    fn malformed_digests_are_refused_with_what_was_wrong() {
        assert!(ContentHash::parse("abc")
            .unwrap_err()
            .to_string()
            .contains("64"));
        let err = ContentHash::parse(&"g".repeat(64)).unwrap_err().to_string();
        assert!(err.contains("not lowercase hex"), "{err}");
    }

    /// Surrounding whitespace is what a digest copied out of a report or piped from another
    /// command arrives with, and rejecting it would be a papercut with no safety in it.
    #[test]
    fn a_digest_may_arrive_with_whitespace_around_it() {
        assert_eq!(
            ContentHash::parse(&format!("  {ABC}\n")).unwrap().as_str(),
            ABC
        );
    }
}
