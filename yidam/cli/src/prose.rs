//! Which of a node's top-level keys carry prose.
//!
//! `description` was the only one anything read, and corpora write more than one. Closing the
//! node schema over the top level rejected **117 nodes of 117** in one derived repository —
//! `summary`, `findings`, `revisions`, `unfilled` — and a projecting consumer 199 of 199
//! ([`crate::cmd::schema`]). None of those keys is on [`crate::parse::CorpusInstance`], so
//! every check reading the parsed node saw a fraction of what the node says.
//!
//! That is not a tidiness complaint, because two families of check disagreed as a result. The
//! byte scanners — [`crate::claims::count_in_node`], [`crate::claims::has_open_claim`] —
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
///
/// Re-exported rather than restated: the reading half of this module is
/// [`yidam_core::prose`], and a second `"description"` here is a second thing to change.
pub use yidam_core::prose::ALWAYS;

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

fn clean(k: &str) -> Option<String> {
    let k = k.trim().to_string();
    (!k.is_empty()).then_some(k)
}

/// What one class declared about prose, read off the corpus model.
///
/// [`crate::corpus::Class`] already trims and drops the empty entries of `prose:`; the
/// properties are filtered and trimmed here for the same reason it does — a `name: ""` an
/// author started and did not finish is not a key any node can carry.
///
/// **`universal.yml` is the one file still parsed here**, and it is not a class: it declares
/// prose keys for every class at once, has no `.ont.yml` stem, and the corpus model holds no
/// record for it. See [`ProseFields::load`].
pub(crate) fn declared(class: &crate::corpus::Class) -> Declaration {
    Declaration {
        class: class.name.clone(),
        keys: class.prose.iter().filter_map(|k| clean(k)).collect(),
        properties: class
            .properties
            .iter()
            .filter(|p| p.prose)
            .filter_map(|p| clean(&p.name))
            .collect(),
    }
}

/// The `prose:` list a `universal.yml` declares — the only prose declaration that is not a
/// class's.
fn universal_prose(text: &str) -> Vec<String> {
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        #[serde(default)]
        prose: Vec<String>,
    }
    let f: Fields = serde_yaml::from_str(text).unwrap_or_default();
    f.prose.iter().filter_map(|k| clean(k)).collect()
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
    ///
    /// The classes come through [`crate::corpus::read_classes`] — the corpus model's parse,
    /// not a second one over the same bytes (#1116) — and are keyed by the `.ont.yml` stem,
    /// which is what `lint` was already doing where it builds this from classes it holds.
    ///
    /// **Off disk, not through an [`crate::corpus::Overlay`].** `lint` is the caller with an
    /// editor behind it and it does not come here; it passes its own classes and its own
    /// `universal.yml` to [`Self::from_declarations`], so an unsaved declaration is measured.
    pub fn load(corpus: &Path) -> Self {
        let universal = std::fs::read_to_string(corpus.join("universal.yml"))
            .map(|t| universal_prose(&t))
            .unwrap_or_default();
        let classes = crate::corpus::read_classes(corpus);
        let declarations: Vec<Declaration> = classes.iter().map(declared).collect();
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
/// **The reading is [`yidam_core::prose::of`]'s and this is the resolution in front of it.**
/// Everything above — the two files, the union, [`ALWAYS`], the per-class fallback — is a
/// question about a corpus on disk. What a node yields *given* the answer is the same question
/// in three languages, and it became a parity function when `compose_embed_text` needed it too
/// (RFC-0007). So the field selection, the qualification of property keys as
/// `properties.method`, and the rule that a declared key the node does not carry as a string
/// yields nothing all live there, once.
pub fn of<'a>(
    inst: &'a CorpusInstance,
    fields: &ProseFields,
    class: &str,
) -> Vec<(String, &'a str)> {
    yidam_core::prose::of(
        inst,
        fields.for_class(class),
        fields.properties_for_class(class),
    )
}

/// The node's prose as one block, the declared fields joined in order.
///
/// What a reader of the whole node reads, which is what a length ceiling and an embedding are
/// both about.
pub fn text(inst: &CorpusInstance, fields: &ProseFields, class: &str) -> String {
    yidam_core::prose::text(
        inst,
        fields.for_class(class),
        fields.properties_for_class(class),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Class text in, declarations out — through the same [`crate::corpus::Class::parse`]
    /// the product reads with, so a case that parses here parses there.
    ///
    /// `name` becomes the file stem, which is what [`crate::corpus::Class::name`] reports
    /// whatever the text's `class:` says.
    fn fields(universal: &str, classes: &[(&str, &str)]) -> ProseFields {
        let parsed: Vec<crate::corpus::Class> = classes
            .iter()
            .map(|(name, text)| crate::corpus::Class::parse(format!("{name}.ont.yml"), *text))
            .collect();
        ProseFields::from_declarations(
            universal_prose(universal),
            parsed.iter().map(declared).collect::<Vec<_>>(),
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

    /// The file stem is the class, whatever the file's `class:` says.
    ///
    /// This reader preferred the declared field until #1116, which filed the declaration
    /// under a name no node resolves to: `class_of` reads an instance's parent directory,
    /// and the directory matches the stem. The argument is on [`crate::corpus::Class::name`].
    #[test]
    fn a_class_declaring_another_name_is_keyed_by_its_file() {
        let f = fields("", &[("finding", "class: measure\nprose: [findings]\n")]);
        assert_eq!(f.for_class("finding"), ["description", "findings"]);
        assert_eq!(
            f.for_class("measure"),
            ["description"],
            "the declared name must not carry the declaration"
        );
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
