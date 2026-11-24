//! The eighth clock: how many releases has the template's origin cut past this pin? (#1511)
//!
//! A derived repository could not tell how far behind its template was. `.yidam.toml` named
//! a commit and a date, and a date cannot tell a quiet upstream from a stale pin: a pin a
//! month old is current if nothing was released since, and stale if three releases were.
//! RFC-0065 counts releases instead, by version, against `release` — the field that names
//! what the pin installs.
//!
//! # The one clock that reads the network
//!
//! Every other clock computes from what the repository records. This one cannot: the releases
//! it counts are the origin's, cut after the pin. So the breach is narrow. One `ls-remote`,
//! no clone and no fetch; no prompt for credentials, so CI without them fails fast; and a
//! failure is a row that says so, never an error. `--strict` still fails only on `Due`, so an
//! unreachable origin never turns a strict run red.
//!
//! # Who acts
//!
//! A person. Adopting a release can change the closed commit vocabulary, a source pack's range
//! or a check's severity, and whether to take it now is a judgement. The prose names
//! `mise run yidam-vendor-update`, and the act stays a person's: every tool act this binary
//! emits starts `yidam`.

use std::path::Path;

use super::{Actor, Clock, State};
use crate::provenance::{base_release, release_version, MANIFEST};

const Q: &str = "How many releases has the origin cut past this pin?";

const UNSET: &str = "`[due] template_after` in .yidam/config.toml";

const REMEDY: &str =
    "read the release notes since the pin, then re-vendor with `mise run yidam-vendor-update`";

const NOT_A_RELEASE: &str = "pin is not a release; re-vendor to record release";

/// What `.yidam.toml` says about the pin, as far as this clock reads it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Pin {
    pub origin: String,
    pub commit: String,
    /// `None` on a pin written before RFC-0065, which had no such field.
    pub release: Option<String>,
}

/// The pin, or `None` when this repository has no `.yidam.toml` and so no pin to age.
///
/// A file that exists and does not parse is still a pin, of nothing readable: every field
/// reads `unknown`, and the clock says it could not measure rather than vanishing.
pub(crate) fn read_pin(root: &Path) -> Option<Pin> {
    let text = std::fs::read_to_string(root.join(MANIFEST)).ok()?;
    let table = toml::from_str::<toml::Value>(&text)
        .ok()
        .and_then(|v| v.get("yidam").cloned());
    let field = |k: &str| -> Option<String> {
        table
            .as_ref()?
            .get(k)?
            .as_str()
            .map(|s| s.trim().to_string())
    };
    Some(Pin {
        origin: field("origin").unwrap_or_else(|| "unknown".to_string()),
        commit: field("commit").unwrap_or_else(|| "unknown".to_string()),
        release: field("release"),
    })
}

/// One release the origin lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Release {
    pub version: (u64, u64, u64),
    pub tag: String,
    /// The commit the tag names: the peeled one, for an annotated tag.
    pub commit: String,
}

/// `git ls-remote --tags` output, read by `newest_release`'s rules: only `cli/vX.Y.Z`
/// counts, and a peeled `^{}` line wins over the bare one, since it is the commit an
/// annotated tag points at. Sorted by version, oldest first.
pub(crate) fn parse_releases(listing: &str) -> Vec<Release> {
    let mut by_tag: std::collections::BTreeMap<String, (bool, String)> = Default::default();
    for line in listing.lines() {
        let Some((sha, refname)) = line.split_once('\t') else {
            continue;
        };
        let Some(tag) = refname.trim().strip_prefix("refs/tags/") else {
            continue;
        };
        let (tag, peeled) = match tag.strip_suffix("^{}") {
            Some(t) => (t, true),
            None => (tag, false),
        };
        if release_version(tag).is_none() {
            continue;
        }
        let keep = by_tag
            .get(tag)
            .is_none_or(|(was_peeled, _)| peeled || !was_peeled);
        if keep {
            by_tag.insert(tag.to_string(), (peeled, sha.trim().to_string()));
        }
    }
    let mut out: Vec<Release> = by_tag
        .into_iter()
        .filter_map(|(tag, (_, commit))| {
            Some(Release {
                version: release_version(&tag)?,
                tag,
                commit,
            })
        })
        .collect();
    out.sort_by_key(|r| r.version);
    out
}

/// The origin's release tags, by one bounded `ls-remote`; `None` when it cannot be read.
///
/// Bounded four ways. `GIT_TERMINAL_PROMPT=0` stops an https credential prompt; an https
/// transfer slower than `http.lowSpeedLimit` for `http.lowSpeedTime` is abandoned; OpenSSH
/// runs in batch mode with a connect timeout; and whatever the transport, the child is killed
/// at [`DEADLINE`]. So CI without credentials, or behind a proxy that holds connections open,
/// fails in seconds rather than waiting on a terminal nobody is at.
///
/// The user's ssh is kept. Batch flags are added only to a command that is OpenSSH by name —
/// `GIT_SSH_COMMAND`, else `core.sshCommand`, else plain `ssh`. A `GIT_SSH` wrapper, or a
/// command that is some other program (plink, a script), is left exactly as configured,
/// because `GIT_SSH_COMMAND` would override it and `-o` may mean nothing to it; the deadline
/// still bounds it.
///
/// An origin starting `-` is refused before it reaches git, where it would read as an option.
pub(crate) fn list_releases(root: &Path, origin: &str) -> Option<String> {
    if origin.is_empty() || origin == "unknown" || origin.starts_with('-') {
        return None;
    }
    let mut git = crate::git::Git::new(root)
        .config("http.lowSpeedLimit", "1000")
        .config("http.lowSpeedTime", "10")
        .env("GIT_TERMINAL_PROMPT", "0");
    if let Some(ssh) = batch_ssh_command(root) {
        git = git.env("GIT_SSH_COMMAND", ssh);
    }
    let child = git
        .args(["ls-remote", "--tags", origin, "refs/tags/cli/v*"])
        .spawn_piped()
        .ok()?;
    wait_bounded(child, DEADLINE)
}

/// How long the one network read may take, end to end, before the row reads unmeasurable.
const DEADLINE: std::time::Duration = std::time::Duration::from_secs(20);

/// The `GIT_SSH_COMMAND` to run with, or `None` to leave the user's ssh as configured.
fn batch_ssh_command(root: &Path) -> Option<String> {
    let set = |s: Option<String>| s.filter(|s| !s.trim().is_empty());
    let configured = set(std::env::var("GIT_SSH_COMMAND").ok()).or_else(|| {
        if set(std::env::var("GIT_SSH").ok()).is_some() {
            // `GIT_SSH` loses to `GIT_SSH_COMMAND`, so setting one here would bypass it.
            return Some(String::new());
        }
        set(crate::git::Git::new(root)
            .args(["config", "core.sshCommand"])
            .try_run())
    });
    let ssh = match configured {
        None => "ssh".to_string(),
        Some(s) if s.is_empty() => return None,
        Some(s) => s,
    };
    is_openssh(&ssh).then(|| format!("{ssh} -o BatchMode=yes -o ConnectTimeout=10"))
}

/// Whether a command line's program is OpenSSH by name, so `-o` means what it means there.
pub(crate) fn is_openssh(command: &str) -> bool {
    let program = command.split_whitespace().next().unwrap_or_default();
    let name = program.rsplit(['/', '\\']).next().unwrap_or_default();
    matches!(name, "ssh" | "ssh.exe")
}

/// The child's stdout when it exits successfully within `deadline`; killed and `None` past it.
pub(crate) fn wait_bounded(
    mut child: std::process::Child,
    deadline: std::time::Duration,
) -> Option<String> {
    use std::io::Read;
    drop(child.stdin.take());
    let mut stdout = child.stdout.take()?;
    // Read on a thread so a listing larger than the pipe buffer cannot stall the child.
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        stdout.read_to_string(&mut s).map(|_| s)
    });
    let start = std::time::Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if start.elapsed() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    };
    let out = reader.join().ok()?.ok()?;
    status.success().then(|| out.trim().to_string())
}

/// Read the clock. `list` is the network, passed in so every state is testable offline.
///
/// `None` when there is no `.yidam.toml`: a projection or a hand copy has no pin, and an
/// `Undeclared` row would read as a corpus that had not set an interval when it may have.
///
/// A clock declined in `[due.declined]` with no interval beside it is not read at all: a
/// corpus that decided against the clock, an offline one most of all, is owed no network
/// read for it. With an interval the decline is contradicted, and the clock reads as usual.
pub(crate) fn clock(
    root: &Path,
    after: Option<u32>,
    declined: bool,
    list: impl FnOnce(&Path, &str) -> Option<String>,
) -> Option<Clock> {
    let pin = read_pin(root)?;
    if declined && after.is_none() {
        return Some(
            Clock::new(
                Clock::TEMPLATE,
                Q,
                State::Undeclared,
                "origin not read: the clock is declined".to_string(),
            )
            .unset(UNSET),
        );
    }
    Some(measure(&pin, after, || list(root, &pin.origin)))
}

/// Every state of RFC-0065 §3's table, from the pin and the origin's listing.
pub(crate) fn measure(
    pin: &Pin,
    after: Option<u32>,
    list: impl FnOnce() -> Option<String>,
) -> Clock {
    // Each failure to measure follows the interval, as every other clock's does.
    let unmeasured = |detail: String| -> Clock {
        match after {
            None => Clock::new(Clock::TEMPLATE, Q, State::Undeclared, detail).unset(UNSET),
            Some(n) => Clock::new(Clock::TEMPLATE, Q, State::Unmeasurable, detail)
                .at(format!("[due] template_after = {n}")),
        }
    };

    let recorded = pin
        .release
        .as_deref()
        .map(base_release)
        .and_then(|b| release_version(b).map(|v| (b.to_string(), v)));
    let Some(listing) = list() else {
        return unmeasured(format!("could not read releases from {}", pin.origin));
    };
    let releases = parse_releases(&listing);

    // A pin written before `release` existed is measured from its commit, when the commit is
    // a release. One written since that records no release is not one.
    let base = recorded.or_else(|| {
        let r = releases.iter().rev().find(|r| r.commit == pin.commit)?;
        Some((r.tag.clone(), r.version))
    });
    let Some((base, version)) = base else {
        return unmeasured(NOT_A_RELEASE.to_string());
    };

    let past: Vec<&Release> = releases.iter().filter(|r| r.version > version).collect();
    let measured = match past.last() {
        Some(newest) => format!(
            "{} release(s) past {base}; newest {}",
            past.len(),
            newest.tag
        ),
        None => format!("no release past {base}"),
    };

    let Some(n) = after else {
        return Clock::new(Clock::TEMPLATE, Q, State::Undeclared, measured).unset(UNSET);
    };
    let interval = format!("[due] template_after = {n}");
    let behind = past.len();
    // `0` makes any release past the pin due, not every pin: a pin at the newest release is
    // owed nothing at any interval.
    // The threshold a reader sees is the count that comes due, so `0` reads as `1` in both
    // states rather than as two different numbers.
    let threshold = n.max(1);
    if behind > 0 && behind >= n as usize {
        Clock::new(
            Clock::TEMPLATE,
            Q,
            State::Due,
            format!("{measured}; due at {threshold}"),
        )
        .at(interval)
        .owing(behind, REMEDY, Actor::Person)
    } else {
        Clock::new(
            Clock::TEMPLATE,
            Q,
            State::Ok,
            format!("{measured}; due at {threshold}"),
        )
        .at(interval)
    }
}
