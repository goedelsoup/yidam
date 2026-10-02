//! A pack's own version, and the range a corpus pins it to (RFC-0048 §3).
//!
//! # Why not the `semver` crate
//!
//! A pin is checked twice, by two programs. `yidam-vendor-update` checks it in shell before it
//! copies a pack, and `yidam source check` checks it here against what was copied. If the two
//! disagree about what `^0.1` admits, a vendor step can copy a pack that the gate then refuses,
//! or the reverse. So the grammar is the small one both can implement exactly:
//!
//! - a version is `MAJOR.MINOR.PATCH`, three non-negative integers and nothing else, with no
//!   pre-release and no build metadata
//! - a range is `^V`, `~V`, `=V` or a bare `V`, where `V` is one, two or three of those
//!   integers. A bare `V` means `^V`, as it does in a `Cargo.toml`.
//!
//! The bounds are cargo's for the same spellings. `tests/task_layer.rs` runs one table of cases
//! through both implementations.

use std::fmt;

/// `MAJOR.MINOR.PATCH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version(pub u64, pub u64, pub u64);

impl Version {
    pub fn parse(raw: &str) -> Option<Self> {
        let parts = numbers(raw)?;
        match parts.as_slice() {
            [major, minor, patch] => Some(Self(*major, *minor, *patch)),
            _ => None,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.0, self.1, self.2)
    }
}

/// The versions a pin admits: at least `low`, and below `high`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Range {
    raw: String,
    low: Version,
    high: Version,
}

impl Range {
    /// `None` for anything outside the grammar in the module note, including the comparators
    /// (`>=`, `<`) and wildcards (`*`, `0.x`) cargo would accept.
    pub fn parse(raw: &str) -> Option<Self> {
        let (op, rest) = match raw.chars().next()? {
            '^' | '~' | '=' => (raw.chars().next(), &raw[1..]),
            _ => (None, raw),
        };
        let parts = numbers(rest)?;
        let at = |i: usize| parts.get(i).copied().unwrap_or(0);
        let low = Version(at(0), at(1), at(2));
        let high = match (op, parts.len()) {
            // `=1.2.3` and `~1.2.3` differ only at full precision.
            (Some('='), 3) => Version(low.0, low.1, low.2 + 1),
            (Some('=' | '~'), 1) => Version(low.0 + 1, 0, 0),
            (Some('=' | '~'), _) => Version(low.0, low.1 + 1, 0),
            // Caret, spelled or bare: the leftmost non-zero part is the one that may not change.
            (_, 1) => Version(low.0 + 1, 0, 0),
            _ if low.0 > 0 => Version(low.0 + 1, 0, 0),
            (_, 2) => Version(0, low.1 + 1, 0),
            _ if low.1 > 0 => Version(0, low.1 + 1, 0),
            _ => Version(0, 0, low.2 + 1),
        };
        Some(Self {
            raw: raw.to_string(),
            low,
            high,
        })
    }

    pub fn admits(&self, v: Version) -> bool {
        self.low <= v && v < self.high
    }
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.raw)
    }
}

/// One to three dot-separated integers, each a non-empty run of ASCII digits.
fn numbers(raw: &str) -> Option<Vec<u64>> {
    let parts: Vec<&str> = raw.split('.').collect();
    if parts.is_empty() || parts.len() > 3 {
        return None;
    }
    parts
        .iter()
        .map(|p| {
            if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
                None
            } else {
                p.parse().ok()
            }
        })
        .collect()
}

/// `(version, range, admitted)`, the cases both implementations are held to.
///
/// Public so `tests/task_layer.rs` can run the shell half over the same rows.
#[doc(hidden)]
pub const CASES: &[(&str, &str, bool)] = &[
    ("0.1.0", "^0.1", true),
    ("0.1.9", "^0.1", true),
    ("0.2.0", "^0.1", false),
    ("0.0.9", "^0.1", false),
    ("0.1.0", "0.1", true),
    ("0.2.0", "0.1", false),
    ("1.4.2", "^1.2", true),
    ("2.0.0", "^1.2", false),
    ("1.1.9", "^1.2", false),
    ("1.9.0", "^1", true),
    ("0.9.0", "^0", true),
    ("1.0.0", "^0", false),
    ("0.3.4", "^0.3.2", true),
    ("0.3.1", "^0.3.2", false),
    ("0.4.0", "^0.3.2", false),
    ("0.0.3", "^0.0.3", true),
    ("0.0.4", "^0.0.3", false),
    ("0.0.9", "^0.0", true),
    ("0.1.0", "^0.0", false),
    ("1.2.9", "~1.2.3", true),
    ("1.3.0", "~1.2.3", false),
    ("1.2.2", "~1.2.3", false),
    ("1.2.0", "~1.2", true),
    ("1.3.0", "~1.2", false),
    ("1.9.9", "~1", true),
    ("2.0.0", "~1", false),
    ("1.2.3", "=1.2.3", true),
    ("1.2.4", "=1.2.3", false),
    ("1.2.7", "=1.2", true),
    ("1.3.0", "=1.2", false),
    ("10.0.0", "^9", false),
    ("9.10.0", "^9.9", true),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_case_holds() {
        for (v, r, want) in CASES {
            let range = Range::parse(r).unwrap_or_else(|| panic!("{r} parses"));
            let version = Version::parse(v).unwrap_or_else(|| panic!("{v} parses"));
            assert_eq!(range.admits(version), *want, "{v} against {r}");
        }
    }

    #[test]
    fn what_the_shell_cannot_read_is_refused_here_too() {
        for bad in [
            "", "^", ">=0.1", "0.x", "*", "^0.1.0.0", "^v0.1", "^0.1-pre", "^ 0.1", "^0..1",
        ] {
            assert!(Range::parse(bad).is_none(), "{bad:?} parsed");
        }
        for bad in ["0.1", "1", "0.1.0-alpha", "0.1.0+b", "v0.1.0", "0.1.x", ""] {
            assert!(Version::parse(bad).is_none(), "{bad:?} parsed");
        }
    }
}
