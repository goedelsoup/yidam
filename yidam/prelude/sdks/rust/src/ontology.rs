//! The class contract, read and published.
//!
//! `.yidam/corpus/<class>.ont.yml` declares what an instance of a class carries and what it
//! may link to. `yidam lint` **enforces** that declaration; this module **publishes** it, as
//! JSON Schema that any editor, validator or CI step can apply without linking against
//! yidam at all.
//!
//! # Why the two must not drift
//!
//! A consumer that validates a corpus for itself and a gate that validates the same corpus
//! can disagree in only one direction that matters: the consumer accepting what the gate
//! rejects, silently, until somebody's build fails on a file that looked fine everywhere
//! else. That is the failure RFC-0002 documents at the node-model layer and RFC-0005 at the
//! MCP layer, one level down.
//!
//! So the compiled schema is deliberately **no stricter than the checks**, and each place
//! the two could have come apart is commented where the decision is made:
//!
//! - Declared properties are typed but **not required** — `missing-property` reports and
//!   does not gate, so a schema demanding them would reject instances the gate accepts.
//! - `additionalProperties` on the property bag is closed only when the class declared any
//!   — matching `undeclared-property`, and matching the silence rule below.
//! - Relationships are **not constrained at all**, because whether an edge is licensed
//!   depends on where its target resolves, which JSON Schema cannot see. See
//!   [`compile_class_schema`].
//!
//! # Silence is not a contract
//!
//! A class that declares no `properties:` has said nothing about properties, and the
//! compiled schema constrains none. This rule lives here rather than in each consumer
//! precisely so the three cannot each decide it differently — which is the whole argument
//! for compiling the ontology instead of describing it.

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

/// One class as the ontology declares it.
///
/// The typed accessor: a consumer reads this rather than reaching into YAML and guessing at
/// field names.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OntologyClass {
    /// The class name. `<class>.ont.yml`'s stem when the file does not say.
    pub name: String,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub properties: Vec<OntologyProperty>,
    #[serde(default)]
    pub edges: Vec<OntologyEdge>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OntologyProperty {
    pub name: String,
    /// `string`, `text`, `date`, `ref`, `claim` — or a type this corpus coined, which is
    /// carried through and left unconstrained.
    #[serde(default, rename = "type")]
    pub property_type: String,
    #[serde(default)]
    pub description: String,
    /// Whether every instance of the class must carry this property.
    ///
    /// **Absent means false**, and not out of timidity: every corpus written before this
    /// field existed was written under a schema where the question could not be asked.
    /// Defaulting to `true` would require a declaration nobody made, in every derived
    /// repository at once — a gate arriving in a corpus that never agreed to it.
    ///
    /// It is what lets `missing-property` gate at all. Without it the check cannot tell
    /// *every instance of this class has this* from *an instance may have this*, and gating
    /// on the second reading asserts a contract the ontology never wrote.
    #[serde(default)]
    pub required: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct OntologyEdge {
    pub relationship: String,
    #[serde(default)]
    pub target: String,
    /// `out` when instances of this class author the link, `in` when the other side does.
    #[serde(default)]
    pub direction: String,
    #[serde(default)]
    pub description: String,
}

/// Classes the ontology says nothing points at.
///
/// The same derivation `orphan-in` exempts on, exposed here so a consumer computing a
/// per-class orphan expectation reads the rule rather than re-deriving it.
///
/// **It takes the whole ontology, and that is the correction.** This was once
/// `OntologyClass::is_source_class(&self)`, reading one class's own edge list for a
/// `direction: in` entry — which reads half the ontology. `B: {target: A, direction: out}`
/// declares that instances of `B` point at instances of `A`; it is the same fact as
/// `A: {direction: in}`, stated from the authoring end, and `target` is *"the class at the
/// other end, whichever end authors the link"*. Reading only a class's own list treated its
/// silence about inbound edges as a positive declaration that nothing points at it. Measured
/// upstream: all three classes of the worked example derived as source classes, so
/// `orphan-in` could not fire anywhere in it.
///
/// Two things it deliberately does not do:
///
/// - **A class declaring no edges at all is not a source class.** It has said nothing about
///   its shape, and reading silence as a declaration would exempt every instance in a corpus
///   whose ontology is not filled in.
/// - **A self-edge does not make a class pointed at.** `reach -downstream-of-> reach` says
///   instances relate to each other, not that every instance is cited — any acyclic
///   self-relation has an endpoint that is not.
pub fn source_classes(classes: &[OntologyClass]) -> BTreeSet<String> {
    let mut pointed: BTreeSet<&str> = BTreeSet::new();
    for c in classes {
        for e in c.edges.iter().filter(|e| e.target != c.name) {
            match e.direction.as_str() {
                "in" => {
                    pointed.insert(c.name.as_str());
                }
                "out" => {
                    pointed.insert(e.target.as_str());
                }
                // A declaration that does not say which way it runs exempts neither end.
                _ => {
                    pointed.insert(c.name.as_str());
                    pointed.insert(e.target.as_str());
                }
            }
        }
    }
    classes
        .iter()
        .filter(|c| !c.edges.is_empty() && !pointed.contains(c.name.as_str()))
        .map(|c| c.name.clone())
        .collect()
}

/// The evidence tokens a `claim` property may hold, in both spellings.
///
/// Bare is what a typed vocabulary stores; bracketed is what a corpus writes after being
/// told the prose scan needs brackets. Both are accepted by the counter, so both are
/// accepted here — a schema that took only one would reject corpora the gate counts.
pub const CLAIM_TOKENS: [&str; 6] = [
    "verified",
    "inference",
    "open",
    "[verified]",
    "[inference]",
    "[open]",
];

/// Read a class definition. `name` is the fallback when the file does not name itself.
///
/// A file that does not parse yields a class that declares nothing, which under the silence
/// rule constrains nothing. That is the same direction `lint` degrades in: a malformed
/// ontology is a finding of its own, not a reason to start rejecting instances.
pub fn parse_class(name: &str, content: &str) -> OntologyClass {
    #[derive(Default, serde::Deserialize)]
    struct Fields {
        class: Option<String>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        description: Option<String>,
        #[serde(default)]
        properties: Vec<OntologyProperty>,
        #[serde(default)]
        edges: Vec<OntologyEdge>,
    }
    let f: Fields = serde_yaml::from_str(content).unwrap_or_default();
    OntologyClass {
        name: f
            .class
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| name.to_string()),
        label: f.label.unwrap_or_default(),
        description: f.description.unwrap_or_default(),
        properties: f.properties,
        edges: f.edges,
    }
}

/// The JSON Schema a declared property's value must satisfy.
///
/// Mirrors `lint`'s `property-type` check exactly, including what it declines to check: a
/// type the corpus coined for itself compiles to `true` — valid against anything — because
/// a schema that rejected every type it had not heard of would make coining one impossible.
fn property_schema(property_type: &str) -> Value {
    match property_type {
        "string" | "text" | "ref" => json!({ "type": "string", "minLength": 1 }),
        // Structural, not a calendar: `2024-02-31` satisfies this and is somebody else's
        // finding. What it catches is a date field carrying prose — and **at whatever
        // precision the corpus knows**, because `1985` is not prose. The three arms match
        // `property-type`'s predicate exactly; a schema stricter than the gate underlines,
        // in the editor, a value the build accepts.
        "date" => json!({ "type": "string", "pattern": "^[0-9]{4}(-[0-9]{2}(-[0-9]{2})?)?$" }),
        // A list is legal here and nowhere else: the counter reads a list of tags as one
        // claim each, so a corpus writing `claim_tag: [open]` unquoted has written a
        // one-element list without meaning to.
        "claim" => json!({
            "anyOf": [
                { "enum": CLAIM_TOKENS },
                { "type": "array", "items": { "enum": CLAIM_TOKENS } }
            ]
        }),
        _ => Value::Bool(true),
    }
}

/// Compile a class definition into a JSON Schema for its instances.
///
/// # What it constrains, and what it does not
///
/// **`required` is emitted for exactly the properties declared `required: true`** (#301).
/// The compiled schema must be no stricter than the gate: before the declaration existed
/// this list was always empty, because `missing-property` could only warn — it could not
/// tell *every instance carries this* from *an instance may* — and a schema demanding a
/// declared property would have rejected instances the gate accepts. The declaration now
/// answers that question once, and `missing-property` gates on the same answer, so the two
/// move together rather than one outrunning the other.
///
/// An empty list is omitted rather than written as `required: []`: that would be a
/// different document for the same meaning, and these schemas are compared byte for byte
/// against the Python and TypeScript compilers.
///
/// **`links[].relationship` is left open.** The gate licenses a relationship only for edges
/// that land on another *instance*: a link to `../<class>.ont.yml` or into the catalog is a
/// citation, and no class declares those. JSON Schema cannot resolve a path, so a schema
/// restricting `relationship` to the declared list would reject the `instance-of` link
/// every instance is required to carry. The declared relationships are published as an
/// annotation instead, under `x-yidam-edges`, where a consumer can offer completion from
/// them without any validator treating them as a constraint.
pub fn compile_class_schema(class: &OntologyClass) -> Value {
    let mut root = Map::new();
    root.insert(
        "$schema".into(),
        json!("https://json-schema.org/draft/2020-12/schema"),
    );
    root.insert(
        "title".into(),
        json!(format!("yidam corpus node — {}", class.name)),
    );
    if !class.description.is_empty() {
        root.insert("description".into(), json!(class.description));
    }
    root.insert("type".into(), json!("object"));

    let mut properties = Map::new();
    // The one thing a per-class schema can say that the shared node schema cannot: which
    // class this file is an instance of. A node filed under the wrong directory is caught
    // by `unknown-class`; this catches the same thing while typing.
    properties.insert("class".into(), json!({ "const": class.name }));

    // Silence is not a contract. A class declaring no properties constrains none — and in
    // particular does not close `additionalProperties`, which would reject every instance
    // in a corpus whose ontology is not filled in.
    if !class.properties.is_empty() {
        let mut declared = Map::new();
        for p in &class.properties {
            let mut schema = property_schema(&p.property_type);
            if let (Value::Object(map), false) = (&mut schema, p.description.is_empty()) {
                map.insert("description".into(), json!(p.description));
            }
            declared.insert(p.name.clone(), schema);
        }
        let mut bag = Map::new();
        bag.insert("type".into(), json!("object"));
        bag.insert("properties".into(), Value::Object(declared));
        // **Emitted for exactly the properties declared `required: true`.** The compiled
        // schema must be no stricter than the gate: before the declaration existed this
        // list was always empty, because `missing-property` could only warn and a schema
        // that required a declared property would have underlined, in the editor, an
        // omission the gate accepts. Now the two move together — the same declaration
        // decides both, so neither can outrun the other.
        let required: Vec<Value> = class
            .properties
            .iter()
            .filter(|p| p.required)
            .map(|p| json!(p.name))
            .collect();
        if !required.is_empty() {
            bag.insert("required".into(), Value::Array(required));
        }
        // Closed, matching `undeclared-property`, which gates.
        bag.insert("additionalProperties".into(), json!(false));
        properties.insert("properties".into(), Value::Object(bag));
    }

    root.insert("properties".into(), Value::Object(properties));
    root.insert("required".into(), json!(["class"]));
    // Permissive at the top level, as the shared node schema is and for the reason measured
    // there: derived corpora carry their own fields, and closing this rejected 117 nodes of
    // 117 in one repository.
    root.insert("additionalProperties".into(), json!(true));

    if !class.edges.is_empty() {
        root.insert(
            "x-yidam-edges".into(),
            Value::Array(
                class
                    .edges
                    .iter()
                    .map(|e| {
                        json!({
                            "relationship": e.relationship,
                            "target": e.target,
                            "direction": e.direction
                        })
                    })
                    .collect(),
            ),
        );
    }

    Value::Object(root)
}

#[cfg(test)]
mod source_class_tests {
    use super::*;

    fn class(name: &str, edges: &[(&str, &str)]) -> OntologyClass {
        OntologyClass {
            name: name.into(),
            edges: edges
                .iter()
                .map(|(target, direction)| OntologyEdge {
                    relationship: "r".into(),
                    target: (*target).into(),
                    direction: (*direction).into(),
                    description: String::new(),
                })
                .collect(),
            ..Default::default()
        }
    }

    /// The correction: an inbound relationship declared from the authoring end.
    #[test]
    fn a_class_another_class_points_at_is_not_a_source_class() {
        let classes = [
            class("gage", &[("concept", "out")]),
            class("concept", &[("concept", "out")]),
        ];
        let sources = source_classes(&classes);
        assert!(!sources.contains("concept"));
        assert!(sources.contains("gage"));
    }

    /// A self-relation has endpoints, so it cannot mean every instance is cited.
    #[test]
    fn a_self_edge_does_not_make_a_class_pointed_at() {
        assert!(source_classes(&[class("reach", &[("reach", "out")])]).contains("reach"));
    }

    /// Silence is not a declaration.
    #[test]
    fn a_class_declaring_no_edges_is_not_a_source_class() {
        assert!(source_classes(&[class("quiet", &[])]).is_empty());
    }

    /// An ambiguous declaration exempts neither end.
    #[test]
    fn a_directionless_declaration_exempts_neither_end() {
        let classes = [class("a", &[("b", "")]), class("b", &[("c", "out")])];
        assert!(source_classes(&classes).is_empty());
    }
}
