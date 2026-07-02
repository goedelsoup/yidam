use anyhow::{Context, Result};
use oxrdf::{Graph, Literal, NamedNode, NamedNodeRef, Triple};
use serde_json::json;
use std::collections::BTreeMap;

use crate::model::{corpus_nodes, DomainModel};

const YIDAM_NS: &str = "https://yidam.dev/ontology#";
const OWL_NS: &str = "http://www.w3.org/2002/07/owl#";
const RDFS_NS: &str = "http://www.w3.org/2000/01/rdf-schema#";
const RDF_NS: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#";
const SKOS_NS: &str = "http://www.w3.org/2004/02/skos/core#";
const PROV_NS: &str = "http://www.w3.org/ns/prov#";
const XSD_NS: &str = "http://www.w3.org/2001/XMLSchema#";

/// The corpus mapped to RDF terms — the shared intermediate both the Turtle
/// and JSON-LD serializers consume, so the two outputs cannot drift.
struct RdfView {
    dataset_iri: String,
    domain: String,
    commit: String,
    genesis: String,
    generated_at_iso: String,
    /// class name → optional `bfo_anchor:` value from its `.ont.yml`.
    classes: BTreeMap<String, Option<String>>,
    instances: Vec<RdfInstance>,
    /// Relationship names (beyond the plain "link") in use, for property decls.
    relationships: Vec<String>,
    /// Link targets that resolve to no known instance; typed `owl:Thing`.
    unresolved: Vec<String>,
}

struct RdfInstance {
    iri: String,
    class: String,
    label: String,
    description: String,
    /// (target IRI, property local name — "linksTo" or a sanitized relationship)
    links: Vec<(String, String)>,
}

fn instance_iri(id: &str) -> String {
    format!("yidam://corpus/{id}")
}

/// Relationship → RDF property local name: "causes" → "causes",
/// "relates to" → "relatesTo". Anything unusable falls back to "linksTo".
fn property_local_name(relationship: &str) -> String {
    let mut out = String::new();
    let mut upper_next = false;
    for ch in relationship.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(if upper_next {
                ch.to_ascii_uppercase()
            } else {
                ch
            });
            upper_next = false;
        } else {
            upper_next = !out.is_empty();
        }
    }
    if out.is_empty() || relationship == "link" {
        "linksTo".to_string()
    } else {
        out
    }
}

/// Unix seconds → ISO-8601 UTC (days-from-civil inverse, Hinnant's algorithm).
fn unix_to_iso(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

fn build_view(model: &DomainModel) -> RdfView {
    let nodes = corpus_nodes(model);
    let known: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let mut classes: BTreeMap<String, Option<String>> = BTreeMap::new();
    for cls in &model.classes {
        let name = crate::model::file_stem(&cls.filename)
            .trim_end_matches(".ont")
            .to_string();
        let content = String::from_utf8_lossy(&cls.content);
        let bfo = serde_yaml::from_str::<serde_yaml::Value>(&content)
            .ok()
            .and_then(|v| v["bfo_anchor"].as_str().map(str::to_string));
        classes.insert(name, bfo);
    }

    let mut relationships: BTreeMap<String, ()> = BTreeMap::new();
    let mut unresolved: BTreeMap<String, ()> = BTreeMap::new();
    let mut instances = Vec::new();
    for node in &nodes {
        // Instances may belong to classes with no .ont.yml on disk yet;
        // the class still exists in RDF.
        classes.entry(node.class.clone()).or_insert(None);
        let links = node
            .links
            .iter()
            .map(|(target, relationship)| {
                if !known.contains(target.as_str()) {
                    unresolved.insert(target.clone(), ());
                }
                let prop = property_local_name(relationship);
                if prop != "linksTo" {
                    relationships.insert(prop.clone(), ());
                }
                (instance_iri(target), prop)
            })
            .collect();
        instances.push(RdfInstance {
            iri: instance_iri(&node.id),
            class: node.class.clone(),
            label: node.label.clone(),
            description: node.description.clone(),
            links,
        });
    }

    RdfView {
        dataset_iri: "yidam://corpus".to_string(),
        domain: model.provenance.domain.clone(),
        commit: model.provenance.commit.clone(),
        genesis: model.provenance.genesis.clone(),
        generated_at_iso: unix_to_iso(model.provenance.generated_at),
        classes,
        instances,
        relationships: relationships.into_keys().collect(),
        unresolved: unresolved.into_keys().collect(),
    }
}

fn build_graph(view: &RdfView) -> Result<Graph> {
    let mut graph = Graph::default();
    let node = |iri: &str| NamedNode::new(iri).with_context(|| format!("invalid IRI: {iri}"));
    let yidam = |local: &str| node(&format!("{YIDAM_NS}{local}"));
    let a = NamedNodeRef::new(&format!("{RDF_NS}type"))?.into_owned();
    let rdfs_label = node(&format!("{RDFS_NS}label"))?;
    let mut insert = |s: &NamedNode, p: &NamedNode, o: oxrdf::Term| {
        graph.insert(&Triple::new(s.clone(), p.clone(), o));
    };

    // Ontology header + provenance
    let dataset = node(&view.dataset_iri)?;
    insert(&dataset, &a, node(&format!("{OWL_NS}Ontology"))?.into());
    insert(
        &dataset,
        &rdfs_label,
        Literal::new_simple_literal(&view.domain).into(),
    );
    insert(
        &dataset,
        &node(&format!("{PROV_NS}generatedAtTime"))?,
        Literal::new_typed_literal(&view.generated_at_iso, node(&format!("{XSD_NS}dateTime"))?)
            .into(),
    );
    insert(
        &dataset,
        &yidam("commit")?,
        Literal::new_simple_literal(&view.commit).into(),
    );
    insert(
        &dataset,
        &yidam("genesisDate")?,
        Literal::new_simple_literal(&view.genesis).into(),
    );

    // Classes
    for (class, bfo) in &view.classes {
        let class_node = yidam(class)?;
        insert(&class_node, &a, node(&format!("{OWL_NS}Class"))?.into());
        insert(
            &class_node,
            &rdfs_label,
            Literal::new_simple_literal(class).into(),
        );
        if let Some(anchor) = bfo {
            if anchor.starts_with("http") {
                insert(
                    &class_node,
                    &node(&format!("{SKOS_NS}exactMatch"))?,
                    node(anchor)?.into(),
                );
            } else {
                insert(
                    &class_node,
                    &yidam("bfoAnchor")?,
                    Literal::new_simple_literal(anchor).into(),
                );
            }
        }
    }

    // Properties: linksTo plus each named relationship as a subproperty
    let links_to = yidam("linksTo")?;
    insert(
        &links_to,
        &a,
        node(&format!("{OWL_NS}ObjectProperty"))?.into(),
    );
    for rel in &view.relationships {
        let prop = yidam(rel)?;
        insert(&prop, &a, node(&format!("{OWL_NS}ObjectProperty"))?.into());
        insert(
            &prop,
            &node(&format!("{RDFS_NS}subPropertyOf"))?,
            links_to.clone().into(),
        );
    }

    // Instances
    for inst in &view.instances {
        let subject = node(&inst.iri)?;
        insert(&subject, &a, yidam(&inst.class)?.into());
        if !inst.label.is_empty() {
            insert(
                &subject,
                &rdfs_label,
                Literal::new_simple_literal(&inst.label).into(),
            );
        }
        if !inst.description.is_empty() {
            insert(
                &subject,
                &node(&format!("{SKOS_NS}definition"))?,
                Literal::new_simple_literal(&inst.description).into(),
            );
        }
        for (target_iri, prop) in &inst.links {
            insert(&subject, &yidam(prop)?, node(target_iri)?.into());
        }
    }

    // Unresolved targets exist as owl:Thing so links stay dereferenceable
    for target in &view.unresolved {
        let subject = node(&instance_iri(target))?;
        insert(&subject, &a, node(&format!("{OWL_NS}Thing"))?.into());
        insert(
            &subject,
            &node(&format!("{RDFS_NS}comment"))?,
            Literal::new_simple_literal("unresolved link target").into(),
        );
    }

    Ok(graph)
}

/// Serialize the corpus as Turtle. Example SPARQL over the output —
/// "all nodes of class concept":
///
/// ```sparql
/// PREFIX yidam: <https://yidam.dev/ontology#>
/// SELECT ?node ?label WHERE { ?node a yidam:concept ; rdfs:label ?label . }
/// ```
pub(crate) fn render_rdf_turtle(model: &DomainModel) -> Result<String> {
    let graph = build_graph(&build_view(model))?;
    let mut serializer = oxttl::TurtleSerializer::new()
        .with_prefix("yidam", YIDAM_NS)?
        .with_prefix("owl", OWL_NS)?
        .with_prefix("rdfs", RDFS_NS)?
        .with_prefix("skos", SKOS_NS)?
        .with_prefix("prov", PROV_NS)?
        .with_prefix("xsd", XSD_NS)?
        .for_writer(Vec::new());
    for triple in graph.iter() {
        serializer.serialize_triple(triple)?;
    }
    let bytes = serializer.finish()?;
    Ok(String::from_utf8(bytes)?)
}

/// Serialize the corpus as JSON-LD (a `@graph` document with a `@context`
/// mapping the same vocabulary the Turtle output uses).
pub(crate) fn render_rdf_jsonld(model: &DomainModel) -> Result<String> {
    let view = build_view(model);

    let mut graph = vec![json!({
        "@id": view.dataset_iri,
        "@type": "owl:Ontology",
        "rdfs:label": view.domain,
        "prov:generatedAtTime": {"@value": view.generated_at_iso, "@type": "xsd:dateTime"},
        "yidam:commit": view.commit,
        "yidam:genesisDate": view.genesis,
    })];

    for (class, bfo) in &view.classes {
        let mut obj = json!({
            "@id": format!("yidam:{class}"),
            "@type": "owl:Class",
            "rdfs:label": class,
        });
        if let Some(anchor) = bfo {
            if anchor.starts_with("http") {
                obj["skos:exactMatch"] = json!({"@id": anchor});
            } else {
                obj["yidam:bfoAnchor"] = json!(anchor);
            }
        }
        graph.push(obj);
    }

    graph.push(json!({
        "@id": "yidam:linksTo",
        "@type": "owl:ObjectProperty",
    }));
    for rel in &view.relationships {
        graph.push(json!({
            "@id": format!("yidam:{rel}"),
            "@type": "owl:ObjectProperty",
            "rdfs:subPropertyOf": {"@id": "yidam:linksTo"},
        }));
    }

    for inst in &view.instances {
        let mut obj = json!({
            "@id": inst.iri,
            "@type": format!("yidam:{}", inst.class),
        });
        if !inst.label.is_empty() {
            obj["rdfs:label"] = json!(inst.label);
        }
        if !inst.description.is_empty() {
            obj["skos:definition"] = json!(inst.description);
        }
        for (target_iri, prop) in &inst.links {
            let key = format!("yidam:{prop}");
            let entry = json!({"@id": target_iri});
            match obj.get_mut(&key) {
                Some(serde_json::Value::Array(arr)) => arr.push(entry),
                Some(existing) => {
                    let prev = existing.take();
                    obj[&key] = json!([prev, entry]);
                }
                None => obj[&key] = entry,
            }
        }
        graph.push(obj);
    }

    for target in &view.unresolved {
        graph.push(json!({
            "@id": instance_iri(target),
            "@type": "owl:Thing",
            "rdfs:comment": "unresolved link target",
        }));
    }

    let doc = json!({
        "@context": {
            "yidam": YIDAM_NS,
            "owl": OWL_NS,
            "rdfs": RDFS_NS,
            "skos": SKOS_NS,
            "prov": PROV_NS,
            "xsd": XSD_NS,
        },
        "@graph": graph,
    });
    Ok(serde_json::to_string_pretty(&doc)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{InstanceFile, OntClass, Provenance, RenderedViews};

    fn test_model() -> DomainModel {
        DomainModel {
            classes: vec![OntClass {
                filename: "concept.ont.yml".into(),
                content: b"class: concept\nbfo_anchor: http://purl.obolibrary.org/obo/BFO_0000001\n"
                    .to_vec(),
            }],
            instances: vec![
                InstanceFile {
                    class: "concept".into(),
                    filename: "alpha.yml".into(),
                    content: b"class: concept\nlabel: Alpha\ndescription: \"First, with \\\"quotes\\\".\"\n\
                               links:\n  - target: gamma.yml\n    relationship: causes\n\
                               \x20 - target: missing.yml\n"
                        .to_vec(),
                },
                InstanceFile {
                    class: "concept".into(),
                    filename: "gamma.yml".into(),
                    content: b"class: concept\nlabel: Gamma\nlinks: []\n".to_vec(),
                },
            ],
            skills: vec![],
            decisions: vec![],
            index: None,
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "test-domain".into(),
                generated_at: 1_780_000_000,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    /// The acceptance-criteria round trip: parse the Turtle back and check
    /// the triples SPARQL "all nodes of class concept" would match.
    #[test]
    fn turtle_round_trips_through_oxttl_parser() {
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        let triples: Vec<_> = oxttl::TurtleParser::new()
            .for_reader(ttl.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .expect("valid Turtle");

        let type_iri = format!("{RDF_NS}type");
        let concept_iri = format!("{YIDAM_NS}concept");
        let concept_instances: Vec<_> = triples
            .iter()
            .filter(|t| {
                t.predicate.as_str() == type_iri
                    && t.object.to_string() == format!("<{concept_iri}>")
            })
            .collect();
        assert_eq!(
            concept_instances.len(),
            2,
            "both instances typed as yidam:concept"
        );

        let causes = format!("{YIDAM_NS}causes");
        assert!(
            triples.iter().any(|t| t.predicate.as_str() == causes
                && t.subject.to_string() == "<yidam://corpus/concept/alpha>"),
            "named relationship becomes a yidam: property"
        );
    }

    #[test]
    fn turtle_has_ontology_header_and_provenance() {
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        assert!(ttl.contains("owl:Ontology"));
        assert!(ttl.contains("test-domain"));
        assert!(ttl.contains("abc1234"));
        assert!(ttl.contains("2026-01-01"));
        assert!(ttl.contains("^^xsd:dateTime"));
        assert!(ttl.contains("skos:exactMatch <http://purl.obolibrary.org/obo/BFO_0000001>"));
    }

    #[test]
    fn unresolved_target_becomes_owl_thing() {
        // Parse rather than substring-match: triple order in the Turtle
        // output follows Graph iteration order, which is not stable.
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        let triples: Vec<_> = oxttl::TurtleParser::new()
            .for_reader(ttl.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert!(triples.iter().any(|t| {
            t.subject.to_string() == "<yidam://corpus/concept/missing>"
                && t.object.to_string() == format!("<{OWL_NS}Thing>")
        }));
        assert!(ttl.contains("unresolved link target"));
    }

    #[test]
    fn jsonld_is_valid_json_with_context_and_graph() {
        let jsonld = render_rdf_jsonld(&test_model()).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        assert_eq!(doc["@context"]["yidam"], YIDAM_NS);
        let graph = doc["@graph"].as_array().unwrap();
        let alpha = graph
            .iter()
            .find(|o| o["@id"] == "yidam://corpus/concept/alpha")
            .expect("alpha present");
        assert_eq!(alpha["@type"], "yidam:concept");
        assert_eq!(alpha["yidam:causes"]["@id"], "yidam://corpus/concept/gamma");
        assert_eq!(alpha["skos:definition"], "First, with \"quotes\".");
    }

    #[test]
    fn property_names_sanitize() {
        assert_eq!(property_local_name("causes"), "causes");
        assert_eq!(property_local_name("relates to"), "relatesTo");
        assert_eq!(property_local_name("link"), "linksTo");
        assert_eq!(property_local_name("???"), "linksTo");
    }

    #[test]
    fn unix_to_iso_is_correct() {
        assert_eq!(unix_to_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_iso(1_780_000_000), "2026-05-28T20:26:40Z");
    }
}
