//! MCP resources — the corpus, skills, and decisions exposed as `yidam://` URIs.

use serde_json::{json, Value};
use std::collections::BTreeMap;

use super::{RpcError, ServerState};

fn text_contents(uri: &str, text: String) -> Value {
    json!({"contents": [{"uri": uri, "mimeType": "text/plain", "text": text}]})
}

/// True when the node counts as an open question, by any of the three arms the frozen
/// contract names: a `?`-prefixed label, an `[open]` claim in the body, or an open tag in a
/// property the class declared `type: claim`. Shared with the `open_questions` tool.
///
/// One predicate, shared with the reports.
///
/// This was a second copy of `label.starts_with('?') || content.contains("[open]")`, so a
/// corpus whose tags are structured was under-reported by the MCP server and by
/// `open-questions` in exactly the same way — which is the kind of agreement that looks like
/// correctness.
pub(crate) fn is_open_question(state: &ServerState, node: &super::Node) -> bool {
    crate::claims::is_open_question(
        &node.label,
        &node.content,
        state.claim_fields.for_class(&node.class),
    )
}

fn classes(state: &ServerState) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for node in &state.nodes {
        *map.entry(node.class.clone()).or_insert(0) += 1;
    }
    map
}

pub(crate) fn list(state: &ServerState) -> Value {
    let mut resources = vec![json!({
        "uri": "yidam://graph/summary",
        "name": "graph summary",
        "description": "Class list, node count, and open-question count",
        "mimeType": "text/plain"
    })];

    for (class, count) in classes(state) {
        resources.push(json!({
            "uri": format!("yidam://corpus/{class}"),
            "name": class,
            "description": format!("All {count} instance(s) of class {class:?}"),
            "mimeType": "text/plain"
        }));
    }
    for node in &state.nodes {
        resources.push(json!({
            "uri": format!("yidam://corpus/{}", node.id),
            "name": node.label,
            "description": node.description,
            "mimeType": "text/plain"
        }));
    }
    for (name, _) in &state.skills {
        resources.push(json!({
            "uri": format!("yidam://skills/{name}"),
            "name": name,
            "mimeType": "text/plain"
        }));
    }
    for (name, _) in &state.decisions {
        resources.push(json!({
            "uri": format!("yidam://decisions/{name}"),
            "name": name,
            "mimeType": "text/plain"
        }));
    }

    json!({"resources": resources})
}

pub(crate) fn read(state: &ServerState, uri: &str) -> Result<Value, RpcError> {
    let not_found = || RpcError::invalid_params(format!("unknown resource: {uri}"));

    if uri == "yidam://graph/summary" {
        return Ok(text_contents(uri, graph_summary(state)));
    }
    if let Some(rest) = uri.strip_prefix("yidam://corpus/") {
        return match rest.split_once('/') {
            // yidam://corpus/<class>/<name> — one instance
            Some(_) => {
                let node = state
                    .nodes
                    .iter()
                    .find(|n| n.id == rest)
                    .ok_or_else(not_found)?;
                Ok(text_contents(uri, node.content.clone()))
            }
            // yidam://corpus/<class> — list of instances in the class
            None => {
                let mut lines: Vec<String> = state
                    .nodes
                    .iter()
                    .filter(|n| n.class == rest)
                    .map(|n| format!("- {} ({}): {}", n.label, n.id, n.description))
                    .collect();
                if lines.is_empty() {
                    return Err(not_found());
                }
                lines.insert(0, format!("Instances of class {rest:?}:"));
                Ok(text_contents(uri, lines.join("\n")))
            }
        };
    }
    if let Some(name) = uri.strip_prefix("yidam://skills/") {
        let (_, content) = state
            .skills
            .iter()
            .find(|(n, _)| n == name)
            .ok_or_else(not_found)?;
        return Ok(text_contents(uri, content.clone()));
    }
    if let Some(name) = uri.strip_prefix("yidam://decisions/") {
        let (_, content) = state
            .decisions
            .iter()
            .find(|(n, _)| n == name)
            .ok_or_else(not_found)?;
        return Ok(text_contents(uri, content.clone()));
    }
    Err(not_found())
}

fn graph_summary(state: &ServerState) -> String {
    let open = state
        .nodes
        .iter()
        .filter(|n| is_open_question(state, n))
        .count();
    let class_lines: Vec<String> = classes(state)
        .iter()
        .map(|(class, count)| format!("  {class}: {count} instance(s)"))
        .collect();
    format!(
        "Domain: {}\nCommit: {}\nNodes: {}\nOpen questions: {}\nSkills: {}\nDecisions: {}\n\
         Vector index: {}\nIndex freshness: {}\nClasses:\n{}",
        state.domain,
        state.commit,
        state.nodes.len(),
        open,
        state.skills.len(),
        state.decisions.len(),
        match &state.retrieval {
            #[cfg(feature = "vector-read")]
            super::Retrieval::Vector(idx) =>
                format!("{} row(s), model {}", idx.rows.len(), idx.model_id),
            super::Retrieval::NoIndex => "absent (no_index)".to_string(),
            #[cfg(not(feature = "vector-read"))]
            super::Retrieval::NoVectorSupport =>
                "present, unreadable by this build (no_vector_support)".to_string(),
        },
        // The same fact the handshake reports, from the same `stale_index()`. This resource
        // and `capabilities.corpus` are two renderings of one answer to "which corpus is
        // this", and the point of #424 is that they stop being two different answers.
        match (state.stale_index(), &state.indexed_commit) {
            (None, _) => "unknown — no working repository to compare against".to_string(),
            (Some(true), Some(indexed)) => {
                format!("STALE — index built at {indexed}, HEAD is {}", state.commit)
            }
            (Some(_), None) => "no index".to_string(),
            (Some(false), Some(_)) => "current".to_string(),
        },
        class_lines.join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::test_state;
    use super::*;

    #[test]
    fn list_includes_all_five_resource_kinds() {
        let state = test_state();
        let result = list(&state);
        let uris: Vec<String> = result["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["uri"].as_str().unwrap().to_string())
            .collect();
        assert!(uris.contains(&"yidam://graph/summary".to_string()));
        assert!(uris.contains(&"yidam://corpus/concept".to_string()));
        assert!(uris.contains(&"yidam://corpus/concept/knowledge-graph".to_string()));
        assert!(uris.contains(&"yidam://skills/my-skill".to_string()));
        assert!(uris.contains(&"yidam://decisions/adr-1".to_string()));
    }

    #[test]
    fn read_instance_returns_yaml_source() {
        let state = test_state();
        let result = read(&state, "yidam://corpus/concept/knowledge-graph").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("label: Knowledge graph"));
    }

    #[test]
    fn read_class_lists_instances() {
        let state = test_state();
        let result = read(&state, "yidam://corpus/concept").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("Knowledge graph"));
        assert!(text.contains("concept/traversal"));
    }

    #[test]
    fn read_graph_summary_counts_open_questions() {
        let state = test_state();
        let result = read(&state, "yidam://graph/summary").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("Nodes: 2"));
        assert!(text.contains("Open questions: 1"));
    }

    #[test]
    fn read_unknown_uri_errors() {
        let state = test_state();
        assert!(read(&state, "yidam://corpus/nope/nothing").is_err());
        assert!(read(&state, "other://x").is_err());
    }
}
