//! The lint ratchet — which error-severity violations are already known.
//!
//! Checks report; this module decides what is acceptable. It exists because the two
//! obvious alternatives both fail, and fail quietly.
//!
//! A gate that fails on every error wedges a repository shut the moment it inherits debt
//! it cannot pay down today — so people reach for `--warn-only`, which switches off every
//! check at once and stays switched off. A gate run under `continue-on-error` reports
//! success whatever the exit code, so it goes green on every commit and the count it
//! prints is read by nobody.
//!
//! A baseline separates the two questions that single exit code was conflating. *Is the
//! corpus clean?* — no, and it says so, in a file anyone can read. *Did this commit make
//! it less clean?* — that can be gated, because inherited debt is enumerable and new debt
//! is not inherited.
//!
//! # What identifies a violation
//!
//! The pair `(check id, node)`, compared as a multiset so a node tripping the same check
//! twice is not silently reduced to once. [`Entry::detail`] is carried so the file reads
//! as a record rather than a list of identifiers, but it is deliberately **not** compared:
//! detail is prose, and rewording a sentence is not a corpus change.
//!
//! # Why a stale baseline fails
//!
//! A baselined violation that no longer occurs fails the gate exactly as an introduced one
//! does. Blocking on good news looks perverse for about as long as it takes to notice that
//! a baseline permitted to be wrong drifts, and a baseline that over-lists silently permits
//! re-introduction of whatever it over-lists. The fix is one command, and the failure
//! message names it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::model::Check;

const HEADER: &str = "\
# Known error-severity lint violations — this corpus's inherited debt.
#
# GENERATED. Rewritten by `yidam lint --bless`; do not hand-edit, except for
# `expire_after`, which is this repository's own argument and is preserved.
#
# `yidam lint` fails on any error-severity violation NOT listed here, and also when
# something listed here no longer occurs — an exact baseline is the only kind that keeps
# its meaning. Neither failure means the corpus got worse on its own; both mean this file
# and the corpus disagree.
#
# Listing a violation here is not absolution. It records that the corpus was already in
# this state when the ratchet was installed, so that the next one is attributable to the
# commit that introduced it.
#
# `since` is the corpus commit at which an entry was first recorded, and it is CARRIED
# FORWARD by later blessings rather than restamped — otherwise re-blessing would forgive
# the debt again and the clock below would never run out.
#
# `expire_after` is how many corpus-touching commits an entry may survive before it stops
# forgiving and the violation gates again. Absent means never, which is where a repository
# starts. Raising it is the supported way to argue for more time — and the argument is then
# in the repository, in a diff somebody reviews, rather than in a build nobody can see.
#
# `detail` is informational. Violations are identified by check id and node, so rewording
# a message does not make the baseline stale.
";

/// One accepted violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub node: String,
    /// The message the check emitted when this baseline was written. Carried so the file
    /// explains itself; never compared.
    #[serde(default)]
    pub detail: String,
    /// The corpus commit at which this entry was **first** recorded.
    ///
    /// Carried forward by every later blessing rather than restamped, which is the whole
    /// of what makes the clock mean anything: if `--bless` reset it, the expiry below
    /// would be cleared by the same command that is supposed to be constrained by it, and
    /// the baseline would be a permanent exemption wearing a deadline.
    ///
    /// Absent on an entry written before this field existed, and on a hand-written one.
    /// Such an entry never expires — it has no clock, and inventing one by stamping it
    /// with today would restart every clock in the file the first time anyone blessed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
}

/// The committed set of violations the gate accepts, keyed by check id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
    /// Corpus-touching commits an entry may survive before it stops forgiving.
    ///
    /// **Declared here rather than in the binary**, and for the same reason the residence
    /// threshold is declared in `.yidam/config.toml`: how long is too long is a judgement
    /// about this corpus, and a number compiled in would be one repository's answer
    /// arriving as a build failure in every other. It sits in *this* file rather than in
    /// the config because what it governs is this file — how long its entries stand.
    ///
    /// `None` — the default, and the state of every baseline until somebody argues about
    /// a number — means entries never expire. That is the ratchet as it has always
    /// behaved, and adopting the clock is opt-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expire_after: Option<usize>,
    #[serde(default)]
    pub violations: BTreeMap<String, Vec<Entry>>,
}

pub fn path(root: &Path) -> PathBuf {
    root.join(".yidam").join("lint-baseline.yml")
}

impl Baseline {
    /// Read the baseline, or an empty one if the file is absent.
    ///
    /// Absent is not an error: a repository that has never blessed anything has no
    /// inherited debt, and every error-severity violation is therefore new.
    pub fn load(root: &Path) -> Result<Self> {
        let p = path(root);
        if !p.exists() {
            return Ok(Self::default());
        }
        let text =
            std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
        serde_yaml::from_str(&text).with_context(|| format!("parsing {}", p.display()))
    }

    pub fn write(&self, root: &Path) -> Result<()> {
        let p = path(root);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = serde_yaml::to_string(self).context("serializing baseline")?;
        std::fs::write(&p, format!("{HEADER}{body}"))
            .with_context(|| format!("writing {}", p.display()))?;
        Ok(())
    }

    /// Build a baseline from the violations of a run that gate.
    ///
    /// `previous` is the baseline being replaced and `head` the corpus commit this run
    /// stands at. An entry that already existed keeps the `since` it already had; only a
    /// genuinely new one is stamped with `head`. That asymmetry is the mechanism: a
    /// blessing records new debt without forgiving old debt a second time, so the clock on
    /// an entry runs from the first time anyone accepted it and not from the last.
    ///
    /// `head` empty — a repository with no commits, or a git call that failed — stamps
    /// nothing. An entry with no clock never expires, which fails toward the ratchet's
    /// existing behaviour rather than toward a deadline nobody set.
    pub fn from_checks(checks: &[Check], previous: &Baseline, head: &str) -> Self {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for check in checks {
            // Per violation, not per check: residence time can escalate one finding of an
            // Info check without escalating its siblings, and the baseline records what
            // gates. Reading `check.severity` here would have quietly recorded none of
            // them — or, for a check whose base severity is Error, all of them.
            //
            // `inherited` is drained as it matches, so a node tripping one check twice
            // takes two distinct stamps rather than both taking the first one's.
            let mut inherited: Vec<&Entry> = previous
                .violations
                .get(check.id)
                .map(|entries| entries.iter().collect())
                .unwrap_or_default();
            let mut entries: Vec<Entry> = check
                .violations
                .iter()
                .filter(|v| check.gates(v))
                .map(|v| {
                    let carried = inherited
                        .iter()
                        .position(|e| e.node == v.node)
                        .map(|i| inherited.remove(i))
                        .and_then(|e| e.since.clone());
                    Entry {
                        node: v.node.clone(),
                        detail: v.detail.clone(),
                        since: carried.or_else(|| (!head.is_empty()).then(|| head.to_string())),
                    }
                })
                .collect();
            if entries.is_empty() {
                continue;
            }
            // Sorted so the file is stable across runs — a baseline that reorders itself
            // produces a diff on every bless and teaches people to ignore the diff.
            entries.sort_by(|a, b| a.node.cmp(&b.node));
            violations.insert(check.id.to_string(), entries);
        }
        Self {
            // The corpus's own argument about how long is long enough. Preserved across
            // blessings: it is the one thing in this generated file a human wrote.
            expire_after: previous.expire_after,
            violations,
        }
    }
}

/// One baseline entry that has outlived the expiry the corpus declared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expired {
    pub check: String,
    pub node: String,
    /// Corpus-touching commits it has stood for.
    pub commits: usize,
}

/// What a run and the committed baseline disagree about.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Error-severity violations present in the run and absent from the baseline.
    pub introduced: Vec<(String, String)>,
    /// Baseline entries with no corresponding violation in the run.
    pub resolved: Vec<(String, String)>,
    /// Entries that have outlived the declared expiry and no longer forgive.
    ///
    /// Reported separately from [`Self::introduced`] rather than folded into it, though
    /// both fail the gate. They are different events and the fix for each is different: an
    /// introduced violation is something this change did, and an expired one is something
    /// the repository agreed to deal with and has not. Telling somebody they introduced a
    /// finding that has been sitting in their baseline for two hundred commits would be a
    /// lie, and a confusing one.
    pub expired: Vec<Expired>,
}

impl Diff {
    pub fn is_clean(&self) -> bool {
        self.introduced.is_empty() && self.resolved.is_empty() && self.expired.is_empty()
    }
}

/// Compare a run against the baseline as a multiset of `(check id, node)`.
///
/// `corpus_commits` is every commit that touched the corpus, oldest first — the clock an
/// entry's `since` is read against. Empty disables expiry entirely, which is what happens
/// outside a git repository and in a corpus with no history: an entry whose age cannot be
/// established has not been shown to be old.
pub fn diff(checks: &[Check], baseline: &Baseline, corpus_commits: &[String]) -> Diff {
    let mut out = Diff::default();

    // Which entries have run out of time. Computed up front so that the matching below can
    // simply decline to consume them: an expired entry is still *listed*, so it must not
    // also be reported as stale for the violation it does still describe.
    let expired: std::collections::HashSet<(&str, &str)> = match baseline.expire_after {
        None => Default::default(),
        Some(limit) => baseline
            .violations
            .iter()
            .flat_map(|(id, entries)| entries.iter().map(move |e| (id.as_str(), e)))
            .filter_map(|(id, e)| {
                let age = super::history::commits_since(corpus_commits, e.since.as_deref()?)?;
                (age >= limit).then_some((id, e.node.as_str()))
            })
            .collect(),
    };

    // Multiset accounting: for each check, walk the run's violations and consume a
    // matching baseline entry per occurrence. Whatever is left on either side is a
    // disagreement. A plain set would hide a node that starts tripping the same check
    // twice, which is precisely the regression the ratchet is for.
    let mut remaining: BTreeMap<&str, Vec<&Entry>> = baseline
        .violations
        .iter()
        .map(|(id, entries)| (id.as_str(), entries.iter().collect()))
        .collect();

    for check in checks {
        let pool = remaining.entry(check.id).or_default();
        for v in check.violations.iter().filter(|v| check.gates(v)) {
            let Some(i) = pool.iter().position(|e| e.node == v.node) else {
                out.introduced.push((check.id.to_string(), v.node.clone()));
                continue;
            };
            // Consumed either way: the entry describes this violation, and leaving it in
            // the pool would report it as stale on top of expired.
            let entry = pool.remove(i);
            if !expired.contains(&(check.id, v.node.as_str())) {
                continue;
            }
            let commits = entry
                .since
                .as_deref()
                .and_then(|sha| super::history::commits_since(corpus_commits, sha))
                .unwrap_or_default();
            out.expired.push(Expired {
                check: check.id.to_string(),
                node: v.node.clone(),
                commits,
            });
        }
    }

    for (id, leftover) in &remaining {
        for entry in leftover {
            out.resolved.push((id.to_string(), entry.node.clone()));
        }
    }

    out.introduced.sort();
    out.resolved.sort();
    out.expired
        .sort_by(|a, b| (&a.check, &a.node).cmp(&(&b.check, &b.node)));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::model::Severity;
    use crate::cmd::lint::model::Violation;

    fn err_check(id: &'static str, nodes: &[&str]) -> Check {
        Check::new(
            id,
            "t",
            Severity::Error,
            "r",
            nodes.iter().map(|n| Violation::new(*n, "d")).collect(),
        )
    }

    fn baseline_of(pairs: &[(&str, &str)]) -> Baseline {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for (id, node) in pairs {
            violations.entry(id.to_string()).or_default().push(Entry {
                node: node.to_string(),
                detail: String::new(),
                since: None,
            });
        }
        Baseline {
            expire_after: None,
            violations,
        }
    }

    #[test]
    fn a_baselined_violation_does_not_gate() {
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]), &[]);
        assert!(d.is_clean(), "{d:?}");
    }

    #[test]
    fn a_new_violation_is_introduced() {
        let checks = vec![err_check("dangling-edge", &["a.yml", "b.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]), &[]);
        assert_eq!(d.introduced, vec![("dangling-edge".into(), "b.yml".into())]);
        assert!(d.resolved.is_empty());
    }

    #[test]
    fn a_fixed_violation_is_resolved_and_still_fails() {
        let checks = vec![err_check("dangling-edge", &[])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]), &[]);
        assert_eq!(d.resolved, vec![("dangling-edge".into(), "a.yml".into())]);
        assert!(!d.is_clean(), "a stale baseline must not pass");
    }

    #[test]
    fn the_same_node_tripping_twice_is_not_collapsed() {
        // Multiset, not set: one baselined occurrence must not excuse two.
        let checks = vec![err_check("dangling-edge", &["a.yml", "a.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]), &[]);
        assert_eq!(
            d.introduced,
            vec![("dangling-edge".into(), "a.yml".into())],
            "the second occurrence is new"
        );
    }

    #[test]
    fn detail_is_not_part_of_identity() {
        let checks = vec![Check::new(
            "dangling-edge",
            "t",
            Severity::Error,
            "r",
            vec![Violation::new("a.yml", "completely rewritten wording")],
        )];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]), &[]);
        assert!(d.is_clean(), "rewording is not a corpus change");
    }

    #[test]
    fn warn_severity_never_reaches_the_baseline() {
        let checks = vec![Check::new(
            "orphan-in",
            "t",
            Severity::Warn,
            "r",
            vec![Violation::new("a.yml", "d")],
        )];
        assert!(Baseline::from_checks(&checks, &Baseline::default(), "")
            .violations
            .is_empty());
        assert!(diff(&checks, &Baseline::default(), &[]).is_clean());
    }

    #[test]
    fn blessing_a_run_makes_it_clean() {
        let checks = vec![err_check("dangling-edge", &["b.yml", "a.yml"])];
        let blessed = Baseline::from_checks(&checks, &Baseline::default(), "");
        assert!(diff(&checks, &blessed, &[]).is_clean());
        // Stable ordering, so a re-bless produces no diff.
        assert_eq!(blessed.violations["dangling-edge"][0].node, "a.yml");
    }

    #[test]
    fn round_trips_through_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        Baseline::from_checks(&checks, &Baseline::default(), "")
            .write(root)
            .unwrap();
        let loaded = Baseline::load(root).unwrap();
        assert!(diff(&checks, &loaded, &[]).is_clean());
    }

    #[test]
    fn an_absent_baseline_loads_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(Baseline::load(tmp.path()).unwrap(), Baseline::default());
    }

    // ── the clock on an entry ─────────────────────────────────────────────────

    /// Six corpus commits, oldest first, as `history::corpus_commits` returns them.
    fn commits() -> Vec<String> {
        (1..=6).map(|i| format!("c{i}")).collect()
    }

    fn aged_baseline(expire_after: Option<usize>, since: &str) -> Baseline {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        violations.insert(
            "dangling-edge".into(),
            vec![Entry {
                node: "a.yml".into(),
                detail: String::new(),
                since: Some(since.into()),
            }],
        );
        Baseline {
            expire_after,
            violations,
        }
    }

    /// The default. A baseline with no declared expiry is the ratchet as it has always
    /// behaved, however old its entries are.
    #[test]
    fn an_entry_never_expires_when_the_corpus_declared_no_number() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &aged_baseline(None, "c1"), &commits());
        assert!(d.is_clean(), "{d:?}");
    }

    #[test]
    fn an_entry_that_outlived_the_declared_expiry_stops_forgiving() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        // Recorded at the first of six corpus commits: it has stood for six.
        let d = diff(&checks, &aged_baseline(Some(6), "c1"), &commits());
        assert_eq!(d.expired.len(), 1, "{d:?}");
        assert_eq!(d.expired[0].commits, 6);
        assert!(!d.is_clean());
        // …and is not *also* reported as introduced or stale. It is one event.
        assert!(d.introduced.is_empty(), "{d:?}");
        assert!(d.resolved.is_empty(), "{d:?}");
    }

    #[test]
    fn an_entry_inside_the_declared_expiry_still_forgives() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &aged_baseline(Some(7), "c1"), &commits());
        assert!(d.is_clean(), "six commits is inside seven: {d:?}");
    }

    /// A sha the corpus history does not contain — a baseline written before a rebase, or
    /// hand-edited. Unreadable is not expired: failing a build over a rewritten history
    /// would be failing over something the corpus never did.
    #[test]
    fn an_entry_whose_commit_is_gone_does_not_expire() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &aged_baseline(Some(1), "rebased-away"), &commits());
        assert!(d.is_clean(), "{d:?}");
    }

    /// An entry written before `since` existed. Stamping it with today would restart every
    /// clock in the file the first time anyone blessed.
    #[test]
    fn an_entry_with_no_clock_does_not_expire() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let mut b = baseline_of(&[("dangling-edge", "a.yml")]);
        b.expire_after = Some(1);
        assert!(diff(&checks, &b, &commits()).is_clean());
    }

    // ── what a blessing preserves ─────────────────────────────────────────────

    /// The mechanism. If `--bless` restamped `since`, the expiry would be cleared by the
    /// same command it is meant to constrain, and the baseline would be a permanent
    /// exemption wearing a deadline.
    #[test]
    fn blessing_again_carries_the_original_clock_forward() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&checks, &Baseline::default(), "c1");
        assert_eq!(
            first.violations["dangling-edge"][0].since.as_deref(),
            Some("c1")
        );

        let second = Baseline::from_checks(&checks, &first, "c6");
        assert_eq!(
            second.violations["dangling-edge"][0].since.as_deref(),
            Some("c1"),
            "re-blessing must not forgive the debt a second time"
        );
    }

    /// New debt is stamped with today even when it lands beside old debt. Otherwise a
    /// corpus could never accrue an entry with an honest start date.
    #[test]
    fn a_newly_blessed_entry_is_stamped_at_head() {
        let old = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&old, &Baseline::default(), "c1");

        let both = [err_check("dangling-edge", &["a.yml", "b.yml"])];
        let second = Baseline::from_checks(&both, &first, "c6");
        let entries = &second.violations["dangling-edge"];
        assert_eq!(entries[0].since.as_deref(), Some("c1"), "a.yml is old debt");
        assert_eq!(entries[1].since.as_deref(), Some("c6"), "b.yml is new");
    }

    /// A node tripping one check twice takes two distinct stamps rather than both taking
    /// the first one's — the same multiset care the diff takes.
    #[test]
    fn a_node_tripping_one_check_twice_keeps_one_clock_per_occurrence() {
        let once = [err_check("dangling-edge", &["a.yml"])];
        let first = Baseline::from_checks(&once, &Baseline::default(), "c1");
        let twice = [err_check("dangling-edge", &["a.yml", "a.yml"])];
        let second = Baseline::from_checks(&twice, &first, "c6");
        let stamps: Vec<Option<&str>> = second.violations["dangling-edge"]
            .iter()
            .map(|e| e.since.as_deref())
            .collect();
        assert_eq!(stamps, vec![Some("c1"), Some("c6")]);
    }

    /// The one thing in this generated file a human wrote.
    #[test]
    fn blessing_preserves_the_corpus_own_argument_about_time() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let previous = Baseline {
            expire_after: Some(200),
            ..Default::default()
        };
        let blessed = Baseline::from_checks(&checks, &previous, "c1");
        assert_eq!(blessed.expire_after, Some(200));
    }

    /// A repository with no history stamps nothing rather than inventing a clock.
    #[test]
    fn a_repository_with_no_commits_stamps_no_clock() {
        let checks = [err_check("dangling-edge", &["a.yml"])];
        let b = Baseline::from_checks(&checks, &Baseline::default(), "");
        assert_eq!(b.violations["dangling-edge"][0].since, None);
    }

    /// The file must round-trip: `expire_after` is read back, and an absent one stays
    /// absent rather than serializing as null.
    #[test]
    fn the_declared_expiry_survives_a_round_trip() {
        let mut b = aged_baseline(Some(150), "c1");
        let text = serde_yaml::to_string(&b).unwrap();
        assert!(text.contains("expire_after: 150"), "{text}");
        assert_eq!(serde_yaml::from_str::<Baseline>(&text).unwrap(), b);

        b.expire_after = None;
        let text = serde_yaml::to_string(&b).unwrap();
        assert!(
            !text.contains("expire_after"),
            "absent stays absent: {text}"
        );
    }
}
