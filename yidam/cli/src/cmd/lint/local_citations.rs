//! A node citing a span of another node in **this** corpus — RFC-0034.
//!
//! # The direction that was missing
//!
//! Citations were checked in two directions and not the one derived corpora actually write in.
//! [`super::citations`] resolves a `cites:` entry into an installed dependency: the far node
//! exists, the span is still in it, the bundle is the one the citation says it read.
//! [`super::line_citations`] resolves a `#L` fragment into a source file this repository owns.
//! Between them sat the join a corpus makes constantly and nothing read — **one node resting on
//! a verbatim span of another node beside it**.
//!
//! `prelude/guidelines/agent-conduct.md` states the rule for both directions in the same
//! sentence — *"Cite a span, not a node"* — and gives the structured form for only one of them.
//! A `cites:` entry has always required `package:`, so the grammar could not name a node in the
//! corpus it was written in. This module is what a citation means when that field is absent.
//!
//! # Why the ambient markdown link could not be read instead
//!
//! Corpora cite each other in prose already: **13,557 markdown links resolve into
//! `.yidam/corpus/*.yml` across seventeen derived repositories**, 86.3% of them from one node to
//! another. The obvious move was to read the quote beside those links, the way
//! `slid-line-citation` reads the quote beside a source citation, and it does not survive
//! contact with the corpora. Under exactly the house rules [`super::line_citations`] applies,
//! 222 of the 12,506 inside the lint walk carry something a quote-detector reads as a quotation
//! — 237 candidates — and **26 of the 237 are text that is actually in the cited node**.
//!
//! The failures are not near-misses, they are a different act. An italic span immediately before
//! a corpus-node link matches at **2 of 194**: node prose is written with heavy emphasis and
//! links sit mid-sentence, so `*…*` there is stress, not transcription. And a blockquote beside
//! a `person/` link is overwhelmingly an interview being attributed to its speaker — the link
//! names **who said it**, not where the words are. A check built on that convention would have
//! reported **211 drifts of which 26 were real**, against a population no corpus opted into.
//!
//! So the anchor has to be declared rather than inferred, and `cites:` is the declaration the
//! prelude already documents. Nothing here fires on a corpus that writes none, which is what
//! makes this safe to ship at Error: the population starts empty by construction, so no corpus
//! that predates it can be put in debt by it. That is `node-too-long`'s condition met without
//! having to argue a number — *"gating there would enforce a contract nobody wrote"* — because
//! writing the citation **is** the declaration the check enforces.
//!
//! # Four checks, one partition, mirroring the external four
//!
//! - **`local-citation-unresolved`** — no `node:`, or this corpus holds no such node.
//! - **`local-citation-span-drift`** — no `span:`, or the span is no longer in that node.
//! - **`local-citation-tag-drift`** — the span is there and the claim governing it does not
//!   carry the standing the citation declared.
//! - **`local-citation-untagged`** — the citation records a span and no `tag:`, so what was
//!   checked is that the text is there and nothing about what it is worth. Info.
//!
//! # The tag check exists locally and cannot exist externally
//!
//! `external-citation-pin-moved` is the nearest external sibling and it is a **Warn** that says
//! the pin moved, because a foreign tag *"is the producer's tag […] It does not transfer, and
//! you cannot check it"*. Locally the producer is this corpus. A `tag:` here is a claim about a
//! node in the same tree, decidable now rather than across an update, and a citation that says
//! `[verified]` over a paragraph this corpus tags `[inference]` is stating something about its
//! own corpus that its own corpus denies. That is an Error, and it is the one finding in this
//! module that no external check has an analogue for.
//!
//! # Whitespace, and nothing else
//!
//! [`super::citations::flatten`], not [`super::line_citations`]'s word reduction — the same
//! normalization the external span check uses, for the same reason and to the same strictness.
//! A YAML folded scalar rewraps on read and the cited node's own prose is wrapped, so raw bytes
//! would fail every citation written the readable way. Case, punctuation, emphasis and wording
//! are compared as written, because those are the changes a span exists to catch.

use std::collections::BTreeMap;

use super::citations::{flatten, truncate, Finding};
use super::model::{Check, Severity, Violation};
use crate::corpus::Node;
use crate::parse::ExternalCitation;

/// The four check ids, named once. A filter keyed on a literal would drift from the id the
/// baseline records the moment either was reworded.
pub const UNRESOLVED: &str = "local-citation-unresolved";
pub const SPAN_DRIFT: &str = "local-citation-span-drift";
pub const TAG_DRIFT: &str = "local-citation-tag-drift";
pub const UNTAGGED: &str = "local-citation-untagged";

/// Whether a `cites:` entry names a node in this corpus rather than one in a dependency.
///
/// **The absence of `package:` is the whole of the test, and it is a partition.** Every
/// citation is read by exactly one of the two modules: `package:` present means
/// [`super::citations`], absent means here. A citation naming neither a package nor a node was
/// `external-citation-unresolved`'s finding and is now this module's, which is the only
/// population that moves — and it moves to a check that says the same thing about the same
/// bytes at the same severity.
#[must_use]
pub fn is_local(cite: &ExternalCitation) -> bool {
    cite.package.is_none()
}

/// The corpus as a citation needs to see it: `<class>/<name>` → that node's bytes.
///
/// Keyed without the extension because that is how `cites:` spells a node — `node:
/// concept/base-flow`, the same identity the external form uses on the far side. A citation
/// that writes `.yml` anyway is resolved too; see [`key_of`].
pub type Corpus = BTreeMap<String, String>;

/// The prefix every node's repository-relative path carries — `paths::yidam_corpus_dir`'s, as a
/// string, because a [`Node`] carries `rel` and not the root it was stripped against.
const CORPUS_PREFIX: &str = ".yidam/corpus/";

/// Every node in this corpus, keyed as a citation would name it.
///
/// Keyed off the **prefix**, not off the last occurrence of `corpus/`: a corpus is free to have
/// a class called `corpus`, and `rsplit_once` would then key `.yidam/corpus/corpus/x.yml` as `x`
/// and quietly answer a citation of `corpus/x` with nothing.
#[must_use]
pub fn corpus(nodes: &[Node]) -> Corpus {
    nodes
        .iter()
        .filter_map(|n| {
            let rel = n.rel.replace('\\', "/");
            let tail = rel.strip_prefix(CORPUS_PREFIX)?;
            Some((tail.strip_suffix(".yml")?.to_string(), n.text.clone()))
        })
        .collect()
}

/// `node:` as a corpus key: leading `./`, a trailing `.yml` and surrounding space removed.
///
/// A node reference is written `class/name` in the `cites:` grammar and `class/name.yml`
/// everywhere a markdown link points at one. Both spellings name the same node, and a check
/// that read only the first would report the second unresolved against a file it can see.
fn key_of(node: &str) -> String {
    let t = node.trim().trim_start_matches("./");
    t.strip_suffix(".yml").unwrap_or(t).to_string()
}

/// The three standings, weakest first — [`crate::claims::WEAKEST_FIRST`].
///
/// Aliased rather than restated. This ordering was declared here, where the first comparison of
/// two standings landed; #858's comparison of an edge's standing to its endpoints' is in a
/// different module, and an ordering with two homes is how two checks come to disagree about
/// which way is up.
use crate::claims::WEAKEST_FIRST;

/// The bare standing a `tag:` value spells, or `None` when it spells none.
///
/// Both spellings, because both are written: `tag: verified` is the form
/// `agent-conduct.md` documents and `tag: "[verified]"` is the same claim with the corpus's
/// prose brackets left on. [`crate::claims::parse_tag`] reads either and is the one reader of
/// what a tag spells; a second one here would be a second answer to that question.
fn declared_standing(tag: &str) -> Option<&'static str> {
    let bracketed = crate::claims::tag_of(tag)?;
    WEAKEST_FIRST
        .into_iter()
        .find(|w| bracketed.trim_matches(['[', ']']) == *w)
}

/// The weakest standing among the claims that overlap `span`, or `None` when none does.
///
/// **Overlap in either direction, not containment.** A cited span is as often two sentences as
/// half of one, and [`crate::claims::claims_in_node`] serves a claim per *statement*. Requiring
/// the claim to contain the span reports "nothing licenses this" against a citation that quotes
/// a whole tagged paragraph; requiring the span to contain the claim misses the citation that
/// quotes a clause. Both readings count, and where several claims overlap the answer is the
/// weakest of them — the same direction `agent-conduct.md` computes a derived assertion's tier
/// in, and the only direction that cannot flatter a citation.
fn governing(text: &str, span: &str, fields: &[String]) -> Option<&'static str> {
    let needle = flatten(span);
    let mut found: Option<&'static str> = None;
    for claim in crate::claims::claims_in_node(text, fields) {
        let statement = flatten(&claim.text);
        if statement.is_empty() {
            continue;
        }
        if !statement.contains(&needle) && !needle.contains(&statement) {
            continue;
        }
        let rank = crate::claims::standing_rank;
        found = Some(match found {
            Some(prev) if rank(prev) <= rank(claim.standing) => prev,
            _ => WEAKEST_FIRST
                .into_iter()
                .find(|w| *w == claim.standing)
                .unwrap_or("open"),
        });
    }
    found
}

/// Everything the four checks would say about one local citation, in check order.
///
/// Shaped like [`super::citations::findings`] and for the same reason: the predicate is here and
/// the checks are filters over it, so a surface that wants to ask whether a citation *would*
/// hold before writing it gets the same answer the gate will give afterwards.
#[must_use]
pub fn findings(cite: &ExternalCitation, corpus: &Corpus, fields: &[String]) -> Vec<Finding> {
    let mut out = Vec::new();
    let mut report = |check, severity, message| {
        out.push(Finding {
            check,
            severity,
            message,
        })
    };

    // ── unresolved: the citation names nothing, or names something that is not here ──
    let target = cite
        .node
        .as_deref()
        .map(key_of)
        .filter(|n: &String| !n.is_empty());
    let Some(target) = target else {
        report(
            UNRESOLVED,
            Severity::Error,
            "a citation with no `package:` names a node in this corpus, and this one names no \
             `node:` either — give it the `<class>/<name>` it rests on, or `package:` if it \
             meant a dependency"
                .to_string(),
        );
        return out;
    };
    let Some(text) = corpus.get(&target) else {
        report(
            UNRESOLVED,
            Severity::Error,
            format!(
                "cites `{target}` and this corpus has no such node — it was renamed or removed, \
                 and the claim resting on it was not read when it was"
            ),
        );
        return out;
    };

    // ── span drift: it is there and says something else ──────────────────────
    let span = cite
        .span
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let Some(span) = span else {
        report(
            SPAN_DRIFT,
            Severity::Error,
            format!(
                "cites `{target}` with no `span:` — a node reference alone rots invisibly, \
                 because the node keeps its name while its content is rewritten"
            ),
        );
        return out;
    };
    if !flatten(text).contains(&flatten(span)) {
        report(
            SPAN_DRIFT,
            Severity::Error,
            format!(
                "cites `{target}` for a span that is no longer in it, even ignoring line \
                 wrapping — this node's own corpus revised the text this claim rests on. Read \
                 the node and decide whether the claim still holds; do not re-quote it to make \
                 the check pass. Span: {}",
                truncate(span)
            ),
        );
        return out;
    }

    // ── tag: the standing declared for a span this corpus itself holds ───────
    match cite.tag.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        None => report(
            UNTAGGED,
            Severity::Info,
            format!(
                "cites `{target}` with no `tag:` — the span is there and nothing records what \
                 it was worth when it was read, so a demotion on the far side of this citation \
                 moves nothing here"
            ),
        ),
        Some(raw) => {
            let Some(declared) = declared_standing(raw) else {
                report(
                    TAG_DRIFT,
                    Severity::Error,
                    format!(
                        "cites `{target}` at `{raw}`, which is not a standing — expected \
                         verified, inference or open"
                    ),
                );
                return out;
            };
            match governing(text, span, fields) {
                Some(found) if found == declared => {}
                Some(found) => report(
                    TAG_DRIFT,
                    Severity::Error,
                    format!(
                        "cites `{target}` at [{declared}] and this corpus holds that span at \
                         [{found}] — the citation is a claim about a node in the same tree, so \
                         the two cannot honestly disagree. Change the citation, or change the \
                         node and say why."
                    ),
                ),
                None => report(
                    TAG_DRIFT,
                    Severity::Error,
                    format!(
                        "cites `{target}` at [{declared}] and no claim in that node governs the \
                         span, so nothing licenses [{declared}] — quote text a tagged claim \
                         covers, or tag the claim being rested on"
                    ),
                ),
            }
        }
    }

    out
}

/// Every violation one check reports over the corpus, from the shared predicate.
fn violations(found: &[(&Node, Vec<Finding>)], id: &str) -> Vec<Violation> {
    found
        .iter()
        .flat_map(|(node, findings)| {
            findings
                .iter()
                .filter(|f| f.check == id)
                .map(move |f| Violation::new(&node.rel, f.message.clone()))
        })
        .collect()
}

/// The four checks, over one walk of the corpus.
///
/// Returned together and destructured at the call site rather than exposed as four public
/// functions, for the reason [`super::citations::checks`] is: four functions is what made four
/// walks look free.
#[must_use]
pub fn checks(nodes: &[Node], fields: &crate::claims::ClaimFields) -> [Check; 4] {
    let corpus = corpus(nodes);
    let found: Vec<(&Node, Vec<Finding>)> = nodes
        .iter()
        .flat_map(|n| {
            let class = n.inst.class.clone().unwrap_or_default();
            n.inst
                .cites
                .as_deref()
                .unwrap_or_default()
                .iter()
                .filter(|c| is_local(c))
                .map(|c| (n, findings(c, &corpus, fields.for_class(&class))))
                .collect::<Vec<_>>()
        })
        .collect();
    [
        local_citation_unresolved(violations(&found, UNRESOLVED)),
        local_citation_span_drift(violations(&found, SPAN_DRIFT)),
        local_citation_tag_drift(violations(&found, TAG_DRIFT)),
        local_citation_untagged(violations(&found, UNTAGGED)),
    ]
}

fn local_citation_unresolved(violations: Vec<Violation>) -> Check {
    Check::new(
        UNRESOLVED,
        "A local citation names a node this corpus does not hold",
        Severity::Error,
        "A `cites:` entry with no `package:` rests on a node in this corpus. Renaming or \
         deleting that node breaks the claim, and nothing else notices: `graph-check` reads \
         `links:` and a citation is deliberately not a link, because a citation is not a \
         relationship and must never enter a traversal. Error, under the baseline ratchet like \
         every other citation defect a person here can fix. The population is empty in a corpus \
         that writes no local citations, so this cannot fail a repository that predates it.",
        violations,
    )
}

fn local_citation_span_drift(violations: Vec<Violation>) -> Check {
    Check::new(
        SPAN_DRIFT,
        "A local citation's span is no longer in the node it names",
        Severity::Error,
        "`agent-conduct.md`: *cite a span, not a node*. A node reference alone rots invisibly, \
         because the node keeps its name while its content is rewritten — and inside one corpus \
         that rewrite is a normal commit, not a dependency bump somebody had to choose. \
         Whitespace is normalized on both sides and nothing else is, so a folded scalar and a \
         re-wrapped quote compare equal while a reworded one does not. The repair is to read \
         the node and decide whether the claim survives; re-quoting the new text to clear the \
         finding is the one response that destroys what the check is for.",
        violations,
    )
}

fn local_citation_tag_drift(violations: Vec<Violation>) -> Check {
    Check::new(
        TAG_DRIFT,
        "A local citation declares a standing its own corpus does not hold",
        Severity::Error,
        "A foreign `tag:` is the producer's and `agent-conduct.md` says it does not transfer and \
         cannot be checked — which is why the external family has no equivalent of this and \
         reports a moved pin at Warn instead. A local one has no such excuse: the producer is \
         this corpus, the claim is in the same tree, and a citation resting on `[verified]` over \
         a paragraph this corpus tags `[inference]` asserts something its own corpus denies. \
         The standing is read from the claims that overlap the span, weakest first, which is the \
         direction a derived assertion's tier is computed in. A span no tagged claim covers is \
         reported too: there, nothing licenses the standing that was declared.",
        violations,
    )
}

fn local_citation_untagged(violations: Vec<Violation>) -> Check {
    Check::new(
        UNTAGGED,
        "A local citation records a span and not what it was worth",
        Severity::Info,
        "The span is there and the claim is anchored, which is most of the value. What is \
         missing is the standing it was read at, and without that a demotion on the far side of \
         the citation moves nothing: the node it rests on can fall from `[verified]` to `[open]` \
         and this citation reads exactly as it did. Info, never gates, never baselined — the \
         remedy is a judgement about the citing node rather than a defect in the corpus, and the \
         same verdict `external-citation-unpinned` reaches about a citation that records what \
         was read and not which state it was read from. The count is the thing to watch.",
        violations,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::Overlay;
    use crate::walk::walk_corpus_instances;
    use std::path::Path;

    /// The node every citation below rests on: three tagged claims at three standings, in one
    /// wrapped block scalar — the shape a corpus actually writes.
    const CITED: &str = "class: reach\nlabel: Tailwater\ndescription: |\n  \
                         The gauge sits at the riffle and is read weekly by the\n  \
                         district [verified]. Discharge below the dam tracks the\n  \
                         release schedule within a day [inference]. Whether the\n  \
                         2019 avulsion moved the control is [open].\n";

    /// A corpus of two nodes: the one doing the citing, and [`CITED`].
    fn fixture(cites: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let corpus = dir.path().join(".yidam/corpus/reach");
        std::fs::create_dir_all(&corpus).unwrap();
        std::fs::write(
            corpus.join("headwater.yml"),
            format!("class: reach\nlabel: Headwater\n{cites}"),
        )
        .unwrap();
        std::fs::write(corpus.join("tailwater.yml"), CITED).unwrap();
        dir
    }

    fn nodes(root: &Path) -> Vec<Node> {
        crate::corpus::load_nodes(
            root,
            &walk_corpus_instances(&crate::paths::yidam_corpus_dir(root)),
            &Overlay::default(),
        )
    }

    fn run(dir: &tempfile::TempDir) -> BTreeMap<&'static str, Check> {
        let fields = crate::claims::ClaimFields::default();
        let [unresolved, drift, tag, untagged] = checks(&nodes(dir.path()), &fields);
        [
            ("unresolved", unresolved),
            ("drift", drift),
            ("tag", tag),
            ("untagged", untagged),
        ]
        .into_iter()
        .collect()
    }

    /// The only violation any check raised, with the check that raised it.
    fn only(dir: &tempfile::TempDir) -> (&'static str, String) {
        let all = run(dir);
        let firing: Vec<(&'static str, String)> = all
            .iter()
            .flat_map(|(name, c)| c.violations.iter().map(move |v| (*name, v.detail.clone())))
            .collect();
        assert_eq!(
            firing.len(),
            1,
            "expected exactly one finding, got {firing:?}"
        );
        firing.into_iter().next().unwrap()
    }

    /// A local citation that resolves, whose span is there and whose tag agrees.
    #[test]
    fn a_local_citation_that_holds_reports_nothing() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: verified\n    span: >-\n      \
             The gauge sits at the riffle and is read weekly by the district\n",
        );
        for (name, check) in run(&dir) {
            assert!(check.passed(), "{name} fired: {:?}", check.violations);
        }
    }

    /// The cited node's prose is hard-wrapped in a block scalar and the span crosses one of
    /// its line breaks. Comparing bytes fails this; the check must not.
    ///
    /// This is the half of the wrapping problem that is actually this module's. A span written
    /// `span: >-` is *folded by the YAML parser* before anything here sees it, so a test using
    /// that spelling passes whether or not the span is normalized at all — which is what a
    /// mutation found, having survived the removal of `flatten(span)` untouched.
    #[test]
    fn a_span_matches_across_the_cited_nodes_line_wrapping() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: inference\n    span: >-\n      \
             Discharge below the dam tracks the release schedule within a day\n",
        );
        assert!(run(&dir)["drift"].passed());
        assert!(run(&dir)["tag"].passed());
    }

    /// The other half, and the only spelling that exercises it: a **literal** block scalar
    /// keeps its newlines, so the span arrives here still wrapped and is normalized here or
    /// not at all.
    #[test]
    fn a_span_written_as_a_literal_scalar_is_normalized_here() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: inference\n    span: |-\n      \
             Discharge below the dam\n      tracks the release\n      schedule within a day\n",
        );
        assert!(
            run(&dir)["drift"].passed(),
            "{:?}",
            run(&dir)["drift"].violations
        );
        assert!(run(&dir)["tag"].passed());
    }

    /// `.yml` is how every markdown link in a corpus spells a node, and it names the same node.
    #[test]
    fn a_node_written_with_its_extension_resolves() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater.yml\n    tag: verified\n    span: \
             \"read weekly by the district\"\n",
        );
        for (name, check) in run(&dir) {
            assert!(check.passed(), "{name} fired: {:?}", check.violations);
        }
    }

    /// A node that is not there.
    #[test]
    fn a_citation_naming_no_such_node_is_unresolved() {
        let dir = fixture("cites:\n  - node: reach/gone\n    span: x\n");
        let (check, detail) = only(&dir);
        assert_eq!(check, "unresolved");
        assert!(detail.contains("no such node"), "{detail}");
    }

    /// Neither a package nor a node: the one population that moved here from the external
    /// check, and it must still say which field is missing.
    #[test]
    fn a_citation_naming_neither_a_package_nor_a_node_says_so() {
        let dir = fixture("cites:\n  - span: x\n");
        let (check, detail) = only(&dir);
        assert_eq!(check, "unresolved");
        assert!(detail.contains("names no `node:`"), "{detail}");
        assert!(detail.contains("`package:`"), "{detail}");
    }

    /// A node reference with no span rots invisibly, and that is the finding.
    #[test]
    fn a_citation_with_no_span_is_drift() {
        let dir = fixture("cites:\n  - node: reach/tailwater\n");
        let (check, detail) = only(&dir);
        assert_eq!(check, "drift");
        assert!(detail.contains("no `span:`"), "{detail}");
    }

    /// A reworded span, and the message that says not to re-quote it.
    #[test]
    fn a_reworded_span_is_drift_and_says_not_to_requote() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    span: \"read fortnightly by the district\"\n",
        );
        let (check, detail) = only(&dir);
        assert_eq!(check, "drift");
        assert!(detail.contains("even ignoring line wrapping"), "{detail}");
        assert!(detail.contains("do not re-quote it"), "{detail}");
    }

    /// The check with no external analogue: the span is there and this corpus holds it at a
    /// standing the citation does not claim.
    #[test]
    fn a_tag_the_corpus_does_not_hold_is_reported_with_both_standings() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: verified\n    span: >-\n      \
             Discharge below the dam tracks the release schedule within a day\n",
        );
        let (check, detail) = only(&dir);
        assert_eq!(check, "tag");
        assert!(detail.contains("at [verified]"), "{detail}");
        assert!(detail.contains("at [inference]"), "{detail}");
    }

    /// Citing an `[open]` span as `[verified]` is the same check and the standing it names is
    /// the one that matters most: the corpus said it does not know.
    #[test]
    fn citing_an_open_span_at_a_stronger_standing_is_reported() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: verified\n    span: >-\n      \
             Whether the 2019 avulsion moved the control\n",
        );
        let (check, detail) = only(&dir);
        assert_eq!(check, "tag");
        assert!(detail.contains("at [open]"), "{detail}");
    }

    /// An `[open]` span cited honestly at `[open]` is not a finding. The corpus is allowed to
    /// rest on its own open questions as long as it says that is what it is doing.
    #[test]
    fn an_open_span_cited_at_open_holds() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: open\n    span: >-\n      \
             Whether the 2019 avulsion moved the control\n",
        );
        for (name, check) in run(&dir) {
            assert!(check.passed(), "{name} fired: {:?}", check.violations);
        }
    }

    /// A span in the node that no tagged claim covers: the text is there and nothing licenses
    /// the standing declared over it.
    #[test]
    fn a_span_no_claim_governs_licenses_nothing() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: verified\n    span: \"label: Tailwater\"\n",
        );
        let (check, detail) = only(&dir);
        assert_eq!(check, "tag");
        assert!(detail.contains("nothing licenses"), "{detail}");
    }

    /// A tag that is not one of the three is a finding rather than an absence — a person who
    /// wrote `tag: probable` plainly meant to declare a standing.
    #[test]
    fn a_tag_outside_the_vocabulary_is_reported_as_one() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    tag: probable\n    span: \
             \"read weekly by the district\"\n",
        );
        let (check, detail) = only(&dir);
        assert_eq!(check, "tag");
        assert!(detail.contains("is not a standing"), "{detail}");
    }

    /// No tag at all is Info and says what is not checked, rather than an error.
    #[test]
    fn a_citation_with_no_tag_is_info_and_never_gates() {
        let dir = fixture(
            "cites:\n  - node: reach/tailwater\n    span: \"read weekly by the district\"\n",
        );
        let all = run(&dir);
        let untagged = &all["untagged"];
        assert_eq!(untagged.severity, Severity::Info);
        assert_eq!(untagged.violations.len(), 1);
        assert!(all["tag"].passed());
    }

    /// The whole reason this is safe to ship at Error: a corpus that writes no local citation
    /// cannot be failed by any of the four.
    #[test]
    fn a_corpus_with_no_local_citations_reports_nothing() {
        let dir = fixture("");
        for (name, check) in run(&dir) {
            assert!(check.passed(), "{name} fired: {:?}", check.violations);
        }
    }

    /// The partition: a citation naming a package is the external module's and is not read
    /// twice.
    #[test]
    fn a_citation_naming_a_package_is_not_a_local_one() {
        let dir = fixture("cites:\n  - package: upstream\n    node: concept/gone\n    span: x\n");
        for (name, check) in run(&dir) {
            assert!(check.passed(), "{name} fired: {:?}", check.violations);
        }
    }

    /// A class called `corpus` is legal, and a citation of one of its nodes must resolve.
    ///
    /// The key used to be taken from the **last** `corpus/` in the path, which keys
    /// `.yidam/corpus/corpus/x.yml` as `x` and answers a citation of `corpus/x` with *this
    /// corpus has no such node* — against a file sitting right there.
    #[test]
    fn a_class_named_corpus_is_keyed_by_its_directory_and_not_by_the_last_match() {
        let dir = fixture(
            "cites:\n  - node: corpus/method\n    span: \"a sentence in the method node\"\n",
        );
        let nested = dir.path().join(".yidam/corpus/corpus");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(
            nested.join("method.yml"),
            "class: corpus\nlabel: Method\ndescription: |\n  a sentence in the method node\n",
        )
        .unwrap();
        // `unresolved` is the one that answers the question: a mis-keyed corpus reports the
        // node as absent. `untagged` fires and is supposed to — the citation declares no
        // standing — and asserting it silent here would be asserting the wrong thing.
        let all = run(&dir);
        assert!(
            all["unresolved"].passed(),
            "{:?}",
            all["unresolved"].violations
        );
        assert!(all["drift"].passed(), "{:?}", all["drift"].violations);
        assert_eq!(all["untagged"].violations.len(), 1);
    }

    /// Two citations in one node are two findings, filed against that node.
    #[test]
    fn each_citation_is_judged_on_its_own() {
        let dir = fixture(
            "cites:\n  - node: reach/gone\n    span: x\n  - node: reach/tailwater\n    \
             span: \"reworded beyond recognition\"\n",
        );
        let all = run(&dir);
        assert_eq!(all["unresolved"].violations.len(), 1);
        assert_eq!(all["drift"].violations.len(), 1);
        assert_eq!(
            all["drift"].violations[0].node,
            ".yidam/corpus/reach/headwater.yml"
        );
    }
}
