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
/// Bounded three ways. `GIT_TERMINAL_PROMPT=0` stops an https credential prompt, and ssh runs
/// in batch mode with a connect timeout, so CI without credentials fails in seconds rather
/// than waiting on a terminal nobody is at. A configured `core.sshCommand` or a
/// `GIT_SSH_COMMAND` is kept, with the batch flags added, so an agent-backed ssh still works.
///
/// An origin starting `-` is refused before it reaches git, where it would read as an option.
pub(crate) fn list_releases(root: &Path, origin: &str) -> Option<String> {
    if origin.is_empty() || origin == "unknown" || origin.starts_with('-') {
        return None;
    }
    const BATCH: &str = "-o BatchMode=yes -o ConnectTimeout=10";
    let ssh = std::env::var("GIT_SSH_COMMAND")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| {
            crate::git::Git::new(root)
                .args(["config", "core.sshCommand"])
                .try_run()
        })
        .unwrap_or_else(|| "ssh".to_string());
    crate::git::Git::new(root)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_SSH_COMMAND", format!("{ssh} {BATCH}"))
        .args(["ls-remote", "--tags", origin, "refs/tags/cli/v*"])
        .run()
        .ok()
}

/// Read the clock. `list` is the network, passed in so every state is testable offline.
///
/// `None` when there is no `.yidam.toml`: a projection or a hand copy has no pin, and an
/// `Undeclared` row would read as a corpus that had not set an interval when it may have.
pub(crate) fn clock(
    root: &Path,
    after: Option<u32>,
    list: impl FnOnce(&Path, &str) -> Option<String>,
) -> Option<Clock> {
    let pin = read_pin(root)?;
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
    if behind > 0 && behind >= n as usize {
        Clock::new(
            Clock::TEMPLATE,
            Q,
            State::Due,
            format!("{measured}; due at {n}"),
        )
        .at(interval)
        .owing(behind, REMEDY, Actor::Person)
    } else {
        Clock::new(
            Clock::TEMPLATE,
            Q,
            State::Ok,
            format!("{measured}; due at {}", n.max(1)),
        )
        .at(interval)
    }
}
