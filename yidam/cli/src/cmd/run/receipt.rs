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

use super::manifest::{Capability, Run};

/// The receipt record's version. Bumped when a consumer that understood the previous version
/// would mis-read this one — see the module doc for why it exists before any consumer does.
///
/// **2** added what produced the output — [`Receipt::model`], [`Receipt::version`],
/// [`Receipt::config`] and [`Receipt::image_digest`] (#475, amended 2026-09-30). Every one is
/// optional and skipped when absent, so the bump is additive: a v1 reader ([`Landed`], and
/// [`Receipt::committed_state`] before it) names the fields it reads and ignores the rest, and
/// the shipped 0.17.0 binary reading a v2 receipt is a test (`receipt_compat`), not a claim.
pub const FORMAT_VERSION: u32 = 2;

/// One file, by repository-relative path and content digest.
///
/// `Deserialize` as well as `Serialize`, because a record format with no reader is the shape
/// #1028 was filed about: `.yidam/runs/` was written from the first commit and nothing but
/// [`Receipt::committed_state`]'s one field ever read it back. [`Receipt::landed`] is that
/// reader, and `doctor` is what asks.
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
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
    /// The digest of the resolved corpus the step was handed, where it was handed one (#1080).
    ///
    /// **In the input state, and not only recorded here.** The document is a function of
    /// [`Self::files`] *and of the CLI's own parser and link resolver*, and the second half is
    /// in nothing else this digest covers. Without it, changing `resolve_target` would leave
    /// every committed answer looking fresh while the thing every calculator actually read had
    /// moved underneath it — which is the failure a receipt exists to make impossible, not a
    /// refinement of it.
    ///
    /// `None` for a step whose `reads` admit no corpus node. Skipped when absent so that a
    /// connector's receipt is the bytes it was before this field existed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_graph_sha256: Option<String>,
    /// The digest of the script a typed calculator applied, where the step is one (RFC-0042).
    ///
    /// **Redundant today, and here deliberately.** `validate_gluon` refuses a declaration whose
    /// `reads` do not cover its `.glu`, so the script is one of [`Self::files`] and its digest is
    /// already in the input state by that route. What this field buys is that the input state does
    /// not *depend* on that rule still being in place: a digest that is only right because a
    /// second check is enforcing it somewhere else is one a later relaxation of that check breaks
    /// in silence, and the thing it would break is every committed answer looking fresh while the
    /// program that produced it had changed.
    ///
    /// It also makes the receipt readable without resolving a glob. `resolved_graph_sha256` is
    /// here on the same argument one layer over — a run should state what it read, not only which
    /// bytes it read it from.
    ///
    /// `None` for the shell arm, where the program is an argv and the file it names is an input
    /// like any other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub script_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Receipt {
    pub format_version: u32,
    pub step: String,
    pub kind: &'static str,
    pub verb: String,
    /// The `run` as declared, so a reader can see what was invoked without the manifest.
    ///
    /// Serialized untagged, so an argv capability's receipt is the sequence it always was and a
    /// typed one's is the table the manifest holds — see [`Run`]. A receipt is a record of a
    /// declaration, and rewriting it into some third spelling would make the two files disagree
    /// about what ran.
    pub run: Run,
    /// The digest of everything that determines this result — see [`Receipt::input_state`].
    pub input_state: String,
    pub input: Input,
    /// What the step declared it writes.
    pub writes: Vec<String>,
    /// What it produced, by digest.
    pub outputs: Vec<File>,
    /// The model an elector ran, where the producer is one (#477).
    ///
    /// **The four fields below are what produced the output, not what it was produced from**,
    /// and none of them is in [`Self::input_state`]. That is the line: the input state decides
    /// whether a step has already run against this corpus, and a new binary or a new image is
    /// not a new corpus — folding them in would re-run every step at every upgrade and commit
    /// nothing but the upgrade. What they are for is the reader that asks *who answered*: #477's
    /// independence lint compares them with the seat's registry row, and a receipt that did not
    /// say what ran leaves that comparison nothing to compare.
    ///
    /// Each is `None` where it does not apply, and absent from the YAML then, so a step that has
    /// none of them writes the bytes a v1 producer wrote but for the version line.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// The version of the program that produced the output, where yidam knows it: this binary's,
    /// for a step whose program is this binary — a built-in, or a typed calculator this binary
    /// interprets. `None` for a shell step, whose program is an argv yidam only launched.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// The digest of the configuration the producer ran under, where it has one beyond the
    /// corpus config already in [`Input::config_sha256`] — an elector's (#477).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    /// The content address of the container image a cluster pod ran, as `sha256:<hex>`.
    ///
    /// Only ever from a digest reference (`…@sha256:<hex>`) — see [`image_digest`]. A tag names
    /// whatever was pushed under it last, so recording one would be recording a claim that
    /// stops being true at the next push; a pod started from a tag records nothing here.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image_digest: Option<String>,
}

/// The digest an image reference pins, or `None` for a reference that does not pin one.
///
/// `registry/name@sha256:<64 hex>` gives `sha256:<64 hex>`; `registry/name:tag` gives `None`,
/// and so does anything after the `@` that is not a whole SHA-256 digest. The reference is the
/// one the pod was started from — Argo substitutes the same workflow parameter into the
/// container's `image` and into this argument — so no registry is asked and no tag is read.
pub fn image_digest(image: &str) -> Option<String> {
    let (_, digest) = image.rsplit_once('@')?;
    let hex = digest.strip_prefix("sha256:")?;
    (hex.len() == 64
        && hex
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)))
    .then(|| digest.to_string())
}

/// This binary's version, for a receipt whose producer is this binary.
pub fn this_version() -> Option<String> {
    Some(env!("CARGO_PKG_VERSION").to_string())
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
    run: &'a Run,
    manifest_sha256: &'a str,
    config_sha256: &'a str,
    reads: &'a [String],
    files: &'a [File],
    /// Absent for a step that was handed no resolved corpus, and absent rather than empty: a
    /// `None` and a digest-of-nothing are different histories, and serializing the second for
    /// the first would move every existing connector's input state for no reason.
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_graph_sha256: Option<&'a str>,
    /// The program, for the arm whose program is not in `run`. See [`Input::script_sha256`].
    #[serde(skip_serializing_if = "Option::is_none")]
    script_sha256: Option<&'a str>,
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
        resolved_graph_sha256: Option<&str>,
        script_sha256: Option<&str>,
    ) -> Result<String> {
        let id = Identity {
            kind: cap.kind.as_str(),
            verb: &cap.verb,
            run: &cap.run,
            manifest_sha256,
            config_sha256,
            reads: &cap.reads,
            files,
            resolved_graph_sha256,
            script_sha256,
            writes: &cap.writes,
        };
        Ok(sha256(
            serde_yaml::to_string(&id)
                .context("serializing the input state")?
                .as_bytes(),
        ))
    }

    /// The digest of a typed calculator's program, out of the inputs the step resolved to.
    ///
    /// **From `files` rather than from the file, and both callers use this.** `validate_gluon`
    /// refuses a declaration whose `reads` do not cover its `.glu`, so the program is one of the
    /// resolved inputs and its digest has already been taken — by `run` out of the commit, by
    /// `doctor` out of the working tree. Digesting the file a second time here would give the two
    /// commands two answers about one step the moment the tree is dirty, and the symptom would be a
    /// typed calculator that `doctor` reports stale forever while `run` says it is up to date.
    ///
    /// That is #1080's lesson repeated one field along: the input state is only an equality check
    /// if every producer of it is the same function.
    ///
    /// `None` for the shell arm, and `None` for a typed one whose script is somehow not among its
    /// inputs — which `validate_gluon` has already refused, so it is the conservative answer to a
    /// question that cannot be asked rather than a case.
    pub fn script_sha256(cap: &Capability, files: &[File]) -> Option<String> {
        let (script, _) = cap.run.gluon()?;
        files
            .iter()
            .find(|f| f.path == script)
            .map(|f| f.sha256.clone())
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

    /// The parts of a committed receipt a reader outside the executor asks about.
    ///
    /// A narrow struct rather than [`Receipt`] itself, for the reason
    /// [`Self::committed_state`] reads one field: this is a *reader* of a format a producer
    /// owns, and one that deserialized the whole record would refuse a receipt written by a
    /// later yidam that added a field. What a reader needs is the input state, so it can say
    /// whether the answer still stands, and the outputs, so it can say whether the files on
    /// disk are the bytes that were recorded.
    ///
    /// `None` only where the text does not parse as YAML at all, which is the same
    /// conservative answer [`Self::committed_state`] gives and for the same reason.
    pub fn landed(text: &str) -> Option<Landed> {
        serde_yaml::from_str::<Landed>(text).ok()
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("serializing the receipt")
    }
}

/// A committed receipt, as a reader of it needs it. See [`Receipt::landed`].
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Landed {
    /// The digest of everything that determined the recorded result.
    #[serde(default)]
    pub input_state: Option<String>,
    /// What the step produced, by digest.
    #[serde(default)]
    pub outputs: Vec<File>,
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
            run: Run::Argv(vec!["sh".into(), "x.sh".into()]),
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
            run: Run::Argv(vec!["sh".into(), "x.sh".into()]),
            input_state: Receipt::input_state(
                &capability(),
                &sha256(b"manifest"),
                &sha256(b""),
                &[File {
                    path: ".yidam/corpus/a.yml".into(),
                    sha256: sha256(b"a"),
                }],
                Some(&sha256(b"graph")),
                None,
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
                resolved_graph_sha256: Some(sha256(b"graph")),
                script_sha256: None,
            },
            writes: vec![".yidam/computed/**".into()],
            outputs: vec![File {
                path: ".yidam/computed/x.yml".into(),
                sha256: sha256(b"x"),
            }],
            model: None,
            version: None,
            config: None,
            image_digest: None,
        }
    }

    /// The bullet #471 states in the order it states it: from this first commit.
    #[test]
    fn a_receipt_carries_its_format_version() {
        let y = receipt().to_yaml().unwrap();
        assert!(
            y.starts_with(&format!("format_version: {FORMAT_VERSION}\n")),
            "{y}"
        );
    }

    #[test]
    fn only_a_digest_reference_yields_an_image_digest() {
        let hex = "0123456789abcdef".repeat(4);
        assert_eq!(
            image_digest(&format!("ghcr.io/x/yidam@sha256:{hex}")),
            Some(format!("sha256:{hex}"))
        );
        assert_eq!(
            image_digest(&format!("ghcr.io/x/yidam:0.17.0@sha256:{hex}")),
            Some(format!("sha256:{hex}")),
            "a tag beside a digest is decoration; the digest is what the runtime pulls"
        );
        for tag in [
            "ghcr.io/x/yidam:latest",
            "ghcr.io/x/yidam",
            "localhost:5000/yidam:dev",
            "ghcr.io/x/yidam@sha256:abc",
            &format!("ghcr.io/x/yidam@sha512:{hex}{hex}"),
            &format!("ghcr.io/x/yidam@sha256:{}", hex.to_uppercase()),
        ] {
            assert_eq!(image_digest(tag), None, "{tag}");
        }
    }

    /// An absent v2 field is absent from the YAML, a present one is written, and the narrow
    /// readers a v1 binary shipped read a v2 receipt to the same answer. (They move no input
    /// state by construction: [`Receipt::input_state`] is not handed them.)
    #[test]
    fn the_producer_fields_are_written_when_present_and_read_past_by_the_narrow_readers() {
        let bare = receipt();
        let y = bare.to_yaml().unwrap();
        for key in ["model:", "version:", "config:", "image_digest:"] {
            assert!(
                !y.lines().any(|l| l.starts_with(key)),
                "an absent `{key}` was written:\n{y}"
            );
        }
        let mut full = receipt();
        full.model = Some("m".into());
        full.version = Some("v".into());
        full.config = Some("c".into());
        full.image_digest = Some("sha256:d".into());
        let y2 = full.to_yaml().unwrap();
        for key in [
            "model: m",
            "version: v",
            "config: c",
            "image_digest: sha256:d",
        ] {
            assert!(y2.lines().any(|l| l == key), "`{key}` is missing:\n{y2}");
        }
        let landed = Receipt::landed(&y2).expect("a v1 reader reads a v2 receipt");
        assert_eq!(
            landed.input_state.as_deref(),
            Some(bare.input_state.as_str())
        );
        assert_eq!(landed.outputs.len(), 1);
        assert_eq!(
            Receipt::committed_state(&y2).as_deref(),
            Some(bare.input_state.as_str())
        );
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
            edited.input.resolved_graph_sha256.as_deref(),
            edited.input.script_sha256.as_deref(),
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

    /// A typed calculator, for the two assertions below.
    fn typed() -> (Capability, Vec<File>) {
        let cap = Capability {
            kind: super::super::manifest::Kind::Calculator,
            run: Run::Gluon {
                gluon: ".yidam/capabilities/x.glu".into(),
                calls: None,
            },
            reads: vec![".yidam/corpus/**".into(), ".yidam/capabilities/**".into()],
            writes: vec![".yidam/computed/**".into()],
            verb: "compute".into(),
            after: vec![],
            ageing_days: None,
        };
        let files = vec![
            File {
                path: ".yidam/capabilities/x.glu".into(),
                sha256: sha256(b"the script"),
            },
            File {
                path: ".yidam/corpus/a.yml".into(),
                sha256: sha256(b"a"),
            },
        ];
        (cap, files)
    }

    /// The program's digest is taken from the inputs, not from the file, and only for the arm whose
    /// program is not in `run`.
    ///
    /// The whole reason this is a function rather than two call sites: `run` and `doctor` resolve
    /// their inputs from different trees — a commit and the working tree — and a typed step whose
    /// digest each command took its own way would be reported stale forever by one of them.
    #[test]
    fn a_typed_step_takes_its_program_digest_from_its_resolved_inputs() {
        let (cap, files) = typed();
        assert_eq!(
            Receipt::script_sha256(&cap, &files),
            Some(sha256(b"the script"))
        );
        // The shell arm has no such field: its program is an argv, and the file that argv names is
        // an input like any other.
        assert_eq!(Receipt::script_sha256(&capability(), &files), None);
    }

    /// Editing a calculator re-runs it.
    ///
    /// The defect `script_sha256` is against, asserted where it can be seen. It holds twice over —
    /// the script is covered by `reads` so it is one of `files` too — and that redundancy is the
    /// point: the input state does not depend on the manifest rule that puts it there still being
    /// enforced somewhere else.
    #[test]
    fn the_input_state_moves_when_the_calculator_script_does() {
        let (cap, files) = typed();
        let before = Receipt::input_state(
            &cap,
            &sha256(b"manifest"),
            &sha256(b""),
            &files,
            None,
            Receipt::script_sha256(&cap, &files).as_deref(),
        )
        .unwrap();
        // Only the digest, and not the file list, so this asserts about the field rather than about
        // `files` carrying the same fact.
        let after = Receipt::input_state(
            &cap,
            &sha256(b"manifest"),
            &sha256(b""),
            &files,
            None,
            Some(&sha256(b"the script, revised")),
        )
        .unwrap();
        assert_ne!(before, after);
    }

    /// And a shell capability's input state is the bytes it was before the field existed.
    ///
    /// `skip_serializing_if` on both sides, for the reason `resolved_graph_sha256` states: a `None`
    /// and a digest-of-nothing are different histories, and serializing the second for the first
    /// would move every already-committed step's identity for no reason.
    #[test]
    fn the_new_field_does_not_move_a_shell_capabilitys_input_state() {
        let files = [File {
            path: ".yidam/corpus/a.yml".into(),
            sha256: sha256(b"a"),
        }];
        let state = Receipt::input_state(
            &capability(),
            &sha256(b"manifest"),
            &sha256(b""),
            &files,
            Some(&sha256(b"graph")),
            None,
        )
        .unwrap();
        assert_eq!(state, receipt().input_state);
        let y = receipt().to_yaml().unwrap();
        assert!(!y.contains("script_sha256"), "{y}");
    }

    #[test]
    fn the_receipt_path_is_one_file_per_step() {
        assert_eq!(Receipt::path("low-flow"), ".yidam/runs/low-flow.yml");
    }
}
