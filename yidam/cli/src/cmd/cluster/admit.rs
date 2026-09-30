//! `cluster admit` — whether a run is owed, decided before a workflow is submitted.
//!
//! #460 decision 8: `due` is the admission gate, plus a cap on open proposal branches per
//! corpus. Two things are read and one number is counted:
//!
//! - **`due`'s clocks**, exactly as `yidam due` reads them. A clock in the `due` state is
//!   owed; the other four states are not, and `Undeclared` in particular is not a quieter
//!   `Due` — a corpus that has set no interval has asked for nothing.
//! - **The manifest's staleness**, as `yidam run --dry-run` reads it. `due` has no clock for
//!   "a step's committed receipt no longer matches the tip", because `due` predates receipts;
//!   a run is owed when one does not match, and this is where that is asked. It is asked
//!   here rather than added to `due` because it is the one clock with no interval to
//!   declare — a stale step is stale, not stale after N days — and `due`'s table is a table
//!   of declared intervals.
//! - **Open proposals**, counted on the remote. A run that would propose while a person
//!   has not merged or rejected the last proposal is a run that widens the queue; the cap is
//!   `[cluster] max_open_proposals`, declared, never compiled in, and reported when it is
//!   not set so that a reader of the record can see the cap is absent rather than infinite.
//!
//! The verdict is in the record, and the exit code is zero either way: a corpus with
//! nothing owed is not a failure, it is the ordinary state of a corpus between changes.

use std::fmt::Write as _;
use std::path::Path;

use anyhow::Result;

use super::{clone_branch, deliver, Admission, RemoteArgs, CONTRACT_VERSION};
use crate::cmd::due::State;
use crate::cmd::run::exec::Scratch;
use crate::cmd::run::manifest::MANIFEST;
use crate::cmd::run::{plan_and_write, Freshness};
use crate::git::Git;
use crate::report::Format;

pub(super) fn run(remote: &RemoteArgs, out: Option<&Path>, format: Format) -> Result<()> {
    let scratch = Scratch::new("admit")?;
    let root = scratch.path().join("corpus");
    clone_branch(&remote.remote, &remote.branch, &root)?;
    let record = admit_in(&root)?;
    deliver(&root, format, out, record, render)
}

/// Decide admission for the clone at `root`, whose `origin` is the remote.
pub(super) fn admit_in(root: &Path) -> Result<Admission> {
    let cfg = crate::config::load_yidam_config(root)?;
    let today = crate::dates::today_days();

    let owed: Vec<String> = crate::cmd::due::read_clocks(root, &cfg.due, today)
        .into_iter()
        .filter(|c| c.state == State::Due)
        .map(|c| c.id.to_string())
        .collect();

    // The proposal branches, so the dry run reads a proposed result as `run --dry-run` does
    // on a checkout that has them: an epistemic step whose result is on `propose/<tip>` is
    // fresh, not owed again. `clone --single-branch` brought the branch alone.
    Git::new(root)
        .args([
            "fetch",
            "-q",
            "origin",
            "+refs/heads/propose/*:refs/heads/propose/*",
        ])
        .run()?;
    let stale: Vec<String> = if root.join(MANIFEST).exists() {
        plan_and_write(root, None, true)?
            .steps
            .into_iter()
            .filter(|s| s.freshness != Freshness::Fresh)
            .map(|s| s.step)
            .collect()
    } else {
        Vec::new()
    };

    let open_proposals = Git::new(root)
        .args(["ls-remote", "--heads", "origin", "refs/heads/propose/*"])
        .run()?
        .lines()
        .filter(|l| !l.trim().is_empty())
        .count();
    let max_open_proposals = cfg.cluster.max_open_proposals;

    let something_owed = !owed.is_empty() || !stale.is_empty();
    let (admitted, because) = match (something_owed, max_open_proposals) {
        (false, _) => (
            false,
            "nothing is owed: no clock is due and every step's committed receipt matches the tip"
                .to_string(),
        ),
        (true, Some(cap)) if open_proposals >= cap => (
            false,
            format!(
                "{open_proposals} proposal branch{} open and [cluster] max_open_proposals is \
                 {cap}; merge or delete one before another run may propose",
                if open_proposals == 1 { " is" } else { "es are" }
            ),
        ),
        (true, _) => (
            true,
            format!(
                "{} clock{} due, {} step{} stale",
                owed.len(),
                if owed.len() == 1 { "" } else { "s" },
                stale.len(),
                if stale.len() == 1 { "" } else { "s" }
            ),
        ),
    };

    Ok(Admission {
        format_version: CONTRACT_VERSION,
        admitted,
        because,
        owed,
        stale,
        open_proposals,
        max_open_proposals,
    })
}

pub(super) fn render(a: &Admission) -> String {
    let mut s = format!(
        "{}: {}\n",
        if a.admitted {
            "admitted"
        } else {
            "not admitted"
        },
        a.because
    );
    if !a.owed.is_empty() {
        let _ = writeln!(s, "  due: {}", a.owed.join(", "));
    }
    if !a.stale.is_empty() {
        let _ = writeln!(s, "  stale: {}", a.stale.join(", "));
    }
    let _ = writeln!(
        s,
        "  open proposals: {} (cap: {})",
        a.open_proposals,
        a.max_open_proposals
            .map_or("not declared".to_string(), |c| c.to_string())
    );
    s
}
