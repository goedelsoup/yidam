//! Counting the evidence markers in a node.
//!
//! Distinct from `yidam_core::corpus::extract_claims`, which parses whole *claims* —
//! line-oriented, one tag per line, returning the claim text. This counts *markers*
//! wherever they appear, which is a different question with a different answer.
//!
//! Properties are included because the corpus routinely records absence there rather than
//! in prose: `estimate: "[open] — not computed"` is a live open claim that a
//! description-only count misses entirely.
//!
//! Markers are matched as exact bracketed tokens. That is deliberately narrow — corpus
//! prose is dense with markdown links, and a looser match would read `[open questions](…)`
//! as an open claim. The three tokens never appear as link text, so exact matching has no
//! false positives to trade against.

/// How much of a node is measured against how much is supposed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClaimCounts {
    pub verified: usize,
    pub inference: usize,
    pub open: usize,
}

impl ClaimCounts {
    pub fn total(&self) -> usize {
        self.verified + self.inference + self.open
    }

    pub fn add(&mut self, other: ClaimCounts) {
        self.verified += other.verified;
        self.inference += other.inference;
        self.open += other.open;
    }

    /// Compact rendering for an index cell: `12v / 3i / 1o`, or `—` when untagged.
    pub fn cell(&self) -> String {
        if self.total() == 0 {
            return "—".to_string();
        }
        format!("{}v / {}i / {}o", self.verified, self.inference, self.open)
    }
}

/// The tokens defined by `prelude/guidelines/agent-conduct.md`.
pub const VERIFIED: &str = "[verified]";
pub const INFERENCE: &str = "[inference]";
pub const OPEN: &str = "[open]";

fn tally(text: &str, counts: &mut ClaimCounts) {
    counts.verified += text.matches(VERIFIED).count();
    counts.inference += text.matches(INFERENCE).count();
    counts.open += text.matches(OPEN).count();
}

/// Count markers in a whole instance file.
///
/// Reads the raw YAML rather than the parsed struct so that markers in property values are
/// seen regardless of how the property is shaped — the corpus puts them in scalars, lists,
/// and nested maps, and a typed walk would have to anticipate each.
pub fn count_in_source(text: &str) -> ClaimCounts {
    let mut counts = ClaimCounts::default();
    tally(text, &mut counts);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_each_marker_in_prose() {
        let c = count_in_source("A [verified] and a [inference] and an [open].");
        assert_eq!(c.verified, 1);
        assert_eq!(c.inference, 1);
        assert_eq!(c.open, 1);
        assert_eq!(c.total(), 3);
    }

    #[test]
    fn counts_markers_in_property_values() {
        // The single most load-bearing open claim in a real corpus lives here, not in prose.
        let yaml =
            "description: no markers here\nproperties:\n  estimate: \"[open] — not computed\"\n";
        assert_eq!(count_in_source(yaml).open, 1);
    }

    #[test]
    fn a_markdown_link_is_not_an_open_claim() {
        let c = count_in_source("see [open questions](../q/index.md) for more");
        assert_eq!(c.open, 0, "matching must be exact-token, not substring");
    }

    #[test]
    fn repeated_markers_all_count() {
        let c = count_in_source("[verified] one. [verified] two.");
        assert_eq!(c.verified, 2);
    }

    #[test]
    fn an_untagged_node_renders_as_a_dash() {
        assert_eq!(ClaimCounts::default().cell(), "—");
    }

    #[test]
    fn a_tagged_node_renders_compactly() {
        let c = ClaimCounts {
            verified: 12,
            inference: 3,
            open: 1,
        };
        assert_eq!(c.cell(), "12v / 3i / 1o");
    }

    #[test]
    fn source_counting_sees_markers_anywhere_in_the_file() {
        let yaml = "class: c\ndescription: |\n  A [verified] fact.\nproperties:\n  estimate: \"[open] — not computed\"\n";
        let c = count_in_source(yaml);
        assert_eq!(c.verified, 1);
        assert_eq!(c.open, 1);
    }

    #[test]
    fn totals_accumulate() {
        let mut a = ClaimCounts::default();
        a.add(count_in_source("[verified]"));
        a.add(count_in_source("[open] [open]"));
        assert_eq!(a.verified, 1);
        assert_eq!(a.open, 2);
    }
}
