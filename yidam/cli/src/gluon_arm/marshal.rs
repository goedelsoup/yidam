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
    /// `i64`, because gluon's `Int` is, and the marshalling in between is not checked.
    ///
    /// `gluon_vm`'s `int_impls!` gives every integer width `Type = VmInt` and converts with a
    /// cast, so an `i32` here would take `Int 3000000000` back from a script as `705032704` —
    /// silently, and in the one direction a receipt cannot catch. It also matches
    /// [`crate::computed`], whose `scalar` keeps a signal integer at `i64`; narrowing here and
    /// widening there would make a calculator's answer depend on which side read it.
    Int(i64),
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

/// `.yidam/corpus/<class>/<name>.yml` as `<class>/<name>`.
fn id_of(rel: &str) -> String {
    // Each fallback is to the value that reached *that* step, not to the argument. Chaining
    // both onto `rel` throws the prefix away again whenever the suffix does not match, which
    // is a wrong id rather than an error — the one failure shape the module note is about.
    let rel = rel.strip_prefix(".yidam/corpus/").unwrap_or(rel);
    rel.strip_suffix(".yml").unwrap_or(rel).to_string()
}

/// A property key as the instance wrote it.
///
/// `properties:` is an untyped mapping, so YAML will hand back a key that is not a string:
/// `2024:` is a number and `true:` is a bool. Reading those through `as_str()` made them the
/// empty string, which is a key the ontology cannot have declared and two such keys colliding
/// with each other — the silent loss [`Value::Unrepresentable`] exists to refuse. Every scalar
/// is rendered as written, and the shapes that are not scalars are rendered rather than dropped.
fn key_of(k: &serde_yaml::Value) -> String {
    match k {
        serde_yaml::Value::String(s) => s.clone(),
        serde_yaml::Value::Bool(b) => b.to_string(),
        serde_yaml::Value::Number(n) => n.to_string(),
        serde_yaml::Value::Null => "~".to_string(),
        // A sequence or a mapping used as a key. Legal YAML, not something an ontology declares,
        // and still better named than blank: the calculator that meets one should be able to
        // print what it met.
        other => serde_yaml::to_string(other)
            .map(|s| s.split_whitespace().collect::<Vec<_>>().join(" "))
            .unwrap_or_else(|e| format!("<unreadable key: {e}>")),
    }
}

/// One YAML value as far as [`Value`] can read it. See the module note.
fn value_of(v: &serde_yaml::Value) -> Value {
    match v {
        serde_yaml::Value::Null => Value::Empty,
        serde_yaml::Value::Bool(b) => Value::Flag(*b),
        serde_yaml::Value::String(s) => Value::Text(s.clone()),
        // An integer stays an integer at the width gluon actually uses. A number too large for
        // even that — YAML admits a `u64` past `i64::MAX` — is a `Number`, which is lossy and
        // says so, rather than a wrap that does not.
        serde_yaml::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Value::Int(i),
            (None, Some(f)) => Value::Number(f),
            _ => Value::Unrepresentable,
        },
        _ => Value::Unrepresentable,
    }
}

/// The corpus in hand as the value the entry point is applied to.
///
/// Takes the already-parsed nodes and classes rather than a root path: this arm exists because
/// the CLI *already holds* the corpus, and a projection that re-read it from disk would give a
/// calculator a second, later reading of the tree than the one the receipt attests to.
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
                        key: key_of(k),
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

/// What the entry point returned, as the `signals:` file [`crate::computed`] reads.
///
/// # The writer this contract never had
///
/// `.yidam/computed/*.yml` has been read by Rust and written only by shell since #471: the example's
/// calculators `printf` the document, and [`crate::computed::Signals::load`] parses it. That was
/// fine while every calculator was a process — a process can only hand back bytes, so the bytes
/// were the interface. A typed calculator hands back a value, so this is where the document is
/// produced, and it is the first time the producing side of the contract is in the binary that owns
/// [`crate::computed::FORMAT_VERSION`].
///
/// Built as a [`serde_yaml::Value`] and serialized, rather than `write!` by line. The reader is
/// `serde_yaml`, so the writer being anything else is two spellings of one format — and a signal
/// whose value is `a: b` or `#` or an empty string is a quoting question a `printf` gets wrong once
/// and then in every committed file.
///
/// # What is refused
///
/// Three shapes, and each is a committed file that would be silently wrong rather than absent:
///
/// - A row with no node. `signals:` is keyed by the node each row is about, and a row keyed by the
///   empty string is a signal attached to nothing that reads as a signal attached to something.
/// - Two rows for one node. The reader attaches a row to the node it names; two would make the
///   answer depend on which came last, which is the script's iteration order and not the corpus's.
/// - A [`Value::Unrepresentable`] in a signal. Inbound that constructor is load-bearing: it names a
///   property shape no declared type describes, which is the thing a calculator must be able to see
///   rather than have hidden. Outbound it says the script is emitting a value it cannot state.
///   Rendering it as a string would make it a signal, and a search would find it.
///
/// A refusal here fails the step, so nothing is committed and nothing is merged — which is the
/// whole difference between this and a `printf` that wrote the file anyway.
pub(crate) fn render(step: &str, script: &str, computed: &Computed) -> anyhow::Result<String> {
    use anyhow::bail;
    use serde_yaml::{Mapping, Value as Yaml};

    let mut rows = Vec::with_capacity(computed.signals.len());
    let mut seen: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    for row in &computed.signals {
        if row.node.is_empty() {
            bail!(
                "`{step}` emitted a row with no `node`, and a `signals:` row is keyed by the node \
                 it is about. A row keyed by nothing reads as a row about something."
            );
        }
        if !seen.insert(row.node.as_str()) {
            bail!(
                "`{step}` emitted two rows for `{}`. The reader attaches a row to the node it \
                 names, so two would make the answer depend on the order the script produced \
                 them in.",
                row.node
            );
        }
        let mut m = Mapping::new();
        m.insert(Yaml::from("node"), Yaml::from(row.node.clone()));
        for signal in &row.values {
            if signal.name.is_empty() {
                bail!(
                    "`{step}` emitted an unnamed signal on `{}`. A signal's name is what reaches a \
                     search, and the empty one reaches nothing.",
                    row.node
                );
            }
            if signal.name == "node" {
                bail!(
                    "`{step}` emitted a signal named `node` on `{}`, which is the key the row is \
                     already keyed by.",
                    row.node
                );
            }
            let value = match &signal.value {
                Value::Text(s) => Yaml::from(s.clone()),
                Value::Int(i) => Yaml::from(*i),
                Value::Number(f) => Yaml::from(*f),
                Value::Flag(b) => Yaml::from(*b),
                // Both constructors are load-bearing *inbound* and meaningless outbound, and the
                // reader agrees: `Signals::load`'s `scalar` reads neither a null nor a non-scalar as
                // a signal, and reports the row as declaring "a list or a mapping" instead. Rendering
                // either one would put a line in a committed file that the consumer of that file
                // refuses — a document written by this binary and unreadable by it.
                //
                // Inbound, `Empty` is a property key written with no value and `Unrepresentable` is a
                // shape no declared property type describes, and a calculator has to be able to see
                // both rather than have them hidden. Outbound they say the script is emitting a
                // signal it has no value for.
                Value::Empty => bail!(
                    "`{step}` emitted `{}` on `{}` as `Empty`, which is a signal with no value. \
                     Inbound that constructor is a property key written with nothing after it; \
                     outbound there is nothing for a reader to attach, and `Signals::load` refuses \
                     the row rather than reading it. A calculator with nothing to say about a node \
                     omits the signal.",
                    signal.name,
                    row.node
                ),
                Value::Unrepresentable => bail!(
                    "`{step}` emitted `{}` on `{}` as `Unrepresentable`, which is the \
                     constructor for a value that cannot be stated. It is how this arm reports a \
                     property shape no declared type describes; a calculator returning one is \
                     emitting a signal it has no value for, and writing it out as text would make \
                     it findable as though it did.",
                    signal.name,
                    row.node
                ),
            };
            if m.insert(Yaml::from(signal.name.clone()), value).is_some() {
                bail!(
                    "`{step}` emitted `{}` twice on `{}`.",
                    signal.name,
                    row.node
                );
            }
        }
        rows.push(Yaml::Mapping(m));
    }

    // `method:` and not a prose block. A shell calculator writes the rule it implements there,
    // because the rule is in its comments and nowhere a reader can reach; a typed calculator's rule
    // is the script, which is a tracked file with a name — so what this says is which file, and the
    // reader who wants the rule reads it.
    //
    // No digest of the script here, though the receipt carries one. The receipt lands in the same
    // commit, and a fact recorded twice in one tree is the shape `receipt`'s "No clock" note refuses.
    let mut method = Mapping::new();
    method.insert(Yaml::from("arm"), Yaml::from("gluon"));
    method.insert(Yaml::from("script"), Yaml::from(script));

    let mut root = Mapping::new();
    // First, and by insertion order: `serde_yaml` preserves it, and `format_version` leading the
    // file is what `receipt.rs`'s own test asserts about the other record format this crate writes.
    root.insert(
        Yaml::from("format_version"),
        Yaml::from(crate::computed::FORMAT_VERSION),
    );
    root.insert(Yaml::from("method"), Yaml::Mapping(method));
    root.insert(Yaml::from("signals"), Yaml::Sequence(rows));

    let body = serde_yaml::to_string(&Yaml::Mapping(root))?;
    Ok(format!(
        "# Computed by the `{step}` calculator (RFC-0042's typed arm) and committed by `yidam run`.\n\
         # Recomputed from the corpus; edit the corpus, not this file.\n\
         {body}"
    ))
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

    #[test]
    fn a_rel_that_is_not_a_yml_keeps_its_prefix_stripped() {
        // Chaining both fallbacks onto the argument made a path the second strip missed come
        // back whole — a wrong id rather than an error, under which a calculator's answer is
        // filed against a node nothing can find.
        assert_eq!(id_of(".yidam/corpus/gage/one.yml"), "gage/one");
        assert_eq!(id_of(".yidam/corpus/gage/one.yaml"), "gage/one.yaml");
        assert_eq!(id_of("gage/one.yml"), "gage/one");
        assert_eq!(id_of("gage/one"), "gage/one");
    }

    #[test]
    fn a_key_yaml_did_not_read_as_a_string_is_still_named() {
        // `properties:` is untyped, so YAML hands back a number for `2024:` and a bool for
        // `true:`. Read through `as_str()` both were the empty string, which collides them with
        // each other and with any other unreadable key.
        let nodes = [node(
            ".yidam/corpus/gage/one.yml",
            "class: gage\nproperties:\n  2024: a year\n  true: a flag\n  1.5: a ratio\n  \
             ? [a, b]\n  : a sequence\n",
        )];
        let c = project(&nodes, &[]);
        let keys: Vec<&str> = c.nodes[0]
            .properties
            .iter()
            .map(|p| p.key.as_str())
            .collect();
        assert_eq!(keys.len(), 4, "a key was dropped: {keys:?}");
        assert!(
            keys.iter().all(|k| !k.is_empty()),
            "a key collapsed to the empty string: {keys:?}"
        );
        assert_eq!(&keys[..3], &["2024", "true", "1.5"]);
    }

    #[test]
    fn an_integer_wider_than_i32_is_an_integer() {
        let nodes = [node(
            ".yidam/corpus/gage/one.yml",
            "class: gage\nproperties:\n  big: 3000000000\n  huge: 18446744073709551615\n",
        )];
        let c = project(&nodes, &[]);
        let got: Vec<&Value> = c.nodes[0].properties.iter().map(|p| &p.value).collect();
        // The first fits `i64` and stays exact. The second is a `u64` past `i64::MAX`, which no
        // integer gluon has can hold, so it is a `Number` — lossy, and saying so.
        assert_eq!(got[0], &Value::Int(3_000_000_000));
        assert!(matches!(got[1], Value::Number(_)), "{:?}", got[1]);
    }

    // ── render ───────────────────────────────────────────────────────────────

    fn row(node: &str, values: &[(&str, Value)]) -> Row {
        Row {
            node: node.to_string(),
            values: values
                .iter()
                .map(|(n, v)| Signal {
                    name: (*n).to_string(),
                    value: v.clone(),
                })
                .collect(),
        }
    }

    /// A corpus holding `nodes`, a manifest declaring `step`, and `text` as what `step` computed.
    ///
    /// The whole fixture rather than the file alone, because [`crate::computed::Signals::load`]
    /// resolves each row against the corpus and attributes each file to the capability that declares
    /// writing it. A row keyed on a node that is not there is dropped and reported, so a test handed
    /// only the document would assert that the reader ignores it.
    fn read_back(step: &str, nodes: &[&str], text: &str) -> crate::computed::Signals {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join(".yidam/computed")).unwrap();
        std::fs::write(root.join(format!(".yidam/computed/{step}.yml")), text).unwrap();
        for node in nodes {
            let path = root.join(format!(".yidam/corpus/{node}.yml"));
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let class = node.split('/').next().unwrap_or_default();
            std::fs::write(&path, format!("class: {class}\nlabel: {node}\n")).unwrap();
        }
        std::fs::write(
            root.join(".yidam/capabilities.toml"),
            format!(
                "[capability.{step}]\nkind = \"calculator\"\n\
                 run = {{ gluon = \".yidam/capabilities/{step}.glu\" }}\n\
                 reads = [\".yidam/corpus/**\", \".yidam/capabilities/**\"]\n\
                 writes = [\".yidam/computed/**\"]\nverb = \"compute\"\n"
            ),
        )
        .unwrap();
        let s = crate::computed::Signals::load(root);
        assert!(s.problems.is_empty(), "{:?}\n{text}", s.problems);
        s
    }

    /// The whole point of having a writer in the binary that owns the reader: the two agree.
    ///
    /// Written to a real directory and read back by [`crate::computed::Signals::load`], because that
    /// is the consumer. A test that parsed the YAML itself would assert that `serde_yaml` round-trips,
    /// which nobody doubted — what was in doubt is whether this document is the one the embedder,
    /// `doctor` and `retrieve` can attach to a node.
    #[test]
    fn what_a_calculator_returns_is_read_back_by_the_reader_of_the_contract() {
        let computed = Computed {
            signals: vec![
                row(
                    "gage/canyon-outlet",
                    &[
                        ("travels_as", Value::Text("verified".into())),
                        ("links", Value::Int(3)),
                        ("q95", Value::Number(12.5)),
                        ("downgraded", Value::Flag(false)),
                    ],
                ),
                row(
                    "gage/upper-fork",
                    &[("travels_as", Value::Text("open".into()))],
                ),
            ],
        };
        let text = render(
            "travel-tier",
            ".yidam/capabilities/travel-tier.glu",
            &computed,
        )
        .unwrap();
        let s = read_back(
            "travel-tier",
            &["gage/canyon-outlet", "gage/upper-fork"],
            &text,
        );
        assert_eq!(s.nodes(), 2, "{text}");
        // Keyed by the repository-relative path, which is what `Signals` resolves the reference
        // grammar to — see [`crate::computed::Signals::by_node`].
        let got = s.for_node(".yidam/corpus/gage/canyon-outlet.yml");
        assert_eq!(
            got.get("travels_as").and_then(|v| v.as_str()),
            Some("verified"),
            "{text}"
        );
        // The integer stays an integer through both sides. `Value::Int` exists for this: a single
        // `Number` constructor would have made `links: 3` read back as `3.0`.
        assert_eq!(got.get("links").and_then(|v| v.as_i64()), Some(3), "{text}");
        assert_eq!(got.get("downgraded").and_then(|v| v.as_bool()), Some(false));
    }

    /// The file says which version of the contract it is, and names its own method.
    #[test]
    fn the_document_leads_with_the_version_of_the_contract() {
        let text = render(
            "x",
            ".yidam/capabilities/x.glu",
            &Computed { signals: vec![] },
        )
        .unwrap();
        let body: Vec<&str> = text.lines().filter(|l| !l.starts_with('#')).collect();
        assert_eq!(body[0], "format_version: 1", "{text}");
        assert!(text.contains("arm: gluon"), "{text}");
        assert!(text.contains("script: .yidam/capabilities/x.glu"), "{text}");
        // And not the script's digest, though the receipt carries one. Both land in the same commit,
        // and a fact recorded twice in one tree is what `receipt`'s "No clock" note refuses.
        assert!(!text.contains("sha256"), "{text}");
    }

    /// A value that needs quoting gets it, which is the argument for a serializer over a `printf`.
    #[test]
    fn a_signal_that_would_break_a_printf_is_quoted() {
        let computed = Computed {
            signals: vec![row(
                "gage/one",
                &[
                    ("note", Value::Text("a: b # not a comment".into())),
                    ("blank", Value::Text(String::new())),
                ],
            )],
        };
        let text = render("x", ".yidam/capabilities/x.glu", &computed).unwrap();
        let s = read_back("x", &["gage/one"], &text);
        assert_eq!(
            s.for_node(".yidam/corpus/gage/one.yml")
                .get("note")
                .and_then(|v| v.as_str()),
            Some("a: b # not a comment"),
            "{text}"
        );
    }

    /// Three outputs that would be silently wrong rather than absent, each refused by name.
    ///
    /// A refusal here fails the step, so nothing is committed — which is the whole difference between
    /// this and a shell calculator that wrote the file anyway.
    #[test]
    fn a_document_that_would_be_silently_wrong_is_refused() {
        let cases: [(Computed, &str); 6] = [
            (
                Computed {
                    signals: vec![row("", &[("a", Value::Int(1))])],
                },
                "no `node`",
            ),
            (
                Computed {
                    signals: vec![
                        row("gage/one", &[("a", Value::Int(1))]),
                        row("gage/one", &[("a", Value::Int(2))]),
                    ],
                },
                "two rows",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("", Value::Int(1))])],
                },
                "unnamed signal",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("node", Value::Int(1))])],
                },
                "already keyed by",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("a", Value::Unrepresentable)])],
                },
                "Unrepresentable",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("a", Value::Empty)])],
                },
                "signal with no value",
            ),
        ];
        for (computed, wanted) in cases {
            let e = render("x", ".yidam/capabilities/x.glu", &computed)
                .unwrap_err()
                .to_string();
            assert!(e.contains(wanted), "expected {wanted:?} in: {e}");
        }
    }
}
