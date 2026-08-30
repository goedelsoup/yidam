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
///
/// **Through `yidam/cli/mcp-contract.json`, which is a symlink to it**, and not up the tree
/// directly. `cargo package` copies only what lives under the crate root, so a path escaping
/// it compiles here and in CI — where the working tree is right there — and fails inside the
/// packaged tarball, which is the one build nothing runs until `cargo publish` runs it. That
/// is after the tag is pushed and the binaries are built. Cargo dereferences the symlink when
/// packaging, so the published crate carries the bytes and the repository keeps one copy.
const CONTRACT: &str = include_str!("../../../mcp-contract.json");

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
        // Whether this server can say if a citation into a dependency holds. True iff it
        // resolved at least one — the same shape as `ontology` above, and a fact about THIS
        // CORPUS rather than about the build: `deps` is not behind the `tonpa` feature,
        // because knowing what a repository depends on needs none of the network that feature
        // buys.
        //
        // A server that resolved none could serve `check_citation` and answer
        // `external-citation-unresolved` to everything, and be literally correct every time.
        // That is the shape #358 caught one tier over: a diagnosis that asserts a fact about a
        // corpus the server does not have. A projected mirror carries no `.yidam/tonpa/`, and
        // telling its caller that `upstream` is not installed would be a statement about the
        // mirror dressed as one about the repository the mirror came from.
        "dependencies": !state.dependencies.is_empty(),
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

/// The tier a tool sits at, or `None` for a name the contract does not carry.
fn tier(name: &str) -> Option<String> {
    contract()["tools"]
        .as_array()?
        .iter()
        .find(|t| t["name"] == name)
        .map(|t| t["tier"].as_str().unwrap_or("core").to_string())
}

/// A tool the contract carries and this server does not back, refused by name (#358).
///
/// The capability block exists so a client learns the holes at connect time "rather than
/// letting a client discover them through tool-not-found errors" — and a server that leaves
/// an unbacked tool dispatchable has re-opened the hole from the other side. Before this,
/// `query` on an unschematised corpus was absent from `tools/list` and answered anyway, with
/// `class-unpopulated` on a corpus that declares nothing: the one diagnosis the contract
/// names as a MUST NOT, delivered by a tool the same server had just said it does not serve.
///
/// It was unreachable to test until `corpus-unschematised/` shipped, because on the corpus
/// every case ran against this server backs every tier there is a tool for.
///
/// `capability-not-supported` and not `unknown tool`: the two are different repairs. One says
/// the caller mistyped a name, the other says this server declines a name that exists — and a
/// client told the first when the second is true will go looking for a spelling error in a
/// tool the contract froze.
fn refuse_unbacked(state: &ServerState, name: &str) -> Option<String> {
    let tier = tier(name)?;
    match backs(&tier, &capabilities(state)) {
        true => None,
        false => Some(format!(
            "capability-not-supported: `{name}` is served only by a server declaring the \
             `{tier}` capability, and this one declares it false. It is absent from \
             `tools/list` for the same reason."
        )),
    }
}

/// Dispatch a tools/call. Tool-level failures come back as MCP tool errors
/// (`isError: true`), not protocol errors — the agent can read and react.
pub(crate) fn call(state: &ServerState, name: &str, args: &Value) -> Value {
    if let Some(refusal) = refuse_unbacked(state, name) {
        return json!({"content": [{"type": "text", "text": refusal}], "isError": true});
    }
    let outcome = match name {
        "retrieve" => retrieve(state, args),
        "get_node" => get_node(state, args),
        "neighbors" => neighbors(state, args),
        "list_nodes" => Ok(list_nodes(state, args)),
        "open_questions" => Ok(open_questions(state)),
        "claims" => Ok(claims(state, args)),
        "check_subject" => check_subject(args),
        "check_citation" => check_citation(state, args),
        "claim_tags" => Ok(claim_tags()),
        "licensed_edges" => licensed_edges(state, args),
        "query" => query(state, args),
        "pack" => pack(state, args),
        "estimate" => estimate(state, args),
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

    // Validated BEFORE anything is searched. A filter naming no declared class cannot produce
    // a true negative, so running the search and reporting the emptiness would be reporting a
    // typo as a fact about the corpus — the failure `query`'s `unknown-class` exists to
    // prevent, which this tool committed for as long as it took a `class` and never read it.
    if let Some(rejection) = super::absence::reject_unknown_class(state, class_filter) {
        return Ok(body(
            state.retrieval.degraded_reason(),
            Vec::new(),
            Some(rejection.to_json()),
            None,
        ));
    }

    #[cfg(feature = "index")]
    if let Retrieval::Vector(index) = &state.retrieval {
        let hits = crate::retrieval::vector::search(index, query, k, |r| {
            class_filter.is_none_or(|c| r.class == c)
        })?;
        let results: Vec<Value> = hits
            .iter()
            .map(|(r, score)| {
                json!({
                    "path": r.path,
                    "class": r.class,
                    "label": r.label,
                    "text": r.text,
                    "score": score,
                })
            })
            .collect();
        let absent = results
            .is_empty()
            .then(|| super::absence::diagnose(state, query, class_filter, true).to_json());
        return Ok(body(None, results, None, absent));
    }
    Ok(keyword_retrieve(
        state,
        query,
        k,
        class_filter,
        state.retrieval.degraded_reason(),
    ))
}

/// The `retrieve` response, around whichever path produced the results.
///
/// One function for both arms. The two halves of the `degraded` convention used to live in
/// different files, and the present-and-null half lived in the module the degradable build
/// does not compile — so the shape a light build owed was written where a light build could
/// not read it.
fn body(
    reason: Option<&'static str>,
    results: Vec<Value>,
    rejected: Option<Value>,
    absence: Option<Value>,
) -> Value {
    json!({
        "degraded": reason.is_some(),
        // *Why* degraded, not just that it is. The bare boolean made two different
        // repositories look identical: one that never built an index, and one whose index
        // this binary cannot read. Both are keyword search; only one is fixed by indexing.
        //
        // Present and null when not degraded, the same convention `origin` follows: a client
        // testing the key must not have to distinguish "not degraded" from "a server too old
        // to say why".
        "degraded_reason": reason,
        // A REJECTION IS NOT AN ABSENCE. `rejected` says the caller's filter is wrong;
        // `absence` says the filter is right and the search came back quiet. At most one is
        // non-null, and both are present always — a client testing a key must not have to
        // distinguish "nothing to report" from "a server too old to report it", which is the
        // convention `degraded_reason` and `origin` already follow here.
        "rejected": rejected,
        // Null exactly when `results` is non-empty. An answer that returned rows is not
        // absent and has nothing to explain.
        "absence": absence,
        "results": results,
    })
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
    let terms = crate::retrieval::terms(query);

    // Local nodes first, then every installed dependency's. Retrieval is the one surface a
    // dependency is allowed on: an agent asking "what is known about X" should be told when
    // the answer lives in a corpus this repository merely cites, not have it withheld — and
    // each result says whose it is, so it can never be mistaken for this repository's claim.
    //
    // A query's similarity anchor does *not* get this reach — see `query::anchor`. The
    // difference is the whole reason the scorer is shared and the candidate set is not.
    let mut scored: Vec<(&super::Node, f32)> = state
        .nodes
        .iter()
        .chain(state.dep_nodes.iter())
        .filter(|n| class_filter.is_none_or(|c| n.class == c))
        .filter_map(|n| {
            let haystack = format!("{} {} {}", n.label, n.description, n.content).to_lowercase();
            crate::retrieval::keyword_score(&terms, &haystack).map(|score| (n, score))
        })
        .collect();
    // Ties break on the qualified id, not the bare one: two corpora may hold the same
    // `class/name`, and ordering that cannot tell them apart is not deterministic.
    scored.sort_by(|a, b| {
        b.1.total_cmp(&a.1)
            .then_with(|| a.0.qualified_id().cmp(&b.0.qualified_id()))
    });
    scored.truncate(k);

    let results: Vec<Value> = scored
        .iter()
        .map(|(n, score)| {
            json!({
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
            })
        })
        .collect();

    let absent = results
        .is_empty()
        .then(|| super::absence::diagnose(state, query, class_filter, false).to_json());
    body(reason, results, None, absent)
}

/// A typed path over the graph — #263's half of hybrid anchoring that an agent can reach.
///
/// The whole tool is a call into `cmd::query`, deliberately. Before this, an agent's only
/// traversal was `neighbors`, which chains outbound and inbound edges unconditionally and
/// filters on neither relationship nor direction — so the surface that argues a scan is the
/// wrong shape offered an agent a scan and a flood. Re-implementing the walk here would have
/// been a second answer to what an edge is, and `retrieve`'s own history is the argument
/// against that.
///
/// **`isError` is not set for a rejected query.** A rejection is an answer: it names the step,
/// the code and the near miss, which is exactly what #261 requires an unknown name to produce
/// instead of an empty result. `check_subject` returns its verdict in the payload for the same
/// reason. `isError` stays for a tool that could not run at all.
fn query(state: &ServerState, args: &Value) -> Result<Value, String> {
    let text = args["query"]
        .as_str()
        .ok_or("missing required argument: query")?;
    let opts = crate::cmd::query::Options {
        select: match args["select"].as_str() {
            Some(select) => select
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect(),
            None => crate::cmd::query::Options::default().select,
        },
        limit: args["limit"]
            .as_u64()
            .unwrap_or(crate::cmd::query::DEFAULT_LIMIT as u64)
            .max(1) as usize,
        anchor_k: args["anchor_k"]
            .as_u64()
            .unwrap_or(crate::cmd::query::DEFAULT_ANCHOR_K as u64)
            .max(1) as usize,
    };
    // ── the spanning boundary (#333) ─────────────────────────────────────────
    //
    // `across` defaults to false and a caller that did not ask cannot see a foreign node.
    // The two graphs are separate objects rather than one graph and a flag, so a local call
    // is *incapable* of spanning rather than merely instructed not to — see
    // `ServerState::graph_across`.
    //
    // Asking to span a repository with no dependencies is not an error and is not silently
    // downgraded either: the local graph answers, and `scope` reads `local`, which is the
    // same thing `yidam query --across` reports in that situation. A caller comparing
    // `scope` against what it asked for can tell the difference; one told only that nothing
    // matched could not.
    let graph = match args["across"].as_bool().unwrap_or(false) {
        true => state.graph_across.as_ref().unwrap_or(&state.graph),
        false => &state.graph,
    };
    // The `Retrieval` this server loaded at startup — the same one `retrieve` answers from,
    // so the two can never report different reasons for the same degradation. That sharing is
    // the one refactor RFC-0018 asked for, and this is the call site it was asked for.
    let ctx = crate::cmd::query::Context::now(graph, Some(&state.retrieval));
    let report = crate::cmd::query::run_on(&ctx, text, &opts);
    serde_json::to_value(&report).map_err(|e| e.to_string())
}

/// What a query would cost, before paying for it (#284).
///
/// Returns no rows, which is the whole asymmetry: the traversal costs this server what an
/// answer costs, and costs the caller a few hundred bytes. The walk happens **once** — the
/// pack figure is derived from the same report rather than from a second run, because a quote
/// that resolved the similarity anchor twice would charge double for the thing it exists to
/// call affordable.
fn estimate(state: &ServerState, args: &Value) -> Result<Value, String> {
    let text = args["query"]
        .as_str()
        .ok_or("missing required argument: query")?;
    let opts = crate::cmd::estimate::Options {
        select: match args["select"].as_str() {
            Some(select) => select
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect(),
            None => crate::cmd::estimate::Options::default().select,
        },
        limit: args["limit"]
            .as_u64()
            .unwrap_or(crate::cmd::query::DEFAULT_LIMIT as u64)
            .max(1) as usize,
        budget: args["budget"].as_u64().map(|b| b as usize),
        anchor_k: args["anchor_k"]
            .as_u64()
            .unwrap_or(crate::cmd::query::DEFAULT_ANCHOR_K as u64)
            .max(1) as usize,
    };
    let ctx = crate::cmd::query::Context::now(&state.graph, Some(&state.retrieval));
    serde_json::to_value(crate::cmd::estimate::run_on(&ctx, text, &opts)).map_err(|e| e.to_string())
}

/// A context pack for one goal (#282).
///
/// Answers from the same `Graph` and the same `Retrieval` as `query` and `retrieve`, so the
/// three can never come to disagree about what this corpus holds or about why retrieval is
/// degraded. That sharing is the whole reason the state is loaded once at startup.
fn pack(state: &ServerState, args: &Value) -> Result<Value, String> {
    let text = args["query"]
        .as_str()
        .ok_or("missing required argument: query")?;
    let opts = crate::cmd::pack::Options {
        // Absent means unbounded, and there is no default. A server that picked one would
        // truncate the first pack a caller ever asked for, and report it honestly — which is
        // still a caller getting less than it asked for without having asked for less.
        budget: args["budget"].as_u64().map(|b| b as usize),
        anchor_k: args["anchor_k"]
            .as_u64()
            .unwrap_or(crate::cmd::query::DEFAULT_ANCHOR_K as u64)
            .max(1) as usize,
    };
    let ctx = crate::cmd::query::Context::now(&state.graph, Some(&state.retrieval));
    serde_json::to_value(crate::cmd::pack::run_on(&ctx, text, &opts)).map_err(|e| e.to_string())
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

/// Whether a `cites:` into a dependency would hold, before it is written (#357).
///
/// # The gap this closes
///
/// RFC-0019 made `cites:` a field of a node and #266 made a citation into a dependency
/// verifiable — by four checks that all run at `lint` time, which is to say *after* the
/// citation has been written into a file and committed to. The surfaces that make a foreign
/// node reachable are exactly the ones that make a bad citation easy to write: `retrieve`
/// answers from a dependency, `get_node` reads one out of it, and #333 gave `query` the
/// dependency set — and none of them says whether leaning on what they returned would stand.
/// A `span:` in particular is a claim about text that no read-tool checks.
///
/// # It calls the checks' own predicate
///
/// [`citations::findings`] is the function the four checks are filters over, so this cannot
/// come to disagree with the gate about what a citation is. That is the same argument `query`
/// makes for being a call into `cmd::query` rather than a second traversal, and the argument
/// `retrieve`'s history makes against a second copy of anything.
///
/// # Never an error
///
/// A citation that will not hold is a verdict in the payload, exactly as `check_subject`
/// returns an unrecognized verb. `isError` stays for a tool that could not run — and this one
/// can always run, because a server that could not answer declines the tool by name at the
/// `dependencies` capability instead of guessing here.
///
/// `package` is required and `node` is not, which is a distinction rather than an oversight.
/// Without a package there is no question — nothing names a corpus to check against. Without
/// a node there is still a question, and its answer is a finding the gate would also give,
/// so it comes back as one.
fn check_citation(state: &ServerState, args: &Value) -> Result<Value, String> {
    use crate::cmd::lint::citations;

    let field = |name: &str| args[name].as_str().map(str::to_string);
    let package = field("package").ok_or("package is required")?;
    let cite = crate::parse::ExternalCitation {
        package: Some(package),
        node: field("node"),
        commit: field("commit"),
        tag: field("tag"),
        span: field("span"),
    };

    let findings = citations::findings(&cite, &state.dependencies);
    // What the four severities mean for a caller about to act: any finding at all means the
    // citation is not clean, and an Error-severity one means the gate goes red. A pin that has
    // moved and an unpinnable path dependency are both worth saying and neither blocks, so a
    // caller that read only `holds` would over-report and one that read only `gates` would
    // write a citation it was told about.
    let gates = findings
        .iter()
        .any(|f| f.severity == crate::cmd::lint::model::Severity::Error);
    let rendered: Vec<Value> = findings
        .iter()
        .map(|f| {
            json!({
                "check": f.check,
                "severity": f.severity.as_str(),
                "message": f.message,
            })
        })
        .collect();

    Ok(json!({
        "citation": {
            "package": cite.package,
            "node": cite.node,
            "commit": cite.commit,
            "tag": cite.tag,
            "span": cite.span,
        },
        "holds": findings.is_empty(),
        "gates": gates,
        "findings": rendered,
        // The installed set travels with the verdict, for `check_subject`'s reason: a caller
        // that got it wrong can correct without a second call. `pin` is the part that cannot
        // be learned any other way over this surface — it is the value a correct `commit:`
        // must carry, and an agent that had to guess it would write
        // `external-citation-pin-moved` into the corpus on its first try.
        "dependencies": state
            .dependencies
            .iter()
            .map(|(name, dep)| json!({
                "package": name,
                "pin": dep.pin,
                "kind": match dep.kind {
                    crate::deps::DependencyKind::Fetched => "fetched",
                    crate::deps::DependencyKind::Path => "path",
                },
            }))
            .collect::<Vec<Value>>(),
    }))
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

    /// A class filter naming no declared class is refused before anything is searched.
    ///
    /// The bug this closes is not that the answer was empty — it is that the answer was empty
    /// and said nothing was wrong, which reads to a caller as a true negative. Searching first
    /// and diagnosing after would have produced `class-unpopulated`, i.e. a sentence about the
    /// corpus in answer to a typo.
    /// Asking to span a repository with no dependencies is not an error.
    ///
    /// It answers locally and says `local`, which is what `yidam query --across` already does
    /// and is the only reading that stays honest: the request was legal, nothing foreign
    /// exists, and `scope` is where a caller learns the difference. Refusing would make an
    /// agent's willingness to compose depend on whether anyone had run `tonpa install`;
    /// answering `across` would claim a dependency set that is not there.
    #[test]
    fn spanning_a_repository_with_no_dependencies_answers_locally_and_says_so() {
        let state = test_state();
        let result = call_ok(&state, "query", json!({"query": "concept", "across": true}));

        assert_eq!(
            result["scope"], "local",
            "scope reports what happened, not what was asked"
        );
        assert!(result["rejected"].is_null());
    }

    #[test]
    fn an_unknown_class_filter_is_rejected_before_the_search() {
        let state = test_state();
        let result = call_ok(
            &state,
            "retrieve",
            json!({"query": "graph", "class": "concpt"}),
        );

        assert_eq!(result["rejected"]["code"], "unknown-class");
        assert!(
            result["rejected"]["message"]
                .as_str()
                .unwrap()
                .contains("did you mean `concept`"),
            "the near miss is the whole value of the rejection: {}",
            result["rejected"]["message"]
        );
        assert!(result["results"].as_array().unwrap().is_empty());
        assert!(
            result["absence"].is_null(),
            "a rejection is not an absence; carrying both answers two questions with one key"
        );
    }

    /// A declared class nobody has written into is a statement about the corpus.
    #[test]
    fn a_declared_class_with_no_instances_says_so() {
        let state = test_state();
        let result = call_ok(
            &state,
            "retrieve",
            json!({"query": "graph", "class": "silent"}),
        );

        assert!(result["rejected"].is_null());
        assert_eq!(result["absence"]["code"], "class-unpopulated");
        assert_eq!(result["absence"]["instances"], 0);
    }

    /// Without an ontology, neither class answer is derivable — and saying so is the answer.
    ///
    /// `retrieve` is `core`, so unlike `query`'s family it cannot hide behind the `ontology`
    /// tier. Reporting `class-unpopulated` here would assert a declaration this corpus has not
    /// made; rejecting would assert the name is wrong on the same missing evidence. The honest
    /// third answer is that this tool cannot tell, which is still worth more to a caller than
    /// the silence it used to get.
    #[test]
    fn an_unschematised_corpus_says_it_cannot_tell_a_typo_from_an_empty_class() {
        let mut state = test_state();
        state.classes.clear();
        let result = call_ok(
            &state,
            "retrieve",
            json!({"query": "graph", "class": "gauge"}),
        );

        assert!(
            result["rejected"].is_null(),
            "nothing declares a class here, so nothing can call this name wrong"
        );
        assert_eq!(result["absence"]["code"], "class-undeclared");
    }

    /// Keyword search that read everything and matched nothing says which of the two it is.
    ///
    /// `instances` is the load-bearing half. *None of four* is a statement about the query's
    /// words; *none of zero* would be a statement about the corpus, and the codes must not be
    /// readable as each other.
    #[test]
    fn words_the_corpus_does_not_use_are_not_reported_as_missing_coverage() {
        let state = test_state();
        let result = call_ok(&state, "retrieve", json!({"query": "hydropeaking ramping"}));

        assert!(result["rejected"].is_null());
        assert_eq!(result["absence"]["code"], "no-term-match");
        assert_eq!(
            result["absence"]["instances"], 4,
            "two local nodes and two from the dependency were all read"
        );
    }

    #[test]
    fn a_query_with_no_searchable_terms_says_so() {
        let state = test_state();
        let result = call_ok(&state, "retrieve", json!({"query": "   "}));

        assert_eq!(result["absence"]["code"], "query-no-terms");
    }

    /// Null exactly when the answer is non-empty — the half a server gets wrong by being
    /// helpful. A code meaning `fine` makes the one field a caller consults about silence
    /// ambiguous, which is the failure it was added to end.
    #[test]
    fn an_answer_that_found_something_carries_neither_key() {
        let state = test_state();
        let result = call_ok(&state, "retrieve", json!({"query": "knowledge graph"}));

        assert!(!result["results"].as_array().unwrap().is_empty());
        assert!(result["rejected"].is_null());
        assert!(result["absence"].is_null());
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

    // ── check_citation (#357) ────────────────────────────────────────────────

    /// The verdict is read from the corpus this server walked at startup.
    ///
    /// The fixture's `corpus_dir` does not exist, so nothing here can be answered from disk.
    /// That is the assertion: a span judged against the working tree would report this node
    /// missing, and a server whose `check_citation` disagreed with its own `get_node` about
    /// what a node says is worse than one that is merely out of date.
    #[test]
    fn a_citation_is_checked_against_the_corpus_this_server_serves() {
        let state = test_state();
        let verdict = call_ok(
            &state,
            "check_citation",
            json!({
                "package": "upstream",
                "node": "concept/only-upstream",
                "commit": "abc1234",
                "span": "Exists in the dependency and nowhere else.",
            }),
        );
        assert_eq!(verdict["holds"], true, "{verdict}");
        assert_eq!(verdict["gates"], false);
        assert_eq!(verdict["findings"].as_array().unwrap().len(), 0);
    }

    /// `holds` and `gates` are two questions, and a moved pin is what separates them.
    ///
    /// A caller reading only `gates` writes a citation it was warned about; one reading only
    /// `holds` treats a warning as a refusal. The span still matches here, so nothing but the
    /// pin is under test.
    #[test]
    fn a_moved_pin_does_not_hold_and_does_not_gate() {
        let state = test_state();
        let verdict = call_ok(
            &state,
            "check_citation",
            json!({
                "package": "upstream",
                "node": "concept/only-upstream",
                "commit": "0000000",
                "span": "Exists in the dependency and nowhere else.",
            }),
        );
        assert_eq!(verdict["holds"], false);
        assert_eq!(verdict["gates"], false, "{verdict}");
        assert_eq!(
            verdict["findings"][0]["check"],
            "external-citation-pin-moved"
        );
        assert_eq!(verdict["findings"][0]["severity"], "warn");
    }

    /// The pin travels back, because it is reachable no other way on this surface.
    ///
    /// It is the value a correct `commit:` must carry. An agent that had to guess it writes
    /// `external-citation-pin-moved` into the corpus on its first try — which is the failure
    /// this tool exists to prevent, reintroduced by the tool itself.
    #[test]
    fn the_installed_set_travels_with_the_verdict() {
        let state = test_state();
        let verdict = call_ok(&state, "check_citation", json!({"package": "nowhere"}));
        assert_eq!(verdict["dependencies"][0]["package"], "upstream");
        assert_eq!(verdict["dependencies"][0]["pin"], "abc1234");
        assert_eq!(verdict["dependencies"][0]["kind"], "fetched");
    }

    /// A citation that will not hold is a verdict, and only an unaskable question is an error.
    ///
    /// `check_subject`'s rule, for `check_subject`'s reason: a tool that failed harder than
    /// the gate would assert a verdict nobody agreed to. Omitting `node` still leaves a
    /// question — and its answer is the frozen check id the gate would give — so it comes back
    /// as a finding. Omitting `package` leaves nothing to check against, and that is the error.
    #[test]
    fn only_a_call_naming_no_package_is_an_error() {
        let state = test_state();
        let answered = call_ok(&state, "check_citation", json!({"package": "upstream"}));
        assert_eq!(answered["holds"], false);
        assert_eq!(
            answered["findings"][0]["check"],
            "external-citation-unresolved"
        );

        let refused = call(&state, "check_citation", &json!({"node": "concept/x"}));
        assert_eq!(refused["isError"], true);
    }

    /// A server with nothing installed refuses the tool rather than answering it.
    ///
    /// It could answer `external-citation-unresolved` to every citation put to it and be
    /// correct every time — while asserting a fact about a dependency set it does not have.
    /// That is the shape #358 caught one tier over, where a server with no `.ont.yml` reported
    /// `class-unpopulated`; the repair there was to decline the tool, and it is the repair
    /// here.
    #[test]
    fn a_server_with_no_dependencies_declines_rather_than_reporting_them_all_unresolved() {
        let mut state = test_state();
        state.dependencies.clear();
        assert_eq!(capabilities(&state)["dependencies"], false);

        let listed = list(&state);
        assert!(
            !listed["tools"]
                .as_array()
                .unwrap()
                .iter()
                .any(|t| t["name"] == "check_citation"),
            "an unbacked tool must not be listed: {listed}"
        );

        let result = call(&state, "check_citation", &json!({"package": "upstream"}));
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert_eq!(result["isError"], true);
        assert!(text.starts_with("capability-not-supported"), "{text}");
    }

    /// The check ids the contract freezes are the ones the gate actually reports.
    ///
    /// Both halves of this tool's promise are strings — the id in `citations` and the id in
    /// `tools.json`'s notes — and nothing else compares them. Renaming a check in the gate
    /// would leave a conforming server answering an id no document names, and a client
    /// branching on the frozen one would silently stop matching.
    #[test]
    fn the_contract_names_the_check_ids_the_gate_reports() {
        use crate::cmd::lint::citations;

        let contract = contract();
        let notes = contract["tools"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["name"] == "check_citation")
            .and_then(|t| t["response"]["notes"].as_str())
            .expect("check_citation documents its response");

        for id in [
            citations::UNRESOLVED,
            citations::SPAN_DRIFT,
            citations::PIN_MOVED,
            citations::UNPINNED,
        ] {
            assert!(
                notes.contains(id),
                "the contract does not name `{id}` — a client branching on the id this \
                 server answers with has nothing frozen to branch on"
            );
        }
    }
}
