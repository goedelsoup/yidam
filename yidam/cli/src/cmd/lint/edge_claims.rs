//! An edge is a claim, and these two checks are the ones that ask it for its standing (#587).
//!
//! # The asymmetry this closes
//!
//! `prelude/guidelines/agent-conduct.md`, *An edge is a claim*, is unambiguous: `a →[requires]→
//! b` asserts that a requires b as flatly as a sentence would, "and it asserts it in the form a
//! reader is least likely to check". Eighteen lines later, *A tag may be a field rather than a
//! sentence* hands a **node** the instrument that argument calls for — a property of `type:
//! claim`, after which `open-questions`, `status`, `corpus-index` and the MCP server all see it.
//!
//! A link had `target` and `relationship`. It could declare neither a standing nor a source,
//! and nothing checked it. So the graph a calculator actually walks held the only claims exempt
//! from the corpus's own evidence discipline, and the remedy the guidelines offered — say what
//! it rests on in the node body and tag it there — put the tag somewhere nothing associates
//! with the edge it is about.
//!
//! The repository that filed #587 had already worked around it: `claim_tag`, and `source` where
//! verified, on every empirical link — 1,972 links, 1,270 empirical and all tagged, 702
//! structural and exempt — enforced by a local crate, because extra keys on a link passed
//! `graph-check` and `lint` untouched. Every location error that corpus made across six phases
//! was an edge, each sitting beside prose that was correctly tagged.
//!
//! [`crate::parse::CorpusLink`] gained the two fields in #714 and carried them uninterpreted.
//! This is what reads them.
//!
//! # Two checks, mirroring the two prose rules
//!
//! - **`edge-untagged`** — an empirical edge that declares no standing, or one whose
//!   `claim_tag` spells none. The mirror of `claim-tag-malformed`, which is the prose rule for
//!   *a marker that looks like a tag to a reader and is no tag to a tool*.
//! - **`edge-verified-unsourced`** — an edge asserting `verified` with no `source:`. The mirror
//!   of `verified-unsourced`, and the same one-directional argument: over-counting evidence is
//!   the flattering error, and it is the one the vocabulary exists to prevent.
//!
//! # The first is opt-in and the second is not
//!
//! `edge-verified-unsourced` fires only on an edge that has **already** claimed `verified`, so
//! its population is empty by construction in a corpus that tags nothing. That is
//! [`super::local_citations`]'s condition met for the same reason: writing the tag is the
//! declaration the check enforces, so no corpus that predates it can be put in debt by it.
//!
//! `edge-untagged` cannot be that. Its population is *every edge*, so unconditionally it would
//! open with one finding per empirical edge — 1,270 in the corpus that asked for it, and the
//! whole graph in one that has never heard of the practice. So it runs only where
//! `.yidam/corpus/universal.yml` declares `edge_claims: required: true`, and skips the
//! relationships that file names as structural. See [`crate::universal::EdgeClaims`] for why
//! the exemption and the gate are separate keys, and why the declaration is corpus-wide rather
//! than per class.
//!
//! # Only edges between instances, and that is the existing boundary
//!
//! [`super::checks::instance_links`] is the reader, exactly as it is for `unlicensed-edge` and
//! `edge-target-class`. A link to `../<class>.ont.yml` and a citation into `catalog/` are not
//! relationships, and asking one of them for an evidence standing would be asking a filing
//! decision what it is worth. A link that resolves to nothing is `dangling-edge`'s finding and
//! is not reported twice here.

use super::checks::{instance_links, nodes_by_path, Node};
use super::model::{Check, Severity, Violation};
use crate::universal::Universal;

/// The two check ids, named once — a filter keyed on a literal would drift from the id the
/// baseline records the moment either was reworded.
pub const UNTAGGED: &str = "edge-untagged";
pub const VERIFIED_UNSOURCED: &str = "edge-verified-unsourced";

/// What a link's `claim_tag` spells.
///
/// **Untyped on the way in, because the class schema admits two spellings.** `type: claim`
/// licenses a bare standing and a list of them, and [`crate::parse::CorpusLink::claim_tag`] is
/// held as a [`serde_yaml::Value`] rather than guessing which a corpus meant. So the reading
/// happens here, once, for both checks.
enum Standing {
    /// No `claim_tag:` key at all, or one holding nothing.
    Absent,
    /// Every entry spells a standing. The bracketed markers, as [`crate::claims`] writes them.
    Declared(Vec<&'static str>),
    /// A value is there and spells no standing — the spelling, as written.
    ///
    /// **Not folded into [`Self::Absent`].** The two are one finding and two repairs: a missing
    /// tag is a claim nobody has graded, and `verifed` is a claim somebody graded into a field
    /// no consumer can read. #587 named this one exactly — *"a typo in `claim_tag` would be
    /// silent"* — and a report that said only *untagged* about it would be describing the
    /// symptom while the cause sat two characters away.
    Unreadable(String),
}

/// The standing a link declares, read through [`crate::claims::parse_tag`].
///
/// One reader and not a second opinion: `parse_tag` accepts the bare `open` a typed vocabulary
/// stores and the `[open]` a corpus writes after being told the scan needs brackets, and it
/// admits a qualified tag — `[verified — as proposed]` — as the standing it names. An edge's
/// field is graded by exactly the rule a node's field is.
fn standing_of(value: Option<&serde_yaml::Value>) -> Standing {
    // Extraction is [`crate::claims::link_tag_spellings`] — the same one the reporting
    // surfaces read through (#857). Two extractions is how a finding that says *untagged* and
    // a tally that counts the tag come to describe one edge two ways.
    let written = crate::claims::link_tag_spellings(value);
    if written.is_empty() {
        return Standing::Absent;
    }
    let mut standings = Vec::new();
    for spelling in &written {
        match crate::claims::parse_tag(spelling) {
            Some(tag) => standings.push(tag.standing),
            // The first unreadable entry, and the whole value is unreadable: a list holding one
            // standing and one typo has not said what the edge is worth either.
            None => return Standing::Unreadable(spelling.trim().to_string()),
        }
    }
    Standing::Declared(standings)
}

/// How a finding names the edge it is about — `located-in → place/tailwater.yml`.
///
/// The relationship **and** the target, because a node may author several edges of the same
/// relationship and a finding naming only one of the two sends the reader looking. The target is
/// as the node wrote it, which is the string to search the file for.
fn edge_address(link: &crate::parse::CorpusLink) -> String {
    format!(
        "`{}` → `{}`",
        link.relationship.as_deref().unwrap_or("(none)"),
        link.target.as_deref().unwrap_or("(none)")
    )
}

/// The two checks, over one walk of the corpus.
///
/// Returned together and destructured at the call site for [`super::citations::checks`]'s
/// reason: two public functions is what makes two walks look free.
#[must_use]
pub fn checks(nodes: &[Node], universal: &Universal) -> [Check; 2] {
    let by_path = nodes_by_path(nodes);
    let required = universal.edge_claims_required();
    let mut untagged = Vec::new();
    let mut unsourced = Vec::new();
    for n in nodes {
        for (link, _) in instance_links(n, &by_path) {
            let rel = link.relationship.as_deref().unwrap_or_default();
            let structural = universal.is_structural_relationship(rel);
            let standing = standing_of(link.claim_tag.as_ref());
            if required && !structural {
                match &standing {
                    Standing::Absent => untagged.push(Violation::new(
                        &n.rel,
                        format!(
                            "{} declares no `claim_tag` — the edge asserts the relationship \
                             and says nothing about whether it is true",
                            edge_address(link)
                        ),
                    )),
                    Standing::Unreadable(written) => untagged.push(Violation::new(
                        &n.rel,
                        format!(
                            "{} carries `claim_tag: {written}`, which spells no standing — \
                             `verified`, `inference` or `open`",
                            edge_address(link)
                        ),
                    )),
                    Standing::Declared(_) => {}
                }
            }
            // Unconditional, and the condition above is deliberately not applied to it: an edge
            // that wrote `verified` has opted in by writing it, whatever the corpus declared,
            // and a structural relationship asserting `verified` is making an empirical claim
            // under an exemption from being asked about one.
            let verified =
                matches!(&standing, Standing::Declared(s) if s.contains(&crate::claims::VERIFIED));
            if verified && link.source.as_deref().unwrap_or_default().trim().is_empty() {
                unsourced.push(Violation::new(
                    &n.rel,
                    format!(
                        "{} asserts `verified` and declares no `source:` — the standing means \
                         supported by a committed primary source, and nothing here names one",
                        edge_address(link)
                    ),
                ));
            }
        }
    }
    [edge_untagged(untagged), edge_verified_unsourced(unsourced)]
}

fn edge_untagged(violations: Vec<Violation>) -> Check {
    Check::new(
        UNTAGGED,
        "An empirical edge that does not say what it rests on",
        Severity::Warn,
        "`agent-conduct.md`: *an edge is a claim written as structure*, asserted in the form a \
         reader is least likely to check. The prose rules have applied to node bodies since the \
         vocabulary existed and the graph — the part a traversal actually walks — was exempt \
         from them. This asks an edge the same question: `claim_tag: verified`, `inference` or \
         `open` on the link, beside `target` and `relationship`, or the honest third option the \
         guidelines name, which is to leave the edge out. A value spelling no standing is \
         reported separately from a missing one, because a typo in the field is a claim graded \
         into somewhere nothing can read it. \
         \
         Runs only where `.yidam/corpus/universal.yml` declares `edge_claims: required: true`, \
         and never on a relationship that file lists as `structural:` — `instance-of`, \
         `concerns`, `subject-of` and whatever else a corpus's bookkeeping is spelled with. \
         Unconditionally it would open with one finding per edge in the graph, which is a gate \
         arriving in a corpus that never agreed to it. Warn, because which of the three \
         standings an edge deserves is the author's judgement and nothing here proposes one.",
        violations,
    )
    .spanning_links()
}

fn edge_verified_unsourced(violations: Vec<Violation>) -> Check {
    Check::new(
        VERIFIED_UNSOURCED,
        "An edge asserted at `verified` that names no source",
        Severity::Warn,
        "The edge half of `verified-unsourced`, and the same one-directional argument: \
         over-counting evidence is the flattering error and it is the one the claim vocabulary \
         exists to prevent. `verified` means supported by a committed primary source; an edge \
         claiming it with no `source:` has claimed a standing it cannot demonstrate. The fix is \
         a source or a demotion, and which of the two is the author's call — nothing here \
         proposes a promotion, ever. \
         \
         This needs no declaration to switch on and gets none: its population is empty in a \
         corpus that tags no edges, because writing `claim_tag: verified` is itself the \
         declaration the check reads. So it cannot put a repository that predates it in debt, \
         and it reports on an edge that asserted `verified` even where the corpus exempted that \
         relationship from being asked — an exemption says the relationship is bookkeeping, and \
         bookkeeping that claims to be verified has stopped being bookkeeping.",
        violations,
    )
    .spanning_links()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::lint::Overlay;

    /// `class: place` with the given links, as an instance writes it.
    fn node(links: &str) -> String {
        format!("class: place\nlabel: A place\nlinks:\n{links}")
    }

    /// One outgoing link, plus a second node for it to land on — because only an edge between
    /// instances is read.
    fn pair(from: &str, to: &str) -> Vec<(String, String)> {
        vec![
            ("place/tailwater.yml".to_string(), node(from)),
            ("place/canyon.yml".to_string(), node(to)),
        ]
    }

    /// The far node, which is always well-formed: a finding must be about the near one.
    const BACK: &str =
        "  - target: ./tailwater.yml\n    relationship: located-in\n    claim_tag: open\n";

    const TAGGED: &str = "edge_claims:\n  required: true\n  structural:\n    - instance-of\n";

    /// `(edge-untagged details, edge-verified-unsourced details)` over one staged corpus.
    fn run(files: &[(String, String)], universal: &str) -> (Vec<String>, Vec<String>) {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for (rel, text) in files {
            let path = tmp.path().join(".yidam/corpus").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            paths.push(path);
        }
        let nodes = super::super::checks::load_nodes(tmp.path(), &paths, &Overlay::default());
        let [untagged, unsourced] = checks(&nodes, &Universal::parse(universal));
        let details =
            |c: Check| -> Vec<String> { c.violations.into_iter().map(|v| v.detail).collect() };
        (details(untagged), details(unsourced))
    }

    /// The finding the issue is about: an edge beside correctly tagged prose, saying nothing.
    #[test]
    fn an_untagged_empirical_edge_is_reported_where_the_corpus_asked() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: located-in\n",
            BACK,
        );
        let (untagged, unsourced) = run(&files, TAGGED);
        assert_eq!(untagged.len(), 1, "{untagged:?}");
        assert!(
            untagged[0].contains("`located-in` \u{2192} `./canyon.yml`"),
            "{untagged:?}"
        );
        assert!(
            untagged[0].contains("declares no `claim_tag`"),
            "{untagged:?}"
        );
        assert!(unsourced.is_empty(), "{unsourced:?}");
    }

    /// The whole reason the gate is a declaration: the same corpus, having said nothing — and
    /// the same corpus having recorded its exemptions without asking for the gate.
    #[test]
    fn nothing_is_reported_untagged_where_the_corpus_declared_nothing() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: located-in\n",
            "  - target: ./tailwater.yml\n    relationship: located-in\n",
        );
        for universal in ["", "edge_claims:\n  structural:\n    - instance-of\n"] {
            let (untagged, unsourced) = run(&files, universal);
            assert!(untagged.is_empty(), "{universal:?}: {untagged:?}");
            assert!(unsourced.is_empty(), "{unsourced:?}");
        }
    }

    /// A relationship the corpus called bookkeeping is not asked what it rests on.
    #[test]
    fn a_structural_relationship_is_exempt() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: instance-of\n",
            BACK,
        );
        let (untagged, _) = run(&files, TAGGED);
        assert!(untagged.is_empty(), "{untagged:?}");
    }

    /// The typo #587 named, told apart from silence: two repairs, so two sentences.
    #[test]
    fn a_value_spelling_no_standing_is_reported_as_itself() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verifed\n",
            BACK,
        );
        let (untagged, unsourced) = run(&files, TAGGED);
        assert_eq!(untagged.len(), 1, "{untagged:?}");
        assert!(untagged[0].contains("claim_tag: verifed"), "{untagged:?}");
        assert!(
            untagged[0].contains("spells no standing"),
            "the typo is not reported as an absence: {untagged:?}"
        );
        assert!(
            unsourced.is_empty(),
            "a typo asserts nothing: {unsourced:?}"
        );
    }

    /// Both spellings a declared claim field admits, and a qualified tag, all read — the edge's
    /// field is graded by exactly the rule a node's field is.
    #[test]
    fn the_spellings_a_node_field_admits_are_the_ones_a_link_admits() {
        for spelling in [
            "open",
            "\"[open]\"",
            "inference",
            "\"[verified \u{2014} as proposed]\"",
        ] {
            let files = pair(
                &format!(
                    "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: {spelling}\n    source: a-record\n"
                ),
                BACK,
            );
            let (untagged, unsourced) = run(&files, TAGGED);
            assert!(untagged.is_empty(), "{spelling}: {untagged:?}");
            assert!(unsourced.is_empty(), "{spelling}: {unsourced:?}");
        }
    }

    /// `verified` with nothing behind it, reported without any declaration at all.
    #[test]
    fn a_verified_edge_with_no_source_is_reported_in_any_corpus() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verified\n",
            "  - target: ./tailwater.yml\n    relationship: located-in\n",
        );
        let (untagged, unsourced) = run(&files, "");
        assert!(
            untagged.is_empty(),
            "no declaration, no untagged findings: {untagged:?}"
        );
        assert_eq!(unsourced.len(), 1, "{unsourced:?}");
        assert!(unsourced[0].contains("asserts `verified`"), "{unsourced:?}");
    }

    /// A blank `source:` is the same silence as no `source:`.
    #[test]
    fn a_blank_source_does_not_discharge_the_standing() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verified\n    source: \"   \"\n",
            BACK,
        );
        let (_, unsourced) = run(&files, "");
        assert_eq!(unsourced.len(), 1, "{unsourced:?}");
    }

    /// An exemption says the relationship is bookkeeping; bookkeeping claiming `verified` has
    /// stopped being bookkeeping, and is still asked for its source.
    #[test]
    fn an_exempt_relationship_claiming_verified_is_still_asked_for_a_source() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: instance-of\n    claim_tag: verified\n",
            BACK,
        );
        let (untagged, unsourced) = run(&files, TAGGED);
        assert!(untagged.is_empty(), "{untagged:?}");
        assert_eq!(unsourced.len(), 1, "{unsourced:?}");
    }

    /// A list of standings is a spelling the class schema admits, so it is one a link admits.
    /// One entry unreadable makes the value unreadable: the edge has not said what it is worth.
    #[test]
    fn a_list_is_read_entry_by_entry() {
        let cases: &[(&str, usize, usize)] = &[
            ("    claim_tag:\n      - inference\n      - open\n", 0, 0),
            ("    claim_tag:\n      - verified\n      - open\n", 0, 1),
            ("    claim_tag:\n      - inference\n      - verifed\n", 1, 0),
        ];
        for (tag, want_untagged, want_unsourced) in cases {
            let files = pair(
                &format!("  - target: ./canyon.yml\n    relationship: located-in\n{tag}"),
                BACK,
            );
            let (untagged, unsourced) = run(&files, TAGGED);
            assert_eq!(untagged.len(), *want_untagged, "{tag}: {untagged:?}");
            assert_eq!(unsourced.len(), *want_unsourced, "{tag}: {unsourced:?}");
        }
    }

    /// The boundary `unlicensed-edge` and `edge-target-class` already draw: a link to the class
    /// file is a filing decision, and asking it for an evidence standing asks the wrong thing.
    /// `structural:` names nothing here, so nothing is exempt by declaration — only by not
    /// being an edge between instances.
    #[test]
    fn a_link_that_is_not_an_edge_between_instances_is_not_asked() {
        let files = vec![(
            "place/tailwater.yml".to_string(),
            node("  - target: ../place.ont.yml\n    relationship: instance-of\n  - target: ../../catalog/gauge-record.md\n    relationship: sources-from\n"),
        )];
        let (untagged, _) = run(&files, "edge_claims:\n  required: true\n");
        assert!(untagged.is_empty(), "{untagged:?}");
    }
}
