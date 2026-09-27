//! The corpus as a typed gluon value, and the computed table as one coming back.
//!
//! # A value, not a scratch tree
//!
//! RFC-0042 §"The calculator receives a typed corpus": the shell arm checks the declared
//! `reads` out of a commit into a directory and points a process at it, because a process can
//! only be handed bytes. This arm hands over a *value*, so there is nothing else the step could
//! have read — which is the difference between the two arms rather than a convenience.
//!
//! Everything here is a projection of types that already exist: [`crate::corpus::Node`],
//! [`crate::corpus::Class`] and the `signals:` table [`crate::computed`] reads. Nothing invents
//! a second model of a node. What it does invent is a *typed* reading of the one untyped field
//! in that model, and that is the whole difficulty.
//!
//! # `properties:` is a YAML mapping, and this arm is statically typed
//!
//! [`yidam_core::corpus::CorpusInstance::properties`] is held untyped on purpose — *"the
//! ontology is the type"* — so a projection into a language with types has to choose a reading
//! of it. [`Value`] is that choice: the scalar cases a class can declare, plus two constructors
//! that name what could not be read rather than hiding it.
//!
//! [`Value::Empty`] is a key written with no value. [`Value::Unrepresentable`] is a sequence or
//! a nested mapping — a shape the ontology's property types (`string`, `text`, `date`,
//! `number`, `ref`, `claim`) do not describe and which this arm therefore cannot type. Both are
//! constructors a script can match on. Neither is silence: a projection that dropped those keys
//! would let a calculator compute over a corpus it could not see the whole of and never say so,
//! and a calculator that is wrong about its input is the one failure a receipt cannot catch.
//!
//! `Int` and `Float` are separate constructors because [`crate::computed`] keeps them separate:
//! its `scalar` reads an integer as an integer, and a `Value` that carried only `Float` would
//! turn every `tier: 3` a calculator emits into `3.0` — a narrowing nothing would report.

use gluon_codegen::{Getable, Pushable, VmType};

use crate::corpus::{Class, Node as CorpusNode};

/// One property value, as far as a type can read it. See the module note.
#[derive(Debug, Clone, PartialEq, Getable, Pushable, VmType)]
pub enum Value {
    Text(String),
    Int(i32),
    Number(f64),
    Flag(bool),
    /// A key written with no value.
    Empty,
    /// A sequence or a nested mapping: a shape no declared property type describes.
    Unrepresentable,
}

/// One entry of a node's `properties:`.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Property {
    pub key: String,
    pub value: Value,
}

/// One outgoing edge, as the instance wrote it.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Link {
    pub target: String,
    pub relationship: String,
}

/// One property a class declares.
///
/// `kind` and not `type`: `type` is a gluon keyword, so a field spelled that way would produce
/// a record no script can select from.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct ClassProperty {
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub prose: bool,
}

/// One class of the ontology.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct ClassView {
    pub name: String,
    pub description: String,
    pub properties: Vec<ClassProperty>,
}

/// One corpus node.
///
/// `id` is the reference form [`crate::computed`] keys a row by — `class/name`, RFC-0032's
/// grammar with no corpus and no revision — so a calculator's output round-trips through the
/// reader without the script having to construct a second spelling of the same name.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct NodeView {
    pub id: String,
    pub class: String,
    pub label: String,
    pub description: String,
    pub properties: Vec<Property>,
    pub links: Vec<Link>,
    pub references: Vec<String>,
    /// Whether the instance file did not parse.
    ///
    /// Carried rather than filtered out, for the reason [`crate::corpus::parse_or_default`]
    /// gives for keeping the node at all: a corpus with one bad file is still a corpus, and a
    /// calculator handed a silently shorter `nodes` would compute a total that is wrong by a
    /// node nobody mentioned.
    pub malformed: bool,
}

/// What the entry point is applied to.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Corpus {
    pub nodes: Vec<NodeView>,
    pub classes: Vec<ClassView>,
}

/// One signal a row carries.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Signal {
    pub name: String,
    pub value: Value,
}

/// One row of the `signals:` table, keyed by the node it is about.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Row {
    pub node: String,
    pub values: Vec<Signal>,
}

/// What the entry point returns.
///
/// No `format_version`. The version of the signal-table contract is
/// [`crate::computed::FORMAT_VERSION`] and it belongs to the binary that writes the file, not
/// to a script a corpus committed — a script that could declare it could declare one this
/// binary does not read, which `Signals::load` would then refuse for a reason the calculator
/// author never chose.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Computed {
    pub signals: Vec<Row>,
}

// Dead in the library and not in the tests, which is exactly the scope RFC-0042's kill
// criterion asks for: the engine is gated and finished, and nothing in `run` or `regen` calls
// it yet. `tests/gluon_arm.rs` and the unit tests below are the only callers today; the surface
// that will be the real one is a later change, and the day it lands these three lose the
// attribute rather than gain a caller nobody asked for.
/// `.yidam/corpus/<class>/<name>.yml` as `<class>/<name>`.
#[allow(dead_code)]
fn id_of(rel: &str) -> String {
    rel.strip_prefix(".yidam/corpus/")
        .unwrap_or(rel)
        .strip_suffix(".yml")
        .unwrap_or(rel)
        .to_string()
}

/// One YAML value as far as [`Value`] can read it. See the module note.
#[allow(dead_code)]
fn value_of(v: &serde_yaml::Value) -> Value {
    match v {
        serde_yaml::Value::Null => Value::Empty,
        serde_yaml::Value::Bool(b) => Value::Flag(*b),
        serde_yaml::Value::String(s) => Value::Text(s.clone()),
        // `i32` and not `i64`, because gluon's `Int` is what the script will see and widening
        // here would make a value that typechecks and then does not fit. A number outside the
        // range is a `Number`, which is lossy and says so, rather than a wrap that does not.
        serde_yaml::Value::Number(n) => {
            match (n.as_i64().and_then(|i| i32::try_from(i).ok()), n.as_f64()) {
                (Some(i), _) => Value::Int(i),
                (None, Some(f)) => Value::Number(f),
                _ => Value::Unrepresentable,
            }
        }
        _ => Value::Unrepresentable,
    }
}

/// The corpus in hand as the value the entry point is applied to.
///
/// Takes the already-parsed nodes and classes rather than a root path: this arm exists because
/// the CLI *already holds* the corpus, and a projection that re-read it from disk would give a
/// calculator a second, later reading of the tree than the one the receipt attests to.
#[allow(dead_code)]
pub(crate) fn project(nodes: &[CorpusNode], classes: &[Class]) -> Corpus {
    Corpus {
        nodes: nodes
            .iter()
            .map(|n| NodeView {
                id: id_of(&n.rel),
                class: n.inst.class.clone().unwrap_or_default(),
                label: n.inst.label.clone().unwrap_or_default(),
                description: n.inst.description.clone().unwrap_or_default(),
                properties: n
                    .inst
                    .properties
                    .iter()
                    .flatten()
                    .map(|(k, v)| Property {
                        key: k.as_str().unwrap_or_default().to_string(),
                        value: value_of(v),
                    })
                    .collect(),
                links: n
                    .inst
                    .links
                    .iter()
                    .flatten()
                    .map(|l| Link {
                        target: l.target.clone().unwrap_or_default(),
                        relationship: l.relationship.clone().unwrap_or_default(),
                    })
                    .collect(),
                references: n.inst.references.clone().unwrap_or_default(),
                malformed: n.malformed.is_some(),
            })
            .collect(),
        classes: classes
            .iter()
            .map(|c| ClassView {
                name: c.name.clone(),
                description: c.description.clone(),
                properties: c
                    .properties
                    .iter()
                    .map(|p| ClassProperty {
                        name: p.name.clone(),
                        kind: p.r#type.clone(),
                        required: p.required,
                        prose: p.prose,
                    })
                    .collect(),
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A node as the corpus reader would hand it over: parsed from its own bytes.
    fn node(rel: &str, yaml: &str) -> CorpusNode {
        CorpusNode::parse(std::path::PathBuf::from(rel), rel, yaml)
    }

    #[test]
    fn a_node_projects_to_the_reference_a_computed_row_is_keyed_by() {
        let nodes = [node(
            ".yidam/corpus/gage/canyon-outlet.yml",
            "class: gage\nlabel: Canyon Outlet\ndescription: A gage\n",
        )];
        let c = project(&nodes, &[]);
        assert_eq!(c.nodes.len(), 1);
        // `class/name`, which is what `computed::node_path` reads back. A projection emitting
        // the `rel` would hand a calculator a key its own output could not be read under.
        assert_eq!(c.nodes[0].id, "gage/canyon-outlet");
        assert_eq!(c.nodes[0].class, "gage");
        assert_eq!(c.nodes[0].label, "Canyon Outlet");
    }

    #[test]
    fn every_property_shape_gets_a_constructor_and_none_is_dropped() {
        let nodes = [node(
            ".yidam/corpus/gage/one.yml",
            "class: gage\nproperties:\n  words: a phrase\n  count: 3\n  ratio: 2.5\n  \
             active: true\n  blank:\n  listed: [a, b]\n  nested: { k: v }\n",
        )];
        let c = project(&nodes, &[]);
        let got: Vec<(&str, &Value)> = c.nodes[0]
            .properties
            .iter()
            .map(|p| (p.key.as_str(), &p.value))
            .collect();
        // Seven keys in, seven out. The two the ontology's property types do not describe are
        // named rather than dropped — see the module note on why silence is the one failure a
        // receipt cannot catch.
        assert_eq!(
            got,
            vec![
                ("words", &Value::Text("a phrase".to_string())),
                ("count", &Value::Int(3)),
                ("ratio", &Value::Number(2.5)),
                ("active", &Value::Flag(true)),
                ("blank", &Value::Empty),
                ("listed", &Value::Unrepresentable),
                ("nested", &Value::Unrepresentable),
            ]
        );
    }

    #[test]
    fn a_malformed_node_is_carried_and_marked() {
        let nodes = [node(
            ".yidam/corpus/gage/bad.yml",
            "class: gage\n  : broken\n",
        )];
        let c = project(&nodes, &[]);
        assert_eq!(
            c.nodes.len(),
            1,
            "a corpus with one bad file is still a corpus"
        );
        assert!(c.nodes[0].malformed);
    }

    #[test]
    fn a_class_projects_its_declared_properties() {
        let classes = [Class::parse(
            ".yidam/corpus/gage.ont.yml",
            "description: A stream gage\nproperties:\n  - name: miles\n    type: number\n    \
             required: true\n  - name: method\n    type: text\n    prose: true\n",
        )];
        let c = project(&[], &classes);
        assert_eq!(c.classes.len(), 1);
        assert_eq!(c.classes[0].name, "gage");
        let p = &c.classes[0].properties;
        assert_eq!(p.len(), 2);
        assert_eq!(
            (p[0].name.as_str(), p[0].kind.as_str(), p[0].required),
            ("miles", "number", true)
        );
        assert!(p[1].prose);
    }
}
