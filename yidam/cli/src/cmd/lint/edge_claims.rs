//! An edge is a claim, and these are the checks that ask it for its standing (#587, #858).
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
//! # Four checks: two mirror prose rules, one compares an edge to its ends, one resolves a name
//!
//! - **`edge-untagged`** — an empirical edge that declares no standing, or one whose
//!   `claim_tag` spells none. The mirror of `claim-tag-malformed`, which is the prose rule for
//!   *a marker that looks like a tag to a reader and is no tag to a tool*.
//! - **`edge-verified-unsourced`** — an edge asserting `verified` with no `source:`. The mirror
//!   of `verified-unsourced`, and the same one-directional argument: over-counting evidence is
//!   the flattering error, and it is the one the vocabulary exists to prevent.
//! - **`edge-standing-unheld`** — an edge asserting a standing **stronger** than one its own
//!   endpoints declare. #587's closing comment named this as the remaining question and #858
//!   answers it; see the section below, because what had to be settled was not the comparison.
//! - **`edge-source-unresolved`** — a `source:` naming nothing this repository holds (#1067).
//!   The edge half of `dangling-edge` rather than of any prose rule, and the one check in the
//!   family that gates; see the last section, because the question was what the key denotes.
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
//! # The third check compares two things, and defining the second one is the whole of it
//!
//! `local-citation-tag-drift` is the precedent and it is close — *a citation resting on
//! `[verified]` over a paragraph this corpus tags `[inference]` asserts something its own corpus
//! denies* — but it compares a declared standing to the claims overlapping **one** span. An edge
//! has two ends and no span, so neither half transfers unexamined.
//!
//! **A node's standing is not the weakest tag in it.** [`crate::claims::node_standing`] carries
//! the argument: `count_in_node` returns counts and not a verdict, a synthesis node carries all
//! three tags by design, and reading its weakest would grade the best-made node in every corpus
//! `[open]`. The definition that survives is the one the vocabulary already provides for the
//! purpose — a property the class declared `type: claim`, which is
//! [`crate::claims::ClaimScope::Node`], *the standing is the node's, not one sentence's*. A node
//! that only tags prose declares no standing and is compared to nothing.
//!
//! **Weakest-first, restated for two ends.** The endpoint standing an edge is measured against
//! is the weaker of the two ends that declare one, and the edge's own is the **strongest** it
//! spells. Both are the direction that cannot flatter: the strongest thing the edge claims is
//! what needs support, and the weakest thing the corpus will say about an end is what it has to
//! rest on. An end declaring nothing is skipped rather than counted as `[open]`, because silence
//! is not a grade.
//!
//! **One direction only.** An `open` edge between two `verified` nodes is not a defect and is
//! never reported: it is a corpus saying it knows both things and not that they are related,
//! which is what the vocabulary is for. Only the edge that outranks an end is a finding.
//!
//! # Warn, and this is the one in the family that cannot be Error
//!
//! Every other contradiction check here ships at Error on one ground — its population is empty
//! by construction, so no repository that predates it can be put in debt by it. That argument is
//! unavailable to this check and saying otherwise would be wrong. Both sides of the comparison
//! are opt-in *separately*: a corpus can adopt edge tags this year on nodes whose `type: claim`
//! properties were written years ago, and the contradiction between them is then a finding it
//! inherits rather than one it wrote. Warn reports it and gates nothing, which is also the
//! severity both siblings here carry, and for the adjacent reason — which of the two sides is
//! wrong is the author's judgement and nothing here proposes an answer.
//!
//! # `lint` and not `graph-check`, though the question is about a walk
//!
//! It only arises once both endpoints are in hand, which is `graph-check`'s business. It is here
//! anyway, for two reasons that outweigh that one: the two checks it extends are here and
//! resolve their endpoints through the same [`super::checks::instance_links`], so this costs no
//! second walk and no second resolver; and a corpus reads one report about its edges' standings
//! rather than two. The seam between the two commands is about structure versus prose, and an
//! evidence standing is prose that happens to be stored on a link.
//!
//! # The fourth resolves a name, and settling what the name denotes was the whole of it
//!
//! `edge-verified-unsourced` asks whether a `verified` edge names a source, and until #1067 that
//! was the whole question: a `source:` naming an entry that was renamed, or that never existed,
//! passed exactly as one that resolves. The repository that filed it had written the check
//! locally, as it had written the tagging discipline locally before #587.
//!
//! **What a `source:` denotes was never written down**, and four derived corpora answered it
//! three ways before this check asked. Measured at the pins #1067 records, over every link
//! `source:` in the tree: `allen-county-ohio-affordability-2026` writes a catalog entry's file
//! stem (140) or a decision record's `id:` (21); `ohio-136th-assembly-hb-96` (54) and
//! `ohio-democratic-party-rural-caucus` (410) write the stem; `allen-county-ohio` (1,360) writes
//! a path relative to the node, exactly as it writes a `links:` target. All 1,985 resolve. So
//! [`Held`] admits all three, because a check that picked one would open with a thousand
//! findings in a corpus whose every citation is sound, and the spelling was never the defect.
//!
//! The decision record is admitted on the filing corpus's own argument: *"several of these
//! edges rest on a rule this repository wrote rather than on a document it retrieved — a
//! `peer-of` edge rests on `comparator-counties` […] for which a catalog entry would be the
//! wrong citation entirely."* A record is addressed by its `id:`, or by its file stem where a
//! corpus keeps the two apart, because nothing here can tell which one a corpus meant.
//!
//! **Error, on the ground the rest of the family cannot claim.** Which standing an edge
//! deserves is the author's judgement, which is why every sibling here is Warn. Whether the
//! thing an edge cites is in this tree is not a judgement at all — it is `dangling-edge`'s kind
//! of question, one key over, and `dangling-edge` gates. The population is empty by
//! construction in a corpus that writes no `source:`, and was measured at zero in every corpus
//! that writes one, so no repository that predates the check is put in debt by it. It is
//! unconditional on the tag: an `inference` edge naming a dead source has still cited nothing.
//!
//! # Only edges between instances, and that is the existing boundary
//!
//! [`super::checks::instance_links`] is the reader, exactly as it is for `unlicensed-edge` and
//! `edge-target-class`. A link to `../<class>.ont.yml` and a citation into `catalog/` are not
//! relationships, and asking one of them for an evidence standing would be asking a filing
//! decision what it is worth. A link that resolves to nothing is `dangling-edge`'s finding and
//! is not reported twice here.

use std::collections::HashSet;
use std::path::PathBuf;

use super::model::{Check, Severity, Violation};
use crate::corpus::{normalize, resolve_target, DecisionRecord, Edges, Node, Source};
use crate::universal::Universal;

/// The check ids, named once — a filter keyed on a literal would drift from the id the
/// baseline records the moment any of them was reworded.
pub const UNTAGGED: &str = "edge-untagged";
pub const VERIFIED_UNSOURCED: &str = "edge-verified-unsourced";
pub const STANDING_UNHELD: &str = "edge-standing-unheld";
pub const SOURCE_UNRESOLVED: &str = "edge-source-unresolved";

/// What a `source:` may name — everything this repository holds that an edge can rest on.
///
/// Three spellings, which are the three the derived corpora were measured writing (see the
/// module doc): a catalog entry by its file stem, a catalog entry by a path relative to the
/// authoring node, and a decision record by its `id:` or its file stem. Built once per lint
/// rather than once per edge, for the reason `citations::checks` resolves once per node.
struct Held {
    /// Catalog entries by normalized path — the same set `verified-unsourced` resolves a
    /// prose citation against, so the two cannot disagree about what is in the catalog.
    catalog_paths: HashSet<PathBuf>,
    /// Catalog entries by file stem: `1889-municipal-register` for
    /// `.yidam/catalog/1889-municipal-register.md`.
    catalog_stems: HashSet<String>,
    /// Decision records by declared `id:` and by file stem. Both, because the corpus that asked
    /// keeps them equal and a corpus that does not has not said which it addresses one by.
    decisions: HashSet<String>,
}

impl Held {
    fn build(sources: &[Source], decisions: &[DecisionRecord]) -> Self {
        let stem = |p: &std::path::Path| p.file_stem().map(|s| s.to_string_lossy().into_owned());
        Self {
            catalog_paths: sources.iter().map(|s| normalize(&s.path)).collect(),
            catalog_stems: sources.iter().filter_map(|s| stem(&s.path)).collect(),
            decisions: decisions
                .iter()
                .flat_map(|d| d.decision.id.clone().into_iter().chain(stem(&d.path)))
                .collect(),
        }
    }

    /// Whether `written`, as node `from` wrote it, names something held here.
    ///
    /// A bare stem tried as a path lands in the node's own class directory, which is never a
    /// catalog entry, so the three readings cannot admit each other's mistakes.
    fn resolves(&self, from: &Node, written: &str) -> bool {
        self.catalog_stems.contains(written)
            || self.decisions.contains(written)
            || self
                .catalog_paths
                .contains(&resolve_target(&from.path, written))
    }
}

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

impl Standing {
    /// The **strongest** standing the edge spells, bare, or `None` where it spells none.
    ///
    /// Strongest and not weakest, which is the opposite of how an endpoint is read and the same
    /// non-flattering direction: what an edge needs support for is the most it claims. A list
    /// holding `verified` and `open` has claimed `verified` about the relationship, exactly as
    /// `edge-verified-unsourced` already reads it.
    fn strongest(&self) -> Option<&'static str> {
        match self {
            Self::Declared(standings) => standings
                .iter()
                .filter_map(|s| crate::claims::bare_standing(s))
                .max_by_key(|s| crate::claims::standing_rank(s)),
            Self::Absent | Self::Unreadable(_) => None,
        }
    }
}

/// The weaker of the two endpoints' declared standings, and which end declared it.
///
/// `None` where neither end declares one — a node that only tags prose has no standing under
/// [`crate::claims::node_standing`]'s definition, and an end that is silent is skipped rather
/// than read as `[open]`, because silence is not a grade.
///
/// Where both declare one the weaker wins, and where they tie the **near** node is named: a
/// finding is reported against the file that authored the edge, so the end a reader can act on
/// from there is the one to name first.
fn endpoint_standing<'a>(
    near: &'a Node,
    far: &'a Node,
    fields: &crate::claims::ClaimFields,
) -> Option<(&'static str, &'a str)> {
    let of = |n: &'a Node| {
        let class = n.inst.class.clone().unwrap_or_default();
        crate::claims::node_standing(&n.text, fields.for_class(&class)).map(|s| (s, n.rel.as_str()))
    };
    match (of(near), of(far)) {
        (Some(a), Some(b)) => Some(
            if crate::claims::standing_rank(b.0) < crate::claims::standing_rank(a.0) {
                b
            } else {
                a
            },
        ),
        (Some(a), None) => Some(a),
        (None, b) => b,
    }
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

/// The four checks, over one walk of the corpus.
///
/// Returned together and destructured at the call site for [`super::citations::checks`]'s
/// reason: four public functions is what makes four walks look free.
#[must_use]
pub fn checks(
    nodes: &[Node],
    edges: &Edges,
    universal: &Universal,
    fields: &crate::claims::ClaimFields,
    sources: &[Source],
    decisions: &[DecisionRecord],
) -> [Check; 4] {
    let required = universal.edge_claims_required();
    let held = Held::build(sources, decisions);
    let mut untagged = Vec::new();
    let mut unsourced = Vec::new();
    let mut unheld = Vec::new();
    let mut unresolved = Vec::new();
    for (i, n) in nodes.iter().enumerate() {
        for (edge, t) in edges.instance_links(i) {
            let (link, far) = (edge.written_as(n), &nodes[t]);
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
            let source = link.source.as_deref().unwrap_or_default().trim();
            if verified && source.is_empty() {
                unsourced.push(Violation::new(
                    &n.rel,
                    format!(
                        "{} asserts `verified` and declares no `source:` — the standing means \
                         supported by a committed primary source, and nothing here names one",
                        edge_address(link)
                    ),
                ));
            }

            // ── the name against what the repository holds (#1067) ────────────
            //
            // Unconditional on the tag and on the declaration, for the reason above: writing
            // `source:` is the opt-in, and a name that resolves to nothing has cited nothing
            // whatever standing sits beside it. A blank value is `edge-verified-unsourced`'s
            // finding where it matters and silence where it does not, never a dead name.
            if !source.is_empty() && !held.resolves(n, source) {
                unresolved.push(Violation::new(
                    &n.rel,
                    format!(
                        "{} names `source: {source}`, which is no catalog entry and no \
                         decision record — name a catalog entry by its file stem or by a path \
                         from this node, or a decision record by its `id:`",
                        edge_address(link)
                    ),
                ));
            }

            // ── the edge against its own ends (#858) ──────────────────────────
            //
            // Unconditional, for `edge-verified-unsourced`'s reason: an edge that wrote a
            // standing opted in by writing it. A structural relationship is not exempt here
            // either — an exemption says the relationship is bookkeeping, and bookkeeping
            // asserting more than the nodes it files has stopped being bookkeeping.
            if let (Some(edge), Some((endpoint, at))) =
                (standing.strongest(), endpoint_standing(n, far, fields))
            {
                // One direction. An `open` edge between two `verified` nodes is a corpus
                // saying it knows both things and not that they are related, which is the
                // vocabulary working rather than failing.
                if crate::claims::standing_rank(edge) > crate::claims::standing_rank(endpoint) {
                    let end = if at == n.rel.as_str() {
                        "this node".to_string()
                    } else {
                        format!("`{at}`")
                    };
                    unheld.push(Violation::new(
                        &n.rel,
                        format!(
                            "{} asserts `[{edge}]` and {end} declares `[{endpoint}]` — the \
                             edge claims more about the relationship than this corpus claims \
                             about what it relates. Demote the edge, or promote the node and \
                             say why",
                            edge_address(link)
                        ),
                    ));
                }
            }
        }
    }
    [
        edge_untagged(untagged),
        edge_verified_unsourced(unsourced),
        edge_standing_unheld(unheld),
        edge_source_unresolved(unresolved),
    ]
}

fn edge_source_unresolved(violations: Vec<Violation>) -> Check {
    Check::new(
        SOURCE_UNRESOLVED,
        "An edge's `source:` names nothing this repository holds",
        Severity::Error,
        "`edge-verified-unsourced` asks whether a `verified` edge names a source, and until this \
         check that was the whole of the question: a `source:` naming a catalog entry that was \
         renamed, or that never existed, passed exactly as one that resolves. This resolves it. \
         A value may name a catalog entry by its file stem, a catalog entry by a path written \
         as a `links:` target is, or a decision record by its `id:` — the three spellings the \
         derived corpora were measured writing, and the last admitted because an edge can rest \
         on a rule this corpus wrote rather than on a document it retrieved, for which a \
         catalog entry would be the wrong citation. \
         \
         Error, on the one ground the rest of this family cannot claim. Which standing an edge \
         deserves is the author's judgement and every sibling here is Warn for it; whether the \
         thing an edge cites is in this tree is not a judgement at all, and is the question \
         `dangling-edge` already gates on, one key over. The population is empty in a corpus \
         that writes no `source:`, and was measured at zero across every corpus that writes \
         one, so no repository that predates the check is put in debt by it. It reads every \
         edge that names a source, whatever the tag beside it: an `inference` edge naming a \
         dead source has still cited nothing.",
        violations,
    )
    .spanning_links()
}

fn edge_standing_unheld(violations: Vec<Violation>) -> Check {
    Check::new(
        STANDING_UNHELD,
        "An edge asserts a standing its own endpoints do not hold",
        Severity::Warn,
        "The graph half of `local-citation-tag-drift`: a citation resting on `[verified]` over a \
         paragraph this corpus tags `[inference]` asserts something its own corpus denies, and an \
         edge asserting `verified` between two nodes this corpus grades `[open]` does the same \
         thing in the form a reader is least likely to check. A node's standing here is a \
         property its class declared `type: claim` — the node's own grade, not the weakest marker \
         in its prose, because a synthesis node carries all three tags by design. A node that \
         declares none is compared to nothing. \
         \
         One-directional, like every check in this family: an `open` edge between two `verified` \
         nodes is not a defect, it is a corpus saying it knows both things and not that they are \
         related, which is what the vocabulary is for. The endpoint compared against is the \
         weaker of the two ends, and the edge's own standing is the strongest it spells. \
         \
         Warn, and this one cannot be Error. Every other contradiction check here gates on the \
         ground that its population is empty by construction; this one's is not, because a corpus \
         can adopt edge tags on a graph whose nodes were graded years earlier and inherit \
         findings it did not create. Which of the two sides is wrong is the author's judgement \
         and nothing here proposes an answer.",
        violations,
    )
    .spanning_links()
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
    use crate::corpus::Overlay;

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

    /// What every staged corpus holds for a `source:` to name: one catalog entry, and two
    /// decision records — one whose `id:` is its file stem and one whose `id:` is not, so the
    /// two ways of addressing a record are told apart.
    const CATALOG: &[&str] = &["a-record"];
    const DECISIONS: &[(&str, &str)] = &[
        ("comparator-counties", "comparator-counties"),
        ("rule-0007", "sampling-frame"),
    ];

    /// `(edge-untagged, edge-verified-unsourced)` details over one staged corpus, with no class
    /// declaring a claim field — so `edge-standing-unheld` has no endpoint standing to read and
    /// the two original checks are measured exactly as they were.
    fn run(files: &[(String, String)], universal: &str) -> (Vec<String>, Vec<String>) {
        let (untagged, unsourced, unheld, unresolved) = run_all(files, universal, &[]);
        assert!(
            unheld.is_empty(),
            "no class declared a claim field, so no node declares a standing: {unheld:?}"
        );
        assert!(
            unresolved.is_empty(),
            "every `source:` these corpora write names the staged entry: {unresolved:?}"
        );
        (untagged, unsourced)
    }

    /// All four checks, with `fields` the properties every class here declared `type: claim`.
    #[allow(clippy::type_complexity)]
    fn run_all(
        files: &[(String, String)],
        universal: &str,
        fields: &[&str],
    ) -> (Vec<String>, Vec<String>, Vec<String>, Vec<String>) {
        let tmp = tempfile::tempdir().unwrap();
        let mut paths = Vec::new();
        for (rel, text) in files {
            let path = tmp.path().join(".yidam/corpus").join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, text).unwrap();
            paths.push(path);
        }
        let nodes = crate::corpus::load_nodes(tmp.path(), &paths, &Overlay::default());
        let catalog = tmp.path().join(".yidam/catalog");
        std::fs::create_dir_all(&catalog).unwrap();
        let entries: Vec<_> = CATALOG
            .iter()
            .map(|stem| {
                let path = catalog.join(format!("{stem}.md"));
                std::fs::write(&path, "---\nobtained: true\n---\nA record.\n").unwrap();
                path
            })
            .collect();
        let sources = crate::corpus::load_sources(tmp.path(), &entries, &Overlay::default());
        let decisions_dir = tmp.path().join(".yidam/decisions");
        std::fs::create_dir_all(&decisions_dir).unwrap();
        for (stem, id) in DECISIONS {
            std::fs::write(
                decisions_dir.join(format!("{stem}.yml")),
                format!("id: {id}\nsummary: A rule this corpus wrote.\n"),
            )
            .unwrap();
        }
        let decisions = crate::corpus::load_decisions(tmp.path(), &decisions_dir);
        let declared: Vec<String> = fields.iter().map(|f| (*f).to_string()).collect();
        let claim_fields = crate::claims::ClaimFields::from_declarations(
            ["place", "concept"]
                .into_iter()
                .map(|c| (c.to_string(), declared.clone())),
        );
        let [untagged, unsourced, unheld, unresolved] = checks(
            &nodes,
            &crate::corpus::Edges::build(&nodes),
            &Universal::parse(universal),
            &claim_fields,
            &sources,
            &decisions,
        );
        let details =
            |c: Check| -> Vec<String> { c.violations.into_iter().map(|v| v.detail).collect() };
        (
            details(untagged),
            details(unsourced),
            details(unheld),
            details(unresolved),
        )
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

    // ── #858: the edge against the standings its own endpoints declare ──────────

    /// `class: place` with a declared node standing and the given links.
    fn graded(standing: &str, links: &str) -> String {
        format!("class: place\nlabel: A place\nstatus: {standing}\nlinks:\n{links}")
    }

    /// The finding the issue is about: `verified` asserted across a relation whose ends are live.
    #[test]
    fn an_edge_outranking_its_endpoints_is_reported() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                graded(
                    "open",
                    "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verified\n    source: a-record\n",
                ),
            ),
            ("place/canyon.yml".to_string(), graded("open", BACK)),
        ];
        let (_, unsourced, unheld, _) = run_all(&files, TAGGED, &["status"]);
        assert!(unsourced.is_empty(), "{unsourced:?}");
        assert_eq!(unheld.len(), 1, "{unheld:?}");
        assert!(
            unheld[0].contains("`located-in` \u{2192} `./canyon.yml`"),
            "{unheld:?}"
        );
        assert!(
            unheld[0].contains("asserts `[verified]`") && unheld[0].contains("`[open]`"),
            "{unheld:?}"
        );
    }

    /// The direction that is not a defect, and the reason the check is one-directional: a corpus
    /// knowing both things and not saying they are related is the vocabulary working.
    #[test]
    fn an_open_edge_between_verified_nodes_is_not_a_finding() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                graded(
                    "verified",
                    "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: open\n",
                ),
            ),
            (
                "place/canyon.yml".to_string(),
                graded("verified", "  - target: ./tailwater.yml\n    relationship: located-in\n    claim_tag: open\n"),
            ),
        ];
        let (_, _, unheld, _) = run_all(&files, TAGGED, &["status"]);
        assert!(unheld.is_empty(), "{unheld:?}");
    }

    /// An edge tied with its ends, and one weaker than them. Neither claims more than it rests on.
    #[test]
    fn an_edge_no_stronger_than_its_ends_is_not_a_finding() {
        for (node, edge) in [("inference", "inference"), ("inference", "open")] {
            let files = vec![
                (
                    "place/tailwater.yml".to_string(),
                    graded(
                        node,
                        &format!("  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: {edge}\n"),
                    ),
                ),
                ("place/canyon.yml".to_string(), graded(node, BACK)),
            ];
            let (_, _, unheld, _) = run_all(&files, TAGGED, &["status"]);
            assert!(unheld.is_empty(), "{node}/{edge}: {unheld:?}");
        }
    }

    /// The weaker of the two ends is the one an edge is measured against, and the finding names
    /// it — otherwise a reader opens the node the edge was authored in and finds nothing wrong.
    #[test]
    fn the_weaker_end_is_the_one_compared_and_the_one_named() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                graded(
                    "verified",
                    "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verified\n    source: a-record\n",
                ),
            ),
            ("place/canyon.yml".to_string(), graded("inference", BACK)),
        ];
        let (_, _, unheld, _) = run_all(&files, TAGGED, &["status"]);
        assert_eq!(unheld.len(), 1, "{unheld:?}");
        assert!(unheld[0].contains("`[inference]`"), "{unheld:?}");
        assert!(
            unheld[0].contains("place/canyon.yml"),
            "the far end is named: {unheld:?}"
        );
    }

    /// A node that only tags prose declares no standing, and silence is not `[open]` — the
    /// definition #858 had to settle, asserted rather than described.
    #[test]
    fn a_node_that_only_tags_prose_declares_no_standing() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                "class: place\nlabel: A place\ndescription: |\n  Much of this is unsettled. [open]\nlinks:\n  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: verified\n    source: a-record\n".to_string(),
            ),
            (
                "place/canyon.yml".to_string(),
                "class: place\nlabel: A place\ndescription: |\n  Also unsettled. [open]\nlinks:\n  - target: ./tailwater.yml\n    relationship: located-in\n    claim_tag: open\n".to_string(),
            ),
        ];
        let (_, _, unheld, _) = run_all(&files, TAGGED, &["status"]);
        assert!(
            unheld.is_empty(),
            "a prose `[open]` is not the node's grade: {unheld:?}"
        );
    }

    /// The synthesis node the definition exists for: all three standings declared, and the
    /// weakest of them is what the node is willing to say about itself.
    #[test]
    fn several_declared_standings_read_weakest_first() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                "class: place\nlabel: A place\nstatus: verified\nprovenance: open\nlinks:\n  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag: inference\n".to_string(),
            ),
            ("place/canyon.yml".to_string(), graded("verified", BACK)),
        ];
        let (_, _, unheld, _) = run_all(&files, TAGGED, &["status", "provenance"]);
        assert_eq!(unheld.len(), 1, "{unheld:?}");
        assert!(unheld[0].contains("`[open]`"), "{unheld:?}");
    }

    /// The strongest thing a listed `claim_tag` claims is what needs support, which is the same
    /// entry `edge-verified-unsourced` reads.
    #[test]
    fn a_listed_edge_tag_is_read_at_its_strongest() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                graded(
                    "open",
                    "  - target: ./canyon.yml\n    relationship: located-in\n    claim_tag:\n      - open\n      - verified\n    source: a-record\n",
                ),
            ),
            ("place/canyon.yml".to_string(), graded("open", BACK)),
        ];
        let (_, _, unheld, _) = run_all(&files, TAGGED, &["status"]);
        assert_eq!(unheld.len(), 1, "{unheld:?}");
        assert!(unheld[0].contains("asserts `[verified]`"), "{unheld:?}");
    }

    /// No declaration switches this on and none is needed: writing the tag is the opt-in, and a
    /// relationship the corpus called bookkeeping is still read on a standing it wrote.
    #[test]
    fn it_needs_no_declaration_and_exempts_no_relationship() {
        let files = vec![
            (
                "place/tailwater.yml".to_string(),
                graded(
                    "open",
                    "  - target: ./canyon.yml\n    relationship: instance-of\n    claim_tag: inference\n",
                ),
            ),
            ("place/canyon.yml".to_string(), graded("open", BACK)),
        ];
        let (untagged, _, unheld, _) = run_all(&files, "", &["status"]);
        assert!(untagged.is_empty(), "{untagged:?}");
        assert_eq!(unheld.len(), 1, "{unheld:?}");
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

    // ── #1067: the name against what the repository holds ──────────────────────

    /// One outgoing edge naming `source`, tagged or not, in a corpus that declared nothing.
    fn sourced(tag: Option<&str>, source: &str) -> Vec<(String, String)> {
        let tag = tag
            .map(|t| format!("    claim_tag: {t}\n"))
            .unwrap_or_default();
        pair(
            &format!("  - target: ./canyon.yml\n    relationship: located-in\n{tag}    source: {source}\n"),
            BACK,
        )
    }

    /// The three spellings measured in the derived corpora, and the two ways of addressing a
    /// decision record. None is a finding.
    #[test]
    fn a_source_naming_something_this_repository_holds_resolves() {
        for source in [
            "a-record",
            "../../catalog/a-record.md",
            "comparator-counties",
            "sampling-frame",
            "rule-0007",
            "\"a-record\"",
        ] {
            let (_, _, _, unresolved) = run_all(&sourced(Some("verified"), source), "", &[]);
            assert!(unresolved.is_empty(), "{source}: {unresolved:?}");
        }
    }

    /// The finding the issue is about: an entry that was renamed, or never existed, cited as
    /// though it were there. Reported without any declaration, whatever the tag beside it.
    #[test]
    fn a_source_naming_nothing_is_reported_whatever_the_tag() {
        for tag in [Some("verified"), Some("inference"), None] {
            let (untagged, unsourced, _, unresolved) = run_all(&sourced(tag, "a-recrod"), "", &[]);
            assert!(untagged.is_empty(), "{tag:?}: {untagged:?}");
            assert!(
                unsourced.is_empty(),
                "{tag:?}: a name is not an absence: {unsourced:?}"
            );
            assert_eq!(unresolved.len(), 1, "{tag:?}: {unresolved:?}");
            assert!(
                unresolved[0].contains("`located-in` \u{2192} `./canyon.yml`"),
                "{unresolved:?}"
            );
            assert!(
                unresolved[0].contains("`source: a-recrod`"),
                "the name as written, which is the string to search for: {unresolved:?}"
            );
        }
    }

    /// A path that lands on a file is not a path that lands in the catalog: a node is not a
    /// source, and a stem read as a path lands in the node's own directory.
    #[test]
    fn a_path_outside_the_catalog_does_not_resolve() {
        for source in [
            "./canyon.yml",
            "../../catalog/a-recrod.md",
            "../place/a-record",
        ] {
            let (_, _, _, unresolved) = run_all(&sourced(None, source), "", &[]);
            assert_eq!(unresolved.len(), 1, "{source}: {unresolved:?}");
        }
    }

    /// A blank value is `edge-verified-unsourced`'s finding, and only where it matters. It is
    /// never reported as a dead name, so a `verified` edge with a blank source gets one finding
    /// and an `inference` edge with one gets none.
    #[test]
    fn a_blank_source_is_an_absence_and_not_a_dead_name() {
        for (tag, want_unsourced) in [(Some("verified"), 1), (Some("inference"), 0), (None, 0)] {
            let (_, unsourced, _, unresolved) = run_all(&sourced(tag, "\"  \""), "", &[]);
            assert_eq!(unsourced.len(), want_unsourced, "{tag:?}: {unsourced:?}");
            assert!(unresolved.is_empty(), "{tag:?}: {unresolved:?}");
        }
    }

    /// An exemption is from being asked for a standing. A structural edge that names a source
    /// has cited something, and what it cited is still held to exist.
    #[test]
    fn a_structural_relationship_naming_a_dead_source_is_still_reported() {
        let files = pair(
            "  - target: ./canyon.yml\n    relationship: instance-of\n    source: a-recrod\n",
            BACK,
        );
        let (untagged, _, _, unresolved) = run_all(&files, TAGGED, &[]);
        assert!(untagged.is_empty(), "{untagged:?}");
        assert_eq!(unresolved.len(), 1, "{unresolved:?}");
    }
}
