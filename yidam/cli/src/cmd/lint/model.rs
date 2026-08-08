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
}

impl Violation {
    pub fn new(node: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            node: node.into(),
            detail: detail.into(),
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
            violations,
        }
    }

    pub fn passed(&self) -> bool {
        self.violations.is_empty()
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
}
