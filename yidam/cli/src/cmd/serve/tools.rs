//! MCP tools — retrieval and traversal over the domain computer.

use serde_json::{json, Value};

use super::resources::is_open_question;
#[cfg(feature = "index")]
use super::Retrieval;
use super::ServerState;

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
    let reason = state.retrieval.degraded_reason();
    json!({
        "contract": contract()["contract"].clone(),
        // Not a tier: `retrieve` is core either way. This says whether the index is loaded,
        // which is the same fact `degraded` reports per call — so it is rendered from the
        // same source, and carries the same reason. A client that reads the handshake
        // learns at connect time what it would otherwise learn one failed search later.
        "retrieve": {"vector": reason.is_none(), "reason": reason},
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

/// Resolve an id to a node this repository owns.
///
/// Bare ids only. A `pkg::class/name` never matches here, which is what keeps a dependency
/// from answering as though it were local.
fn find_node<'a>(state: &'a ServerState, id: &str) -> Option<&'a super::Node> {
    let id = id.trim().trim_end_matches(".yml");
    if id.contains("::") {
        return None;
    }
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

/// Resolve an id that may name a dependency's node, as `pkg::class/name`.
///
/// `retrieve` hands back qualified ids, so they have to be usable — an id a client is shown
/// and then cannot fetch is a worse affordance than not surfacing the node at all. Reading
/// one is allowed; it is *citing* one that is not.
fn find_any_node<'a>(state: &'a ServerState, id: &str) -> Option<&'a super::Node> {
    let id = id.trim().trim_end_matches(".yml");
    match id.split_once("::") {
        Some((pkg, rest)) => state
            .dep_nodes
            .iter()
            .find(|n| n.origin.as_deref() == Some(pkg) && n.id == rest),
        None => find_node(state, id),
    }
}

fn retrieve(state: &ServerState, args: &Value) -> Result<Value, String> {
    let query = args["query"]
        .as_str()
        .ok_or("missing required argument: query")?;
    let k = args["k"].as_u64().unwrap_or(5).max(1) as usize;
    let class_filter = args["class"].as_str();

    #[cfg(feature = "index")]
    if let Retrieval::Vector(index) = &state.retrieval {
        return super::vector::retrieve(index, query, k, class_filter);
    }
    Ok(keyword_retrieve(
        state,
        query,
        k,
        class_filter,
        state.retrieval.degraded_reason(),
    ))
}

/// Fallback when no vector index exists: case-insensitive term matching over
/// label, description, and body, scored by the fraction of query terms hit.
fn keyword_retrieve(
    state: &ServerState,
    query: &str,
    k: usize,
    class_filter: Option<&str>,
    reason: Option<&'static str>,
) -> Value {
    let terms: Vec<String> = query
        .to_lowercase()
        .split_whitespace()
        .map(str::to_string)
        .collect();

    // Local nodes first, then every installed dependency's. Retrieval is the one surface a
    // dependency is allowed on: an agent asking "what is known about X" should be told when
    // the answer lives in a corpus this repository merely cites, not have it withheld — and
    // each result says whose it is, so it can never be mistaken for this repository's claim.
    let mut scored: Vec<(&super::Node, f32)> = state
        .nodes
        .iter()
        .chain(state.dep_nodes.iter())
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
    // Ties break on the qualified id, not the bare one: two corpora may hold the same
    // `class/name`, and ordering that cannot tell them apart is not deterministic.
    scored.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| a.0.qualified_id().cmp(&b.0.qualified_id()))
    });
    scored.truncate(k);

    json!({
        "degraded": true,
        // *Why* degraded, not just that it is. The bare boolean made two different
        // repositories look identical: one that never built an index, and one whose index
        // this binary cannot read. Both are keyword search; only one is fixed by indexing.
        "degraded_reason": reason,
        "results": scored.iter().map(|(n, score)| json!({
            "id": n.qualified_id(),
            "path": match &n.origin {
                Some(pkg) => format!(".yidam/tonpa/{pkg}/corpus/{}.yml", n.id),
                None => format!(".yidam/corpus/{}.yml", n.id),
            },
            // Always present, and null for this repository's own nodes rather than absent:
            // a consumer testing for the key must not have to distinguish "local" from "an
            // older server that never said".
            "origin": n.origin,
            "class": n.class,
            "label": n.label,
            "text": n.description,
            "score": score,
        })).collect::<Vec<_>>()
    })
}

fn get_node(state: &ServerState, args: &Value) -> Result<Value, String> {
    let id = args["id"].as_str().ok_or("missing required argument: id")?;
    let node = find_any_node(state, id).ok_or_else(|| format!("node not found: {id}"))?;
    Ok(json!({
        "id": node.qualified_id(),
        "origin": node.origin,
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
    // A qualified id names a dependency's node. `retrieve` and `get_node` will both answer
    // for one; traversal will not, and the difference is the whole boundary — an edge is a
    // claim, and this repository has asserted none into a corpus it merely installed. Say
    // that, rather than reporting a node that demonstrably exists as missing.
    if id.contains("::") {
        return Err(format!(
            "{id} belongs to an installed dependency; traversal does not cross corpus              boundaries. Read it with get_node, or search it with retrieve."
        ));
    }
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
        .filter(|n| is_open_question(state, n))
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

    /// A dependency's nodes are searchable, and every result says whose it is.
    ///
    /// Withholding them would be the wrong answer to "what is known about X": an agent
    /// asking that should be told when the answer lives in a corpus this repository merely
    /// installed. Labelling them is what keeps that from reading as this repository's claim.
    #[test]
    fn retrieve_spans_dependencies_and_labels_their_origin() {
        let state = super::super::tests::test_state();
        let out = call_ok(
            &state,
            "retrieve",
            json!({"query": "knowledge graph", "k": 10}),
        );
        let results = out["results"].as_array().expect("results");

        let foreign: Vec<&Value> = results
            .iter()
            .filter(|r| r["origin"].as_str() == Some("upstream"))
            .collect();
        assert_eq!(
            foreign.len(),
            1,
            "the dependency's node must be findable: {results:#?}"
        );
        assert_eq!(foreign[0]["id"], "upstream::concept/knowledge-graph");
        assert_eq!(
            foreign[0]["path"], ".yidam/tonpa/upstream/corpus/concept/knowledge-graph.yml",
            "a foreign node's path must point into the dependency, not into this corpus"
        );

        // `origin` is null for local nodes rather than absent, so a consumer testing the key
        // never has to distinguish "local" from "a server too old to say".
        let local: Vec<&Value> = results.iter().filter(|r| r["origin"].is_null()).collect();
        assert!(
            !local.is_empty(),
            "local nodes must still be returned: {results:#?}"
        );
    }

    /// An id `retrieve` hands out has to be usable, or surfacing the node was a worse
    /// affordance than hiding it.
    #[test]
    fn get_node_reads_a_dependency_by_its_qualified_id() {
        let state = super::super::tests::test_state();
        let out = call_ok(
            &state,
            "get_node",
            json!({"id": "upstream::concept/knowledge-graph"}),
        );
        assert_eq!(out["id"], "upstream::concept/knowledge-graph");
        assert_eq!(out["origin"], "upstream");
        assert_eq!(out["label"], "Knowledge graph (upstream)");
    }

    /// The bare id must keep answering with THIS repository's node, not the dependency's.
    ///
    /// Both corpora hold `concept/knowledge-graph`. If a bare lookup could fall through to a
    /// dependency, installing one would silently change what this repository says about
    /// itself — which is the failure the whole boundary exists to prevent.
    #[test]
    fn a_bare_id_never_resolves_to_a_dependency() {
        let state = super::super::tests::test_state();
        let out = call_ok(&state, "get_node", json!({"id": "concept/knowledge-graph"}));
        assert_eq!(out["id"], "concept/knowledge-graph");
        assert!(
            out["origin"].is_null(),
            "a bare id must resolve locally: {out:#?}"
        );
        assert_eq!(out["label"], "Knowledge graph");
    }

    /// A bare id that only a dependency holds must not resolve at all.
    ///
    /// This is the assertion that actually pins the boundary. The colliding-id test above
    /// passes even with the rule removed, because the local node happens to be searched
    /// first — ordering rescues it, and a test rescued by ordering pins nothing. A node that
    /// exists *only* upstream has no local candidate to win, so it is reachable exactly when
    /// the boundary is gone.
    #[test]
    fn a_bare_id_holding_only_in_a_dependency_is_not_found() {
        let state = super::super::tests::test_state();
        let out = call(&state, "get_node", &json!({"id": "concept/only-upstream"}));
        assert_eq!(
            out["isError"], true,
            "a bare id must never reach a dependency, even when nothing local shadows it: \
             {out:#?}"
        );

        // And it IS reachable by its qualified id — otherwise this test would pass by the
        // node simply being absent from the fixture.
        let ok = call_ok(
            &state,
            "get_node",
            json!({"id": "upstream::concept/only-upstream"}),
        );
        assert_eq!(ok["label"], "Only upstream");
    }

    /// Traversal does not cross into a dependency, and says so.
    ///
    /// An edge is a claim; this repository has asserted none into a corpus it installed. The
    /// node demonstrably exists, so reporting it as "not found" would be a lie about the
    /// reason.
    #[test]
    fn neighbors_refuses_to_cross_a_corpus_boundary() {
        let state = super::super::tests::test_state();
        let out = call(
            &state,
            "neighbors",
            &json!({"id": "upstream::concept/knowledge-graph"}),
        );
        let text = out["content"][0]["text"].as_str().unwrap_or_default();
        assert_eq!(out["isError"], true, "crossing must be refused: {out:#?}");
        assert!(
            text.contains("does not cross"),
            "the refusal must give the reason, not just fail: {text}"
        );
    }

    #[test]
    fn unknown_tool_is_a_tool_error() {
        let state = test_state();
        let result = call(&state, "bogus", &json!({}));
        assert_eq!(result["isError"], true);
    }
}
