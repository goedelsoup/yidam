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
mod store;

pub use cache::{Cache, Verdict};
pub use cas::ContentHash;
pub use config::{resolve, VaultConfig, ONLY_VAULT};
pub use store::{open, Store};
