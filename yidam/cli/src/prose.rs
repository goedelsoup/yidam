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
    /// The names under `properties:` a class flagged `prose: true`, in declaration order.
    ///
    /// **A second axis and not more entries in the first**, because they are read out of
    /// different places in the node: a top-level key is read off the document, and these are
    /// read out of the `properties` mapping. Folding them into one list would make the
    /// lookup ambiguous the moment a corpus writes a top-level `method` and a property
    /// `method`, which nothing forbids.
    ///
    /// No universal union here. `universal.yml`'s `prose:` names top-level keys, and a
    /// property belongs to the class that declared it — a corpus cannot flag a property on a
    /// class that never wrote one.
    props_by_class: BTreeMap<String, Vec<String>>,
    /// The set for a class that declared none of its own — universal plus [`ALWAYS`].
    default: Vec<String>,
}

/// What one class declared about prose: its top-level keys, and its flagged properties.
///
/// A named struct rather than a tuple because the two lists are not interchangeable and a
/// caller passing them in the wrong order would compile.
pub struct Declaration {
    pub class: String,
    /// Top-level keys from the class's `prose:` list.
    pub keys: Vec<String>,
    /// Property names the class declared `prose: true`, in declaration order.
    pub properties: Vec<String>,
}

/// What a declaration file says about prose: the `class:` it named itself, its `prose:` list,
/// and the properties it flagged.
fn declared(text: &str) -> (Option<String>, Vec<String>, Vec<String>) {
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        #[serde(default)]
        class: Option<String>,
        #[serde(default)]
        prose: Vec<String>,
        #[serde(default)]
        properties: Vec<Property>,
    }
    #[derive(Default, serde::Deserialize)]
    struct Property {
        #[serde(default)]
        name: String,
        #[serde(default)]
        prose: bool,
    }
    let f: Fields = serde_yaml::from_str(text).unwrap_or_default();
    let clean = |k: String| {
        let k = k.trim().to_string();
        (!k.is_empty()).then_some(k)
    };
    let keys = f.prose.into_iter().filter_map(clean).collect();
    let props = f
        .properties
        .into_iter()
        .filter(|p| p.prose)
        .filter_map(|p| clean(p.name))
        .collect();
    (f.class.filter(|c| !c.is_empty()), keys, props)
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
            props_by_class: BTreeMap::new(),
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
            let (named, keys, properties) = declared(&text);
            let class = named.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".ont.yml"))
                    .unwrap_or_default()
                    .to_string()
            });
            Declaration {
                class,
                keys,
                properties,
            }
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
        declarations: impl IntoIterator<Item = Declaration>,
    ) -> Self {
        let mut by_class = BTreeMap::new();
        let mut props_by_class = BTreeMap::new();
        for d in declarations {
            by_class.insert(d.class.clone(), unioned(&universal, &d.keys));
            if !d.properties.is_empty() {
                props_by_class.insert(d.class, d.properties);
            }
        }
        Self {
            by_class,
            props_by_class,
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

    /// The properties of `class` whose values are prose, in declaration order.
    ///
    /// Empty for a class that flagged none, and empty for a class nothing declared — unlike
    /// [`Self::for_class`], which falls back to the universal set. There is no universal
    /// property: `universal.yml` names top-level keys, and a property is declared by the class
    /// that owns it.
    pub fn properties_for_class(&self, class: &str) -> &[String] {
        self.props_by_class
            .get(class)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }
}

/// This node's prose: the declared top-level keys, then the properties flagged `prose: true`.
///
/// **Takes the whole [`ProseFields`] and the class rather than a key list**, so a caller
/// cannot ask for one axis and silently miss the other. That is not hypothetical — the
/// top-level-only reading is what `node-too-long`, `missing-description` and `embed` all had,
/// and it missed 14.5% of what the measured corpora write (#746).
///
/// A free function and not a method, because the type is `yidam_core`'s and the *declaration*
/// is not: which keys carry prose is a per-class fact this repository reads out of
/// `<class>.ont.yml`, and an SDK that decided it would be answering a question the ontology
/// owns.
///
/// Reads `description` off its own field, other top-level keys out of
/// [`CorpusInstance::extra`], and flagged properties out of `properties`, so a caller cannot
/// get a different answer depending on which key it asked about. A declared key the node does
/// not carry, or carries as something other than a string, yields nothing: a `findings:`
/// holding a list is a real state and not prose, and guessing at a rendering for it would put
/// words in the corpus's mouth.
///
/// Property keys come back qualified — `properties.method` — because a corpus may write a
/// top-level `method` and a property `method`, and a caller rendering the name in a finding
/// must be able to say which one it means.
pub fn of<'a>(
    inst: &'a CorpusInstance,
    fields: &ProseFields,
    class: &str,
) -> Vec<(String, &'a str)> {
    let mut out: Vec<(String, &'a str)> = fields
        .for_class(class)
        .iter()
        .filter_map(|key| {
            let value = match key.as_str() {
                ALWAYS => inst.description.as_deref(),
                other => inst.extra.get(other).and_then(serde_yaml::Value::as_str),
            }?;
            (!value.trim().is_empty()).then_some((key.clone(), value))
        })
        .collect();

    let props = inst.properties.as_ref();
    out.extend(
        fields
            .properties_for_class(class)
            .iter()
            .filter_map(|name| {
                let value = props?.get(name.as_str())?.as_str()?;
                (!value.trim().is_empty()).then_some((format!("properties.{name}"), value))
            }),
    );
    out
}

/// The node's prose as one block, the declared fields joined in order.
///
/// What a reader of the whole node reads, which is what a length ceiling and an embedding are
/// both about.
pub fn text(inst: &CorpusInstance, fields: &ProseFields, class: &str) -> String {
    of(inst, fields, class)
        .into_iter()
        .map(|(_, v)| v.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fields(universal: &str, classes: &[(&str, &str)]) -> ProseFields {
        ProseFields::from_declarations(
            declared(universal).1,
            classes.iter().map(|(name, text)| {
                let (_, keys, properties) = declared(text);
                Declaration {
                    class: (*name).to_string(),
                    keys,
                    properties,
                }
            }),
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

    fn inst(yaml: &str) -> CorpusInstance {
        crate::parse::parse_instance(yaml)
    }

    /// The whole of #746 in one assertion: a node whose substance is under `properties`.
    #[test]
    fn a_flagged_property_is_prose() {
        let f = fields(
            "",
            &[(
                "measure",
                "class: measure\nproperties:\n  - name: method\n    type: string\n    prose: true\n",
            )],
        );
        let n =
            inst("class: measure\nlabel: L\nproperties:\n  method: |\n    How it was computed.\n");
        assert_eq!(
            of(&n, &f, "measure"),
            [("properties.method".to_string(), "How it was computed.\n")]
        );
    }

    /// The key comes back qualified, because a corpus may write both and a finding must say
    /// which one it names.
    #[test]
    fn a_top_level_key_and_a_property_of_the_same_name_are_told_apart() {
        let f = fields(
            "",
            &[(
                "measure",
                "class: measure\nprose: [method]\nproperties:\n  - name: method\n    prose: true\n",
            )],
        );
        let n = inst("class: measure\nmethod: at the top\nproperties:\n  method: in the bag\n");
        assert_eq!(
            of(&n, &f, "measure"),
            [
                ("method".to_string(), "at the top"),
                ("properties.method".to_string(), "in the bag"),
            ]
        );
    }

    /// Top-level prose first, then properties in the order the class declared them.
    #[test]
    fn properties_follow_the_declared_keys_in_declaration_order() {
        let f = fields(
            "",
            &[(
                "measure",
                "class: measure\nproperties:\n  - name: b\n    prose: true\n  - name: a\n    prose: true\n",
            )],
        );
        let n = inst("class: measure\ndescription: first\nproperties:\n  a: second\n  b: third\n");
        assert_eq!(
            text(&n, &f, "measure"),
            "first\nthird\nsecond",
            "declaration order, not alphabetical and not the node's own order"
        );
    }

    /// A corpus that flags nothing reads exactly as it did before this existed.
    #[test]
    fn a_class_flagging_no_property_is_unchanged() {
        let f = fields(
            "",
            &[("measure", "class: measure\nproperties:\n  - name: method\n")],
        );
        let n = inst("class: measure\ndescription: only this\nproperties:\n  method: not read\n");
        assert_eq!(
            of(&n, &f, "measure"),
            [("description".to_string(), "only this")]
        );
    }

    /// A flagged property holding something other than a string yields nothing, for the
    /// reason a top-level key holding a list does: it is a real state, not prose.
    #[test]
    fn a_flagged_property_that_is_not_a_string_yields_nothing() {
        let f = fields(
            "",
            &[(
                "measure",
                "class: measure\nproperties:\n  - name: method\n    prose: true\n",
            )],
        );
        let n = inst("class: measure\nproperties:\n  method:\n    - a list\n");
        assert!(of(&n, &f, "measure").is_empty());
    }

    /// There is no universal property. `universal.yml` names top-level keys, and a property
    /// belongs to the class that declared it.
    #[test]
    fn a_class_nothing_declared_has_no_prose_properties() {
        let f = fields("prose: [summary]", &[("measure", "class: measure\n")]);
        assert!(f.properties_for_class("measure").is_empty());
        assert!(f.properties_for_class("never-declared").is_empty());
        assert_eq!(f.for_class("never-declared"), ["description", "summary"]);
    }
}
