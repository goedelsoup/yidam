//! The vault — bytes addressed by content, with the record of them kept in git.
//!
//! RFC-0023 states the constraint the whole design rests on:
//!
//! > **A vault stores bytes. Git stores the record of them** — which bytes, and which vault
//! > they are allowed in.
//!
//! Every pointer into a vault is a committed file, so a vault holds no mutable state at all.
//! A stale vault cannot lie, because the digest is in the commit; losing one costs no
//! knowledge claim, only the time to re-fetch; and garbage collection is exactly computable,
//! because the live set is the set the working tree names.
//!
//! # What is here, and what is deliberately not
//!
//! This module is **ungated**. It needs `sha2`, `hex` and `std`, all of which the light
//! `reports` build already has, so a binary with no network stack compiled in can still hash
//! an artifact, cache it, verify it, and read a vault on a mounted archive.
//!
//! That is the same split `deps.rs` arrived at for `tonpa`: the feature buys the *network*,
//! and reading a file, resolving a path and hashing bytes are none of them. The S3 transport
//! lands behind its own feature and reaches everything here through [`Store`], which is
//! synchronous for the reason given in `store.rs`.

mod cache;
mod cas;
mod config;
#[cfg(feature = "vault-s3")]
mod creds;
mod policy;
#[cfg(feature = "vault-s3")]
mod s3;
#[cfg(feature = "vault-s3")]
mod sigv4;
mod store;

pub use cache::{Cache, Verdict};
pub use cas::ContentHash;
pub use config::{resolve, VaultConfig, ONLY_VAULT};
pub use policy::{may_push, named_artifacts, read_private_paths, Disposition, Named};
pub use store::{open, Store};

#[cfg(feature = "vault-s3")]
pub use s3::{S3Store, SignedRequest, MAX_SINGLE_PUT};

/// Whether a vault's credentials are in the environment, without building a store.
///
/// `doctor` needs to ask this and must not construct a client to do it: building one resolves
/// credentials *and* a runtime, and reporting "cannot build a runtime" where the answer is
/// "you have not exported a key" would be a diagnosis about the wrong thing.
///
/// In a build without the transport there are no credentials to want, so the question is
/// vacuously answered — the same shape `paths.rs` uses for reading a tonpa lock in a binary
/// that cannot fetch one.
pub fn credentials_available(vault: &str) -> anyhow::Result<()> {
    #[cfg(feature = "vault-s3")]
    {
        creds::resolve(vault, |k| std::env::var(k).ok()).map(|_| ())
    }
    #[cfg(not(feature = "vault-s3"))]
    {
        let _ = vault;
        Ok(())
    }
}
