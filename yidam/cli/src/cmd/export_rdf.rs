use anyhow::{Context, Result};
use oxrdf::{Graph, Literal, NamedNode, NamedNodeRef, Triple};
use serde_json::json;
use std::collections::BTreeMap;

use super::export::unix_to_iso;
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
/// A class's foundational alignment, as RDF needs it.
///
/// Both halves reach the graph. `ontology` and `ty` become literals unconditionally, so a
/// corpus that declared an alignment without looking up an IRI still exports the fact that it
/// did — which `bfo_anchor:` never managed, being a bare URI with nowhere to say *which*
/// ontology it came from and no UFO form at all. `iri` becomes `skos:exactMatch` when the
/// class supplies one.
#[derive(Clone)]
struct Alignment {
    ontology: String,
    ty: String,
    iri: Option<String>,
}

/// Read `foundational_type:`, falling back to the retired `bfo_anchor:` for one release.
///
/// The fallback exists because #613 found the two had never been connected: bootstrap wrote
/// `foundational_type:` (when it was not telling authors to write `bfo_type:`) and this
/// export read `bfo_anchor:`, so no corpus could satisfy both. Any corpus that guessed
/// `bfo_anchor:` from the old `domain-computer.md` still exports, and `lint` now names the
/// field so the repair is visible rather than silent.
fn alignment_of(content: &str) -> Option<Alignment> {
    let v = serde_yaml::from_str::<serde_yaml::Value>(content).ok()?;
    let ft = &v["foundational_type"];
    if let Some(ontology) = ft["ontology"].as_str() {
        return Some(Alignment {
            ontology: ontology.to_string(),
            ty: ft["type"].as_str().unwrap_or_default().to_string(),
            iri: ft["iri"].as_str().map(str::to_string),
        });
    }
    v["bfo_anchor"].as_str().map(|anchor| Alignment {
        ontology: "bfo".to_string(),
        ty: String::new(),
        iri: Some(anchor.to_string()),
    })
}

struct RdfView {
    dataset_iri: String,
    domain: String,
    commit: String,
    genesis: String,
    generated_at_iso: String,
    /// class name → its declared foundational alignment, when it has one.
    classes: BTreeMap<String, Option<Alignment>>,
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

fn build_view(model: &DomainModel) -> RdfView {
    let nodes = corpus_nodes(model);
    let known: std::collections::HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let mut classes: BTreeMap<String, Option<Alignment>> = BTreeMap::new();
    for cls in &model.classes {
        let name = crate::model::file_stem(&cls.filename)
            .trim_end_matches(".ont")
            .to_string();
        let content = String::from_utf8_lossy(&cls.content);
        classes.insert(name, alignment_of(&content));
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
    for (class, align) in &view.classes {
        let class_node = yidam(class)?;
        insert(&class_node, &a, node(&format!("{OWL_NS}Class"))?.into());
        insert(
            &class_node,
            &rdfs_label,
            Literal::new_simple_literal(class).into(),
        );
        if let Some(align) = align {
            if !align.ontology.is_empty() {
                insert(
                    &class_node,
                    &yidam("foundationalOntology")?,
                    Literal::new_simple_literal(&align.ontology).into(),
                );
            }
            if !align.ty.is_empty() {
                insert(
                    &class_node,
                    &yidam("foundationalType")?,
                    Literal::new_simple_literal(&align.ty).into(),
                );
            }
            // Only an absolute IRI can be an `exactMatch` object. A class that wrote
            // something else into `iri:` gets it as a literal rather than a parse failure —
            // the alignment is still worth exporting, and `lint` is where a malformed IRI
            // is somebody's to fix.
            if let Some(iri) = &align.iri {
                if iri.starts_with("http") {
                    insert(
                        &class_node,
                        &node(&format!("{SKOS_NS}exactMatch"))?,
                        node(iri)?.into(),
                    );
                } else {
                    insert(
                        &class_node,
                        &yidam("foundationalIri")?,
                        Literal::new_simple_literal(iri).into(),
                    );
                }
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

    for (class, align) in &view.classes {
        let mut obj = json!({
            "@id": format!("yidam:{class}"),
            "@type": "owl:Class",
            "rdfs:label": class,
        });
        if let Some(align) = align {
            if !align.ontology.is_empty() {
                obj["yidam:foundationalOntology"] = json!(align.ontology);
            }
            if !align.ty.is_empty() {
                obj["yidam:foundationalType"] = json!(align.ty);
            }
            if let Some(iri) = &align.iri {
                if iri.starts_with("http") {
                    obj["skos:exactMatch"] = json!({"@id": iri});
                } else {
                    obj["yidam:foundationalIri"] = json!(iri);
                }
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
    /// #613 — the export read `bfo_anchor:` and bootstrap wrote `foundational_type:`, so no
    /// corpus could satisfy both and an aligned corpus exported no `skos:exactMatch` at all.
    /// These are the shapes that were never once exercised.
    mod foundational_alignment {
        use super::*;

        fn model_with(class_yaml: &str) -> DomainModel {
            let mut m = test_model();
            m.classes = vec![OntClass {
                filename: "concept.ont.yml".into(),
                content: class_yaml.as_bytes().to_vec(),
            }];
            m
        }

        #[test]
        fn an_iri_becomes_exact_match_in_both_serializations() {
            let m = model_with(
                "class: concept\nfoundational_type:\n  ontology: ufo\n  type: relator\n  iri: https://purl.org/nemo/gufo#Relator\n",
            );
            let ttl = render_rdf_turtle(&m).unwrap();
            let jsonld = render_rdf_jsonld(&m).unwrap();
            for (name, out) in [("turtle", &ttl), ("json-ld", &jsonld)] {
                assert!(
                    out.contains("https://purl.org/nemo/gufo#Relator"),
                    "{name} dropped the alignment IRI"
                );
                assert!(out.contains("relator"), "{name} dropped the type");
                assert!(out.contains("ufo"), "{name} dropped the ontology");
            }
        }

        #[test]
        fn an_alignment_without_an_iri_still_reaches_rdf() {
            // The regression that made this a bug: `bfo_anchor:` could carry a URI and
            // nothing else, so a corpus that had declared an alignment but not looked up an
            // IRI exported no trace of having done so.
            let m = model_with(
                "class: concept\nfoundational_type:\n  ontology: bfo\n  type: continuant\n",
            );
            let ttl = render_rdf_turtle(&m).unwrap();
            assert!(ttl.contains("continuant"), "the type must reach the graph");
            assert!(ttl.contains("foundationalOntology"), "so must the ontology");
            assert!(
                !ttl.contains("exactMatch"),
                "with no iri there is nothing to claim an exact match with"
            );
        }

        #[test]
        fn a_ufo_alignment_exports_which_no_bfo_anchor_ever_could() {
            let m =
                model_with("class: concept\nfoundational_type:\n  ontology: ufo\n  type: kind\n");
            assert!(render_rdf_turtle(&m).unwrap().contains("\"ufo\""));
        }

        #[test]
        fn the_retired_bfo_anchor_is_still_read() {
            // Read for one release so a corpus that guessed it from the old
            // `domain-computer.md` keeps exporting. `lint` names the field either way.
            let m = model_with(
                "class: concept\nbfo_anchor: http://purl.obolibrary.org/obo/BFO_0000002\n",
            );
            let ttl = render_rdf_turtle(&m).unwrap();
            assert!(ttl.contains("skos:exactMatch") || ttl.contains("exactMatch"));
            assert!(ttl.contains("BFO_0000002"));
        }

        #[test]
        fn foundational_type_wins_over_a_stray_bfo_anchor() {
            let m = model_with(
                "class: concept\nbfo_anchor: http://purl.obolibrary.org/obo/BFO_0000001\nfoundational_type:\n  ontology: bfo\n  type: continuant\n  iri: http://purl.obolibrary.org/obo/BFO_0000002\n",
            );
            let ttl = render_rdf_turtle(&m).unwrap();
            assert!(
                ttl.contains("BFO_0000002"),
                "the declared field is the answer"
            );
            assert!(
                !ttl.contains("BFO_0000001"),
                "the retired field must not also be emitted — two exactMatches is two claims"
            );
        }

        #[test]
        fn a_class_with_no_alignment_emits_none_of_it() {
            let m = model_with("class: concept\n");
            let ttl = render_rdf_turtle(&m).unwrap();
            assert!(!ttl.contains("foundational"));
            assert!(!ttl.contains("exactMatch"));
        }
    }
}
