//! Which of a node's top-level keys carry prose.
//!
//! `description` was the only one anything read, and corpora write more than one. Closing the
//! node schema over the top level rejected **117 nodes of 117** in one derived repository —
//! `summary`, `findings`, `revisions`, `unfilled` — and a projecting consumer 199 of 199
//! ([`crate::cmd::schema`]). None of those keys is on [`crate::parse::CorpusInstance`], so
//! every check reading the parsed node saw a fraction of what the node says.
//!
//! That is not a tidiness complaint, because two families of check disagreed as a result. The
//! byte scanners — [`crate::claims::count_in_node`], [`crate::claims::is_open_question`] —
//! take the file's whole text and always saw all of it. The field readers took
//! `description` alone. On one corpus the two answer **118 lines against 21** (#674), and the
//! same release told it its nodes were sprawling and that they were not.
//!
//! # The corpus declares it, and no key name is blessed
//!
//! A fixed list of blessed names is the shape that was already measured and rejected: a
//! closed set is what sent 117 nodes of 117 to be reshaped around a validator, and the harm
//! was never the red squiggle but that *the first thing a consumer does is nest their data to
//! make the squiggle stop*. The corpus that coins the fifth name is the one this exists for.
//!
//! So it is declared, in the shape `claim_tag` already has — and in two places, because the
//! two were measured to be different questions:
//!
//! - **`<class>.ont.yml`** — `prose: [findings]`, a key this class's instances carry.
//! - **`universal.yml`** — `prose: [summary, findings]`, apparatus every class may carry.
//!   `seeded_because` is the precedent one file over: declaring it per class would have been
//!   sixteen copies of one decision, and a seventeenth class would silently not have it.
//!
//! # Union, and `description` is always in it
//!
//! The effective set is `{description} ∪ universal ∪ class`. Two decisions there, and both
//! follow rules the ontology already keeps.
//!
//! **Union rather than override**, because membership of a set is not a type. Universal
//! *properties* let a class win, and must, since two declarations of one property's `type`
//! contradict each other. Two declarations that a key holds prose agree, so there is nothing
//! for the more specific one to win.
//!
//! **`description` is in the set unconditionally**, including for a class that declares
//! `prose:` and omits it. Silence is not a contract: naming `findings` says findings is
//! prose, and on its own it never said *and description is not*. Reading it as the second
//! would let one added declaration silently stop measuring the field every corpus writes.
//!
//! Absent both files the set is `[description]` — which is every corpus written before this
//! existed, so nothing changes for them.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use crate::parse::CorpusInstance;

/// The prose keys every node carries whatever anything declares.
pub const ALWAYS: &str = "description";

/// Which top-level keys hold prose, per class.
///
/// Built with the union already applied, so a caller cannot forget to add `description` or
/// the universal set — the mistake that would make this a second, weaker copy of the rule.
#[derive(Debug, Clone)]
pub struct ProseFields {
    by_class: BTreeMap<String, Vec<String>>,
    /// The set for a class that declared none of its own — universal plus [`ALWAYS`].
    default: Vec<String>,
}

/// The `prose:` list a declaration file names, and the `class:` it named itself, if any.
fn declared(text: &str) -> (Option<String>, Vec<String>) {
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        #[serde(default)]
        class: Option<String>,
        #[serde(default)]
        prose: Vec<String>,
    }
    let f: Fields = serde_yaml::from_str(text).unwrap_or_default();
    let keys = f
        .prose
        .into_iter()
        .map(|k| k.trim().to_string())
        .filter(|k| !k.is_empty())
        .collect();
    (f.class.filter(|c| !c.is_empty()), keys)
}

fn unioned(universal: &[String], own: &[String]) -> Vec<String> {
    let mut set: BTreeSet<&str> = BTreeSet::new();
    set.insert(ALWAYS);
    set.extend(universal.iter().map(String::as_str));
    set.extend(own.iter().map(String::as_str));
    set.into_iter().map(str::to_string).collect()
}

/// A corpus that declares nothing — which still reads `description` as prose.
///
/// Written out rather than derived. A derived `Default` gives an *empty* default set, which
/// would read as *this corpus has no prose at all* and silently stop measuring the one field
/// every corpus writes.
impl Default for ProseFields {
    fn default() -> Self {
        Self {
            by_class: BTreeMap::new(),
            default: vec![ALWAYS.to_string()],
        }
    }
}

impl ProseFields {
    /// Read every class definition in a corpus, and `universal.yml` beside them.
    pub fn load(corpus: &Path) -> Self {
        let universal = std::fs::read_to_string(corpus.join("universal.yml"))
            .map(|t| declared(&t).1)
            .unwrap_or_default();
        let declarations = crate::walk::walk_ont_files(corpus).into_iter().map(|path| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (named, keys) = declared(&text);
            let class = named.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".ont.yml"))
                    .unwrap_or_default()
                    .to_string()
            });
            (class, keys)
        });
        Self::from_declarations(universal, declarations)
    }

    /// The same declaration, from an ontology a caller has already parsed.
    ///
    /// [`Self::load`] walks `.ont.yml` from disk. A caller holding the classes — `lint` does,
    /// and `query::Graph` reconstructs them from git blobs — would otherwise read every one a
    /// second time, and a graph rebuilt at a past commit holds an ontology that is not on
    /// disk at all, where the second read answers with today's declaration about another
    /// year's corpus. The same argument [`crate::claims::ClaimFields::from_declarations`]
    /// records.
    pub fn from_declarations(
        universal: Vec<String>,
        declarations: impl IntoIterator<Item = (String, Vec<String>)>,
    ) -> Self {
        Self {
            by_class: declarations
                .into_iter()
                .map(|(class, own)| (class, unioned(&universal, &own)))
                .collect(),
            default: unioned(&universal, &[]),
        }
    }

    /// The prose keys an instance of `class` may carry, sorted, always including
    /// [`ALWAYS`].
    ///
    /// A class nothing declared gets the universal set rather than an empty one: a corpus
    /// that named its apparatus once has named it for the class it forgot to write a file
    /// for too.
    pub fn for_class(&self, class: &str) -> &[String] {
        self.by_class
            .get(class)
            .map(Vec::as_slice)
            .unwrap_or(&self.default)
    }
}

/// This node's prose, in the keys the ontology declares as prose, in that order.
///
/// A free function and not a method, because the type is `yidam_core`'s and the *declaration*
/// is not: which keys carry prose is a per-class fact this repository reads out of
/// `<class>.ont.yml`, and an SDK that decided it would be answering a question the ontology
/// owns.
///
/// Reads `description` off its own field and everything else out of [`CorpusInstance::extra`],
/// so a caller cannot get a different answer depending on which key it asked about. A declared
/// key the node does not carry, or carries as something other than a string, yields nothing: a
/// `findings:` holding a list is a real state and not prose, and guessing at a rendering for it
/// would put words in the corpus's mouth.
pub fn of<'a>(inst: &'a CorpusInstance, declared: &'a [String]) -> Vec<(&'a str, &'a str)> {
    declared
        .iter()
        .filter_map(|key| {
            let value = match key.as_str() {
                ALWAYS => inst.description.as_deref(),
                other => inst.extra.get(other).and_then(serde_yaml::Value::as_str),
            }?;
            (!value.trim().is_empty()).then_some((key.as_str(), value))
        })
        .collect()
}

/// The node's prose as one block, the declared fields joined in order.
///
/// What a reader of the whole node reads, which is what a length ceiling and an embedding are
/// both about.
pub fn text(inst: &CorpusInstance, declared: &[String]) -> String {
    of(inst, declared)
        .into_iter()
        .map(|(_, v)| v.trim_end())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(universal: &str, classes: &[(&str, &str)]) -> ProseFields {
        ProseFields::from_declarations(
            declared(universal).1,
            classes
                .iter()
                .map(|(name, text)| ((*name).to_string(), declared(text).1)),
        )
    }

    /// Every corpus written before this existed, and the reason the default is not empty.
    #[test]
    fn the_default_is_description_rather_than_nothing() {
        assert_eq!(
            ProseFields::default().for_class("anything"),
            ["description"]
        );
    }

    #[test]
    fn a_corpus_declaring_nothing_reads_description_and_nothing_else() {
        let f = fields("", &[("gage", "class: gage\n")]);
        assert_eq!(f.for_class("gage"), ["description"]);
        assert_eq!(f.for_class("never-heard-of-it"), ["description"]);
    }

    /// The union, from the class's end.
    #[test]
    fn a_class_declaration_adds_to_description() {
        let f = fields("", &[("finding", "class: finding\nprose: [findings]\n")]);
        assert_eq!(f.for_class("finding"), ["description", "findings"]);
    }

    /// The union, from the corpus's end — and it reaches a class that declared nothing,
    /// which is the whole reason `universal.yml` exists.
    #[test]
    fn a_universal_declaration_reaches_every_class() {
        let f = fields(
            "prose: [summary]\n",
            &[("gage", "class: gage\n"), ("reach", "class: reach\n")],
        );
        assert_eq!(f.for_class("gage"), ["description", "summary"]);
        assert_eq!(f.for_class("reach"), ["description", "summary"]);
        assert_eq!(f.for_class("undeclared"), ["description", "summary"]);
    }

    /// Both ends at once, deduplicated, and `description` not repeated when named twice.
    #[test]
    fn the_two_declarations_union_rather_than_override() {
        let f = fields(
            "prose: [summary]\n",
            &[(
                "finding",
                "class: finding\nprose: [findings, description]\n",
            )],
        );
        assert_eq!(
            f.for_class("finding"),
            ["description", "findings", "summary"]
        );
    }

    /// Silence is not a contract: naming `findings` never said `description` is not prose.
    #[test]
    fn a_class_that_names_other_keys_still_carries_description() {
        let f = fields("", &[("finding", "class: finding\nprose: [findings]\n")]);
        assert!(f.for_class("finding").contains(&"description".to_string()));
    }

    /// A file that does not parse declares nothing rather than taking the corpus down —
    /// the direction `parse_class` already degrades in.
    #[test]
    fn an_unparseable_declaration_declares_nothing() {
        let f = fields("", &[("broken", "class: [this is: not: a class\n")]);
        assert_eq!(f.for_class("broken"), ["description"]);
    }

    /// A key written with surrounding whitespace, and an empty entry, are not keys.
    #[test]
    fn blank_entries_are_not_keys() {
        let f = fields("", &[("gage", "class: gage\nprose: ['  ', ' summary ']\n")]);
        assert_eq!(f.for_class("gage"), ["description", "summary"]);
    }
}
