use anyhow::Result;
use std::collections::{HashMap, HashSet};

use crate::model::{corpus_nodes, DomainModel};

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn truncate(s: &str, max: usize) -> &str {
    match s.char_indices().nth(max) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Serialize the corpus link graph as GraphML for Gephi, Cytoscape, yEd, etc.
///
/// One `<node>` per corpus instance (stable id `<class>/<name>`) with class,
/// description, commit, and link-count attributes; one directed `<edge>` per
/// resolved link with a `type` attribute carrying the relationship. Edges to
/// unknown targets are skipped with a stderr warning — a dangling link is a
/// corpus lint problem, not an export failure.
///
/// The schema is small enough to write directly; every attribute has the
/// `<key>` declaration the GraphML spec requires.
pub(crate) fn render_graphml(model: &DomainModel) -> Result<String> {
    let nodes = corpus_nodes(model);
    let known: HashSet<&str> = nodes.iter().map(|n| n.id.as_str()).collect();

    let mut incoming: HashMap<&str, usize> = HashMap::new();
    for node in &nodes {
        for (target, _) in &node.links {
            *incoming.entry(target.as_str()).or_insert(0) += 1;
        }
    }

    let mut out = String::new();
    out.push_str(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<graphml xmlns="http://graphml.graphdrawing.org/xmlns"
         xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
         xsi:schemaLocation="http://graphml.graphdrawing.org/xmlns http://graphml.graphdrawing.org/xmlns/1.0/graphml.xsd">
  <key id="label" for="node" attr.name="label" attr.type="string"/>
  <key id="class" for="node" attr.name="class" attr.type="string"/>
  <key id="description" for="node" attr.name="description" attr.type="string"/>
  <key id="commit" for="node" attr.name="commit" attr.type="string"/>
  <key id="outgoing_links" for="node" attr.name="outgoing_links" attr.type="int"/>
  <key id="incoming_links" for="node" attr.name="incoming_links" attr.type="int"/>
  <key id="type" for="edge" attr.name="type" attr.type="string"/>
"#,
    );
    out.push_str(&format!(
        "  <graph id=\"{}\" edgedefault=\"directed\">\n",
        xml_escape(&model.provenance.domain)
    ));

    for node in &nodes {
        out.push_str(&format!(
            "    <node id=\"{}\">\n      <data key=\"label\">{}</data>\n      \
             <data key=\"class\">{}</data>\n      <data key=\"description\">{}</data>\n      \
             <data key=\"commit\">{}</data>\n      <data key=\"outgoing_links\">{}</data>\n      \
             <data key=\"incoming_links\">{}</data>\n    </node>\n",
            xml_escape(&node.id),
            xml_escape(&node.label),
            xml_escape(&node.class),
            xml_escape(truncate(&node.description, 200)),
            xml_escape(&model.provenance.commit),
            node.links.len(),
            incoming.get(node.id.as_str()).copied().unwrap_or(0),
        ));
    }

    let mut edge_id = 0usize;
    for node in &nodes {
        for (target, relationship) in &node.links {
            if !known.contains(target.as_str()) {
                eprintln!(
                    "[warn] graphml: dangling link {} → {} — edge skipped",
                    node.id, target
                );
                continue;
            }
            out.push_str(&format!(
                "    <edge id=\"e{edge_id}\" source=\"{}\" target=\"{}\">\n      \
                 <data key=\"type\">{}</data>\n    </edge>\n",
                xml_escape(&node.id),
                xml_escape(target),
                xml_escape(relationship),
            ));
            edge_id += 1;
        }
    }

    out.push_str("  </graph>\n</graphml>\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DomainModel, InstanceFile, Provenance, RenderedViews};
    use quick_xml::events::Event;
    use quick_xml::Reader;

    fn model_with(instances: Vec<InstanceFile>) -> DomainModel {
        DomainModel {
            classes: vec![],
            instances,
            skills: vec![],
            decisions: vec![],
            index: None,
            provenance: Provenance {
                commit: "abc1234".into(),
                genesis: "2026-01-01".into(),
                domain: "test-domain".into(),
                generated_at: 0,
            },
            rendered: RenderedViews {
                corpus_index: String::new(),
                graph_check: String::new(),
                decisions_log: String::new(),
                skills_index: String::new(),
            },
        }
    }

    fn instance(class: &str, filename: &str, yaml: &str) -> InstanceFile {
        InstanceFile {
            class: class.into(),
            filename: filename.into(),
            content: yaml.as_bytes().to_vec(),
        }
    }

    fn test_model() -> DomainModel {
        model_with(vec![
            instance(
                "concept",
                "alpha.yml",
                "class: concept\nlabel: Alpha & <Beta>\ndescription: \"First node\"\n\
                 links:\n  - target: gamma.yml\n    relationship: causes\n\
                 \x20 - target: missing.yml\n    relationship: link\n",
            ),
            instance(
                "concept",
                "gamma.yml",
                "class: concept\nlabel: Gamma\ndescription: Second node\nlinks: []\n",
            ),
        ])
    }

    /// Round-trip through a real XML parser: node/edge counts and escaping.
    #[test]
    fn round_trips_through_quick_xml() {
        let m = test_model();
        let xml = render_graphml(&m).unwrap();

        let mut reader = Reader::from_str(&xml);
        let mut nodes = 0;
        let mut edges = 0;
        loop {
            match reader.read_event().expect("valid XML") {
                Event::Start(e) if e.name().as_ref() == b"node" => nodes += 1,
                Event::Start(e) if e.name().as_ref() == b"edge" => edges += 1,
                Event::Eof => break,
                _ => {}
            }
        }
        assert_eq!(nodes, m.instances.len());
        assert_eq!(edges, 1, "resolved edge kept, dangling edge skipped");
    }

    #[test]
    fn nodes_carry_attributes_and_edges_carry_type() {
        let xml = render_graphml(&test_model()).unwrap();
        assert!(xml.contains(r#"<node id="concept/alpha">"#));
        assert!(xml.contains(r#"<data key="label">Alpha &amp; &lt;Beta&gt;</data>"#));
        assert!(xml.contains(r#"<data key="class">concept</data>"#));
        assert!(xml.contains(r#"<data key="outgoing_links">2</data>"#));
        assert!(xml.contains(r#"<data key="incoming_links">1</data>"#));
        assert!(xml.contains(r#"source="concept/alpha" target="concept/gamma""#));
        assert!(xml.contains(r#"<data key="type">causes</data>"#));
        assert!(!xml.contains("missing"), "dangling edge is not emitted");
    }

    #[test]
    fn description_truncates_to_200_chars() {
        let long = "x".repeat(400);
        let m = model_with(vec![instance(
            "concept",
            "long.yml",
            &format!("class: concept\nlabel: Long\ndescription: {long}\nlinks: []\n"),
        )]);
        let xml = render_graphml(&m).unwrap();
        assert!(xml.contains(&"x".repeat(200)));
        assert!(!xml.contains(&"x".repeat(201)));
    }
}
