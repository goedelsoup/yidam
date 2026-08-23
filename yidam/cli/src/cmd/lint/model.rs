//! What a check is, and how severe its findings are.
//!
//! A check reports; it does not decide what is acceptable. That decision belongs to the
//! baseline ([`super::baseline`]), and keeping the two apart is what lets a corpus carry
//! known debt without either silencing the check or wedging the gate shut.

use std::fmt;

/// How much a violation matters.
///
/// Only [`Severity::Error`] gates. The other two exist because a check worth running is
/// not always a check worth failing on: a node with no incoming links may be newly
/// authored rather than orphaned, and a catalog entry nothing cites may be a source
/// registered ahead of the extraction that will use it. Reporting those as errors trains
/// people to pass `--warn-only`, which turns every check off at once.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Gates. The graph is wrong in a way that breaks traversal or contradicts itself.
    Error,
    /// Reported. Probably wrong, but legitimately not always.
    Warn,
    /// Reported. Worth seeing; frequently fine.
    Info,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warn => "warn",
            Severity::Info => "info",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One node failing one check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    /// Repo-relative path of the offending node. Half of the identity the baseline
    /// compares on — so it must be stable across runs and across machines.
    pub node: String,
    /// Prose describing this particular failure. Carried into the baseline so the file
    /// reads as a record, but never compared: rewording a message is not a corpus change.
    pub detail: String,
    /// How long this condition has held, for a check whose subject is corpus *state*.
    ///
    /// `None` for every other kind of finding, and the distinction is the one
    /// `prelude/GRAPH.md` draws about the commit checks: history cannot be rewritten to
    /// fix a verb, so a finding about an immutable event has no clock and gating on it
    /// could only ever be noise. An orphaned node can be linked or deleted today, so it
    /// has one.
    pub age: Option<super::history::Age>,
}

impl Violation {
    pub fn new(node: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            detail: detail.into(),
            age: None,
        }
    }
}

/// One invariant, its findings, and why anyone should care.
#[derive(Debug, Clone)]
pub struct Check {
    /// Stable identifier. The other half of the baseline's identity — renaming one
    /// invalidates every baseline entry that mentions it, so don't.
    pub id: &'static str,
    pub title: &'static str,
    pub severity: Severity,
    /// Why this check exists. Printed with `--explain`, because a check whose rationale
    /// lives only in a commit message gets deleted by the next person to trip it.
    pub rationale: &'static str,
    /// Commits a dated finding may hold before it escalates to [`Severity::Error`].
    ///
    /// **Declared by the corpus, never hard-coded**, and `None` — no escalation ever — is
    /// the default. A number compiled into the binary would be one repository's judgement
    /// applied to every other, and a corpus that has just adopted this cannot argue with
    /// it. Set through `.yidam/config.toml`; see [`crate::config::LintConfig`].
    pub escalate_after: Option<usize>,
    pub violations: Vec<Violation>,
}

impl Check {
    pub fn new(
        id: &'static str,
        title: &'static str,
        severity: Severity,
        rationale: &'static str,
        violations: Vec<Violation>,
    ) -> Self {
        Self {
            id,
            title,
            severity,
            rationale,
            escalate_after: None,
            violations,
        }
    }

    pub fn passed(&self) -> bool {
        self.violations.is_empty()
    }

    /// The severity of one finding: the check's, unless residence time has escalated it.
    ///
    /// This is the function every consumer of severity must go through, and the reason the
    /// field is not enough on its own. `orphan-in` is Info because a node authored this
    /// morning legitimately has no inbound edges yet — a statement about a *young* finding
    /// that says nothing about one which has survived two hundred commits. Level alone
    /// cannot tell those apart; age can, and it is the discriminator that makes a
    /// corpus-state check gate-eligible at all.
    pub fn severity_of(&self, v: &Violation) -> Severity {
        match (self.escalate_after, &v.age) {
            (Some(n), Some(age)) if age.commits >= n => Severity::Error,
            _ => self.severity,
        }
    }

    /// The highest severity any of this check's findings carries.
    ///
    /// What the text report heads the block with. A check whose base severity is Info and
    /// one of whose findings has escalated must not print `INFO` above a finding that
    /// fails the build.
    pub fn effective_severity(&self) -> Severity {
        self.violations
            .iter()
            .map(|v| self.severity_of(v))
            .min()
            .unwrap_or(self.severity)
    }

    /// Declare the escalation threshold this corpus asked for.
    ///
    /// Builder-shaped so the check functions stay pure — they take nodes and return
    /// findings, and know nothing about a config file or a repository root.
    pub fn escalating_after(mut self, commits: Option<usize>) -> Self {
        self.escalate_after = commits;
        self
    }

    /// Whether this finding gates. The single question the baseline and the diff ask.
    pub fn gates(&self, v: &Violation) -> bool {
        self.severity_of(v) == Severity::Error
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_orders_error_first() {
        let mut s = vec![Severity::Info, Severity::Error, Severity::Warn];
        s.sort();
        assert_eq!(s, vec![Severity::Error, Severity::Warn, Severity::Info]);
    }

    #[test]
    fn a_check_with_no_violations_passes() {
        let c = Check::new("x", "X", Severity::Error, "because", vec![]);
        assert!(c.passed());
    }

    fn aged(node: &str, commits: usize) -> Violation {
        Violation {
            node: node.into(),
            detail: "d".into(),
            age: Some(crate::cmd::lint::history::Age {
                sha: "abc".into(),
                ts: 1,
                commits,
            }),
        }
    }

    /// The default is that nothing escalates. A threshold compiled into the binary would
    /// be one corpus's judgement arriving as a build failure in every other.
    #[test]
    fn a_corpus_that_declares_no_threshold_escalates_nothing() {
        let c = Check::new(
            "orphan-in",
            "T",
            Severity::Info,
            "r",
            vec![aged("n", 10_000)],
        );
        assert_eq!(c.severity_of(&c.violations[0]), Severity::Info);
        assert!(!c.gates(&c.violations[0]));
    }

    #[test]
    fn a_finding_at_the_threshold_escalates_and_a_younger_sibling_does_not() {
        let c = Check::new(
            "orphan-in",
            "T",
            Severity::Info,
            "r",
            vec![aged("old", 100), aged("older", 101), aged("young", 99)],
        )
        .escalating_after(Some(100));

        assert_eq!(c.severity_of(&c.violations[0]), Severity::Error, "at N");
        assert_eq!(c.severity_of(&c.violations[1]), Severity::Error, "past N");
        assert_eq!(c.severity_of(&c.violations[2]), Severity::Info, "under N");
        // The block heads at the worst it contains, so INFO never sits above a failure.
        assert_eq!(c.effective_severity(), Severity::Error);
    }

    /// A finding with no clock cannot age. This is the commit checks' case: history cannot
    /// be rewritten to fix a verb, so there is nothing for a threshold to act on.
    #[test]
    fn a_finding_with_no_age_never_escalates() {
        let c = Check::new(
            "x",
            "T",
            Severity::Warn,
            "r",
            vec![Violation::new("n", "d")],
        )
        .escalating_after(Some(1));
        assert_eq!(c.severity_of(&c.violations[0]), Severity::Warn);
    }

    /// Escalation only ever raises. A threshold must not quietly downgrade an Error check
    /// whose findings happen to be young.
    #[test]
    fn a_young_finding_on_an_error_check_still_gates() {
        let c = Check::new(
            "dangling-edge",
            "T",
            Severity::Error,
            "r",
            vec![aged("n", 1)],
        )
        .escalating_after(Some(100));
        assert_eq!(c.severity_of(&c.violations[0]), Severity::Error);
        assert!(c.gates(&c.violations[0]));
    }
}
