//! The phase record — what a phase began from, and what it has done since.
//!
//! `prelude/PHASES.md` specifies a phase as a name, an **input state**, a body of agent work,
//! declared outputs and a `--no-ff` merge. Until this record nothing in a repository held any
//! of it: `cmd/phases.rs` derived a phase's state from its ref's namespace, so *active* meant
//! *a ref matching a glob* and an interrupted phase was indistinguishable from a healthy one.
//!
//! # Why it is committed, and on the phase's own branch
//!
//! RFC-0028 §3 ranks two evidence sources: *"where a run record exists **for a ref**, the
//! record is authoritative for state."* For a ref — not for the checkout. `yidam phases` reads
//! remote-tracking refs, which is the fresh-clone shape CI has, and a record that lived only
//! in a working tree would answer for the one branch somebody happens to be standing on and
//! for no other. So the record is a committed file on `phase/<slug>`, read back with
//! `git show <ref>:<path>`, and a phase opened on a machine you have never seen is as legible
//! as one you opened yourself.
//!
//! # What the snapshot pins, and why the kuten revision is in it twice
//!
//! [`Input`] is RFC-0026 §1's input state — *"a commit sha plus a digest over the config and
//! manifest that governed it"* — plus the kuten the phase's declared type was validated
//! against. The commit sha already pins the vendored kuten in-tree, so the revision is
//! redundant *as data*; it is recorded anyway for RFC-0026's own reason, that
//! `has this already run against this corpus` should be **an equality check rather than a
//! heuristic**. Comparing two integers answers *was this phase started against the kuten this
//! repository now holds*; reaching the same answer from the sha means re-reading a file out of
//! a commit and re-parsing it, which is the heuristic in a costume.
//!
//! RFC-0028 §2's refuse-or-annotate rule then applies to the pair: *"a run record must not
//! validate its type against a list that changed under it."* [`Record::revision_skew`] is that
//! comparison, and every surface that reads a record reports it rather than proceeding
//! quietly.
//!
//! # No clock
//!
//! For `cmd/run/receipt.rs`'s reason, unchanged: the commit carrying the record has a
//! committer date, which is the real one, and a second date written into the file would be the
//! same fact recorded twice with only one forgeable copy.

use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// The record format's version, here from the first commit for the reason
/// [`crate::cmd::run::receipt::FORMAT_VERSION`] gives: a field added after release strands
/// every already-shipped producer, and the failure lands at the reader.
pub const FORMAT_VERSION: u32 = 1;

/// Where a phase's record lives.
///
/// One file per phase, under `.yidam/phases/`, beside `.yidam/runs/` for the same reason that
/// directory gives: the series is this file's git history, which is where a repository's
/// series of anything already lives.
pub fn path(slug: &str) -> String {
    format!(".yidam/phases/{slug}.yml")
}

/// `phase/outcome-axis` → `outcome-axis`; `outcome-axis` → itself.
///
/// Takes the last segment rather than stripping a known prefix, so a remote-tracking
/// `origin/phase/outcome-axis` and a local `phase/outcome-axis` resolve to one record.
pub fn slug_of(ref_or_name: &str) -> &str {
    ref_or_name.rsplit('/').next().unwrap_or(ref_or_name)
}

/// The state a phase's inputs were snapshotted at when it opened.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Input {
    /// The baseline commit the phase branched from. Never the working tree.
    pub commit: String,
    /// `.yidam/capabilities.toml`'s digest, or the digest of nothing where there is none.
    pub manifest_sha256: String,
    /// `.yidam/config.toml`'s digest, or the digest of nothing where there is none.
    pub config_sha256: String,
    /// The kuten this repository declared when the phase opened, if it declared one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kuten: Option<String>,
    /// That kuten's revision — the field [`Record::revision_skew`] compares.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kuten_revision: Option<u32>,
}

/// What the executor did about one step of the phase's plan.
///
/// The vocabulary is [`crate::cmd::run::Outcome`]'s, carried rather than restated: a step this
/// says `ran` is one the run reported as having been invoked, and a consumer comparing a
/// receipt against this record is comparing two records of one event.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Step {
    pub step: String,
    /// `ran` or `skipped`, as the run reported it.
    pub outcome: String,
    /// The commit the step landed, where it landed one. A step that recomputed the bytes
    /// already committed lands nothing, and that is not a failure — see
    /// `cmd/run`'s `unchanged_outputs`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// One phase, as the repository holds it.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Record {
    pub format_version: u32,
    /// The phase's slug — the ref's last segment, and this file's stem.
    pub phase: String,
    /// The type declared at `start`, validated against the vendored kuten's list where the
    /// repository holds one. Free text where it holds none, because a repository with no
    /// kuten has no list to be wrong about — RFC-0028 Erratum 1: *"`render_block`'s no-kuten
    /// arm prints no types at all, and that is correct."*
    pub r#type: String,
    pub input: Input,
    /// The plan `yidam phase run` resolved, in the order it resolved it. Absent until a run
    /// begins, which is the difference between *a phase that has not run* and *a phase whose
    /// run stopped partway* — the two [`Record::state`] has to tell apart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plan: Option<Vec<String>>,
    /// Steps recorded as they completed. The prefix of `plan` this phase has got through.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub completed: Vec<Step>,
}

impl Record {
    /// A phase at its opening: a snapshot, a declared type, and no run.
    pub fn opened(slug: &str, r#type: &str, input: Input) -> Self {
        Self {
            format_version: FORMAT_VERSION,
            phase: slug.to_string(),
            r#type: r#type.to_string(),
            input,
            plan: None,
            completed: Vec::new(),
        }
    }

    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("serializing the phase record")
    }

    pub fn parse(text: &str) -> Result<Self> {
        serde_yaml::from_str(text).context("parsing the phase record")
    }

    /// Whether `step` is already recorded as done, which is what makes a run resumable.
    pub fn is_completed(&self, step: &str) -> bool {
        self.completed.iter().any(|s| s.step == step)
    }

    /// The steps of the resolved plan this phase has not got through yet.
    pub fn remaining(&self) -> Vec<String> {
        self.plan
            .iter()
            .flatten()
            .filter(|s| !self.is_completed(s))
            .cloned()
            .collect()
    }

    /// What this record says the phase is: `active` or `interrupted`.
    ///
    /// **It never says `settled`,** and the omission is the design rather than a gap. A record
    /// is committed on the phase's own branch, so it is written strictly before the merge that
    /// settles the phase and could only claim settlement by predicting one. Settlement is a
    /// fact about the *baseline* — is this ref an ancestor of it — which is the question
    /// [`crate::git::ref_state`] already answers and the reason RFC-0028 §3 keeps both sources
    /// and ranks them instead of collapsing them into one.
    ///
    /// So the record answers the one thing the ref cannot: a plan was resolved and its steps
    /// did not all complete, which from outside the process is indistinguishable from a
    /// healthy phase nobody has touched today.
    pub fn state(&self) -> &'static str {
        match self.plan.as_deref() {
            Some(plan) if plan.iter().any(|s| !self.is_completed(s)) => "interrupted",
            _ => "active",
        }
    }

    /// Whether the kuten has moved under this phase since it opened.
    ///
    /// `Some((then, now))` when the repository's declared revision differs from the one the
    /// phase's type was validated against. RFC-0028 §2 is the rule — *cross-revision
    /// comparison refuses or annotates, never silently proceeds* — and every reader here
    /// annotates: a phase in flight across a re-vendor is an ordinary event and refusing to
    /// report on it would strand the work rather than the mismatch.
    pub fn revision_skew(&self, held: Option<u32>) -> Option<(Option<u32>, Option<u32>)> {
        (self.input.kuten_revision != held).then_some((self.input.kuten_revision, held))
    }
}

/// The record committed at `git_ref`: `None` where the ref carries none, `Err` where it
/// carries one that does not parse.
///
/// Reads through `git show` rather than the working tree, which is what lets one repository
/// answer for a phase that exists only as `origin/phase/<slug>`.
///
/// **Absence and malformation are different answers**, and the two callers want different
/// things from them. A command acting on *this* phase must not read a corrupted record as an
/// unrecorded one and tell somebody to open the phase they are standing in; a table of
/// twenty-six refs must not be wedged by one bad file. So the distinction is made here once
/// and [`read_at`] is the arm that discards it.
pub fn read(root: &Path, git_ref: &str, slug: &str) -> Result<Option<Record>> {
    let out = crate::git::Git::new(root)
        .arg("show")
        .rev(format!("{git_ref}:{}", path(slug)))
        .output()
        .context("reading the phase record out of git")?;
    if !out.status.success() {
        return Ok(None);
    }
    let text = String::from_utf8(out.stdout).context("the phase record is not UTF-8")?;
    Record::parse(&text)
        .map(Some)
        .with_context(|| format!("{git_ref}:{} is not a phase record", path(slug)))
}

/// The record committed at `git_ref`, or `None` where there is none to be had.
///
/// The lossy arm of [`read`], for `cmd/run/receipt.rs`'s reason applied to a different file:
/// one malformed committed record should cost a row its record, not the table its run.
pub fn read_at(root: &Path, git_ref: &str, slug: &str) -> Option<Record> {
    read(root, git_ref, slug).ok().flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input() -> Input {
        Input {
            commit: "a".repeat(40),
            manifest_sha256: "m".repeat(64),
            config_sha256: "c".repeat(64),
            kuten: Some("inquiry".into()),
            kuten_revision: Some(2),
        }
    }

    fn opened() -> Record {
        Record::opened("outcome-axis", "Investigation", input())
    }

    #[test]
    fn a_record_carries_its_format_version_first() {
        let y = opened().to_yaml().unwrap();
        assert!(y.starts_with("format_version: 1\n"), "{y}");
    }

    /// The property the absent clock buys — `cmd/run/receipt.rs`'s, restated here because a
    /// record written twice from one state must not produce a commit the second time.
    #[test]
    fn two_records_of_one_state_are_byte_identical() {
        assert_eq!(opened().to_yaml().unwrap(), opened().to_yaml().unwrap());
    }

    #[test]
    fn a_record_round_trips() {
        let r = opened();
        assert_eq!(Record::parse(&r.to_yaml().unwrap()).unwrap(), r);
    }

    /// The whole point of the record, in three lines: a phase that has never run and a phase
    /// whose run finished are both healthy, and one that stopped partway is not.
    #[test]
    fn a_plan_with_a_step_left_is_interrupted_and_nothing_else_is() {
        let mut r = opened();
        assert_eq!(r.state(), "active", "no plan resolved yet");

        r.plan = Some(vec!["low-flow".into(), "travel-tier".into()]);
        assert_eq!(r.state(), "interrupted");

        r.completed.push(Step {
            step: "low-flow".into(),
            outcome: "ran".into(),
            commit: Some("abc1234".into()),
        });
        assert_eq!(r.state(), "interrupted", "one of two is still partway");

        r.completed.push(Step {
            step: "travel-tier".into(),
            outcome: "skipped".into(),
            commit: None,
        });
        assert_eq!(r.state(), "active");
        assert!(r.remaining().is_empty());
    }

    /// A record never claims settlement, however complete its plan — see [`Record::state`].
    #[test]
    fn a_record_cannot_report_itself_settled() {
        let mut r = opened();
        r.plan = Some(vec!["low-flow".into()]);
        r.completed.push(Step {
            step: "low-flow".into(),
            outcome: "ran".into(),
            commit: None,
        });
        assert_eq!(r.state(), "active");
        assert_ne!(r.state(), "settled");
    }

    #[test]
    fn revision_skew_is_the_comparison_and_absence_is_a_value() {
        assert_eq!(opened().revision_skew(Some(2)), None);
        assert_eq!(opened().revision_skew(Some(3)), Some((Some(2), Some(3))));
        assert_eq!(opened().revision_skew(None), Some((Some(2), None)));

        let mut no_kuten = opened();
        no_kuten.input.kuten = None;
        no_kuten.input.kuten_revision = None;
        assert_eq!(no_kuten.revision_skew(None), None, "held none, opened none");
        assert_eq!(no_kuten.revision_skew(Some(2)), Some((None, Some(2))));
    }

    /// One record per phase however the ref that names it is spelled.
    #[test]
    fn a_slug_is_the_last_segment_local_or_remote() {
        assert_eq!(slug_of("phase/outcome-axis"), "outcome-axis");
        assert_eq!(slug_of("origin/phase/outcome-axis"), "outcome-axis");
        assert_eq!(slug_of("outcome-axis"), "outcome-axis");
    }

    #[test]
    fn the_record_path_is_one_file_per_phase() {
        assert_eq!(path("outcome-axis"), ".yidam/phases/outcome-axis.yml");
    }

    /// Absence is the supported state — every phase in every repository predates this record
    /// (RFC-0028 §3, *"refs without run records exist forever"*) — and malformation must cost
    /// the row its record rather than the table its run.
    #[test]
    fn an_unparseable_record_is_no_record() {
        assert!(Record::parse("«").is_err());
        assert!(Record::parse("format_version: 1\nphase: x\n").is_err());
    }
}
