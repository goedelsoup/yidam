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
# GENERATED. Rewritten wholesale by `yidam lint --bless`; do not hand-edit.
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
}

/// The committed set of violations the gate accepts, keyed by check id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Baseline {
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

    /// Build a baseline from the error-severity violations of a run.
    pub fn from_checks(checks: &[Check]) -> Self {
        let mut violations: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for check in checks {
            // Per violation, not per check: residence time can escalate one finding of an
            // Info check without escalating its siblings, and the baseline records what
            // gates. Reading `check.severity` here would have quietly recorded none of
            // them — or, for a check whose base severity is Error, all of them.
            let mut entries: Vec<Entry> = check
                .violations
                .iter()
                .filter(|v| check.gates(v))
                .map(|v| Entry {
                    node: v.node.clone(),
                    detail: v.detail.clone(),
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
        Self { violations }
    }
}

/// What a run and the committed baseline disagree about.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Diff {
    /// Error-severity violations present in the run and absent from the baseline.
    pub introduced: Vec<(String, String)>,
    /// Baseline entries with no corresponding violation in the run.
    pub resolved: Vec<(String, String)>,
}

impl Diff {
    pub fn is_clean(&self) -> bool {
        self.introduced.is_empty() && self.resolved.is_empty()
    }
}

/// Compare a run against the baseline as a multiset of `(check id, node)`.
pub fn diff(checks: &[Check], baseline: &Baseline) -> Diff {
    let mut out = Diff::default();

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
            match pool.iter().position(|e| e.node == v.node) {
                Some(i) => {
                    pool.remove(i);
                }
                None => out.introduced.push((check.id.to_string(), v.node.clone())),
            }
        }
    }

    for (id, leftover) in &remaining {
        for entry in leftover {
            out.resolved.push((id.to_string(), entry.node.clone()));
        }
    }

    out.introduced.sort();
    out.resolved.sort();
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
            });
        }
        Baseline { violations }
    }

    #[test]
    fn a_baselined_violation_does_not_gate() {
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]));
        assert!(d.is_clean(), "{d:?}");
    }

    #[test]
    fn a_new_violation_is_introduced() {
        let checks = vec![err_check("dangling-edge", &["a.yml", "b.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]));
        assert_eq!(d.introduced, vec![("dangling-edge".into(), "b.yml".into())]);
        assert!(d.resolved.is_empty());
    }

    #[test]
    fn a_fixed_violation_is_resolved_and_still_fails() {
        let checks = vec![err_check("dangling-edge", &[])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]));
        assert_eq!(d.resolved, vec![("dangling-edge".into(), "a.yml".into())]);
        assert!(!d.is_clean(), "a stale baseline must not pass");
    }

    #[test]
    fn the_same_node_tripping_twice_is_not_collapsed() {
        // Multiset, not set: one baselined occurrence must not excuse two.
        let checks = vec![err_check("dangling-edge", &["a.yml", "a.yml"])];
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]));
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
        let d = diff(&checks, &baseline_of(&[("dangling-edge", "a.yml")]));
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
        assert!(Baseline::from_checks(&checks).violations.is_empty());
        assert!(diff(&checks, &Baseline::default()).is_clean());
    }

    #[test]
    fn blessing_a_run_makes_it_clean() {
        let checks = vec![err_check("dangling-edge", &["b.yml", "a.yml"])];
        let blessed = Baseline::from_checks(&checks);
        assert!(diff(&checks, &blessed).is_clean());
        // Stable ordering, so a re-bless produces no diff.
        assert_eq!(blessed.violations["dangling-edge"][0].node, "a.yml");
    }

    #[test]
    fn round_trips_through_yaml() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        let checks = vec![err_check("dangling-edge", &["a.yml"])];
        Baseline::from_checks(&checks).write(root).unwrap();
        let loaded = Baseline::load(root).unwrap();
        assert!(diff(&checks, &loaded).is_clean());
    }

    #[test]
    fn an_absent_baseline_loads_empty() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert_eq!(Baseline::load(tmp.path()).unwrap(), Baseline::default());
    }
}
