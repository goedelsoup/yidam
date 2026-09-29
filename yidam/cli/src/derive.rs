//! A derivation checked against the corpus it rests on — RFC-0045.
//!
//! # What this runs
//!
//! `agent-conduct.md` states three rules for a claim that leaves the repository: cite a span and
//! not a node, compute the tier as the weakest standing beneath it, and do not route around a
//! refusal in the cited block. Four derived repositories each built a gate for them, and all four
//! agreed on the first two. This module is those two, run from the template, and the third made
//! decidable by a declaration.
//!
//! An **artifact** is a `.yml` file, or a `.md` file with YAML frontmatter, under a `[derive]
//! paths` glob. It reads four keys — `claim`, `reach`, `cites` and `answers` — and ignores every
//! other, because the rest of the document is the repository's own.
//!
//! # The citation is RFC-0034's, called and not copied
//!
//! A `cites:` entry here is the package-less entry a node writes, and
//! [`crate::cmd::lint::local_citations::findings`] is its predicate. Resolution, the span check
//! and the tag check are that function's, reached from a second caller, so a citation the gate
//! accepts in a node is accepted in an artifact on the same terms and in the same words. What is
//! added here is what a node's citation never needed: the **paragraph** the span sits in.
//!
//! # A paragraph is a blank-line block of one string
//!
//! Every string the node holds except its `refuses:` and `cites:` entries, split on a blank line.
//! That is the split corpus A and `allen-county-ohio` already make, and the one RFC-0045's
//! measurement used: 0 of 1,552 citations cross one. `refuses:` is excluded because a refusal's
//! own declaration quotes it, and a span found only there would be found in the declaration
//! rather than the prose. `cites:` is excluded because its spans are another node's words.
//!
//! # A refusal is declared, and only a declaration gates
//!
//! A refusal in a cited paragraph must be answered by the artifact. The refusals are the ones the
//! node lists under `refuses:`, and nothing here guesses at one: measured against 62 human
//! rulings, the best lexicon found 12 of 33 and the widest flagged 22 paragraphs that were not
//! refusals. The widest is kept as [`proposes_refusal`], and it reports at Info over cited
//! paragraphs only. It never changes the exit code.

use std::collections::BTreeSet;
use std::path::Path;

use serde::Serialize;

use crate::claims::standing_rank;
use crate::cmd::lint::citations::{flatten, truncate};
use crate::cmd::lint::local_citations::{self, key_of};
use crate::cmd::lint::Severity;
use crate::corpus::Node;
use crate::parse::ExternalCitation;

/// The finding ids, named once. RFC-0045 §3.5's table, in its order.
pub const UNRESOLVED: &str = "derive-unresolved";
pub const SPAN_DRIFT: &str = "derive-span-drift";
pub const CROSSES_PARAGRAPH: &str = "derive-span-crosses-paragraph";
pub const TAG_DRIFT: &str = "derive-tag-drift";
pub const BEYOND_REACH: &str = "derive-beyond-reach";
pub const UNANSWERED_REFUSAL: &str = "derive-unanswered-refusal";
pub const STALE_ANSWER: &str = "derive-stale-answer";
pub const NO_REACH: &str = "derive-no-reach";
pub const REFUSAL_CANDIDATE: &str = "derive-refusal-candidate";

/// The three reaches and the weakest standing each admits, from `agent-conduct.md`'s own
/// sentences: `[verified]` may reach public material, `[inference]` reaches attributed memos and
/// backgrounders, and `[open]` does not leave the repository.
pub const REACHES: [(&str, &str); 3] = [
    ("public", "verified"),
    ("attributed", "inference"),
    ("internal", "open"),
];

// ── a node's prose, as paragraphs ─────────────────────────────────────────────

/// Keys whose strings are not this node's prose. See the module note.
const NOT_PROSE: [&str; 2] = ["refuses", "cites"];

/// One string the node holds, whole and split into its blank-line paragraphs.
pub(crate) struct Field {
    pub whole: String,
    pub paragraphs: Vec<String>,
}

/// Every prose string in a node, in document order.
///
/// Any string, not only `description`: corpus C cites a property in 65 of 84 citations and
/// corpus B cites a top-level `source_note`, and a rule that read one field would fail 107
/// citations that are correct. A node that does not parse has no prose to find anything in.
pub(crate) fn prose_fields(text: &str) -> Vec<Field> {
    fn walk(v: &serde_yaml::Value, out: &mut Vec<Field>) {
        match v {
            serde_yaml::Value::String(s) => out.push(Field {
                whole: s.clone(),
                paragraphs: s
                    .split("\n\n")
                    .filter(|p| !p.trim().is_empty())
                    .map(str::to_string)
                    .collect(),
            }),
            serde_yaml::Value::Sequence(items) => items.iter().for_each(|i| walk(i, out)),
            serde_yaml::Value::Mapping(m) => m.values().for_each(|i| walk(i, out)),
            serde_yaml::Value::Tagged(t) => walk(&t.value, out),
            _ => {}
        }
    }
    let Ok(serde_yaml::Value::Mapping(top)) = serde_yaml::from_str::<serde_yaml::Value>(text)
    else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for (k, v) in &top {
        if k.as_str().is_some_and(|k| NOT_PROSE.contains(&k)) {
            continue;
        }
        walk(v, &mut out);
    }
    out
}

/// Whether `span` is in the node's prose at all, whitespace collapsed on both sides.
///
/// `refusal-span-drift`'s predicate. Whole fields and not paragraphs, because what that check
/// asks is whether the declaration still describes text that exists; where the text sits is
/// `derive check`'s question.
pub(crate) fn in_prose(fields: &[Field], span: &str) -> bool {
    let needle = flatten(span);
    !needle.is_empty() && fields.iter().any(|f| flatten(&f.whole).contains(&needle))
}

/// The paragraphs `span` sits inside, flattened.
fn paragraphs_holding(fields: &[Field], span: &str) -> Vec<String> {
    let needle = flatten(span);
    fields
        .iter()
        .flat_map(|f| f.paragraphs.iter())
        .map(|p| flatten(p))
        .filter(|p| p.contains(&needle))
        .collect()
}

// ── the proposer ──────────────────────────────────────────────────────────────

/// Corpus A's negation list, as its wide detector reads it. Substrings, not words: that is how
/// the detector was measured, at 0.970 recall and 0.593 precision against its own rulings.
const NEGATIONS: [&str; 43] = [
    "does not",
    "do not",
    "did not",
    "cannot",
    "can not",
    "is not",
    "are not",
    "was not",
    "were not",
    "no retrieved",
    "nothing here",
    "nothing retrieved",
    "nothing in",
    "no claim",
    "not claimed",
    "says nothing",
    "say nothing",
    "no source",
    "never",
    "declines",
    "declined",
    "refuses",
    "refused",
    "none",
    "nothing",
    "nowhere",
    "no document",
    "unretrieved",
    "not retrieved",
    "nor can",
    "nor does",
    "no route",
    "no official",
    "no recorded",
    "no baseline",
    "not been",
    "not found",
    "asserts nothing",
    "shows nothing",
    "nothing about",
    "will not",
    "no filed",
    "not established",
];

/// Whether a paragraph looks like it refuses something: it carries `[inference]` or `[open]`, and
/// a negation.
///
/// **This proposes and never gates.** Its precision is why: four in ten of what it flags are not
/// refusals. Its recall is why it is kept at all — an advisory that misses a refusal is the silent
/// failure the rule exists to prevent — and it runs only over paragraphs an artifact cites, which
/// bounds the noise to the text an argument actually rests on.
pub(crate) fn proposes_refusal(paragraph: &str) -> bool {
    let low = paragraph.to_lowercase();
    (low.contains("[inference]") || low.contains("[open]"))
        && NEGATIONS.iter().any(|n| low.contains(n))
}

// ── artifacts ─────────────────────────────────────────────────────────────────

/// One `answers:` entry: the refusal it answers, by node and text, and the answer.
#[derive(Debug, Default, serde::Deserialize)]
struct Answer {
    node: Option<String>,
    refusal: Option<String>,
    answer: Option<String>,
}

/// The four keys an artifact is read for. Every other key is the repository's.
#[derive(Debug, Default, serde::Deserialize)]
struct Artifact {
    claim: Option<String>,
    reach: Option<String>,
    #[serde(default)]
    cites: Option<Vec<ExternalCitation>>,
    #[serde(default)]
    answers: Option<Vec<Answer>>,
}

/// The YAML an artifact file holds: the whole of a `.yml`, the frontmatter of a `.md`.
///
/// `None` for a `.md` with no frontmatter. That is not an artifact — a README beside a dossier is
/// under the same glob and says nothing to check — and skipping it is not a finding.
fn artifact_yaml(rel: &str, text: &str) -> Option<String> {
    if rel.ends_with(".md") {
        let body = text
            .strip_prefix("---\n")
            .or_else(|| text.strip_prefix("---\r\n"))?;
        let end = body.find("\n---").map(|i| i + 1)?;
        return Some(body[..end].to_string());
    }
    Some(text.to_string())
}

/// Every artifact file under the declared globs, repository-relative and sorted.
///
/// Walked from each glob's literal prefix rather than from the root, so a repository with a large
/// build directory beside its dossier does not pay for walking it. Working-tree files, not tracked
/// ones: a gate run before a commit has to see the artifact the commit adds.
pub(crate) fn artifact_files(root: &Path, globs: &[String]) -> Vec<String> {
    let mut found = BTreeSet::new();
    for glob in globs {
        let prefix: Vec<&str> = glob.split('/').take_while(|s| !s.contains('*')).collect();
        let base = root.join(prefix.join("/"));
        if base.is_file() {
            found.insert(prefix.join("/"));
            continue;
        }
        for entry in walkdir::WalkDir::new(&base)
            .into_iter()
            .filter_entry(|e| e.file_name() != ".git")
            .filter_map(Result::ok)
            .filter(|e| e.file_type().is_file())
        {
            let Ok(rel) = entry.path().strip_prefix(root) else {
                continue;
            };
            let rel = rel.to_string_lossy().replace('\\', "/");
            let artifact = [".yml", ".yaml", ".md"].iter().any(|x| rel.ends_with(x));
            if artifact && crate::kuten::glob_covers(glob, &rel) {
                found.insert(rel);
            }
        }
    }
    found.into_iter().collect()
}

// ── the report ────────────────────────────────────────────────────────────────

/// One thing `derive check` says about one artifact.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub severity: &'static str,
    /// The node the finding is about, where it is about one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<String>,
    pub message: String,
}

/// One `cites:` entry as the check read it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CitedSpan {
    pub node: Option<String>,
    pub span: Option<String>,
    /// The weakest claim overlapping the span, or `open` where none does. `None` when the span
    /// could not be located, so no standing was read.
    pub standing: Option<&'static str>,
}

/// One artifact, its computed tier, and what was found.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Derivation {
    pub path: String,
    pub claim: Option<String>,
    pub reach: Option<String>,
    /// The weakest standing over every located span. `open` for an artifact that cites nothing,
    /// because nothing beneath it licenses more. `None` when no span could be located.
    pub tier: Option<&'static str>,
    pub spans: Vec<CitedSpan>,
    pub findings: Vec<Finding>,
}

impl Derivation {
    pub fn errors(&self) -> usize {
        self.findings
            .iter()
            .filter(|f| f.severity == Severity::Error.as_str())
            .count()
    }
}

fn finding(
    check: &'static str,
    severity: Severity,
    node: Option<&str>,
    message: String,
) -> Finding {
    Finding {
        check,
        severity: severity.as_str(),
        node: node.map(str::to_string),
        message,
    }
}

/// The corpus as the check reads it: each node's bytes, its prose, and its declared refusals.
pub(crate) struct Cited<'a> {
    by_key: local_citations::Corpus,
    nodes: std::collections::BTreeMap<String, &'a Node>,
    fields: &'a crate::claims::ClaimFields,
}

impl<'a> Cited<'a> {
    pub(crate) fn new(nodes: &'a [Node], fields: &'a crate::claims::ClaimFields) -> Self {
        let by_key = local_citations::corpus(nodes);
        let nodes = nodes
            .iter()
            .filter_map(|n| {
                let rel = n.rel.replace('\\', "/");
                let key = rel
                    .strip_prefix(".yidam/corpus/")?
                    .strip_suffix(".yml")?
                    .to_string();
                Some((key, n))
            })
            .collect();
        Self {
            by_key,
            nodes,
            fields,
        }
    }
}

/// Check one artifact's bytes against the corpus.
pub(crate) fn check_artifact(rel: &str, text: &str, cited: &Cited) -> Option<Derivation> {
    let yaml = artifact_yaml(rel, text)?;
    let mut findings = Vec::new();
    let artifact: Artifact = match serde_yaml::from_str::<serde_yaml::Value>(&yaml) {
        Ok(serde_yaml::Value::Mapping(m)) => {
            match serde_yaml::from_value(serde_yaml::Value::Mapping(m)) {
                Ok(a) => a,
                Err(e) => {
                    findings.push(finding(
                        NO_REACH,
                        Severity::Error,
                        None,
                        format!(
                            "does not read as an artifact ({e}) — so it declares no reach and \
                             cites nothing this check can hold it to"
                        ),
                    ));
                    Artifact::default()
                }
            }
        }
        Ok(_) | Err(_) => {
            findings.push(finding(
                NO_REACH,
                Severity::Error,
                None,
                "is not a YAML mapping — so it declares no reach and cites nothing this check \
                 can hold it to"
                    .to_string(),
            ));
            Artifact::default()
        }
    };

    // ── reach ────────────────────────────────────────────────────────────────
    let reach = artifact.reach.as_deref().map(str::trim);
    let admitted = reach.and_then(|r| REACHES.iter().find(|(name, _)| *name == r));
    if findings.is_empty() && admitted.is_none() {
        let message = match reach {
            None => "declares no `reach:` — say where this claim is going (public, attributed \
                     or internal), and the check will say whether the evidence lets it"
                .to_string(),
            Some(r) => format!(
                "declares `reach: {r}`, which is not a reach — expected public, attributed or \
                 internal"
            ),
        };
        findings.push(finding(NO_REACH, Severity::Error, None, message));
    }

    // ── each citation ────────────────────────────────────────────────────────
    let cites = artifact.cites.unwrap_or_default();
    let mut spans = Vec::new();
    // (node key, flattened refusal) → the refusal as written, owed an answer.
    let mut owed: Vec<(String, String, String)> = Vec::new();
    // (node key, flattened refusal) the artifact quotes itself, and so rests on.
    let mut carried: BTreeSet<(String, String)> = BTreeSet::new();
    let mut candidates: BTreeSet<(String, String)> = BTreeSet::new();

    for cite in &cites {
        let key = cite.node.as_deref().map(key_of).filter(|k| !k.is_empty());
        let class = key
            .as_deref()
            .and_then(|k| cited.nodes.get(k))
            .map(|n| crate::paths::class_of_path(&n.path))
            .unwrap_or_default();
        let mut located = true;
        for f in local_citations::findings(cite, &cited.by_key, cited.fields.for_class(&class)) {
            let id = match f.check {
                local_citations::UNRESOLVED => UNRESOLVED,
                local_citations::SPAN_DRIFT => SPAN_DRIFT,
                local_citations::TAG_DRIFT => TAG_DRIFT,
                // No `tag:` is Info in a node and nothing here: the standing is computed, and
                // an artifact that declared one would only be declaring a copy of it.
                _ => continue,
            };
            if id != TAG_DRIFT {
                located = false;
            }
            findings.push(finding(id, f.severity, key.as_deref(), f.message));
        }
        let span = cite.span.as_deref().map(str::trim).unwrap_or_default();
        let mut entry = CitedSpan {
            node: key.clone(),
            span: cite.span.clone(),
            standing: None,
        };
        let (Some(key), Some(node), true) = (
            key.as_deref(),
            key.as_deref().and_then(|k| cited.nodes.get(k)),
            located,
        ) else {
            spans.push(entry);
            continue;
        };

        let fields = prose_fields(&node.text);
        let paragraphs = paragraphs_holding(&fields, span);
        if paragraphs.is_empty() {
            let (id, message) = if in_prose(&fields, span) {
                (
                    CROSSES_PARAGRAPH,
                    format!(
                        "cites `{key}` for a span that runs across a blank line — a span in no \
                         single paragraph has no single standing and no single refusal scope. \
                         Cite each paragraph separately. Span: {}",
                        truncate(span)
                    ),
                )
            } else {
                (
                    SPAN_DRIFT,
                    format!(
                        "cites `{key}` for text that is in the file and in none of its prose — \
                         a span quoted from a `refuses:` or `cites:` entry is the node's \
                         declaration, not its words. Span: {}",
                        truncate(span)
                    ),
                )
            };
            findings.push(finding(id, Severity::Error, Some(key), message));
            spans.push(entry);
            continue;
        }

        let standing = local_citations::governing(&node.text, span, cited.fields.for_class(&class))
            .unwrap_or("open");
        entry.standing = Some(standing);
        spans.push(entry);

        // ── the refusals in the paragraphs this span sits in ──
        let refusals: Vec<&str> = node
            .inst
            .refuses
            .as_deref()
            .unwrap_or_default()
            .iter()
            .filter_map(|r| r.span.as_deref())
            .filter(|s| !s.trim().is_empty())
            .collect();
        let quoted = flatten(span);
        for paragraph in &paragraphs {
            let mut declared_here = false;
            for refusal in &refusals {
                let flat = flatten(refusal);
                if !paragraph.contains(&flat) {
                    continue;
                }
                declared_here = true;
                let id = (key.to_string(), flat.clone());
                if quoted.contains(&flat) {
                    carried.insert(id);
                } else if !owed.iter().any(|(k, f, _)| (k, f) == (&id.0, &id.1)) {
                    owed.push((id.0, id.1, refusal.trim().to_string()));
                }
            }
            if !declared_here && proposes_refusal(paragraph) {
                candidates.insert((key.to_string(), paragraph.clone()));
            }
        }
    }

    // ── answers ──────────────────────────────────────────────────────────────
    let answers = artifact.answers.unwrap_or_default();
    let answered = |key: &str, flat: &str| {
        answers.iter().any(|a| {
            a.node.as_deref().map(key_of).as_deref() == Some(key)
                && a.refusal.as_deref().map(flatten).as_deref() == Some(flat)
                && a.answer.as_deref().is_some_and(|t| !t.trim().is_empty())
        })
    };
    for (key, flat, written) in &owed {
        if carried.contains(&(key.clone(), flat.clone())) || answered(key, flat) {
            continue;
        }
        findings.push(finding(
            UNANSWERED_REFUSAL,
            Severity::Error,
            Some(key),
            format!(
                "rests on a paragraph of `{key}` that declares a refusal, and does not answer \
                 it — add an `answers:` entry naming this node and this refusal, and say why the \
                 claim survives it. Refusal: {}",
                truncate(written)
            ),
        ));
    }
    for a in &answers {
        let key = a.node.as_deref().map(key_of).unwrap_or_default();
        let flat = a.refusal.as_deref().map(flatten).unwrap_or_default();
        let declared = owed.iter().any(|(k, f, _)| (k, f) == (&key, &flat))
            || carried.contains(&(key.clone(), flat.clone()));
        if !declared {
            findings.push(finding(
                STALE_ANSWER,
                Severity::Error,
                (!key.is_empty()).then_some(key.as_str()),
                format!(
                    "answers a refusal of `{key}` that no cited paragraph declares — the refusal \
                     was reworded or withdrawn, or the citation moved, and this answer is now \
                     about text that is gone. Refusal: {}",
                    truncate(&flat)
                ),
            ));
        }
    }

    // ── tier and reach ───────────────────────────────────────────────────────
    let standings: Vec<&'static str> = spans.iter().filter_map(|s| s.standing).collect();
    let tier = if cites.is_empty() {
        Some("open")
    } else {
        standings.iter().copied().min_by_key(|s| standing_rank(s))
    };
    if let (Some(tier), Some((name, weakest))) = (tier, admitted) {
        if standing_rank(tier) < standing_rank(weakest) {
            let why = if cites.is_empty() {
                "cites nothing, so nothing beneath it licenses more than [open]".to_string()
            } else {
                format!("rests on a span this corpus holds at [{tier}]")
            };
            findings.push(finding(
                BEYOND_REACH,
                Severity::Error,
                None,
                format!(
                    "declares `reach: {name}`, which admits [{weakest}] and stronger, and {why} \
                     — narrow the reach, or cite evidence that carries it"
                ),
            ));
        }
    }

    for (key, paragraph) in &candidates {
        findings.push(finding(
            REFUSAL_CANDIDATE,
            Severity::Info,
            Some(key),
            format!(
                "rests on a paragraph of `{key}` that reads like a refusal and declares none — \
                 if it refuses an inference, declare it under that node's `refuses:`; if not, \
                 nothing is owed. Paragraph: {}",
                truncate(paragraph)
            ),
        ));
    }

    Some(Derivation {
        path: rel.to_string(),
        claim: artifact.claim.map(|c| flatten(&c)),
        reach: reach.map(str::to_string),
        tier,
        spans,
        findings,
    })
}

/// Every artifact under `globs`, checked against the corpus at `root`.
pub(crate) fn check(
    root: &Path,
    globs: &[String],
    corpus: &crate::corpus::Corpus,
) -> Vec<Derivation> {
    let fields = crate::claims::ClaimFields::from_classes(corpus.classes());
    let cited = Cited::new(corpus.nodes(), &fields);
    artifact_files(root, globs)
        .into_iter()
        .filter_map(|rel| {
            let text = std::fs::read_to_string(root.join(&rel)).ok()?;
            check_artifact(&rel, &text, &cited)
        })
        .collect()
}

/// The weakest standing a reach admits — for a caller rendering the table.
#[must_use]
pub fn admitted_by(reach: &str) -> Option<&'static str> {
    REACHES
        .iter()
        .find(|(name, _)| *name == reach)
        .map(|(_, w)| *w)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::claims::ClaimFields;

    /// Every finding id, in table order — the roster a test holds the documentation to.
    const FINDINGS: [&str; 9] = [
        UNRESOLVED,
        SPAN_DRIFT,
        CROSSES_PARAGRAPH,
        TAG_DRIFT,
        BEYOND_REACH,
        UNANSWERED_REFUSAL,
        STALE_ANSWER,
        NO_REACH,
        REFUSAL_CANDIDATE,
    ];

    /// The reference documents every finding this command can report, by its id.
    #[test]
    fn every_finding_is_documented() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for doc in [
            "docs/cli-reference.md",
            "docs/rfcs/0045-derivation-check.md",
        ] {
            let text = std::fs::read_to_string(root.join(doc)).unwrap();
            for id in FINDINGS {
                assert!(
                    text.contains(&format!("`{id}`")),
                    "{doc} does not name `{id}`"
                );
            }
        }
    }

    /// Two paragraphs at two standings, the second declaring a refusal.
    const CITED: &str = "class: reach\nlabel: Tailwater\ndescription: |\n  \
                         The gauge sits at the riffle and is read weekly by the\n  \
                         district [verified].\n\n  \
                         Discharge tracks the release schedule [inference]. The\n  \
                         record does not say the dam caused the 2019 avulsion.\n\
                         refuses:\n  \
                         - span: The record does not say the dam caused the 2019 avulsion.\n    \
                         inference: the dam caused the 2019 avulsion\n";

    /// One paragraph that reads like a refusal and declares none.
    const UNDECLARED: &str = "class: reach\nlabel: Headwater\ndescription: |\n  \
                              Whether the spring feeds the reach is [open]. No source says.\n  \
                              The survey does not reach it.\n";

    fn nodes() -> Vec<Node> {
        [("tailwater", CITED), ("headwater", UNDECLARED)]
            .into_iter()
            .map(|(name, text)| {
                let rel = format!(".yidam/corpus/reach/{name}.yml");
                Node::parse(Path::new("/r").join(&rel), rel, text)
            })
            .collect()
    }

    fn check(artifact: &str) -> Derivation {
        let nodes = nodes();
        let fields = ClaimFields::default();
        let cited = Cited::new(&nodes, &fields);
        check_artifact("dossier/a.yml", artifact, &cited).expect("a .yml is an artifact")
    }

    fn ids(d: &Derivation) -> Vec<&'static str> {
        d.findings.iter().map(|f| f.check).collect()
    }

    const VERIFIED: &str =
        "cites:\n  - node: reach/tailwater\n    span: read weekly by the district\n";
    const INFERENCE: &str =
        "cites:\n  - node: reach/tailwater\n    span: Discharge tracks the release schedule\n";

    #[test]
    fn a_verified_span_carries_a_public_claim() {
        let d = check(&format!("claim: Weekly.\nreach: public\n{VERIFIED}"));
        assert_eq!(d.tier, Some("verified"));
        assert!(d.findings.is_empty(), "{:?}", d.findings);
    }

    #[test]
    fn the_tier_is_the_weakest_span_and_the_reach_must_admit_it() {
        let both = format!(
            "reach: public\n{VERIFIED}  - node: reach/tailwater\n    span: Discharge tracks the release schedule\n"
        );
        let d = check(&both);
        assert_eq!(d.tier, Some("inference"));
        assert!(ids(&d).contains(&BEYOND_REACH), "{:?}", d.findings);
    }

    #[test]
    fn a_refusal_in_a_cited_paragraph_is_owed_an_answer() {
        let d = check(&format!("reach: attributed\n{INFERENCE}"));
        assert_eq!(ids(&d), [UNANSWERED_REFUSAL], "{:?}", d.findings);

        let answered = format!(
            "reach: attributed\n{INFERENCE}answers:\n  - node: reach/tailwater.yml\n    \
             refusal: >-\n      The record does not say the dam\n      caused the 2019 avulsion.\n    \
             answer: The memo claims only the schedule, not the cause.\n"
        );
        let d = check(&answered);
        assert!(d.findings.is_empty(), "{:?}", d.findings);
    }

    #[test]
    fn an_empty_answer_answers_nothing() {
        let d = check(&format!(
            "reach: attributed\n{INFERENCE}answers:\n  - node: reach/tailwater\n    \
             refusal: The record does not say the dam caused the 2019 avulsion.\n    answer: \"\"\n"
        ));
        assert_eq!(ids(&d), [UNANSWERED_REFUSAL], "{:?}", d.findings);
    }

    /// Quoting the refusal is resting on it, not routing around it.
    #[test]
    fn a_span_that_quotes_the_refusal_owes_nothing() {
        let d = check(
            "reach: internal\ncites:\n  - node: reach/tailwater\n    \
             span: The record does not say the dam caused the 2019 avulsion.\n",
        );
        assert!(d.findings.is_empty(), "{:?}", d.findings);
        // An untagged sentence stands at [open], refusal or not.
        assert_eq!(d.tier, Some("open"));
    }

    #[test]
    fn a_refusal_in_an_uncited_paragraph_owes_nothing_and_its_answer_is_stale() {
        let d = check(&format!(
            "reach: public\n{VERIFIED}answers:\n  - node: reach/tailwater\n    \
             refusal: The record does not say the dam caused the 2019 avulsion.\n    answer: x\n"
        ));
        assert_eq!(ids(&d), [STALE_ANSWER], "{:?}", d.findings);
    }

    #[test]
    fn a_span_across_a_blank_line_is_in_no_paragraph() {
        let d = check(
            "reach: internal\ncites:\n  - node: reach/tailwater\n    \
             span: district [verified]. Discharge tracks\n",
        );
        assert_eq!(ids(&d), [CROSSES_PARAGRAPH], "{:?}", d.findings);
        assert_eq!(d.tier, None);
    }

    /// The node quotes its refusal under `refuses:`; that is a declaration, not prose.
    #[test]
    fn a_span_found_only_in_the_declaration_is_drift() {
        let d = check(
            "reach: internal\ncites:\n  - node: reach/tailwater\n    \
             span: the dam caused the 2019 avulsion\n",
        );
        // The prose has this text too, so it is found — and, quoting only part of the refusal,
        // owes it an answer.
        assert_eq!(ids(&d), [UNANSWERED_REFUSAL], "{:?}", d.findings);
        let d = check(
            "reach: internal\ncites:\n  - node: reach/tailwater\n    \
             span: \"inference: the dam\"\n",
        );
        assert_eq!(ids(&d), [SPAN_DRIFT], "{:?}", d.findings);
    }

    #[test]
    fn the_citation_findings_are_the_gates_under_this_commands_ids() {
        let d = check("reach: internal\ncites:\n  - node: reach/nowhere\n    span: x\n");
        assert_eq!(ids(&d), [UNRESOLVED], "{:?}", d.findings);
        let d =
            check("reach: internal\ncites:\n  - node: reach/tailwater\n    span: read monthly\n");
        assert_eq!(ids(&d), [SPAN_DRIFT], "{:?}", d.findings);
        let d = check(&format!("reach: public\n{VERIFIED}    tag: open\n"));
        assert_eq!(ids(&d), [TAG_DRIFT], "{:?}", d.findings);
    }

    #[test]
    fn no_reach_or_no_citation_is_an_error() {
        let d = check(VERIFIED);
        assert_eq!(ids(&d), [NO_REACH], "{:?}", d.findings);
        let d = check("reach: everywhere\n");
        assert!(ids(&d).contains(&NO_REACH), "{:?}", d.findings);
        // Citing nothing licenses nothing beyond [open].
        let d = check("reach: attributed\nclaim: Unsupported.\n");
        assert_eq!(d.tier, Some("open"));
        assert_eq!(ids(&d), [BEYOND_REACH], "{:?}", d.findings);
        let d = check("reach: internal\n");
        assert!(d.findings.is_empty(), "{:?}", d.findings);
        let d = check("- a list\n");
        assert_eq!(ids(&d), [NO_REACH], "{:?}", d.findings);
    }

    #[test]
    fn an_undeclared_refusal_is_proposed_and_never_fails() {
        let d = check(
            "reach: internal\ncites:\n  - node: reach/headwater\n    \
             span: Whether the spring feeds the reach is [open].\n",
        );
        assert_eq!(ids(&d), [REFUSAL_CANDIDATE], "{:?}", d.findings);
        assert_eq!(d.errors(), 0);
    }

    #[test]
    fn a_markdown_file_is_an_artifact_only_with_frontmatter() {
        let nodes = nodes();
        let fields = ClaimFields::default();
        let cited = Cited::new(&nodes, &fields);
        assert!(check_artifact("dossier/README.md", "# Dossier\n", &cited).is_none());
        let d = check_artifact(
            "dossier/memo.md",
            &format!("---\nreach: public\n{VERIFIED}---\n\n# Memo\n"),
            &cited,
        )
        .unwrap();
        assert_eq!(d.tier, Some("verified"));
        assert!(d.findings.is_empty(), "{:?}", d.findings);
    }

    #[test]
    fn the_proposer_needs_both_a_tag_and_a_negation() {
        assert!(proposes_refusal("It is [open]. Nothing retrieved says so."));
        assert!(!proposes_refusal("It is [open]. The survey is pending."));
        assert!(!proposes_refusal("The survey does not reach it."));
    }

    #[test]
    fn artifacts_are_found_under_the_globs_and_nowhere_else() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for f in [
            "dossier/a.yml",
            "dossier/deep/b.md",
            "dossier/c.txt",
            "notes/d.yml",
        ] {
            let p = root.join(f);
            std::fs::create_dir_all(p.parent().unwrap()).unwrap();
            std::fs::write(p, "").unwrap();
        }
        assert_eq!(
            artifact_files(root, &["dossier/**".to_string()]),
            ["dossier/a.yml", "dossier/deep/b.md"]
        );
        assert!(artifact_files(root, &["absent/**".to_string()]).is_empty());
    }
}
