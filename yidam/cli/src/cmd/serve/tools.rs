//! MCP tools — retrieval and traversal over the domain computer.

use serde_json::{json, Value};

use super::resources::is_open_question;
use super::{IndexState, ServerState};

/// The frozen tool contract, compiled in.
///
/// `include_str!` and not a runtime read: a server that could not find its own contract
/// would have to decide what to serve without one, and there is no good answer to that. The
/// file is the single freeze — the E2E test reads it too, rather than restating the list,
/// which is what let three servers drift to one shared name out of five capabilities.
const CONTRACT: &str = include_str!("../../../../prelude/sdks/parity/mcp/tools.json");

/// What this server can actually back.
///
/// Filled honestly rather than optimistically. `phases` and `sangha` need the working git
/// repository and this server reads a built model on disk, so both are false — an explicit
/// statement an agent can read once, instead of a hole it discovers through a
/// tool-not-found error on the call it cared about.
pub(crate) fn capabilities(state: &ServerState) -> Value {
    json!({
        "contract": contract()["contract"].clone(),
        // Not a tier: `retrieve` is core either way. This says whether the index is loaded,
        // which is the same fact `degraded` reports per call.
        "retrieve": {"vector": state.index.is_some()},
        "graph": true,
        "phases": false,
        "sangha": false,
        "resources": true,
    })
}

fn contract() -> Value {
    serde_json::from_str(CONTRACT).expect("the compiled-in MCP contract is valid JSON")
}

/// Whether a tier is backed, given what this server declares.
fn backs(tier: &str, capabilities: &Value) -> bool {
    tier == "core" || capabilities[tier].as_bool().unwrap_or(false)
}

/// The tools this server serves: every core tool, plus the optional ones it declares.
///
/// Derived from the contract rather than written beside it, so a tool added to the contract
/// and not to this server fails the conformance check instead of quietly not existing.
pub(crate) fn list(state: &ServerState) -> Value {
    let contract = contract();
    let capabilities = capabilities(state);
    let tools: Vec<Value> = contract["tools"]
        .as_array()
        .expect("tools is an array")
        .iter()
        .filter(|t| backs(t["tier"].as_str().unwrap_or("core"), &capabilities))
        .map(|t| {
            json!({
                "name": t["name"],
                "description": t["description"],
                "inputSchema": t["inputSchema"],
            })
        })
        .collect();
    json!({ "tools": tools })
}

/// Dispatch a tools/call. Tool-level failures come back as MCP tool errors
/// (`isError: true`), not protocol errors — the agent can read and react.
pub(crate) fn call(state: &ServerState, name: &str, args: &Value) -> Value {
    let outcome = match name {
        "retrieve" => retrieve(state, args),
        "get_node" => get_node(state, args),
        "neighbors" => neighbors(state, args),
        "list_nodes" => Ok(list_nodes(state, args)),
        "open_questions" => Ok(open_questions(state)),
        other => Err(format!("unknown tool: {other}")),
    };
    match outcome {
        Ok(result) => {
            let text = serde_json::to_string_pretty(&result).unwrap_or_default();
            json!({"content": [{"type": "text", "text": text}]})
        }
        Err(message) => {
            json!({"content": [{"type": "text", "text": message}], "isError": true})
        }
    }
}

fn find_node<'a>(state: &'a ServerState, id: &str) -> Option<&'a super::Node> {
    let id = id.trim().trim_end_matches(".yml");
    state
        .nodes
        .iter()
        .find(|n| n.id == id)
        // Tolerate full repo paths like .yidam/corpus/<class>/<name>.yml
        .or_else(|| {
            state
                .nodes
                .iter()
                .find(|n| id.ends_with(&format!("corpus/{}", n.id)))
        })
}

fn retrieve(state: &ServerState, args: &Value) -> Result<Value, String> {
    let query = args["query"]
        .as_str()
        .ok_or("missing required argument: query")?;
    let k = args["k"].as_u64().unwrap_or(5).max(1) as usize;
    let class_filter = args["class"].as_str();

    match &state.index {
        Some(index) => vector_retrieve(index, query, k, class_filter),
        None => Ok(keyword_retrieve(state, query, k, class_filter)),
    }
}

fn vector_retrieve(
    index: &IndexState,
    query: &str,
    k: usize,
    class_filter: Option<&str>,
) -> Result<Value, String> {
    let mut embedder = index.embedder.borrow_mut();
    if embedder.is_none() {
        let (model, _, _) = crate::cmd::index_build::resolve_model(&index.model_id)
            .map_err(|e| format!("resolving embedding model: {e}"))?;
        let loaded = fastembed::TextEmbedding::try_new(fastembed::InitOptions::new(model))
            .map_err(|e| format!("loading embedding model {}: {e}", index.model_id))?;
        *embedder = Some(loaded);
    }
    let query_vec = embedder
        .as_ref()
        .expect("embedder initialised above")
        .embed(vec![query.to_string()], None)
        .map_err(|e| format!("embedding query: {e}"))?
        .remove(0);

    // Index vectors are L2-normalized (see embed.config.json), so cosine
    // similarity reduces to the dot product.
    let mut scored: Vec<(&super::VectorRow, f32)> = index
        .rows
        .iter()
        .filter(|r| class_filter.is_none_or(|c| r.class == c))
        .map(|r| {
            let score: f32 = r.vector.iter().zip(&query_vec).map(|(a, b)| a * b).sum();
            (r, score)
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1));
    scored.truncate(k);

    Ok(json!({
        "degraded": false,
        "results": scored.iter().map(|(r, score)| json!({
            "path": r.path,
            "class": r.class,
            "label": r.label,
            "text": r.text,
            "score": score,
        })).collect::<Vec<_>>()
    }))
}

/// Fallback when no vector index exists: case-insensitive term matching over
/// label, description, and body, scored by the fraction of query terms hit.
fn keyword_retrieve(
    state: &ServerState,
    query: &str,
    k: usize,
    class_filter: Option<&str>,
) -> Value {
    let terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect();

    let mut scored: Vec<(&super::Node, f32)> = state
        .nodes
        .iter()
        .filter(|n| class_filter.is_none_or(|c| n.class == c))
        .filter_map(|n| {
            let haystack = format!("{} {} {}", n.label, n.description, n.content).to_lowercase();
            let hits = terms
                .iter()
                .filter(|t| haystack.contains(t.as_str()))
                .count();
            if hits == 0 || terms.is_empty() {
                None
            } else {
                Some((n, hits as f32 / terms.len() as f32))
            }
        })
        .collect();
    scored.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.id.cmp(&b.0.id)));
    scored.truncate(k);

    json!({
        "degraded": true,
        "results": scored.iter().map(|(n, score)| json!({
            "path": format!(".yidam/corpus/{}.yml", n.id),
            "class": n.class,
            "label": n.label,
            "text": n.description,
            "score": score,
        })).collect::<Vec<_>>()
    })
}

fn get_node(state: &ServerState, args: &Value) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("missing required argument: id")?;
    let node = find_node(state, id).ok_or_else(|| format!("node not found: {id}"))?;
    Ok(json!({
        "id": node.id,
        "class": node.class,
        "label": node.label,
        "description": node.description,
        "content": node.content,
        "links": node.links.iter().map(|(target, rel)| json!({
            "target": target,
            "relationship": rel,
        })).collect::<Vec<_>>()
    }))
}

fn neighbors(state: &ServerState, args: &Value) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("missing required argument: id")?;
    let depth = args["depth"].as_u64().unwrap_or(1).max(1) as usize;
    let start = find_node(state, id).ok_or_else(|| format!("node not found: {id}"))?;

    // The traversal itself lives in `cmd::graph`, in the light build, because `yidam
    // neighbors` and the editor's neighbourhood view have to answer this the same way.
    // It used to live here — behind the heavy `index` feature — where nothing else could
    // reach it.
    let edges: Vec<(String, String, String)> = state
        .nodes
        .iter()
        .flat_map(|n| {
            n.links
                .iter()
                .map(move |(target, rel)| (n.id.clone(), target.clone(), rel.clone()))
        })
        .collect();

    let found: Vec<Value> = crate::cmd::graph::walk_neighbors(&edges, &start.id, depth)
        .into_iter()
        .map(|hit| {
            let (label, description) = state
                .nodes
                .iter()
                .find(|n| n.id == hit.id)
                .map(|n| (n.label.clone(), n.description.clone()))
                .unwrap_or_default();
            json!({
                "id": hit.id,
                "label": label,
                "description": description,
                "relationship": hit.relationship,
                "direction": hit.direction,
                "depth": hit.depth,
            })
        })
        .collect();

    Ok(json!({"id": start.id, "neighbors": found}))
}

/// The tool form of `yidam://corpus/<class>`.
///
/// Core rather than optional so a tools-only server is first-class — one with no MCP
/// resource channel at all should still be able to answer "what is in this corpus" without
/// its callers learning a second access pattern.
fn list_nodes(state: &ServerState, args: &Value) -> Value {
    let class = args["class"].as_str();
    let nodes: Vec<Value> = state
        .nodes
        .iter()
        .filter(|n| class.is_none_or(|c| n.class == c))
        .map(|n| {
            json!({
                "id": n.id,
                "class": n.class,
                "label": n.label,
                "description": n.description,
            })
        })
        .collect();
    json!({"nodes": nodes})
}

fn open_questions(state: &ServerState) -> Value {
    let questions: Vec<Value> = state
        .nodes
        .iter()
        .filter(|n| is_open_question(n))
        .map(|n| {
            json!({
                "id": n.id,
                "label": n.label,
                "path": format!(".yidam/corpus/{}.yml", n.id),
            })
        })
        .collect();
    json!({"open_questions": questions})
}

#[cfg(test)]
mod tests {
    use super::super::tests::test_state;
    use super::*;

    fn call_ok(state: &ServerState, name: &str, args: Value) -> Value {
        let result = call(state, name, &args);
        assert!(
            result["isError"].as_bool() != Some(true),
            "tool {name} errored: {result}"
        );
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }

    #[test]
    fn retrieve_without_index_degrades_to_keyword() {
        let state = test_state();
        let result = call_ok(&state, "retrieve", json!({"query": "knowledge graph"}));
        assert_eq!(result["degraded"], true);
        let results = result["results"].as_array().unwrap();
        assert!(!results.is_empty());
        assert_eq!(results[0]["class"], "concept");
        assert_eq!(results[0]["label"], "Knowledge graph");
    }

    #[test]
    fn retrieve_honors_class_filter_and_k() {
        let state = test_state();
        let result = call_ok(
            &state,
            "retrieve",
            json!({"query": "graph", "class": "nonexistent", "k": 1}),
        );
        assert!(result["results"].as_array().unwrap().is_empty());
    }

    #[test]
    fn get_node_returns_content_and_links() {
        let state = test_state();
        let result = call_ok(&state, "get_node", json!({"id": "concept/knowledge-graph"}));
        assert_eq!(result["label"], "Knowledge graph");
        assert_eq!(result["links"][0]["target"], "concept/traversal");
        assert_eq!(result["links"][0]["relationship"], "enables");
    }

    #[test]
    fn neighbors_walks_both_directions() {
        let state = test_state();
        // traversal has no outgoing links, but knowledge-graph points at it
        let result = call_ok(&state, "neighbors", json!({"id": "concept/traversal"}));
        let found = result["neighbors"].as_array().unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0]["id"], "concept/knowledge-graph");
        assert_eq!(found[0]["direction"], "in");
    }

    #[test]
    fn open_questions_flags_question_labels() {
        let state = test_state();
        let result = call_ok(&state, "open_questions", json!({}));
        let qs = result["open_questions"].as_array().unwrap();
        assert_eq!(qs.len(), 1);
        assert_eq!(qs[0]["id"], "concept/traversal");
    }

    #[test]
    fn unknown_tool_is_a_tool_error() {
        let state = test_state();
        let result = call(&state, "bogus", &json!({}));
        assert_eq!(result["isError"], true);
    }
}
