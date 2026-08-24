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

impl OntologyClass {
    /// A class nothing is meant to point at: it declares edges, and none of them inbound.
    ///
    /// The same derivation `orphan-in` exempts on, exposed here so a consumer computing a
    /// per-class orphan expectation reads the rule rather than re-deriving it.
    ///
    /// A class that declares no edges at all is **not** a source class. It has said nothing
    /// about its shape, and reading silence as a declaration would exempt every instance in
    /// a corpus whose ontology is not filled in.
    pub fn is_source_class(&self) -> bool {
        !self.edges.is_empty() && !self.edges.iter().any(|e| e.direction == "in")
    }
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
        // finding. What it catches is a date field carrying prose.
        "date" => json!({ "type": "string", "pattern": "^[0-9]{4}-[0-9]{2}-[0-9]{2}$" }),
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
/// # What it does not constrain, and why
///
/// **`required` is never emitted for a declared property.** `missing-property` reports and
/// does not gate — the declaration has no `required` field, so it cannot say whether every
/// instance carries a property or merely may. A schema demanding them would reject
/// instances the gate accepts, which is the drift this compiler exists to prevent.
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
        properties.insert(
            "properties".into(),
            json!({
                "type": "object",
                "properties": Value::Object(declared),
                // Closed, matching `undeclared-property`, which gates.
                "additionalProperties": false
            }),
        );
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
