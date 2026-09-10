use anyhow::{Context, Result};
use oxrdf::{Graph, Literal, NamedNode, NamedNodeRef, Triple};
use serde_json::json;
use std::collections::BTreeMap;

use super::export::unix_to_iso;
use crate::model::{corpus_nodes, DomainModel};

/// The ontology namespace — an origin this project demonstrably owns.
///
/// It was `https://yidam.dev/ontology#` and that domain does not resolve: `curl` answers 000,
/// not 404. It shipped in every corpus's exported triples, which makes it worse than a dead
/// link — an unowned namespace already published in other people's data is registrable by a
/// stranger, who would then be authoritative for terms this project minted (RFC-0032 §2).
///
/// `goedelsoup.github.io/yidam` is the one origin with a demonstrated 200. It is where the docs
/// site is served from, and the docs site is versioned — `/yidam/` is the release build and
/// `/yidam/main/` is not — so **the term URIs here are stable while the document they resolve to
/// is not yet published**: a page describing these terms appears at `/yidam/main/…` on merge and
/// at this path with the next `cli/v*` tag. The gate below asserts *ownership* rather than
/// resolution, deliberately, because a check keyed on an artifact that does not exist yet goes
/// red the day it lands and teaches everyone to ignore it.
const YIDAM_NS: &str = "https://goedelsoup.github.io/yidam/ontology#";

/// Every prefix a subject or predicate this exporter mints is allowed to have.
///
/// `urn:yidam:` for identity, the docs origin for terms. Read by
/// `every_minted_iri_is_owned_or_a_urn`, which is the guard RFC-0032 §2 asks for: the export
/// already tests *other people's* alignment IRIs for a scheme it can dereference and never
/// applied that test to its own.
///
/// **Written as a literal, and deliberately not built from [`YIDAM_NS`].** The first version of
/// this list was `&["urn:yidam:", YIDAM_NS]`, which is vacuous: moving the namespace back to
/// `https://yidam.dev/` moved the allow-list with it and the guard stayed green. A guard whose
/// notion of *allowed* is derived from the value under test passes by construction. Mutation
/// testing is what found it, which is the second time in this file's history that a check had to
/// be broken on purpose before it was worth its green.
///
/// The cost is that moving to a different owned origin means editing two places. That is the
/// right cost: *is this origin ours* is a claim a person makes, not one a constant can.
///
/// Test-only: it is an assertion about the constant above, not an input to the export. It lives
/// here rather than in the test module so that a reader changing [`YIDAM_NS`] sees the claim they
/// are also making.
#[cfg(test)]
const MINTED_PREFIXES: &[&str] = &["urn:yidam:", "https://goedelsoup.github.io/"];
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

/// The RDF subject for one node: a `urn:yidam:` name, never a `yidam://` IRI.
///
/// Three things were wrong with `yidam://corpus/{id}`. No RDF consumer can dereference the
/// scheme; the authority slot held the collection kind `corpus` rather than a corpus, so two
/// corpora holding `concept/foo` minted the *same subject* and merging their triples silently
/// conflated two nodes; and it was a fourth hand-built spelling of an id.
///
/// So the name is built by [`yidam_core::uri::render_reference`] — the one place an identifier is
/// built — and re-schemed. `yidam://<corpus>/node/<class>/<name>` becomes
/// `urn:yidam:<corpus>/node/<class>/<name>`. That is a transformation of the rendered form rather
/// than a second assembly from parts, which is the distinction RFC-0032 §4.6 is about: one
/// renderer, and a scheme swap on its output.
///
/// **A URN and not a locator, because no corpus can declare a base yet.** RFC-0032 §4.2 says a
/// subject uses the locator where one is declared and a `urn:` form where none is. `public_base`
/// is RFC-0027's and unshipped, so the locator branch would be a branch nothing can reach — the
/// surface-with-no-consumer shape this repository keeps finding. It lands with `public_base`.
fn instance_iri(corpus: &str, id: &str) -> String {
    urn_of(&reference_for(corpus, id))
}

/// A link target as a reference, which is not always a node.
///
/// [`resolve_link_target`](crate::model::resolve_link_target) returns a target verbatim when it
/// escapes the corpus directory, and every such target in the measured corpora is one shape:
/// `../../catalog/<file>.md` — 37 of 185 subjects in one corpus and 18 of 810 in another, 55 in
/// total, all of them catalog entries. Minting those as nodes produced
/// `urn:yidam:<hash>/node/../../catalog/x.md`: a subject with relative path segments inside it,
/// claiming to be a node, for a thing that is a catalog entry.
///
/// RFC-0032's grammar already has the `catalog` kind for this, so the fix is to use it rather than
/// to invent an escape. Nothing else escapes — the measurement found exactly one shape — so this
/// is complete rather than a first case.
fn reference_for(corpus: &str, target: &str) -> yidam_core::uri::Reference {
    let (kind, path) = match target.rsplit_once("catalog/") {
        Some((prefix, entry)) if prefix.contains("..") || prefix.is_empty() => (
            yidam_core::uri::Kind::Catalog,
            entry.strip_suffix(".md").unwrap_or(entry).to_string(),
        ),
        _ => (yidam_core::uri::Kind::Node, target.to_string()),
    };
    yidam_core::uri::Reference {
        corpus: Some(corpus.to_string()),
        kind,
        path,
        rev: None,
        fragment: None,
    }
}

/// What names the corpus in a subject: the genesis hash, short.
///
/// **The declared package name was the first answer and it was wrong**, which measuring found
/// before this shipped: zero of the sixteen corpora with tracked nodes declare one — none of them
/// has a `.yidam/tonpa.toml` at all — so every subject would have carried the same fallback and
/// two corpora holding `concept/foo` would still have minted the same subject. The fix for §2
/// would have left §2 in place.
///
/// Twelve hex characters, because a subject is read by machines and a full 40 in every triple buys
/// nothing; git's own collision margin at twelve is far beyond the number of corpora that will
/// ever exist. It is a slug by construction, so it needs no escaping and
/// `reference_conforms` holds for every subject this exporter mints.
///
/// The readable name still belongs in an identifier a person types — that is RFC-0032 §4.5 and it
/// is unchanged. The two are different jobs, which is the same split §4.2 makes between an
/// identifier and a locator.
fn corpus_component(model: &DomainModel) -> Result<String> {
    match &model.provenance.genesis_hash {
        Some(hash) => Ok(hash.chars().take(12).collect()),
        // Refused rather than substituted. A constant here — `local`, `unknown`, anything —
        // is the same string for every corpus that reaches this branch, so two of them merged
        // into one triple store would conflate their nodes: exactly the defect §2 is about,
        // reintroduced by the code that fixes it. An export that stops is recoverable; a
        // published subject that silently names another corpus's node is not.
        //
        // The branch is reachable, and by two different routes. A repository with no root
        // commit to name — an empty one, a shallow clone without it — and a corpus that is a
        // *directory inside* a repository rather than one itself, which is #792: git walks up,
        // so every corpus nested in one host was handed the host's genesis and the four under
        // `examples/` all minted `urn:yidam:094509a128f4`.
        //
        // The message has to name both, because the older wording ("git answered nothing")
        // is false for the second and its advice — "pass --root" — is what the person just
        // did. A refusal that describes the wrong cause sends them somewhere there is
        // nothing to find.
        None => anyhow::bail!(
            "the RDF export names its subjects with this corpus's own genesis commit, and \
             this corpus has none. Either it is not a git repository, or it is a directory \
             inside one — in which case git answers with the enclosing repository, whose \
             genesis names a different corpus and is shared by every corpus nested in it. \
             Every other format works without this; RDF does not, because a subject that \
             cannot say which corpus it belongs to conflates nodes when two corpora are \
             merged into one store. Give the corpus a repository of its own — `git init` in \
             it, or copy it out with `yidam clone` — or export a format that does not name \
             subjects."
        ),
    }
}

/// A rendered identifier, re-schemed as a URN.
///
/// `yidam://` is not a registered scheme and nothing dereferences it. `urn:` is, and a URN is
/// the correct shape for a name that is deliberately *not* a location — which is what an
/// identifier that must survive a `.yiz` tarball, a private corpus and an offline clone is.
/// RFC-0032 considered and rejected `urn:yidam:` as the *primary* form, for reasons about client
/// familiarity that do not apply to an RDF subject; this is the one job it named a URN as better
/// at than either alternative.
fn urn_of(reference: &yidam_core::uri::Reference) -> String {
    let rendered = yidam_core::uri::render_reference(reference);
    match rendered.strip_prefix("yidam://") {
        Some(rest) => format!("urn:yidam:{rest}"),
        // Unreachable for a reference carrying a corpus, which every caller here supplies. Not
        // an `expect`: an export that panicked on a corpus it could not name would be a worse
        // failure than one that names it a little less well.
        None => format!("urn:yidam:{rendered}"),
    }
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

fn build_view(model: &DomainModel) -> Result<RdfView> {
    let corpus = corpus_component(model)?;
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
                    // The IRI, not the id: `build_graph` and the JSON-LD renderer both read
                    // this and neither can see the package name, so re-deriving it there would
                    // mean two more places that build an identifier.
                    unresolved.insert(instance_iri(&corpus, target), ());
                }
                let prop = property_local_name(relationship);
                if prop != "linksTo" {
                    relationships.insert(prop.clone(), ());
                }
                (instance_iri(&corpus, target), prop)
            })
            .collect();
        instances.push(RdfInstance {
            iri: instance_iri(&corpus, &node.id),
            class: node.class.clone(),
            label: node.label.clone(),
            description: node.description.clone(),
            links,
        });
    }

    Ok(RdfView {
        // The dataset is the corpus itself, so it is named the same way its nodes are. It was
        // `yidam://corpus` — a bare authority, identical for every corpus that ever exported.
        dataset_iri: format!("urn:yidam:{corpus}"),
        domain: model.provenance.domain.clone(),
        commit: model.provenance.commit.clone(),
        genesis: model.provenance.genesis.clone(),
        generated_at_iso: unix_to_iso(model.provenance.generated_at),
        classes,
        instances,
        relationships: relationships.into_keys().collect(),
        unresolved: unresolved.into_keys().collect(),
    })
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
        let subject = node(target)?;
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
    let graph = build_graph(&build_view(model)?)?;
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
    let view = build_view(model)?;

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
            "@id": target,
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
                genesis_hash: Some("abcdef012345deadbeef".into()),
                ..crate::model::test_provenance()
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    /// Every namespace this exporter does **not** mint, and is right not to.
    ///
    /// A guard that allowed anything would pass, so the foreign set is listed and the assertion
    /// below is over what is left: an IRI that is neither one of these nor one of ours is a
    /// namespace somebody invented in passing.
    const FOREIGN_NS: &[&str] = &[OWL_NS, RDFS_NS, RDF_NS, SKOS_NS, PROV_NS, XSD_NS];

    /// Every IRI in the output, or only the ones this exporter *mints*.
    ///
    /// The distinction is the difference between the two guards below, and getting it wrong is
    /// how the first draft of `every_minted_iri_is_owned_or_a_urn` failed: it flagged
    /// `http://purl.obolibrary.org/obo/BFO_0000001`, which a corpus declared as a foundational
    /// alignment. A `skos:exactMatch` object is *by definition* a foreign ontology's IRI — that
    /// is what an alignment is — so ownership cannot be asserted over objects.
    ///
    /// Subjects and predicates are ours: a subject is a thing this corpus is speaking about and a
    /// predicate is either a term we minted or a standard one. The scheme guard runs over
    /// everything, because *no* IRI in the output should be in a scheme nothing resolves.
    fn iris_of(ttl: &str, minted_only: bool) -> Vec<String> {
        let triples: Vec<_> = oxttl::TurtleParser::new()
            .for_reader(ttl.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .expect("valid Turtle");
        assert!(!triples.is_empty(), "the scan read no triples at all");
        let mut out = Vec::new();
        for triple in &triples {
            out.push(triple.subject.to_string());
            out.push(triple.predicate.as_str().to_string());
            if !minted_only {
                if let oxrdf::Term::NamedNode(n) = &triple.object {
                    out.push(n.as_str().to_string());
                }
            }
        }
        out.iter()
            .map(|s| s.trim_start_matches('<').trim_end_matches('>').to_string())
            .filter(|s| s.contains(':'))
            .collect()
    }

    /// No emitted IRI is in a scheme nothing can dereference.
    ///
    /// This is RFC-0032 §2 as a test. The export already applied the equivalent check to *other
    /// people's* alignment IRIs — `iri.starts_with("http")`, demoting anything else to a literal
    /// — and never applied it to its own subjects, which were `yidam://corpus/<class>/<name>`
    /// in a scheme no consumer resolves and with no corpus component, so two corpora holding
    /// `concept/foo` minted the same subject.
    ///
    /// Mutation-checked by putting `yidam://corpus` back: see
    /// `the_scheme_guard_catches_the_form_it_replaced`.
    #[test]
    fn no_emitted_iri_is_in_an_unresolvable_scheme() {
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        for iri in iris_of(&ttl, false) {
            let scheme = iri.split(':').next().unwrap_or_default();
            assert!(
                matches!(scheme, "http" | "https" | "urn"),
                "emitted IRI in scheme {scheme:?}, which no RDF consumer resolves: {iri}"
            );
        }

        // And the same over JSON-LD, whose `@id`s are the same identifiers through a second
        // serializer. Both consume one `RdfView`, and a guard reading only one of them would
        // pass while the other drifted — which is the whole reason that intermediate exists.
        let jsonld = render_rdf_jsonld(&test_model()).unwrap();
        let doc: serde_json::Value = serde_json::from_str(&jsonld).unwrap();
        // An `@id` may be a *compact* IRI — `yidam:concept` — which is a CURIE resolved through
        // `@context`, not a scheme. The first draft of this guard read `yidam:concept` as scheme
        // `yidam` and failed. Expanding through the context first is the stronger check anyway,
        // because it also asserts that what the prefix expands *to* is resolvable.
        let context = doc["@context"].as_object().expect("a JSON-LD @context");
        let expand = |s: &str| -> String {
            match s.split_once(':') {
                Some((prefix, local)) => match context.get(prefix).and_then(|v| v.as_str()) {
                    Some(ns) => format!("{ns}{local}"),
                    None => s.to_string(),
                },
                None => s.to_string(),
            }
        };
        let mut ids = 0usize;
        let mut check = |s: &str| {
            let full = expand(s);
            let scheme = full.split(':').next().unwrap_or_default();
            assert!(
                matches!(scheme, "http" | "https" | "urn"),
                "JSON-LD @id in scheme {scheme:?}: {s} (expands to {full})"
            );
            ids += 1;
        };
        fn walk(v: &serde_json::Value, check: &mut impl FnMut(&str)) {
            match v {
                serde_json::Value::Object(map) => {
                    for (k, inner) in map {
                        if k == "@id" {
                            if let Some(s) = inner.as_str() {
                                check(s);
                            }
                        }
                        walk(inner, check);
                    }
                }
                serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, check)),
                _ => {}
            }
        }
        // `@graph` holds every node *and* the dataset — the ontology object is the first entry
        // there, not a top-level `@id`. Walking the document root instead would read `@context`'s
        // namespace values as identifiers.
        walk(&doc["@graph"], &mut check);
        assert!(ids >= 3, "only {ids} @id(s) examined in the JSON-LD");
    }

    /// And every IRI this exporter mints is under an origin the project owns, or a URN.
    ///
    /// The scheme test above would pass a namespace at someone else's `https://` origin. This is
    /// the other half: `yidam.dev` was NXDOMAIN and shipping in every corpus's triples, which
    /// makes it registrable by a stranger who would then be authoritative for terms this project
    /// minted.
    #[test]
    fn every_minted_iri_is_owned_or_a_urn() {
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        let mut checked = 0usize;
        for iri in iris_of(&ttl, true) {
            if FOREIGN_NS.iter().any(|ns| iri.starts_with(ns)) {
                continue;
            }
            checked += 1;
            assert!(
                MINTED_PREFIXES.iter().any(|p| iri.starts_with(p)),
                "minted IRI under no prefix this project owns: {iri}"
            );
        }
        // A scan that skipped everything would pass the loop above having checked nothing — the
        // shape every guard in this repository is written against.
        assert!(
            checked >= 5,
            "only {checked} minted IRI(s) examined, so this is reading the wrong triples"
        );
    }

    /// Every minted subject is a *conforming* reference, not merely an owned string.
    ///
    /// The guard above asks whether a prefix is ours. This asks whether what follows it is a
    /// reference at all, by parsing it back through [`yidam_core::uri`] and applying
    /// `reference_conforms` — the same predicate `name-not-a-slug` enforces on a corpus.
    ///
    /// It is here because the ownership guard passed while 20% of one real corpus's subjects were
    /// malformed: `resolve_link_target` returns a target verbatim when it escapes the corpus
    /// directory, and 55 subjects across two corpora were
    /// `urn:yidam:<hash>/node/../../catalog/<file>.md` — relative path segments inside an
    /// identifier, typed as a node, for a thing that is a catalog entry. A prefix check cannot see
    /// that. Parsing can.
    #[test]
    fn every_minted_subject_is_a_conforming_reference() {
        let ttl = render_rdf_turtle(&test_model()).unwrap();
        let mut checked = 0usize;
        for iri in iris_of(&ttl, true) {
            let Some(rest) = iri.strip_prefix("urn:yidam:") else {
                continue;
            };
            // The dataset subject is the corpus *itself* — `urn:yidam:<corpus>`, an authority
            // with no path — and that is deliberately not a reference: the grammar addresses
            // things in a corpus, and a corpus is what they are addressed relative to. It has
            // no `/`, which is how it is told apart here.
            if !rest.contains('/') {
                continue;
            }
            checked += 1;
            let reference = yidam_core::uri::parse_reference(&format!("yidam://{rest}"))
                .unwrap_or_else(|| panic!("minted subject does not parse: {iri}"));
            assert!(
                yidam_core::uri::reference_conforms(&reference),
                "minted subject is not a conforming reference: {iri}"
            );
        }
        assert!(
            checked >= 3,
            "only {checked} urn subject(s) examined, so this is reading the wrong triples"
        );
    }

    /// A link out of the corpus into the catalog is a `catalog` reference, not a node.
    #[test]
    fn a_catalog_link_is_typed_catalog_and_not_a_node_with_dots_in_it() {
        let reference = reference_for("abc123", "../../catalog/lsc-redbook.md");
        assert_eq!(reference.kind, yidam_core::uri::Kind::Catalog);
        assert_eq!(reference.path, "lsc-redbook");
        assert_eq!(
            urn_of(&reference),
            "urn:yidam:abc123/catalog/lsc-redbook",
            "the shape 55 subjects across two corpora had wrong"
        );
        assert!(yidam_core::uri::reference_conforms(&reference));

        // …and a node target is still a node.
        let node = reference_for("abc123", "concept/alpha");
        assert_eq!(node.kind, yidam_core::uri::Kind::Node);
        assert_eq!(node.path, "concept/alpha");
    }

    /// A corpus whose genesis commit cannot be read is refused, not given a shared name.
    ///
    /// Any constant here is the same string for every corpus that reaches the branch, so two of
    /// them merged into one store would conflate their nodes — the defect §2 is about,
    /// reintroduced by its own fix. The branch is reachable: a worktree copied out of its parent,
    /// a shallow clone without the root commit, or a `.yiz` extracted to a plain directory all
    /// land here, and the bundle manifest cannot supply it because it records `genesis` as an ISO
    /// date rather than a digest.
    #[test]
    fn a_corpus_with_no_genesis_commit_is_refused_rather_than_named_like_every_other() {
        let mut model = test_model();
        model.provenance.genesis_hash = None;
        let err = render_rdf_turtle(&model).expect_err("an unidentifiable corpus must not export");
        let message = err.to_string();
        assert!(
            message.contains("genesis commit"),
            "the refusal must say what is missing: {message}"
        );
        // Every other format is unaffected — RDF is the one whose output is meant to be merged
        // with other people's data, which is why it is the one that cannot guess.
        assert!(
            render_rdf_jsonld(&model).is_err(),
            "both serializers refuse, since both consume one view"
        );
    }

    /// The guards catch the form they replaced.
    ///
    /// Not a mutation of the source — a direct assertion that the old strings fail the two
    /// predicates the tests above apply. A guard is only worth its green if the thing it was
    /// written against would have gone red, and running that as a test rather than by hand means
    /// it stays true.
    #[test]
    fn the_scheme_guard_catches_the_form_it_replaced() {
        for old in ["yidam://corpus/concept/alpha", "yidam://corpus"] {
            let scheme = old.split(':').next().unwrap_or_default();
            assert!(
                !matches!(scheme, "http" | "https" | "urn"),
                "the old subject form would pass the scheme guard: {old}"
            );
        }
        // …and the old namespace passes the scheme test and fails the ownership one, which is
        // why both exist.
        let dead = "https://yidam.dev/ontology#concept";
        assert!(dead.starts_with("http"), "premise: it was an https IRI");
        assert!(
            !MINTED_PREFIXES.iter().any(|p| dead.starts_with(p)),
            "the NXDOMAIN namespace would pass the ownership guard"
        );
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
                && t.subject.to_string() == "<urn:yidam:abcdef012345/node/concept/alpha>"),
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
            t.subject.to_string() == "<urn:yidam:abcdef012345/node/concept/missing>"
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
            .find(|o| o["@id"] == "urn:yidam:abcdef012345/node/concept/alpha")
            .expect("alpha present");
        assert_eq!(alpha["@type"], "yidam:concept");
        assert_eq!(
            alpha["yidam:causes"]["@id"],
            "urn:yidam:abcdef012345/node/concept/gamma"
        );
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
