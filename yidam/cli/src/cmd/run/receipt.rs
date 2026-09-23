//! The receipt — what ran, against what, producing which bytes.
//!
//! RFC-0023's sentence applied to execution: *a vault stores bytes, git stores the record of
//! them.* A receipt is that record for a computation rather than a fetch, and it is committed
//! in the same commit as the output it describes, so a tree that carries the bytes and one
//! that carries the account of them are the same tree.
//!
//! # `format_version` is here from the first commit
//!
//! Not added later, and the order is the whole point. A field added after release strands
//! every already-shipped producer: a consumer reading it crashes against the binary that
//! never wrote it, and the failure is at the reader, in a repository whose own CLI is fine.
//! `report.rs` states the same handshake for the report contract. This is the one for the
//! record format, and they are versioned separately because they change for different
//! reasons — a receipt field is a change to what a corpus commits, a report field is a change
//! to what a consumer parses.
//!
//! # No clock
//!
//! A receipt carries no timestamp. The commit it lands in has a committer date, which is the
//! real one; a second date written into the file would be the same fact recorded twice and
//! the only copy anyone could forge. It also makes a receipt a pure function of its input
//! state — which is what lets a re-run against an unchanged corpus produce a byte-identical
//! tree and therefore no commit at all, rather than an empty one per invocation.

use anyhow::{Context, Result};
use serde::Serialize;

use super::manifest::Capability;

/// The receipt record's version. Bumped when a consumer that understood the previous version
/// would mis-read this one — see the module doc for why it exists before any consumer does.
pub const FORMAT_VERSION: u32 = 1;

/// One file, by repository-relative path and content digest.
#[derive(Debug, Clone, Serialize)]
pub struct File {
    pub path: String,
    pub sha256: String,
}

/// The state a run was performed against.
///
/// RFC-0026 §1: *"a commit sha plus a digest over the config and manifest that governed it,
/// so **has this already run against this corpus** is an equality check rather than a
/// heuristic."* All three, so that an unchanged corpus read by an edited manifest is a
/// different input state and not a false match.
#[derive(Debug, Clone, Serialize)]
pub struct Input {
    /// The commit the step's inputs were materialized from. Never the working tree.
    pub commit: String,
    pub manifest_sha256: String,
    /// `.yidam/config.toml`'s digest, or the digest of nothing where there is no config.
    pub config_sha256: String,
    /// What the step declared it reads.
    pub reads: Vec<String>,
    /// What it was actually given — the declaration resolved against the commit.
    pub files: Vec<File>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub format_version: u32,
    pub step: String,
    pub kind: &'static str,
    pub verb: String,
    /// argv as declared, so a reader can see what was invoked without the manifest.
    pub run: Vec<String>,
    /// The digest of everything that determines this result — see [`Receipt::input_state`].
    pub input_state: String,
    pub input: Input,
    /// What the step declared it writes.
    pub writes: Vec<String>,
    /// What it produced, by digest.
    pub outputs: Vec<File>,
}

/// The part of a receipt that decides whether a run has already happened.
///
/// RFC-0026 §1 wants *"has this already run against this corpus"* to be an equality check
/// rather than a heuristic, and names the input state as a commit sha plus a digest over the
/// config and manifest. The commit sha is in the receipt and is deliberately **not** in this
/// digest, which was found by running the command twice.
///
/// A run advances the branch, so the second run of an unchanged corpus stands at a different
/// parent. Fold the parent into the identity and every invocation is a new input state, each
/// producing a commit whose only novelty is the sha of the one before it — a log of how often
/// somebody ran the command, growing without bound, of exactly the shape the no-op path
/// exists to prevent. What decides the answer is what the step read and what it declared, so
/// that is what is hashed. The commit sha stays in the receipt, where it says which corpus
/// commit these bytes were computed from, and that remains true across a re-run that changes
/// nothing.
#[derive(Serialize)]
struct Identity<'a> {
    kind: &'a str,
    verb: &'a str,
    run: &'a [String],
    manifest_sha256: &'a str,
    config_sha256: &'a str,
    reads: &'a [String],
    files: &'a [File],
    writes: &'a [String],
}

impl Receipt {
    /// Where a step's receipt lives.
    ///
    /// One path per step rather than one per run: the series is the git history of this file,
    /// which is where a repository's series of anything already lives. A directory of
    /// per-run receipts would be a second, unreviewed history growing without bound beside
    /// the one that is reviewed.
    pub fn path(step: &str) -> String {
        format!(".yidam/runs/{step}.yml")
    }

    /// The digest a later run compares against, over everything that determines the result.
    ///
    /// Takes the declaration whole rather than field by field: every field of it is part of
    /// the identity, so a signature that enumerated them would have to be revisited — and
    /// silently could not be — each time a field is added. #472 added two, `after` and
    /// `ageing_days`, and neither needed a line here: both were in the input state the day
    /// they parsed, which is the property this signature exists for.
    pub fn input_state(
        cap: &Capability,
        manifest_sha256: &str,
        config_sha256: &str,
        files: &[File],
    ) -> Result<String> {
        let id = Identity {
            kind: cap.kind.as_str(),
            verb: &cap.verb,
            run: &cap.run,
            manifest_sha256,
            config_sha256,
            reads: &cap.reads,
            files,
            writes: &cap.writes,
        };
        Ok(sha256(
            serde_yaml::to_string(&id)
                .context("serializing the input state")?
                .as_bytes(),
        ))
    }

    /// The `input_state` of the receipt already committed at a parent, if there is one.
    ///
    /// Read as YAML rather than by line, and a receipt that does not parse answers `None` —
    /// which re-runs the step. Refusing instead would let one malformed committed file wedge
    /// a corpus out of its own pipeline, and re-running is idempotent by construction.
    pub fn committed_state(text: &str) -> Option<String> {
        #[derive(serde::Deserialize)]
        struct Landed {
            input_state: Option<String>,
        }
        serde_yaml::from_str::<Landed>(text).ok()?.input_state
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("serializing the receipt")
    }
}

/// The digest every field above is written in.
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    let mut h = sha2::Sha256::new();
    h.update(bytes);
    hex::encode(h.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability() -> Capability {
        Capability {
            kind: super::super::manifest::Kind::Calculator,
            run: vec!["sh".into(), "x.sh".into()],
            reads: vec![".yidam/corpus/**".into()],
            writes: vec![".yidam/computed/**".into()],
            verb: "compute".into(),
            after: vec![],
            ageing_days: None,
        }
    }

    fn receipt() -> Receipt {
        Receipt {
            format_version: FORMAT_VERSION,
            step: "low-flow".into(),
            kind: "calculator",
            verb: "compute".into(),
            run: vec!["sh".into(), "x.sh".into()],
            input_state: Receipt::input_state(
                &capability(),
                &sha256(b"manifest"),
                &sha256(b""),
                &[File {
                    path: ".yidam/corpus/a.yml".into(),
                    sha256: sha256(b"a"),
                }],
            )
            .unwrap(),
            input: Input {
                commit: "a".repeat(40),
                manifest_sha256: sha256(b"manifest"),
                config_sha256: sha256(b""),
                reads: vec![".yidam/corpus/**".into()],
                files: vec![File {
                    path: ".yidam/corpus/a.yml".into(),
                    sha256: sha256(b"a"),
                }],
            },
            writes: vec![".yidam/computed/**".into()],
            outputs: vec![File {
                path: ".yidam/computed/x.yml".into(),
                sha256: sha256(b"x"),
            }],
        }
    }

    /// The bullet #471 states in the order it states it: from this first commit.
    #[test]
    fn a_receipt_carries_its_format_version() {
        let y = receipt().to_yaml().unwrap();
        assert!(y.starts_with("format_version: 1\n"), "{y}");
    }

    /// The property the absent clock buys, asserted rather than described.
    #[test]
    fn two_receipts_for_one_input_state_are_byte_identical() {
        assert_eq!(receipt().to_yaml().unwrap(), receipt().to_yaml().unwrap());
    }

    #[test]
    fn a_receipt_names_the_commit_it_was_computed_from() {
        let y = receipt().to_yaml().unwrap();
        assert!(y.contains(&"a".repeat(40)), "{y}");
    }

    /// The defect the digest was introduced for, asserted where it can be seen: two runs an
    /// unchanged corpus apart stand at different commits and have the same input state.
    #[test]
    fn the_input_state_does_not_move_when_only_the_parent_commit_does() {
        let mut later = receipt();
        later.input.commit = "b".repeat(40);
        assert_ne!(receipt().input.commit, later.input.commit);
        assert_eq!(receipt().input_state, later.input_state);
    }

    /// And it does move when what the step read does.
    #[test]
    fn the_input_state_moves_when_an_input_file_does() {
        let mut edited = receipt();
        edited.input.files[0].sha256 = sha256(b"edited");
        let restated = Receipt::input_state(
            &capability(),
            &edited.input.manifest_sha256,
            &edited.input.config_sha256,
            &edited.input.files,
        )
        .unwrap();
        assert_ne!(receipt().input_state, restated);
    }

    #[test]
    fn a_committed_receipt_yields_its_input_state() {
        let r = receipt();
        assert_eq!(
            Receipt::committed_state(&r.to_yaml().unwrap()),
            Some(r.input_state)
        );
        assert_eq!(Receipt::committed_state("not: a receipt"), None);
        assert_eq!(Receipt::committed_state("«"), None);
    }

    #[test]
    fn the_receipt_path_is_one_file_per_step() {
        assert_eq!(Receipt::path("low-flow"), ".yidam/runs/low-flow.yml");
    }
}
