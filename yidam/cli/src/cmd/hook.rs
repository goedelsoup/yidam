//! `yidam hook`: the loop's two session hooks for Claude Code (#1444).
//!
//! #1443 gave `cycle` a memory, and a memory nobody consults is the state #1438 started from:
//! the 2026-11-04 scan found 25 of 29 derived-repo sessions resuming after a compaction and,
//! in 1,681 calls after one, not one asking where the loop stood. The answer has to arrive
//! without being asked for, so it is a hook.
//!
//! - **`session-start`** prints `cycle --brief` — the delta since the last round included —
//!   and records the round. Claude Code adds a SessionStart hook's stdout to the session's
//!   context, on a fresh start and after a compaction alike.
//! - **`stop`** records the round the turn ended on and, when anything moved since the last
//!   one, says what as a `systemMessage`. It answers *did that turn help* without the agent
//!   asking.
//!
//! # A hook never fails the session
//!
//! Every path exits zero, and every failure prints nothing: not a corpus, not bootstrapped,
//! git refusing, a record unread. A hook that errors is a hook a person removes, and a hook
//! removed is the state this exists to end. So the functions here return `Option`, and
//! `None` is silence.
//!
//! The Stop hook does not continue the conversation — no `decision: "block"`, no exit 2 —
//! and it returns at once when Claude Code reports `stop_hook_active`: what moved is told to
//! the person, and it is the next session's brief that tells the agent.
//!
//! # Cheap when nothing moved
//!
//! Stop fires after every turn, and a `cycle` read cost 0.6 s to 5.6 s on the corpora
//! measured. The round records [`round::fingerprint`]; a stop whose fingerprint matches the
//! last record's has nothing to report and reads nothing else.

use std::fmt::Write as _;
use std::io::{IsTerminal as _, Read as _};
use std::path::Path;

use super::cycle::round::{self, Round, SinceLast};
use super::cycle::{read_cycle, render_brief, BRIEF_BUDGET};

/// Which hook is firing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum HookEvent {
    /// Claude Code's SessionStart: print the brief into the session's context.
    SessionStart,
    /// Claude Code's Stop: record the round and tell the person what moved.
    Stop,
}

/// Lines of the delta a stop shows before counting the rest. A `systemMessage` is read in
/// passing; the full delta is `yidam cycle --since-last`.
const STOP_LINES: usize = 5;

/// `yidam hook <event>`. Always `Ok`: see the module's first rule.
pub fn hook(root: Option<&Path>, event: HookEvent) -> anyhow::Result<()> {
    let input = read_input();
    let Ok(root) = crate::paths::resolve_root(root) else {
        return Ok(());
    };
    let today = crate::dates::today_days();
    let out = match event {
        HookEvent::SessionStart => session_start(&root, today),
        HookEvent::Stop => stop(&root, &input, today),
    };
    if let Some(out) = out {
        println!("{out}");
    }
    Ok(())
}

/// Claude Code's hook input, or `Null`. Never waits on a terminal: run by hand, there is no
/// input coming.
fn read_input() -> serde_json::Value {
    let stdin = std::io::stdin();
    if stdin.is_terminal() {
        return serde_json::Value::Null;
    }
    let mut text = String::new();
    if stdin.lock().read_to_string(&mut text).is_err() {
        return serde_json::Value::Null;
    }
    serde_json::from_str(&text).unwrap_or(serde_json::Value::Null)
}

/// The round as read now, with the fingerprint taken **before** the read: a file changed
/// mid-read then makes the next stop's fingerprint differ, never match a round that missed it.
fn read_round(root: &Path, today: i64) -> Option<(super::cycle::CycleReport, Round)> {
    crate::paths::require_yidam_repo(root).ok()?;
    let fingerprint = round::fingerprint(root, today);
    let report = read_cycle(root, false, today).ok()?;
    let mut now = Round::of(root, &report);
    now.fingerprint = fingerprint;
    Some((report, now))
}

/// The brief, and the round recorded. `None` outside a corpus.
pub(crate) fn session_start(root: &Path, today: i64) -> Option<String> {
    let (mut report, now) = read_round(root, today)?;
    match round::load(root, now.branch.as_deref()) {
        Ok(Some(last)) => report.since_last = Some(SinceLast::between(&last, &now)),
        Ok(None) => report.since_last_unread = Some(round::no_record(now.branch.as_deref())),
        Err(why) => report.since_last_unread = Some(why),
    }
    // A record that cannot be written costs the next stop its comparison, not this brief.
    let _ = round::store(root, &now);
    Some(render_brief(&report, &now, BRIEF_BUDGET))
}

/// The `systemMessage` for what moved this turn, and the round recorded. `None` when nothing
/// moved, when there was no round to compare with, or when Claude Code is already continuing.
pub(crate) fn stop(root: &Path, input: &serde_json::Value, today: i64) -> Option<String> {
    if input
        .get("stop_hook_active")
        .and_then(serde_json::Value::as_bool)
        == Some(true)
    {
        return None;
    }
    crate::paths::require_yidam_repo(root).ok()?;
    let branch = crate::cmd::propose::write::current_branch(root);
    let last = round::load(root, branch.as_deref()).ok().flatten();
    if let Some(last) = &last {
        if last.fingerprint.is_some() && last.fingerprint == round::fingerprint(root, today) {
            return None;
        }
    }
    let (report, now) = read_round(root, today)?;
    let _ = round::store(root, &now);
    let delta = SinceLast::between(last.as_ref()?, &now);
    let lines = delta.lines();
    if lines.is_empty() {
        return None;
    }
    let mut msg = format!("yidam: since the round at {}", delta.short_head());
    for l in lines.iter().take(STOP_LINES) {
        msg.push_str("\n  ");
        msg.push_str(l);
    }
    if lines.len() > STOP_LINES {
        let _ = write!(
            msg,
            "\n  …and {} more — `yidam cycle --since-last`",
            lines.len() - STOP_LINES
        );
    }
    if !report.blocked.is_empty() {
        let _ = write!(
            msg,
            "\n{} finding(s) would fail a gate — `yidam cycle`",
            report.blocked.len()
        );
    }
    Some(serde_json::json!({ "systemMessage": msg }).to_string())
}

#[cfg(test)]
mod tests;
