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
//!
//! # A pipeline's second stage, and why its signals are not on the nodes
//!
//! [`Corpus::signals`] is what a calculator has already been told by the calculators that ran
//! before it (#1105). Without it the arm reached exactly the calculators that read only the
//! graph: `examples/streamflow`'s `disclosure-envelope` declares
//! `reads = [".yidam/computed/travel-tier.yml"]` and its whole job is to partition the corpus by
//! an answer another capability committed, so a typed entry point handed no signals had no input
//! for the rule it would compute.
//!
//! It is a field on [`Corpus`] and not one on [`NodeView`], and the reason is the declaration
//! rather than taste. The tree a step stands in holds exactly what its `reads` resolve to, so a
//! second stage that reads one computed file and no corpus path is handed signals about nodes
//! that are *not* in `nodes` — which is the case this field exists for. Attaching them to the
//! node would mean a typed calculator could only reach a previous step's answer by also
//! declaring `.yidam/corpus/**`, so the typed arm would need a **wider** declaration than the
//! shell arm to express the same rule. That is backwards for an arm whose whole claim is that
//! `reads` bounds the value exactly.
//!
//! The join a calculator wants is therefore its own, and it is cheap: [`Row::node`] and
//! [`NodeView::id`] are the same reference form, so a script that wants both matches one against
//! the other. [`Row`] and [`Signal`] again, not a second pair of record types — a calculator
//! reads signals in the shape it writes them, which is what makes two stages of a pipeline
//! composable at all.

use gluon_codegen::{Getable, Pushable, VmType};

use crate::computed::Signals;
use crate::corpus::{Class, Edges, Node as CorpusNode};

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

/// One outgoing edge: as the instance wrote it, and where it lands.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Link {
    /// The `target:` as written — a path relative to the authoring node's own directory.
    pub target: String,
    pub relationship: String,
    /// The corpus node this link lands on, as [`NodeView::id`] spells it, or `None`.
    ///
    /// **The reason this field exists is #1080.** `target:` is `../gage/valley-bridge.yml`, and
    /// turning that into a node takes the authoring node's directory, `.` and `..` normalization,
    /// and a lookup — which `examples/streamflow`'s calculator used to do in an awk
    /// `function resolve(base, t)` that was *"a second answer to the question `corpus/edges.rs`
    /// already answers and which nothing compared against it"*. #1080 deleted it by handing the
    /// shell arm a resolved graph. A typed arm that shipped the written target alone would put
    /// that resolver back, in a language with no string library to write it in, and RFC-0042's
    /// claim that this arm has *"no parse to get wrong"* would be false in the one place it is
    /// tested. [`crate::corpus::Edges`] is the single resolver, and this is its answer.
    ///
    /// `None` is the link that lands on no node *here* — a class definition, a catalog anchor, or
    /// a target that is not there at all. A constructor and not the empty string, because a
    /// calculator has to be able to count them: the chain rule that silently skipped them would
    /// compute a tier travelling further than its evidence, which is the one direction that rule
    /// exists to prevent.
    pub resolved: Option<String>,
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

/// What the entry point is applied to.
#[derive(Debug, Clone, Getable, Pushable, VmType)]
pub struct Corpus {
    pub nodes: Vec<NodeView>,
    pub classes: Vec<ClassView>,
    /// What the calculators that ran before this one committed, as
    /// [`crate::computed::Signals`] reads it back.
    ///
    /// The same [`Row`] the entry point returns, so a stage of a pipeline reads its input in the
    /// shape the stage before it wrote — see the module note for why this is here and not on
    /// [`NodeView`], and for the join a calculator does itself.
    ///
    /// Bounded by the step's `reads` and by nothing else: a calculator that declares no
    /// `.yidam/computed/` path is handed `[]`, which is the same answer as a corpus where nothing
    /// has been computed. A script cannot tell those apart and does not need to — in both, there
    /// is nothing it was told.
    pub signals: Vec<Row>,
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
    /// What the calculator counted about the run as a whole.
    ///
    /// The `signals:` table says one thing about one node, and there are facts a calculator
    /// computes that are about none of them — how many nodes it saw, how many its rule moved,
    /// how many links pointed outside the corpus. `examples/streamflow`'s shell calculators
    /// `printf` exactly that block, and it is the reason this field exists rather than those
    /// counts being attached to some node: *"a property on a node reads as a fact about the
    /// subject when it is a fact about a calculation"*, and a corpus-wide count attached to one
    /// node is that sentence one level up.
    ///
    /// [`Signal`] again, and not a second record type. A summary entry is a name and a value
    /// about one subject, exactly as a row's is; the subject is the run instead of a node. Reusing
    /// it means the refusals are one function — see [`signal_value`] — so a `summary:` cannot
    /// commit a shape a row could not.
    ///
    /// Not read by [`crate::computed::Signals::load`], on purpose. That module allows unknown
    /// keys because *"a computed file's other keys are a calculator's own account of its method
    /// and its summary, which this module has no business having an opinion about"* — so this
    /// writes the block a person and a diff read, and claims no signal name with it.
    pub summary: Vec<Signal>,
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

/// One committed signal as far as [`Value`] can read it.
///
/// A second function beside [`value_of`] because it reads a different thing: a property is YAML as
/// the instance wrote it, and a signal has already been through
/// [`crate::computed::Signals`]'s own reading of the table. That reader admits four shapes and
/// refuses the rest — it reports a row declaring `null`, a list or a mapping as a problem and
/// carries no value for it — so the four arms below are the whole of what can arrive here.
///
/// The last arm is therefore unreachable rather than a case, and it is `Unrepresentable` and not
/// a `panic!` or a silent drop for the reason [`Value`] exists at all: a calculator that cannot
/// see the whole of its input has to be *told*, and a constructor it can match on is how. If a
/// later reader admits a shape this one cannot type, a script sees the constructor that says so
/// instead of a signal that quietly is not there.
fn signal_read(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::String(s) => Value::Text(s.clone()),
        serde_json::Value::Bool(b) => Value::Flag(*b),
        serde_json::Value::Number(n) => match (n.as_i64(), n.as_f64()) {
            (Some(i), _) => Value::Int(i),
            (None, Some(f)) => Value::Number(f),
            _ => Value::Unrepresentable,
        },
        serde_json::Value::Null => Value::Empty,
        _ => Value::Unrepresentable,
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
///
/// `signals` is the same: [`Signals::in_reads`] over the tree the step stands in, read once by
/// the caller. See [`Corpus::signals`] for what a calculator does with it.
pub(crate) fn project(
    nodes: &[CorpusNode],
    classes: &[Class],
    edges: &Edges,
    signals: &Signals,
) -> Corpus {
    Corpus {
        nodes: nodes
            .iter()
            .enumerate()
            .map(|(i, n)| NodeView {
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
                // Zipped by position rather than re-resolved. `Edges::out` is indexed by the
                // node's position in `nodes` and each `Edge` carries the index of the `links:`
                // entry it came from, so the written form and the resolved one are two readings of
                // one entry — which is what stops this projection from being a second resolver.
                links: {
                    let resolved = edges.out(i);
                    n.inst
                        .links
                        .iter()
                        .flatten()
                        .enumerate()
                        .map(|(j, l)| Link {
                            target: l.target.clone().unwrap_or_default(),
                            relationship: l.relationship.clone().unwrap_or_default(),
                            resolved: resolved
                                .iter()
                                .find(|e| e.index == j)
                                .and_then(|e| e.to)
                                .map(|k| id_of(&nodes[k].rel)),
                        })
                        .collect()
                },
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
        signals: signals
            .by_reference()
            .map(|(node, values)| Row {
                node: node.to_string(),
                values: values
                    .iter()
                    .map(|(name, v)| Signal {
                        name: name.clone(),
                        value: signal_read(v),
                    })
                    .collect(),
            })
            .collect(),
    }
}

/// One signal's value as the reader reads it, or the refusal that says why it cannot be written.
///
/// `place` is where the signal sat, as the message says it: ``on `gage/one` `` for a row of the
/// table, ``in its `summary:` `` for a count about the run. One function for both, because each
/// refusal below is a property of [`Value`] and of what [`crate::computed::Signals::load`] reads
/// back — not of which key the signal is under. A `summary:` that could commit a shape a row
/// cannot would be a second answer about the same constructors.
///
/// Both refused constructors are load-bearing *inbound* and meaningless outbound, and the reader
/// agrees: `Signals::load`'s `scalar` reads neither a null nor a non-scalar as a signal, and
/// reports the row as declaring "a list or a mapping" instead. Rendering either one would put a
/// line in a committed file that the consumer of that file refuses — a document written by this
/// binary and unreadable by it.
///
/// Inbound, `Empty` is a property key written with no value and `Unrepresentable` is a shape no
/// declared property type describes, and a calculator has to be able to see both rather than have
/// them hidden. Outbound they say the script is emitting a signal it has no value for.
fn signal_value(step: &str, place: &str, signal: &Signal) -> anyhow::Result<serde_yaml::Value> {
    use anyhow::bail;
    use serde_yaml::Value as Yaml;

    Ok(match &signal.value {
        Value::Text(s) => Yaml::from(s.clone()),
        Value::Int(i) => Yaml::from(*i),
        Value::Number(f) => Yaml::from(*f),
        Value::Flag(b) => Yaml::from(*b),
        Value::Empty => bail!(
            "`{step}` emitted `{}` {place} as `Empty`, which is a signal with no value. \
             Inbound that constructor is a property key written with nothing after it; \
             outbound there is nothing for a reader to attach, and `Signals::load` refuses \
             the row rather than reading it. A calculator with nothing to say about a node \
             omits the signal.",
            signal.name
        ),
        Value::Unrepresentable => bail!(
            "`{step}` emitted `{}` {place} as `Unrepresentable`, which is the \
             constructor for a value that cannot be stated. It is how this arm reports a \
             property shape no declared type describes; a calculator returning one is \
             emitting a signal it has no value for, and writing it out as text would make \
             it findable as though it did.",
            signal.name
        ),
    })
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
///
/// # `summary:`, and why it is not a signal
///
/// [`Computed::summary`] is written as a mapping at the end of the document and is about the run
/// rather than about any node. It claims no signal name — the reader never looks at it — so a
/// count called `nodes` here does not collide with the `nodes` some other calculator keys onto a
/// node, which is the collision `Signals::load` refuses corpus-wide. It is omitted when the script
/// emitted none.
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
            let value = signal_value(step, &format!("on `{}`", row.node), signal)?;
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

    // The run's own counts, refused by the same three rules a row's signals are. Built here rather
    // than inside the loop above because it is about none of the rows.
    let mut summary = Mapping::new();
    for signal in &computed.summary {
        if signal.name.is_empty() {
            bail!(
                "`{step}` emitted an unnamed count in its `summary:`. A summary is a block a \
                 person reads, and a blank key names nothing for them either."
            );
        }
        // No refusal of `node` here, unlike a row's. A row is *keyed* by that word, so a signal
        // spelled the same way would overwrite the key it is attached to; a summary is keyed by
        // nothing, and a calculator counting something it wants to call `node` is not colliding
        // with anything.
        let value = signal_value(step, "in its `summary:`", signal)?;
        if summary
            .insert(Yaml::from(signal.name.clone()), value)
            .is_some()
        {
            bail!(
                "`{step}` emitted `{}` twice in its `summary:`.",
                signal.name
            );
        }
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
    // Last, where both shell calculators put theirs, so a reader who opens the two files in one
    // corpus reads one shape. Omitted when the script emitted none rather than written as an empty
    // mapping: `summary: {}` is a claim that nothing was counted, and absence is the truthful
    // statement that this calculator does not summarize.
    if !summary.is_empty() {
        root.insert(Yaml::from("summary"), Yaml::Mapping(summary));
    }

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

    /// `project`, with the graph resolved from the nodes in hand.
    ///
    /// Built here rather than passed in, because every caller wants the resolution of *these*
    /// nodes — which is also what the executor does: `Corpus::edges()` resolves the tree it
    /// parsed, and a test that handed over some other graph would be projecting a corpus that
    /// does not exist.
    fn projected(nodes: &[CorpusNode], classes: &[Class]) -> Corpus {
        project(nodes, classes, &Edges::build(nodes), &Signals::default())
    }

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
        let c = projected(&nodes, &[]);
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
        let c = projected(&nodes, &[]);
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

    /// A link arrives resolved, and one that lands on no node here says so rather than resolving
    /// to something.
    ///
    /// The whole of #1080's argument, one arm along: `../gage/valley-bridge.yml` becomes
    /// `gage/valley-bridge` here and not in the script, so no calculator writes the normalizer
    /// again. The `instance-of` link in the same file is the negative case — a class definition is
    /// not a corpus node, and a chain rule has to be able to tell.
    #[test]
    fn a_link_arrives_resolved_and_a_target_that_is_no_node_says_so() {
        let nodes = [
            node(
                ".yidam/corpus/reach/lower-canyon.yml",
                "class: reach\nlinks:\n  - target: ../reach.ont.yml\n    \
                 relationship: instance-of\n  - target: ../gage/valley-bridge.yml\n    \
                 relationship: measured-by\n  - target: ./nowhere.yml\n    \
                 relationship: downstream-of\n",
            ),
            node(".yidam/corpus/gage/valley-bridge.yml", "class: gage\n"),
        ];
        let c = projected(&nodes, &[]);
        let got: Vec<(&str, Option<&str>)> = c.nodes[0]
            .links
            .iter()
            .map(|l| (l.relationship.as_str(), l.resolved.as_deref()))
            .collect();
        assert_eq!(
            got,
            vec![
                // A class definition is not a corpus node.
                ("instance-of", None),
                ("measured-by", Some("gage/valley-bridge")),
                // Nor is a target that is not there.
                ("downstream-of", None),
            ]
        );
        // And the written form is still there, because a calculator reporting a broken link has to
        // be able to say what was written rather than that nothing resolved.
        assert_eq!(c.nodes[0].links[2].target, "./nowhere.yml");
    }

    #[test]
    fn a_malformed_node_is_carried_and_marked() {
        let nodes = [node(
            ".yidam/corpus/gage/bad.yml",
            "class: gage\n  : broken\n",
        )];
        let c = projected(&nodes, &[]);
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
        let c = projected(&[], &classes);
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
        let c = projected(&nodes, &[]);
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
        let c = projected(&nodes, &[]);
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

    /// One named value, for a row's `values` or for the run's `summary`.
    fn signal(name: &str, value: Value) -> Signal {
        Signal {
            name: name.to_string(),
            value,
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
            summary: vec![],
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
            &Computed {
                signals: vec![],
                summary: vec![],
            },
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

    /// A count about the run is written, and it claims no signal name while it is.
    ///
    /// The second half is the one that had to be checked. A signal name is corpus-wide and
    /// `Signals::load` refuses a name two files declare, so a `summary:` whose counts *were* signals
    /// would make `nodes` unavailable to every other calculator in the corpus. [`read_back`] asserts
    /// `problems` is empty, and this asserts the count did not reach a node either.
    #[test]
    fn a_count_about_the_run_is_written_and_claims_no_signal_name() {
        let computed = Computed {
            signals: vec![row(
                "gage/one",
                &[("travels_as", Value::Text("open".into()))],
            )],
            summary: vec![
                signal("nodes", Value::Int(1)),
                signal("downgraded", Value::Int(0)),
                signal("links_to_non_nodes", Value::Int(2)),
            ],
        };
        let text = render("x", ".yidam/capabilities/x.glu", &computed).unwrap();
        let s = read_back("x", &["gage/one"], &text);

        let doc: serde_yaml::Value = serde_yaml::from_str(&text).unwrap();
        let summary = doc.get("summary").expect("the block is written");
        assert_eq!(
            summary.get("nodes").and_then(|v| v.as_i64()),
            Some(1),
            "{text}"
        );
        assert_eq!(
            summary.get("links_to_non_nodes").and_then(|v| v.as_i64()),
            Some(2),
            "{text}"
        );

        // Not a signal: `nodes` is a count about the run and is attached to no node, so the name
        // stays free for a calculator that does key it onto one.
        let got = s.for_node(".yidam/corpus/gage/one.yml");
        assert!(!got.contains_key("nodes"), "{got:?}\n{text}");
        assert_eq!(s.files.len(), 1, "{text}");
        assert!(
            !s.files[0].names.contains(&"nodes".to_string()),
            "the summary claimed a signal name: {:?}\n{text}",
            s.files[0].names
        );
    }

    /// A calculator that counted nothing writes no block, rather than an empty one.
    ///
    /// `summary: {}` is a claim that nothing was counted. Absence is the truthful statement that
    /// this calculator does not summarize, and it is also what keeps the document a script written
    /// before this field existed produces byte-identical.
    #[test]
    fn a_calculator_that_counted_nothing_writes_no_summary() {
        let text = render(
            "x",
            ".yidam/capabilities/x.glu",
            &Computed {
                signals: vec![row("gage/one", &[("a", Value::Int(1))])],
                summary: vec![],
            },
        )
        .unwrap();
        assert!(!text.contains("summary"), "{text}");
    }

    /// The summary is refused for the same shapes a row is, and named as a summary while it is.
    #[test]
    fn a_summary_that_would_be_silently_wrong_is_refused() {
        let cases: [(Vec<Signal>, &str); 4] = [
            (vec![signal("", Value::Int(1))], "unnamed count"),
            (
                vec![
                    signal("nodes", Value::Int(1)),
                    signal("nodes", Value::Int(2)),
                ],
                "twice in its `summary:`",
            ),
            (vec![signal("nodes", Value::Empty)], "signal with no value"),
            (
                vec![signal("nodes", Value::Unrepresentable)],
                "Unrepresentable",
            ),
        ];
        for (summary, wanted) in cases {
            let e = render(
                "x",
                ".yidam/capabilities/x.glu",
                &Computed {
                    signals: vec![],
                    summary,
                },
            )
            .unwrap_err()
            .to_string();
            assert!(e.contains(wanted), "expected {wanted:?} in: {e}");
            assert!(
                e.contains("summary"),
                "a summary was refused without saying it was one: {e}"
            );
        }
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
            summary: vec![],
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
                    summary: vec![],
                },
                "no `node`",
            ),
            (
                Computed {
                    signals: vec![
                        row("gage/one", &[("a", Value::Int(1))]),
                        row("gage/one", &[("a", Value::Int(2))]),
                    ],
                    summary: vec![],
                },
                "two rows",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("", Value::Int(1))])],
                    summary: vec![],
                },
                "unnamed signal",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("node", Value::Int(1))])],
                    summary: vec![],
                },
                "already keyed by",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("a", Value::Unrepresentable)])],
                    summary: vec![],
                },
                "Unrepresentable",
            ),
            (
                Computed {
                    signals: vec![row("gage/one", &[("a", Value::Empty)])],
                    summary: vec![],
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
