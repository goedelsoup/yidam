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

/// The properties one class flagged `retrievable: true`, read off the corpus model.
fn declared(class: &crate::corpus::Class) -> Declaration {
    Declaration {
        class: class.name.clone(),
        properties: class
            .properties
            .iter()
            .filter(|p| p.retrievable)
            .filter_map(|p| {
                let n = p.name.trim().to_string();
                (!n.is_empty()).then_some(n)
            })
            .collect(),
    }
}

impl Retrievable {
    /// Read every class definition in a corpus.
    ///
    /// Walks `.ont.yml` a second time rather than sharing [`ProseFields::load`]'s walk. The
    /// two are read together by exactly one caller — `embed` — and a combined loader would
    /// put a retrieval concern inside the type `node-too-long` and `missing-description`
    /// hold, which is the coupling this module exists to avoid. The walk is a directory
    /// listing bounded by the number of classes.
    ///
    /// **A second walk, and since #1116 not a second parse.** What this used to duplicate was
    /// not the directory listing but the reading of a class file — its own serde struct over
    /// the same bytes [`crate::corpus::Class`] had already described, one of four. The parse
    /// is [`crate::corpus::read_classes`]'s now, and `retrievable: true` is declared on
    /// [`crate::corpus::ClassProperty`] where `prose: true` beside it always was.
    pub fn load(corpus: &Path) -> Self {
        Self::from_declarations(crate::corpus::read_classes(corpus).iter().map(declared))
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

/// The three field lists `class` declared, in the shape the SDK's assembler takes.
///
/// **Takes both declarations**, for the reason [`crate::prose::of`] takes the whole
/// [`ProseFields`] and the class: a caller asking one and forgetting the other is the mistake
/// the shape is built to prevent, and it is the exact mistake #717 found `embed` making.
/// [`yidam_core::embed::EmbedFields`] carries the same guarantee one layer out, which is why
/// this returns it rather than a tuple.
///
/// This is the whole of what the CLI contributes to composing a node's embedding: resolving
/// `<class>.ont.yml` and `universal.yml` into three lists. What is done with them is
/// [`yidam_core::embed::compose_embed_text`], because a second assembler over one corpus is a
/// second vector space (RFC-0007).
pub fn fields(
    prose: &ProseFields,
    retrievable: &Retrievable,
    class: &str,
) -> yidam_core::embed::EmbedFields {
    yidam_core::embed::EmbedFields {
        prose_keys: prose.for_class(class).to_vec(),
        prose_properties: prose.properties_for_class(class).to_vec(),
        retrievable_properties: retrievable.for_class(class).to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::CorpusInstance;

    /// Class text in, declarations out — through the same [`crate::corpus::Class::parse`]
    /// the product reads with. `name` becomes the file stem, which is the class's name.
    fn retrievable(classes: &[(&str, &str)]) -> Retrievable {
        let parsed: Vec<crate::corpus::Class> = classes
            .iter()
            .map(|(name, text)| crate::corpus::Class::parse(format!("{name}.ont.yml"), *text))
            .collect();
        Retrievable::from_declarations(parsed.iter().map(declared).collect::<Vec<_>>())
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

    /// #717 in one assertion. Both declared strings are in the class's retrievable list, and
    /// the property nobody flagged is not.
    ///
    /// Asserted on the list rather than on composed text, because composing is
    /// [`yidam_core::embed`]'s and is fixtured there. What is this module's is which names
    /// come out of the declaration, and in what order — the class's, not sorted.
    #[test]
    fn the_flagged_properties_reach_the_field_list_in_declaration_order() {
        let f = fields(
            &prose_fields(&[("gage", &[], &[])]),
            &retrievable(&[("gage", GAGE)]),
            "gage",
        );
        assert_eq!(f.retrievable_properties, ["parameter", "units"]);
        assert_eq!(
            f.prose_keys,
            ["description"],
            "the retrievable axis must not widen the prose one"
        );
    }

    /// The file stem is the class, whatever the file's `class:` says — #1116, and the
    /// argument is on [`crate::corpus::Class::name`]. Keyed by the declared field, a
    /// flagged property reaches no node, because `class_of` resolves a node by its
    /// directory and the directory is the stem.
    #[test]
    fn a_class_declaring_another_name_is_keyed_by_its_file() {
        let r = retrievable(&[(
            "gage",
            GAGE.replace("class: gage", "class: measure").as_str(),
        )]);
        let p = prose_fields(&[("gage", &[], &[])]);
        assert_eq!(
            fields(&p, &r, "gage").retrievable_properties,
            ["parameter", "units"]
        );
        assert!(
            fields(&p, &r, "measure").retrievable_properties.is_empty(),
            "the declared name must not carry the declaration"
        );
    }

    /// Every corpus written before this field existed, which is the reason absent means
    /// false: a class that flagged nothing asks for exactly what prose asks for.
    #[test]
    fn a_class_flagging_nothing_asks_for_exactly_its_prose() {
        let p = prose_fields(&[("gage", &[], &[])]);
        for r in [
            Retrievable::default(),
            retrievable(&[("gage", "class: gage\n")]),
        ] {
            let f = fields(&p, &r, "gage");
            assert!(f.retrievable_properties.is_empty());
            assert_eq!(f.prose_keys, p.for_class("gage"));
            assert_eq!(f.prose_properties, p.properties_for_class("gage"));
        }
    }

    /// The implication runs one way. A prose property is retrievable without being flagged;
    /// a retrievable property is not thereby prose, so nothing reading prose sees it.
    ///
    /// The two axes arrive on `EmbedFields` as two lists and the union is taken inside the
    /// assembler, which is what lets `node-too-long` and `missing-description` keep asking
    /// the narrower question off the same declaration.
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

        let f = fields(&p, &retrievable(&[("gage", GAGE)]), "gage");
        assert!(f.prose_properties.is_empty());
        assert_eq!(
            yidam_core::embed::of(&node, &f),
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

    /// A property flagged both arrives on both lists, and the assembler emits it once.
    ///
    /// The de-duplication is [`yidam_core::embed::of`]'s and is fixtured there; what is
    /// asserted here is that this module hands it both flags rather than resolving the
    /// overlap itself and leaving the SDK a case it never sees.
    #[test]
    fn a_property_flagged_prose_and_retrievable_reaches_both_lists() {
        let both = "class: measure\n\
            properties:\n\
            \x20 - name: method\n\
            \x20   prose: true\n\
            \x20   retrievable: true\n";
        let f = fields(
            &prose_fields(&[("measure", &[], &["method"])]),
            &retrievable(&[("measure", both)]),
            "measure",
        );
        assert_eq!(f.prose_properties, ["method"]);
        assert_eq!(f.retrievable_properties, ["method"]);

        let node = inst("class: measure\nproperties:\n  method: How it was computed.\n");
        assert_eq!(
            yidam_core::embed::of(&node, &f),
            [("properties.method".to_string(), "How it was computed.")]
        );
    }

    /// A top-level key and a property of the same name stay two declarations, so the
    /// de-duplication one layer out cannot collide them.
    #[test]
    fn a_top_level_key_and_a_property_of_the_same_name_are_told_apart() {
        let ont = "class: measure\n\
            prose: [parameter]\n\
            properties:\n\
            \x20 - name: parameter\n\
            \x20   retrievable: true\n";
        let f = fields(
            &prose_fields(&[("measure", &["parameter"], &[])]),
            &retrievable(&[("measure", ont)]),
            "measure",
        );
        assert_eq!(f.prose_keys, ["description", "parameter"]);
        assert_eq!(f.retrievable_properties, ["parameter"]);
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
