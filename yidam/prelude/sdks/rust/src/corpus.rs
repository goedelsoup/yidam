//! The corpus node, as it is written on disk.
//!
//! # One model, and the one that ships
//!
//! This module used to hold a Markdown parser — `parse_node`, `extract_claims`,
//! `extract_links`: H1 title, kind derived from the path prefix, one claim per line, links
//! read out of `[label](target)`. Three SDKs agreed about it exactly, for seven minor
//! versions of the parity surface, and **no product ever called any of it**. RFC-0002 named
//! that split and RFC-0013 closed it in favour of the YAML instance; neither function the
//! close specified was ever written, and both documents were recorded `Implemented` anyway.
//!
//! A node is a YAML document. That is what the CLI walks, what the reports count, what the
//! editor declines to reimplement, and what every derived corpus commits — measured over
//! eighteen of them (#713): 2,762 instance nodes, and **not one Markdown node**. Every `.md`
//! under `.yidam/corpus/` is a class `README.md` or `ACTIONS.md`, which is guidance and not a
//! node. So there is no `project_markdown` here either: an ingestion path for an authoring
//! style no corpus uses would be this module's own mistake, one function later.
//!
//! # Unknown keys are kept, and that reverses RFC-0013
//!
//! RFC-0013 §"`properties:` surfaced, typed, deny-unknown" specified that this parser set
//! `deny_unknown_fields`, so an unspecced key is an error rather than a silent drop. That is
//! wrong on evidence gathered since, and it is reversed here rather than carried forward.
//!
//! Closing the node shape rejects **117 nodes of 117** in one derived repository — which
//! writes `summary`, `findings`, `revisions` and `unfilled` at the top level — and did the
//! same to a projecting consumer. Worse than rejection is what serde does without it: those
//! keys are *dropped*, so a checker reading the parsed node measures a fraction of what the
//! node says. On one corpus that is 118 lines against 21 (#674), and across the population
//! the gap is 21.8% of a node's bytes against 50.5% (#713).
//!
//! So [`CorpusInstance::extra`] keeps every top-level key this struct does not name. Which of
//! them carry prose is declared per class in the ontology, not decided here.
//!
//! # A YAML date is a string, spelled as written
//!
//! This is the divergence the surface exists to catch, and it is not hypothetical: 41% of the
//! nodes in the measured population carry a date. Given `occurred: 1886-07-04`:
//!
//! | | reads it as |
//! |---|---|
//! | Rust, `serde_yaml` | `"1886-07-04"` |
//! | TypeScript, `yaml` (YAML 1.2 core schema) | `"1886-07-04"` |
//! | Python, `pyyaml` (YAML 1.1) | `datetime.date(1886, 7, 4)` — which `json.dumps` refuses |
//!
//! Two of three agree by accident of which YAML version their library implements. The
//! contract is the majority reading **and the one the corpus means**: a timestamp is a
//! scalar string carrying its source text, because every consumer of these values reads them
//! as text. The Python SDK strips the timestamp resolver to get there; the fixture
//! `unquoted-dates-stay-text.toml` is what fails when it is put back.

use serde::{Deserialize, Serialize};

/// One relationship, as the instance wrote it.
///
/// `claim_tag` and `source` are **carried and not interpreted**. They are on a large share of
/// the edges in every corpus measured and were being dropped on the floor by a struct that
/// named neither; what reads them is #587's question, and a parser that discards them settles
/// it by default in the one direction nobody argued for.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct CorpusLink {
    pub target: Option<String>,
    pub relationship: Option<String>,
    /// A standing, or a list of them — the class schema admits both spellings, so this is
    /// held untyped rather than guessing which a corpus meant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claim_tag: Option<serde_yaml::Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
}

/// One claim resting on a node in an installed dependency (RFC-0019).
///
/// Beside `links:` and never inside it: a foreign node may be read and may not be an edge
/// target, so a citation is a different object from a relationship.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ExternalCitation {
    pub package: Option<String>,
    pub node: Option<String>,
    pub commit: Option<String>,
    pub tag: Option<String>,
    pub span: Option<String>,
}

/// A corpus node: `.yidam/corpus/<class>/<name>.yml`.
#[derive(Debug, Clone, PartialEq, Default, Deserialize)]
pub struct CorpusInstance {
    pub class: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// The typed fields the class declares, as the instance actually wrote them.
    ///
    /// Untyped because the *ontology* is the type: keys and value shapes are declared per
    /// class in `<class>.ont.yml`, so a struct here would be a second, weaker declaration of
    /// the same thing.
    #[serde(default)]
    pub properties: Option<serde_yaml::Mapping>,
    #[serde(default)]
    pub links: Option<Vec<CorpusLink>>,
    #[serde(default)]
    pub cites: Option<Vec<ExternalCitation>>,
    /// Every other top-level key, kept rather than dropped. See the module note.
    #[serde(flatten)]
    pub extra: serde_yaml::Mapping,
}

/// Parse one corpus node.
///
/// **No `path` argument, and RFC-0013 specified one.** The only use a path had was deriving
/// the node's kind from its directory, which is the Markdown model's move and the one
/// RFC-0002 rejected by name when it made `class:` an explicit field. A parameter carried in
/// so it can be ignored is that model surviving as a signature. Where a caller does want the
/// directory as a fallback for an unstated `class:`, that is the caller's policy — it is
/// contested (#587) and it does not belong in the parser.
///
/// **Unparseable is the default instance, not an error.** Every call site in the CLI already
/// spells this `serde_yaml::from_str(&text).unwrap_or_default()`, ten times over. A node that
/// will not parse is reported by the gate that reads it, and a parser that panicked or
/// returned `Err` here would take a whole corpus down for one bad file.
pub fn parse_instance(text: &str) -> CorpusInstance {
    serde_yaml::from_str(text).unwrap_or_default()
}

impl CorpusInstance {
    /// This node as one JSON object, which is the cross-language form of the contract.
    ///
    /// `extra` is a named field here and a `#[serde(flatten)]` on the struct, so the two
    /// forms differ deliberately: flattening is how the keys are *read*, and nesting is how
    /// they are *compared*. A fixture that could not tell a declared key from a coined one
    /// would pass while the split it exists to check was broken.
    pub fn to_json(&self) -> serde_json::Value {
        fn yaml_to_json(v: &serde_yaml::Value) -> serde_json::Value {
            serde_json::to_value(v).unwrap_or(serde_json::Value::Null)
        }
        serde_json::json!({
            "class": self.class,
            "label": self.label,
            "description": self.description,
            "properties": self.properties.as_ref().map(|m| {
                yaml_to_json(&serde_yaml::Value::Mapping(m.clone()))
            }),
            "links": self.links.as_ref().map(|links| {
                links
                    .iter()
                    .map(|l| {
                        serde_json::json!({
                            "target": l.target,
                            "relationship": l.relationship,
                            "claim_tag": l.claim_tag.as_ref().map(yaml_to_json),
                            "source": l.source,
                        })
                    })
                    .collect::<Vec<_>>()
            }),
            "cites": self.cites.as_ref().map(|cites| {
                cites
                    .iter()
                    .map(|c| {
                        serde_json::json!({
                            "package": c.package,
                            "node": c.node,
                            "commit": c.commit,
                            "tag": c.tag,
                            "span": c.span,
                        })
                    })
                    .collect::<Vec<_>>()
            }),
            "extra": yaml_to_json(&serde_yaml::Value::Mapping(self.extra.clone())),
        })
    }
}
