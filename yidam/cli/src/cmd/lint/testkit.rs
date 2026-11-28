use std::path::PathBuf;

use super::checks::{interval_gap, interval_overlap};
use super::model::Check;
use crate::corpus::{Class, Edges, Node};

/// The resolved graph for a slice of test nodes.
///
/// The four edge-reading checks take one rather than each building a path map of their
/// own (#925); the driver builds it once per run, and a test builds it from exactly the
/// slice it hands the check.
pub(super) fn edges_of(nodes: &[Node]) -> Edges {
    Edges::build(nodes)
}

/// A corpus declaring no universal properties — the state every corpus starts in, and
/// what these cases are about. The universal path has its own tests below and in
/// `crate::universal`.
pub(super) const NONE: crate::universal::Universal = crate::universal::Universal::empty();

pub(super) fn node(rel: &str, yaml: &str) -> Node {
    let n = Node::parse(PathBuf::from(rel), rel, yaml);
    // What the `.unwrap()` here used to say: a fixture that does not parse would exercise
    // the empty record rather than the case its test is named after.
    assert!(n.malformed.is_none(), "fixture: {:?}", n.malformed);
    n
}

// ── claim-tag-malformed over class prose (#603) ───────────────────────────

/// Write a class file and load it the way `lint` does, so these exercise the loader
/// keeping the file's bytes rather than a struct literal that could not lose them.
pub(super) fn loaded_class(name: &str, yaml: &str) -> (tempfile::TempDir, Vec<Class>) {
    let dir = tempfile::tempdir().unwrap();
    let p = dir.path().join(format!("{name}.ont.yml"));
    std::fs::write(&p, yaml).unwrap();
    let classes = crate::corpus::load_classes(dir.path(), &[p], &crate::corpus::Overlay::default());
    (dir, classes)
}

// ── class-asserts-purpose ─────────────────────────────────────────────────

/// Serialized and re-parsed rather than assembled, so [`Class::text`] and
/// [`Class::description`] cannot disagree. A literal with an empty `text` would let a
/// prose check written against this helper pass while reading nothing.
pub(super) fn class(rel: &str, description: &str) -> Class {
    let mut m = serde_yaml::Mapping::new();
    m.insert("description".into(), description.into());
    // Declares an inbound edge, so instances are expected to be pointed at. The
    // source-class arm is exercised by `orphan_in`'s own tests.
    m.insert(
        "edges".into(),
        serde_yaml::from_str("- relationship: cited-by\n  target: concept\n  direction: in\n")
            .unwrap(),
    );
    Class::parse(rel, serde_yaml::to_string(&m).unwrap())
}

// ── the class contract ────────────────────────────────────────────────────

/// A class as the ontology writes it, parsed by the same deserializer `load_classes`
/// uses and built by the same builder — so a test cannot pass against a shape the YAML
/// could never produce, and cannot go on passing against a field the real builder has
/// started reading and this one has not. It used to assemble the struct itself, which
/// is the third answer to *what a class is* that [`Class::parse`] exists to
/// prevent; it silently ignored `implemented_by:` the day the field was added.
pub(super) fn class_from(name: &str, yaml: &str) -> Class {
    Class::parse(format!(".yidam/corpus/{name}.ont.yml"), yaml)
}

// ── interval-overlap ──────────────────────────────────────────────────────

pub(super) const TENURE: &str = "properties:\n  - name: began\n    type: date\n  \
                          - name: ended\n    type: date\n\
                          edges:\n  - relationship: of-office\n    target: office\n    \
                          direction: out\n";

/// A tenure class, with the interval declaration `interval` appended to [`TENURE`].
pub(super) fn tenure(interval: &str) -> Class {
    class_from("tenure", &format!("{TENURE}{interval}"))
}

/// A tenure of `office` from `began` to `ended`, where `None` writes no end at all.
pub(super) fn term(name: &str, office: &str, began: &str, ended: Option<&str>) -> Node {
    let ended = ended
        .map(|e| format!("  ended: \"{e}\"\n"))
        .unwrap_or_default();
    Node::parse(
        PathBuf::from(format!("/repo/.yidam/corpus/tenure/{name}.yml")),
        format!(".yidam/corpus/tenure/{name}.yml"),
        format!(
            "class: tenure\nlabel: {name}\nproperties:\n  began: \"{began}\"\n{ended}\
                 links:\n  - target: ../office/{office}.yml\n    relationship: of-office\n"
        ),
    )
}

pub(super) fn office(name: &str) -> Node {
    Node::parse(
        PathBuf::from(format!("/repo/.yidam/corpus/office/{name}.yml")),
        format!(".yidam/corpus/office/{name}.yml"),
        format!("class: office\nlabel: {name}\n"),
    )
}

pub(super) const SEATED: &str =
    "interval:\n  start: began\n  end: ended\n  exclusive_over: of-office\n  \
                          capacity: seats\n";

/// An office carrying `seats` as written, where `None` writes no `seats` at all.
pub(super) fn seated_office(name: &str, seats: Option<&str>) -> Node {
    let seats = seats
        .map(|s| format!("properties:\n  seats: {s}\n"))
        .unwrap_or_default();
    Node::parse(
        PathBuf::from(format!("/repo/.yidam/corpus/office/{name}.yml")),
        format!(".yidam/corpus/office/{name}.yml"),
        format!("class: office\nlabel: {name}\n{seats}"),
    )
}

/// `interval-overlap` over the given tenures and offices, with an `office` class declaring
/// `seats` as the corpus that asked for it does: `type: string`.
pub(super) fn seated(interval: &str, offices: Vec<Node>, terms: Vec<Node>) -> Check {
    let office = class_from("office", "properties:\n  - name: seats\n    type: string\n");
    let mut nodes = offices;
    nodes.extend(terms);
    let edges = Edges::build(&nodes);
    interval_overlap(&nodes, &edges, &[tenure(interval), office])
}

// ── interval-gap ──────────────────────────────────────────────────────────

pub(super) const COMPLETE: &str =
    "interval:\n  start: began\n  end: ended\n  exclusive_over: of-office\n  \
                            capacity: seats\n  complete: line_complete\n";

/// An office whose properties are the YAML lines `props`, each written as `key: value`.
pub(super) fn office_with(name: &str, props: &[&str]) -> Node {
    let props = match props {
        [] => String::new(),
        _ => format!(
            "properties:\n{}",
            props.iter().map(|p| format!("  {p}\n")).collect::<String>()
        ),
    };
    Node::parse(
        PathBuf::from(format!("/repo/.yidam/corpus/office/{name}.yml")),
        format!(".yidam/corpus/office/{name}.yml"),
        format!("class: office\nlabel: {name}\n{props}"),
    )
}

/// `interval-gap` over the given tenures and offices, with an `office` class declaring
/// `seats` and `line_complete`.
pub(super) fn gaps(interval: &str, offices: Vec<Node>, terms: Vec<Node>) -> Check {
    let office = class_from(
        "office",
        "properties:\n  - name: seats\n    type: string\n  \
             - name: line_complete\n    type: boolean\n",
    );
    let mut nodes = offices;
    nodes.extend(terms);
    let edges = Edges::build(&nodes);
    interval_gap(&nodes, &edges, &[tenure(interval), office])
}

/// The declared set, for a class that names prose keys beyond `description`.
pub(super) fn declaring(class: &str, keys: &[&str]) -> crate::prose::ProseFields {
    declaring_with(class, keys, &[])
}

/// The same, for a class that also flags properties as prose.
pub(super) fn declaring_with(
    class: &str,
    keys: &[&str],
    properties: &[&str],
) -> crate::prose::ProseFields {
    crate::prose::ProseFields::from_declarations(
        Vec::new(),
        [crate::prose::Declaration {
            class: class.to_string(),
            keys: keys.iter().map(|k| (*k).to_string()).collect(),
            properties: properties.iter().map(|p| (*p).to_string()).collect(),
        }],
    )
}

pub(super) const GAGE: &str = "\
properties:
  - name: parameter
    type: string
  - name: claim_tag
    type: claim
edges:
  - relationship: sources-from
    target: concept
    direction: out
";

pub(super) fn gage_corpus(gage_yaml: &str) -> (Vec<Node>, Vec<Class>) {
    let nodes = vec![
        node(".yidam/corpus/gage/outlet.yml", gage_yaml),
        node(
            ".yidam/corpus/concept/hydropeaking.yml",
            "class: concept\nlinks: []\n",
        ),
    ];
    let classes = vec![
        class_from("gage", GAGE),
        class_from("concept", "properties: []\nedges: []\n"),
    ];
    (nodes, classes)
}
