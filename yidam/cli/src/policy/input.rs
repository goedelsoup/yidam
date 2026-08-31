//! Building the document a decision is asked about.
//!
//! This is the *facts* half of RFC-0024's split, and it is the half that has to know things:
//! which paths a repository declared private, whether each of them holds anything but a
//! placeholder, and what a derived artifact was computed from. None of that is a judgement, and
//! none of it belongs in a `.rego` file — `holds_content` in particular is a filesystem walk,
//! and Rego has no filesystem.
//!
//! Kept here rather than at the call sites so that the equivalence tests and the callers that
//! replace `may_push` in #440 are asking the *same* question in the same shape. Two builders
//! for one decision is the shape this whole RFC exists to remove.

use std::path::Path;

use serde_json::{json, Value};

use crate::vault::{holds_content, Derived, Named};

/// The declared-private paths, each with whether it actually holds material.
///
/// The pair rather than the bare string, because the two decisions want different halves:
/// `disclose/record` judges a path and never asks what is inside it, and `disclose/derived`
/// asks precisely that. Supplying both lets one library serve both without either decision
/// guessing.
pub fn private_paths(root: &Path, declared: &[String]) -> Value {
    Value::Array(
        declared
            .iter()
            .map(|p| json!({"path": p, "holds_content": holds_content(&root.join(p))}))
            .collect(),
    )
}

/// `disclose/record` — a catalog artifact, judged by what its record says.
///
/// Carries no `vault:`. Routing is `Vaults::route`'s question and the two are kept apart
/// deliberately; see `vault::may_push`.
pub fn record(root: &Path, a: &Named, declared: &[String]) -> Value {
    let mut subject = json!({
        "rel": a.rel,
        "kind": a.kind,
        "sha256": a.hash.to_string(),
    });
    if let Some(r) = a.redistributable {
        subject["redistributable"] = json!(r);
    }
    if let Some(b) = a.bytes {
        subject["bytes"] = json!(b);
    }
    json!({
        "repo": {"private_paths": private_paths(root, declared)},
        "subject": subject,
    })
}

/// `disclose/derived` — a computed artifact, judged by what it was built from.
///
/// `sources` comes from `vault::derived_sources`, which is the binary that owns the bundle
/// answering for it. That is the half of #443 this fixes at the root: the release workflow used
/// to keep its own copy of that list, and the copy was wrong.
pub fn derived(root: &Path, d: Derived, declared: &[String]) -> Value {
    json!({
        "repo": {"private_paths": private_paths(root, declared)},
        "subject": {
            "kind": d.kind(),
            "sources": crate::vault::derived_sources(d),
        },
    })
}

/// `disclose/at_rest` — material sitting in the working tree.
///
/// `is_private` is a fact about the forge and cannot be discovered offline, so it is passed in.
/// In CI it is `github.event.repository.private`, which the runner already has — no API call, so
/// the job stays hermetic.
pub fn at_rest(root: &Path, declared: &[String], is_private: bool) -> Value {
    json!({
        "repo": {
            "is_private": is_private,
            "private_paths": private_paths(root, declared),
        },
    })
}
