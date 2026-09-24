//! Which of a node's properties belong in its embedding although they are not prose.
//!
//! [`crate::prose`] answers *what does this node say*. Three readers ask it — `node-too-long`,
//! `missing-description` and `embed` — and for two of them prose is exactly the right
//! question. For the third it is nearly the right question and not the same one.
//!
//! `examples/streamflow`'s gage declares:
//!
//! ```yaml
//! properties:
//!   parameter: "00060"
//!   units: cubic feet per second
//! ```
//!
//! Neither string was in the node's vector, so a query for `cubic feet per second` could not
//! reach the node that declares the phrase, and neither could a query for the parameter
//! code — which is the identifier the catalog entry for this source is *organised around*
//! (#717). Identifiers, codes and units are the part of a corpus most likely to be typed
//! verbatim into a query, and they were the part excluded.
//!
//! # Why this is not `prose: true`
//!
//! `prose: true` would put both strings in the embedding, and that is the whole of its
//! appeal. It would also say a two-token parameter code is prose, which it is not in any
//! sense [`crate::prose`]'s own doc comment uses, and the claim does not stay in the
//! embedding: `node-too-long` would count `00060` toward the node's length, and
//! `missing-description` would accept a node that says nothing but `00060` as a node with
//! something said about it. A flag whose name is wrong in two of its three readers is a flag
//! that will be set wrongly.
//!
//! So retrievability is a third axis beside `required` and `prose`, on the same argument
//! [`crate::prose::ProseFields`] made for its second: the two are different questions about
//! the same property, and one flag answering both is a flag that cannot be set for one
//! without asserting the other.
//!
//! ```yaml
//! # <class>.ont.yml
//! properties:
//!   - name: parameter
//!     type: string
//!     retrievable: true
//!   - name: method
//!     type: string
//!     prose: true      # already retrievable — prose is what an embedding is mostly made of
//! ```
//!
//! # Prose is retrievable, and the union is taken here
//!
//! A class need not flag a prose property `retrievable` as well. `embed` has composed every
//! declared prose field since #746, so the set this module hands it is *prose, then the
//! flagged properties prose did not already carry* — and a property flagged both appears
//! once, not twice. Taking that union in one place is what stops a corpus having to know
//! which flag implies which.
//!
//! The implication runs one way only. Prose is retrievable because an embedding is built out
//! of what a node says; a retrievable identifier is not thereby prose, which is the whole
//! distinction above.
//!
//! # Absent means false, and there is no universal property
//!
//! Absent means false, for [`crate::corpus::ClassProperty::required`]'s reason
//! exactly: a corpus written before the field existed never had the chance to say. A class
//! declaring nothing produces byte-identical `.yidam/embeddings/` output to before this
//! existed, which is every corpus today.
//!
//! # The flag is not on `ClassProperty`
//!
//! [`crate::corpus::ClassProperty`] carries `required` and `prose` because the
//! checks read them off a class the linter has already parsed. Nothing in `lint` reads this
//! one and nothing should: flagging a property changes what is retrievable and changes no
//! report. A `retrievable` field sitting unread on the struct every check holds is an
//! invitation to a second reader that answers the question differently from this one, which
//! is how `node-too-long` and [`crate::claims`] came to disagree about a fifth of the corpus.
//! So this module parses the declaration it owns, the way [`crate::prose`] parses the one it
//! owns.
//!
//! And it is per class, with no `universal.yml` union — the rule
//! [`crate::prose::ProseFields::properties_for_class`] already keeps. `universal.yml` names
//! *top-level keys*; a property belongs to the class that declared it, and a corpus cannot
//! flag a property on a class that never wrote one.
//!
//! # A corpus that flags one must re-embed
//!
//! Node text is what an index is built from, so this is the change rather than a side effect
//! of it — the same contract `prose: true` carries, and for the same reason.

use std::collections::BTreeMap;
use std::path::Path;

use crate::parse::CorpusInstance;
use crate::prose::ProseFields;

/// Which of a class's properties are worth retrieving on, per class.
#[derive(Debug, Clone, Default)]
pub struct Retrievable {
    /// Property names a class flagged `retrievable: true`, in declaration order.
    ///
    /// Declaration order and not alphabetical, matching
    /// [`ProseFields::properties_for_class`]: the ontology is where the order was chosen, and
    /// re-sorting it here would make the embedded text depend on a rule nobody wrote down.
    ///
    /// A derived `Default` is correct here, unlike [`ProseFields`]'s: an empty map means *no
    /// class flagged anything*, which is the true state of every corpus predating the field.
    /// `ProseFields` had to be written out because its empty default would have read as *this
    /// corpus has no prose at all* and silently stopped measuring `description`.
    by_class: BTreeMap<String, Vec<String>>,
}

/// What one class declared about retrievability.
///
/// A named struct rather than a bare tuple, for [`crate::prose::Declaration`]'s reason: a
/// caller passing a class name where a property list goes would compile.
pub struct Declaration {
    pub class: String,
    /// Property names the class declared `retrievable: true`, in declaration order.
    pub properties: Vec<String>,
}

/// The `class:` a declaration file named itself, and the properties it flagged.
fn declared(text: &str) -> (Option<String>, Vec<String>) {
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        #[serde(default)]
        class: Option<String>,
        #[serde(default)]
        properties: Vec<Property>,
    }
    #[derive(Default, serde::Deserialize)]
    struct Property {
        #[serde(default)]
        name: String,
        #[serde(default)]
        retrievable: bool,
    }
    let f: Fields = serde_yaml::from_str(text).unwrap_or_default();
    let props = f
        .properties
        .into_iter()
        .filter(|p| p.retrievable)
        .filter_map(|p| {
            let n = p.name.trim().to_string();
            (!n.is_empty()).then_some(n)
        })
        .collect();
    (f.class.filter(|c| !c.is_empty()), props)
}

impl Retrievable {
    /// Read every class definition in a corpus.
    ///
    /// Walks `.ont.yml` a second time rather than sharing [`ProseFields::load`]'s walk. The
    /// two are read together by exactly one caller — `embed` — and a combined loader would
    /// put a retrieval concern inside the type `node-too-long` and `missing-description`
    /// hold, which is the coupling this module exists to avoid. The walk is a directory
    /// listing bounded by the number of classes.
    pub fn load(corpus: &Path) -> Self {
        Self::from_declarations(crate::walk::walk_ont_files(corpus).into_iter().map(|path| {
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            let (named, properties) = declared(&text);
            let class = named.unwrap_or_else(|| {
                path.file_name()
                    .and_then(|n| n.to_str())
                    .and_then(|n| n.strip_suffix(".ont.yml"))
                    .unwrap_or_default()
                    .to_string()
            });
            Declaration { class, properties }
        }))
    }

    /// The same declaration, from an ontology a caller has already parsed.
    ///
    /// The reason [`crate::prose::ProseFields::from_declarations`] records: a caller holding
    /// the classes would otherwise read every one a second time, and a graph rebuilt at a
    /// past commit holds an ontology that is not on disk at all.
    pub fn from_declarations(declarations: impl IntoIterator<Item = Declaration>) -> Self {
        Self {
            by_class: declarations
                .into_iter()
                .filter(|d| !d.properties.is_empty())
                .map(|d| (d.class, d.properties))
                .collect(),
        }
    }

    /// The properties of `class` worth retrieving on, in declaration order.
    ///
    /// Empty for a class that flagged none, and empty for a class nothing declared. There is
    /// no fallback set, because there is no universal property.
    pub fn for_class(&self, class: &str) -> &[String] {
        self.by_class.get(class).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Everything about this node that belongs in its embedding: its prose, then the properties
/// flagged `retrievable` that prose did not already carry.
///
/// **Takes both declarations**, for the reason [`crate::prose::of`] takes the whole
/// [`ProseFields`] and the class: a caller asking one and forgetting the other is the mistake
/// the shape is built to prevent, and it is the exact mistake #717 found `embed` making.
///
/// Property keys come back qualified — `properties.parameter` — so a caller rendering the
/// name can say which of a top-level `parameter` and a property `parameter` it means. That
/// qualification is also what makes the de-duplication against prose exact rather than a
/// guess: a property flagged `prose: true` and `retrievable: true` arrives under one key from
/// both sides and is emitted once.
///
/// A flagged property the node does not carry, or carries as something other than a string,
/// yields nothing — the rule [`crate::prose::of`] keeps, and for its reason: a value holding
/// a list is a real state, not something to guess a rendering for.
pub fn of<'a>(
    inst: &'a CorpusInstance,
    prose: &ProseFields,
    retrievable: &Retrievable,
    class: &str,
) -> Vec<(String, &'a str)> {
    let mut out = crate::prose::of(inst, prose, class);
    let already: std::collections::BTreeSet<String> = out.iter().map(|(k, _)| k.clone()).collect();

    let props = inst.properties.as_ref();
    out.extend(retrievable.for_class(class).iter().filter_map(|name| {
        let key = format!("properties.{name}");
        if already.contains(&key) {
            return None;
        }
        let value = props?.get(name.as_str())?.as_str()?;
        (!value.trim().is_empty()).then_some((key, value))
    }));
    out
}

/// The node's retrievable text as one block, in the order [`of`] returns it.
///
/// What `embed` composes the node's vector from, beside its label and its edge names.
pub fn text(
    inst: &CorpusInstance,
    prose: &ProseFields,
    retrievable: &Retrievable,
    class: &str,
) -> String {
    of(inst, prose, retrievable, class)
        .into_iter()
        .map(|(_, v)| v.trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn retrievable(classes: &[(&str, &str)]) -> Retrievable {
        Retrievable::from_declarations(classes.iter().map(|(name, text)| Declaration {
            class: (*name).to_string(),
            properties: declared(text).1,
        }))
    }

    /// The prose side, declared directly rather than parsed out of the same YAML.
    ///
    /// Two readers of one file is what this module deliberately does *not* build, and a test
    /// helper that parsed the class text for both axes would hide which flag a case is
    /// actually exercising.
    fn prose_fields(classes: &[(&str, &[&str], &[&str])]) -> ProseFields {
        ProseFields::from_declarations(
            vec![],
            classes
                .iter()
                .map(|(name, keys, properties)| crate::prose::Declaration {
                    class: (*name).to_string(),
                    keys: keys.iter().map(|k| (*k).to_string()).collect(),
                    properties: properties.iter().map(|p| (*p).to_string()).collect(),
                }),
        )
    }

    fn inst(yaml: &str) -> CorpusInstance {
        crate::parse::parse_instance(yaml)
    }

    const GAGE: &str = "class: gage\n\
        properties:\n\
        \x20 - name: parameter\n\
        \x20   type: string\n\
        \x20   retrievable: true\n\
        \x20 - name: units\n\
        \x20   type: string\n\
        \x20   retrievable: true\n\
        \x20 - name: claim_tag\n\
        \x20   type: claim\n";

    const NODE: &str = "class: gage\n\
        description: The station below the impoundment.\n\
        properties:\n\
        \x20 parameter: \"00060\"\n\
        \x20 units: cubic feet per second\n\
        \x20 claim_tag: inference\n";

    /// #717 in one assertion. Both declared strings are in the node's embedding target, and
    /// the property nobody flagged is not.
    #[test]
    fn a_flagged_property_reaches_the_embedding() {
        let t = text(
            &inst(NODE),
            &prose_fields(&[("gage", &[], &[])]),
            &retrievable(&[("gage", GAGE)]),
            "gage",
        );
        assert!(t.contains("00060"), "the parameter code: {t:?}");
        assert!(t.contains("cubic feet per second"), "the units: {t:?}");
        assert!(
            !t.contains("inference"),
            "claim_tag was not flagged, and is constant across the corpus: {t:?}"
        );
    }

    /// The order: prose first, then the properties in the order the class declared them.
    #[test]
    fn properties_follow_the_prose_in_declaration_order() {
        assert_eq!(
            of(
                &inst(NODE),
                &prose_fields(&[("gage", &[], &[])]),
                &retrievable(&[("gage", GAGE)]),
                "gage",
            ),
            [
                (
                    "description".to_string(),
                    "The station below the impoundment."
                ),
                ("properties.parameter".to_string(), "00060"),
                ("properties.units".to_string(), "cubic feet per second"),
            ]
        );
    }

    /// Every corpus written before this field existed, which is the reason absent means
    /// false: identical to what [`crate::prose::text`] alone produces.
    #[test]
    fn a_class_flagging_nothing_composes_exactly_its_prose() {
        let p = prose_fields(&[("gage", &[], &[])]);
        let node = inst(NODE);
        assert_eq!(
            text(&node, &p, &Retrievable::default(), "gage"),
            crate::prose::text(&node, &p, "gage")
        );
        assert_eq!(
            text(
                &node,
                &p,
                &retrievable(&[("gage", "class: gage\n")]),
                "gage"
            ),
            crate::prose::text(&node, &p, "gage")
        );
    }

    /// Prose is already retrievable, and a property flagged both is emitted once.
    #[test]
    fn a_property_flagged_prose_and_retrievable_appears_once() {
        let both = "class: measure\n\
            properties:\n\
            \x20 - name: method\n\
            \x20   prose: true\n\
            \x20   retrievable: true\n";
        let node = inst("class: measure\nproperties:\n  method: How it was computed.\n");
        assert_eq!(
            of(
                &node,
                &prose_fields(&[("measure", &[], &["method"])]),
                &retrievable(&[("measure", both)]),
                "measure",
            ),
            [("properties.method".to_string(), "How it was computed.")]
        );
    }

    /// The implication runs one way. A prose property is retrievable without being flagged;
    /// a retrievable property is not thereby prose, so nothing reading prose sees it.
    #[test]
    fn retrievable_does_not_make_a_property_prose() {
        let p = prose_fields(&[("gage", &[], &[])]);
        let node = inst(NODE);
        assert_eq!(
            crate::prose::of(&node, &p, "gage"),
            [(
                "description".to_string(),
                "The station below the impoundment."
            )],
            "node-too-long and missing-description must not see the parameter code"
        );
        assert!(text(&node, &p, &retrievable(&[("gage", GAGE)]), "gage").contains("00060"));
    }

    /// A top-level key and a property of the same name are told apart, so the
    /// de-duplication against prose cannot collide them.
    #[test]
    fn a_top_level_key_and_a_property_of_the_same_name_are_told_apart() {
        let ont = "class: measure\n\
            prose: [parameter]\n\
            properties:\n\
            \x20 - name: parameter\n\
            \x20   retrievable: true\n";
        let node =
            inst("class: measure\nparameter: at the top\nproperties:\n  parameter: \"00060\"\n");
        assert_eq!(
            of(
                &node,
                &prose_fields(&[("measure", &["parameter"], &[])]),
                &retrievable(&[("measure", ont)]),
                "measure",
            ),
            [
                ("parameter".to_string(), "at the top"),
                ("properties.parameter".to_string(), "00060"),
            ]
        );
    }

    /// A flagged property holding something other than a string yields nothing, for the
    /// reason a prose key holding a list does: it is a real state, not text.
    #[test]
    fn a_flagged_property_that_is_not_a_string_yields_nothing() {
        let ont = "class: measure\nproperties:\n  - name: codes\n    retrievable: true\n";
        let node = inst("class: measure\nproperties:\n  codes:\n    - \"00060\"\n");
        assert!(of(
            &node,
            &prose_fields(&[("measure", &["parameter"], &[])]),
            &retrievable(&[("measure", ont)]),
            "measure",
        )
        .is_empty());
    }

    /// A flagged property the node does not carry is not an empty line in the vector.
    #[test]
    fn a_flagged_property_the_node_omits_yields_nothing() {
        let ont = "class: gage\nproperties:\n  - name: parameter\n    retrievable: true\n";
        let node = inst("class: gage\ndescription: Only this.\n");
        assert_eq!(
            text(
                &node,
                &prose_fields(&[("gage", &[], &[])]),
                &retrievable(&[("gage", ont)]),
                "gage",
            ),
            "Only this."
        );
    }

    /// There is no universal property: a class nothing declared flags nothing, and cannot
    /// inherit a flag from `universal.yml`, which names top-level keys.
    #[test]
    fn a_class_nothing_declared_flags_nothing() {
        let r = retrievable(&[("gage", GAGE)]);
        assert!(r.for_class("reach").is_empty());
        assert!(r.for_class("never-declared").is_empty());
    }

    /// A file that does not parse declares nothing rather than taking the corpus down — the
    /// direction every other declaration reader degrades in.
    #[test]
    fn an_unparseable_declaration_declares_nothing() {
        assert!(
            retrievable(&[("broken", "class: [this is: not: a class\n")])
                .for_class("broken")
                .is_empty()
        );
    }

    /// A name written with surrounding whitespace, and an empty one, are not names.
    #[test]
    fn blank_names_are_not_names() {
        let r = retrievable(&[(
            "gage",
            "class: gage\nproperties:\n  - name: '  '\n    retrievable: true\n  - name: ' units '\n    retrievable: true\n",
        )]);
        assert_eq!(r.for_class("gage"), ["units"]);
    }
}
