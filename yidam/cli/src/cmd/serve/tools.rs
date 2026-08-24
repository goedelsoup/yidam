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
        // The class contract, from `.ont.yml`. True because this server reads the corpus
        // on disk, ontology included. A projected mirror holding nodes and edges and no
        // class definitions declares false — optional is not the same as absent.
        "ontology": !state.classes.is_empty(),
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
        "claims" => Ok(claims(state, args)),
        "check_subject" => check_subject(args),
        "claim_tags" => Ok(claim_tags()),
        "licensed_edges" => licensed_edges(state, args),
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

// ── assertions, not documents (contract 0.5.0) ────────────────────────────────

/// The claims a corpus makes, with the standing each is made at.
///
/// Every other tool here returns a node. The unit of assertion is not the node — a node is
/// 2–10 sentences by the model's own rule, so an agent asking what is known about something
/// pays node-sized tokens for a claim-sized answer and learns the standing only if the tag
/// survived into the prose it was handed.
///
/// Extraction is [`crate::claims::claims_in_node`], the list form of the counter every
/// report already routes through — and deliberately not the SDK's `extract_claims`, which
/// is a line-oriented parser for the markdown node model and reads `class: gage` as a claim
/// over a YAML instance. The two are held equal per tag by a unit test.
///
/// **Local nodes only.** A dependency's assertions are its corpus's; composition is
/// retrieval-only and `retrieve` is where a foreign node is reachable, marked with `origin`.
fn claims(state: &ServerState, args: &Value) -> Value {
    let want_standing = args["standing"].as_str();
    let want_class = args["class"].as_str();
    let want_node = args["node"].as_str();
    let k = args["k"].as_u64().unwrap_or(50) as usize;

    let mut all = Vec::new();
    for node in state.nodes.iter().filter(|n| n.is_local()) {
        if want_class.is_some_and(|c| c != node.class) {
            continue;
        }
        if want_node.is_some_and(|id| find_node(state, id).map(|n| n.id.as_str()) != Some(&node.id))
        {
            continue;
        }
        let fields = state.claim_fields.for_class(&node.class);
        for claim in crate::claims::claims_in_node(&node.content, fields) {
            if want_standing.is_some_and(|s| s != claim.standing) {
                continue;
            }
            all.push(json!({
                // A node-scoped standing has no sentence of its own — the subject is the
                // node, so the node is what a reader needs to see. `property` says where
                // the standing came from either way.
                "text": match claim.scope {
                    crate::claims::ClaimScope::Node => node.label.clone(),
                    crate::claims::ClaimScope::Statement => claim.text.clone(),
                },
                "standing": claim.standing,
                "scope": claim.scope,
                "property": claim.property,
                "node": node.id,
                "class": node.class,
                "sources": state.citations.get(&node.id).cloned().unwrap_or_default(),
            }));
        }
    }

    // `total` before `k`, always. "Here are 5 claims" and "here are 5 of 41" license
    // different next actions, and only the second lets an agent decide to spend more.
    let total = all.len();
    all.truncate(k);
    json!({ "claims": all, "returned": all.len(), "total": total })
}

/// What a class declares it may link to.
///
/// Asked before writing a link rather than discovered from a failing gate — which is the
/// whole difference between a practice an agent complies with by remembering and one it
/// complies with by asking.
///
/// **The class is its filename.** `load_classes` keys by the `<class>.ont.yml` stem and
/// `unknown-class` compares an instance's `class:` against the set of stems, so the stem is
/// what governs; answering from the file's own `class:` field would license edges for a
/// class no instance belongs to.
fn licensed_edges(state: &ServerState, args: &Value) -> Result<Value, String> {
    let want = args["class"]
        .as_str()
        .ok_or("class is required")?
        .trim()
        .trim_end_matches(".ont.yml");

    let Some((name, class)) = state.classes.iter().find(|(n, _)| n == want) else {
        return Err(format!(
            "no class `{want}` — this corpus declares: {}",
            state
                .classes
                .iter()
                .map(|(n, _)| n.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };

    // Every declared edge, both directions. An edge is documented from both ends and the
    // licensing check ignores direction, so filtering to `out` would answer a question the
    // gate does not ask.
    let edges: Vec<Value> = class
        .edges
        .iter()
        .map(|e| {
            json!({
                "relationship": e.relationship,
                // An empty target licenses ANY target class, rather than none.
                "target": if e.target.is_empty() { Value::Null } else { json!(e.target) },
                "direction": e.direction,
                "description": e.description,
            })
        })
        .collect();

    // Silence is not a contract, and the two answers are opposite. A client that read an
    // empty list as "may link to nothing" would report every instance in a corpus whose
    // ontology is not filled in.
    let note = if edges.is_empty() {
        format!(
            "`{name}` declares no edges, so it has said nothing about what it may link to — \
             the gate does not check its links. This is not the same as licensing nothing."
        )
    } else {
        format!(
            "`{name}` licenses these {} relationship(s). Licensing applies to edges between \
             instances: the `instance-of` link to `../{name}.ont.yml` and a citation into \
             the catalog are not relationships and no class declares them.",
            edges.len()
        )
    };

    Ok(json!({
        "class": name,
        "declares_edges": !edges.is_empty(),
        "edges": edges,
        "note": note,
    }))
}

/// Whether a commit subject is in the closed vocabulary, before the commit is written.
///
/// Calls the function `yidam vocabulary --check` calls, and serializes what it returns.
/// Rebuilding the verb parse here would be its fourth copy and would lose the scope-suffix
/// rule, which is the one that catches the common mistake.
///
/// **Never an error.** An unrecognized verb is a finding in the payload: the gate reports it
/// at warn severity because history cannot be rewritten to fix a verb, and a tool that
/// failed harder than the gate would assert a verdict nobody agreed to.
fn check_subject(args: &Value) -> Result<Value, String> {
    let subject = args["subject"].as_str().ok_or("subject is required")?;
    let checked = crate::cmd::vocabulary::check_subject(subject);
    let mut out = serde_json::to_value(&checked).map_err(|e| e.to_string())?;
    // The closed list travels with the verdict, so a caller that got it wrong can correct
    // without a second call — which is the point of asking before the act.
    out["vocabulary"] = json!(crate::cmd::vocabulary::vocabulary_verbs());
    Ok(out)
}

/// The three evidence tags, what each means, and how each may be written.
///
/// Tens of tokens instead of a prose file held in context every session. The prose stays
/// where the reasoning lives; this is the content.
fn claim_tags() -> Value {
    let tags: Vec<Value> = crate::claims::TAG_MEANINGS
        .iter()
        .map(|(bare, bracketed, meaning)| {
            json!({
                "standing": bare,
                // Both spellings, because both are accepted — and they are accepted in
                // different places, which is the part worth saying.
                "in_prose": bracketed,
                "in_property": [bare, bracketed],
                "meaning": meaning,
            })
        })
        .collect();
    json!({
        "tags": tags,
        "note": "In prose, only the bracketed token is scanned — a bare `open` in a sentence \
                 is a word. In a property the class declared `type: claim`, both spellings \
                 are read. Write the tag alone and put any citation beside it: \
                 `[verified — Pearl 2009]` matches nothing and is counted as no claim at \
                 all, so it looks tagged to a reader and reads as bare assertion to every \
                 tool.",
    })
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
