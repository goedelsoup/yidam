//! A round's memory: what `cycle --record` keeps, and what `--since-last` reads back (#1443).
//!
//! A loop runs orient, choose, act, verify, record, repeat, and before this nothing kept the
//! *record* half. `lint` compares against the committed baseline, not against the last round;
//! `score` has no overall number by design; nothing diffed two `cycle` runs. So an agent that
//! had just acted could not ask whether the act helped, and an agent resuming after a
//! compaction — 25 of 29 derived-repo sessions in the 2026-11-04 scan — could not ask what the
//! previous round left.
//!
//! # Local state, never provenance
//!
//! The record is `.yidam/loop/<branch>.json`, and nothing commits it. It says what *this
//! checkout* saw when a loop last looked, which is a fact about a working session rather than
//! about the corpus — the same kind of fact as `.yidam/record/calls.jsonl`, and kept out of
//! git for the same reason. **The directory ignores itself**: `store` writes a `.gitignore`
//! holding `*` beside the first record, so a derived repository scaffolded before this
//! existed needs no migration for `git add -A` to leave it alone.
//!
//! # One record per branch
//!
//! #1443 left this open, and it is decided here for the loop it names: one that works across
//! phase branches. With a single file, a round on `phase/a` followed by one on `main` would
//! report everything that differs between two branches as what the round did. Detached HEAD
//! gets `HEAD.json`, which no branch can collide with: git refuses `HEAD` as a branch name.
//!
//! # Lists, and no total
//!
//! The other open question was whether "worse" can be stated without becoming a score a person
//! learns to read as a defect count — `due`'s founding worry, and `cycle`'s by inheritance.
//! This takes the conservative arm. Each half of the delta is two lists, what arrived and what
//! left, keyed by the identity its own surface already uses: a finding by `(gate, check,
//! node)` as the baseline keys it, a clock by id, a step by name, a ref by name. There is no
//! count across halves and no verdict, and `--strict` keeps the meaning it had: what is owed
//! or blocked *now*, which already fails on every arrival that gates.

use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};

use super::CycleReport;
use crate::cmd::due::State;
use crate::git::Git;

/// Where the records live, repo-relative.
pub const LOOP_DIR: &str = ".yidam/loop";

/// The record's own format. A record written by another format is reported unread, not
/// guessed at: a delta against a record this binary misread is a delta nobody can trust.
const FORMAT: u32 = 1;

/// One finding, by the identity the baseline compares on.
///
/// `gate` as well, because `cycle`'s blocking half composes two gates and a graph-check issue on
/// a node is a different fact from a lint finding on it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
pub struct Finding {
    pub gate: String,
    pub check: String,
    pub node: String,
}

/// What one round saw. Identities only: the delta is over what is present, and the details
/// are the live report's to give.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Round {
    pub format: u32,
    /// `None` on a detached HEAD.
    pub branch: Option<String>,
    /// The commit the round stood on, in full.
    pub head: String,
    /// Every gating finding, accepted or not — a multiset, sorted.
    pub findings: Vec<Finding>,
    /// The ids of the clocks that were due.
    pub due: Vec<String>,
    pub stale_steps: Vec<String>,
    /// Inquiry refs in flight.
    pub in_flight: Vec<String>,
    /// Local `propose/*` heads, without `refs/heads/`.
    pub proposals: Vec<String>,
    /// What the checkout looked like when the round was read, from [`fingerprint`]: absent
    /// unless a hook recorded the round. Absent in a record written before #1444, which
    /// reads as format 1 still — a record without it is one no fingerprint can match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fingerprint: Option<String>,
}

impl Round {
    /// Read the round off a report, plus the two facts a report does not carry: where HEAD is,
    /// and which proposals exist.
    pub fn of(root: &Path, report: &CycleReport) -> Self {
        let mut findings = report.findings.clone();
        findings.sort();
        let mut due: Vec<String> = report
            .owed
            .iter()
            .filter(|c| c.state == State::Due)
            .map(|c| c.id.to_string())
            .collect();
        due.sort();
        let mut stale_steps: Vec<String> =
            report.stale_steps.iter().map(|s| s.step.clone()).collect();
        stale_steps.sort();
        let mut in_flight: Vec<String> = report
            .in_flight
            .iter()
            .map(|f| f.ref_name.clone())
            .collect();
        in_flight.sort();
        Self {
            format: FORMAT,
            branch: crate::cmd::propose::write::current_branch(root),
            head: crate::cmd::propose::write::head(root)
                .map(|(full, _)| full)
                .unwrap_or_default(),
            findings,
            due,
            stale_steps,
            in_flight,
            proposals: proposals(root),
            fingerprint: None,
        }
    }

    /// The head as a reader is shown it.
    pub fn short_head(&self) -> &str {
        self.head.get(..8).unwrap_or(&self.head)
    }
}

/// Local proposal heads. Local only, for `stale_steps`' reason: this reads the checkout it
/// stands in and stays offline.
fn proposals(root: &Path) -> Vec<String> {
    let listed = Git::new(root)
        .args(["for-each-ref", "--format=%(refname)", "refs/heads/propose/"])
        .try_run()
        .unwrap_or_default();
    let mut out: Vec<String> = listed
        .lines()
        .filter_map(|l| l.strip_prefix("refs/heads/"))
        .map(str::to_string)
        .collect();
    out.sort();
    out
}

/// A digest of everything a round's halves are read from, so a hook can tell that nothing
/// could have moved without reading the corpus (#1444).
///
/// The Stop hook fires after every turn, and a `cycle` read costs from half a second to
/// several on the derived corpora measured. Most turns change nothing a round sees, and this
/// is what lets those turns cost one `git status` instead. It covers HEAD, every ref and the
/// object it names (in-flight refs, proposals), each path `git status` reports with its size
/// and modification time (the working tree, which `lint` walks), and the day (the clocks).
///
/// **A match means nothing changed; a mismatch means only that something might have.** A
/// change git cannot see — an edit to an ignored file — is missed, and those are not files a
/// round reads. `None` when git will not answer, which no stored fingerprint matches.
pub fn fingerprint(root: &Path, today: i64) -> Option<String> {
    use sha2::{Digest, Sha256};
    let head = Git::new(root)
        .args(["rev-parse", "--verify", "-q", "HEAD"])
        .try_run()
        .unwrap_or_default();
    let refs = Git::new(root)
        .args(["for-each-ref", "--format=%(objectname) %(refname)"])
        .try_run()?;
    // Raw, not `try_run`: that trims, and the first entry's status column can be a space.
    let out = Git::new(root)
        .args([
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--ignore-submodules",
        ])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let status = String::from_utf8_lossy(&out.stdout).into_owned();
    let mut h = Sha256::new();
    for part in [head.as_str(), refs.as_str(), status.as_str()] {
        h.update(part.as_bytes());
        h.update([0]);
    }
    h.update(today.to_le_bytes());
    let mut items = status.split('\0').filter(|i| !i.is_empty());
    while let Some(item) = items.next() {
        let (xy, path) = (item.get(..2).unwrap_or(""), item.get(3..).unwrap_or(""));
        // A rename or copy is followed by its source path, which is not an entry.
        if xy.contains('R') || xy.contains('C') {
            items.next();
        }
        let stamp = std::fs::symlink_metadata(root.join(path)).ok().map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_nanos());
            (m.len(), mtime)
        });
        h.update(format!("{path}\0{stamp:?}\0").as_bytes());
    }
    Some(h.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The record file for `branch`, absolute.
pub fn path(root: &Path, branch: Option<&str>) -> PathBuf {
    root.join(LOOP_DIR)
        .join(format!("{}.json", branch.unwrap_or("HEAD")))
}

/// The last round recorded on `branch`: `Ok(None)` when there is none, `Err` with a sentence
/// when there is one this binary cannot read.
pub fn load(root: &Path, branch: Option<&str>) -> std::result::Result<Option<Round>, String> {
    let p = path(root, branch);
    let text = match std::fs::read_to_string(&p) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("{} could not be read: {e}", rel(root, &p))),
    };
    let value: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| format!("{} is not JSON: {e}", rel(root, &p)))?;
    let format = value.get("format").and_then(serde_json::Value::as_u64);
    if format != Some(u64::from(FORMAT)) {
        return Err(format!(
            "{} was recorded in format {}, and this binary reads format {FORMAT} — \
             `yidam cycle --record` replaces it",
            rel(root, &p),
            format.map_or_else(|| "none".to_string(), |f| f.to_string())
        ));
    }
    serde_json::from_value(value)
        .map(Some)
        .map_err(|e| format!("{} does not hold a round: {e}", rel(root, &p)))
}

/// Write `round` as its branch's record, replacing the last one.
///
/// Through a temporary file and a rename, so a reader never sees half a record; and beside a
/// `.gitignore` of `*`, written once, so the directory is never staged.
pub fn store(root: &Path, round: &Round) -> Result<PathBuf> {
    let p = path(root, round.branch.as_deref());
    let dir = root.join(LOOP_DIR);
    std::fs::create_dir_all(p.parent().unwrap_or(&dir))
        .with_context(|| format!("creating {}", rel(root, &dir)))?;
    let ignore = dir.join(".gitignore");
    if !ignore.exists() {
        std::fs::write(
            &ignore,
            "# Round records from `yidam cycle --record`: local loop state, never committed.\n*\n",
        )
        .with_context(|| format!("writing {}", rel(root, &ignore)))?;
    }
    let tmp = p.with_extension(format!("json.{}.tmp", std::process::id()));
    let mut text = serde_json::to_string_pretty(round)?;
    text.push('\n');
    std::fs::write(&tmp, text).with_context(|| format!("writing {}", rel(root, &tmp)))?;
    std::fs::rename(&tmp, &p).with_context(|| format!("writing {}", rel(root, &p)))?;
    Ok(p)
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

/// What arrived in one half of the report since the last round, and what left.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct Moved<T> {
    pub arrived: Vec<T>,
    pub left: Vec<T>,
}

impl<T: Ord + Clone> Moved<T> {
    /// Multiset difference both ways, so a node tripping one check twice where it tripped it
    /// once is an arrival — the baseline's own accounting, and for its reason.
    fn between(before: &[T], after: &[T]) -> Self {
        Self {
            arrived: minus(after, before),
            left: minus(before, after),
        }
    }
}

fn minus<T: Ord + Clone>(x: &[T], y: &[T]) -> Vec<T> {
    let mut pool: Vec<&T> = y.iter().collect();
    let mut out = Vec::new();
    for v in x {
        match pool.iter().position(|w| *w == v) {
            Some(i) => {
                pool.remove(i);
            }
            None => out.push(v.clone()),
        }
    }
    out.sort();
    out
}

/// The delta between the last recorded round and this one.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct SinceLast {
    /// The commit the recorded round stood on.
    pub head: String,
    pub findings: Moved<Finding>,
    /// Clocks newly due, and clocks discharged.
    pub due: Moved<String>,
    pub stale_steps: Moved<String>,
    /// Refs that started, and refs no longer in flight — settled, merged or deleted.
    pub in_flight: Moved<String>,
    /// Proposal heads opened, and heads gone — merged or deleted.
    pub proposals: Moved<String>,
}

impl SinceLast {
    pub fn between(last: &Round, now: &Round) -> Self {
        Self {
            head: last.head.clone(),
            findings: Moved::between(&last.findings, &now.findings),
            due: Moved::between(&last.due, &now.due),
            stale_steps: Moved::between(&last.stale_steps, &now.stale_steps),
            in_flight: Moved::between(&last.in_flight, &now.in_flight),
            proposals: Moved::between(&last.proposals, &now.proposals),
        }
    }

    pub fn short_head(&self) -> &str {
        self.head.get(..8).unwrap_or(&self.head)
    }

    /// One line per item: `+` arrived, `-` left, then what it is. Empty is *nothing moved*,
    /// which is only ever said about a delta that was computed — a branch with no record has no
    /// delta, and is never reported as this.
    pub fn lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        let mut half = |m: &Moved<String>, what: &str| {
            for v in &m.arrived {
                out.push(format!("+ {what} {v}"));
            }
            for v in &m.left {
                out.push(format!("- {what} {v}"));
            }
        };
        let findings = Moved {
            arrived: self
                .findings
                .arrived
                .iter()
                .map(|f| format!("{} {} {}", f.gate, f.check, f.node))
                .collect(),
            left: self
                .findings
                .left
                .iter()
                .map(|f| format!("{} {} {}", f.gate, f.check, f.node))
                .collect(),
        };
        half(&findings, "finding");
        half(&self.due, "due");
        half(&self.stale_steps, "stale step");
        half(&self.in_flight, "in flight");
        half(&self.proposals, "proposal");
        out
    }
}

/// The sentence `--since-last` reports when this branch has no record.
pub fn no_record(branch: Option<&str>) -> String {
    format!(
        "no round is recorded for {} — `yidam cycle --record` writes one, and until then there \
         is nothing to compare against (which is not the same as nothing having moved)",
        branch.map_or_else(
            || "a detached HEAD".to_string(),
            |b| format!("branch `{b}`")
        )
    )
}

#[cfg(test)]
mod tests;
