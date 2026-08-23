//! `yidam serve --mcp` — expose the domain computer as an MCP server.
//!
//! The protocol layer is hand-rolled stdio JSON-RPC rather than an MCP SDK
//! dependency: the surface we need (initialize, resources, tools) is small,
//! and the available crates require a newer toolchain than the repo pins.
//!
//! All reads come from the already-built corpus and index on disk via
//! [`load_domain_model`] — no live git operations. If HEAD has advanced past
//! the indexed commit the startup banner warns but the server keeps serving.

mod resources;
mod tools;

use anyhow::Result;
use fastembed::TextEmbedding;
use serde_json::{json, Value};
use std::cell::RefCell;
use std::io::{BufRead, Write};
use std::path::Path;

use crate::embed_config::EmbedConfig;
use crate::git::head_commit_short;
use crate::model::{corpus_nodes, file_stem as stem, index_rows, load_domain_model};
use crate::paths::repo_root;

/// One corpus instance, parsed for serving. The id (`<class>/<name>`) is
/// what `get_node` and `neighbors` accept.
pub(crate) use crate::model::NodeView as Node;
/// One row of the vector index (from `index/corpus.arrow`).
pub(crate) use crate::model::VectorRow;

pub(crate) struct IndexState {
    pub rows: Vec<VectorRow>,
    pub model_id: String,
    /// Lazily initialised on the first `retrieve` call — loading model
    /// weights takes seconds and many sessions never call `retrieve`.
    pub embedder: RefCell<Option<TextEmbedding>>,
}

pub(crate) struct ServerState {
    pub domain: String,
    pub commit: String,
    pub nodes: Vec<Node>,
    /// Nodes from installed dependencies (`.yidam/tonpa/<pkg>/`).
    ///
    /// A separate field rather than more entries in `nodes`, because the distinction is
    /// load-bearing: these are searchable and readable, and are never edge targets. Folding
    /// them in would make every existing traversal cross a boundary it was never argued
    /// across, silently.
    pub dep_nodes: Vec<Node>,
    /// (name without extension, content)
    pub skills: Vec<(String, String)>,
    /// (name without extension, content)
    pub decisions: Vec<(String, String)>,
    pub index: Option<IndexState>,
    /// Commit the index was built at, from `index/meta.json`.
    pub indexed_commit: Option<String>,
    /// Which properties of each class carry an evidence tag.
    ///
    /// Loaded here so the MCP server's open-question answer is the reports' answer. It was a
    /// second copy of the predicate, and a corpus with structured tags was under-reported by
    /// both in the same way — which is the kind of agreement that looks like correctness.
    pub claim_fields: crate::claims::ClaimFields,
}

impl ServerState {
    pub(crate) fn load(root: &Path) -> Result<Self> {
        let model = load_domain_model(root)?;

        let nodes = corpus_nodes(&model);
        let dep_nodes = crate::model::all_dependency_nodes(root);

        let skills = model
            .skills
            .iter()
            .map(|s| {
                (
                    stem(&s.filename),
                    String::from_utf8_lossy(&s.content).into_owned(),
                )
            })
            .collect();
        let decisions = model
            .decisions
            .iter()
            .map(|d| {
                (
                    stem(&d.filename),
                    String::from_utf8_lossy(&d.content).into_owned(),
                )
            })
            .collect();

        let (index, indexed_commit) = match &model.index {
            Some(idx) => {
                let rows = index_rows(idx)?;
                // The reproducibility contract is authoritative for the model;
                // fall back to meta.json for indexes built before it existed.
                let model_id = idx
                    .embed_config
                    .as_ref()
                    .map(|c: &EmbedConfig| c.model_id.clone())
                    .or_else(|| idx.meta["model_name"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let indexed_commit = idx.meta["indexed_commit"].as_str().map(str::to_string);
                (
                    Some(IndexState {
                        rows,
                        model_id,
                        embedder: RefCell::new(None),
                    }),
                    indexed_commit,
                )
            }
            None => (None, None),
        };

        let claim_fields = crate::claims::ClaimFields::load(&crate::paths::yidam_corpus_dir(root));
        Ok(ServerState {
            domain: model.provenance.domain,
            commit: model.provenance.commit,
            nodes,
            dep_nodes,
            skills,
            decisions,
            index,
            indexed_commit,
            claim_fields,
        })
    }
}

/// Serve the domain computer over MCP stdio. Blocks until stdin closes.
pub fn serve_mcp() -> Result<()> {
    let root = repo_root()?;
    let state = ServerState::load(&root)?;

    // Banner goes to stderr — stdout carries only JSON-RPC frames.
    eprintln!(
        "yidam MCP server — domain {:?}, {} node(s), {} skill(s), {} decision(s)",
        state.domain,
        state.nodes.len(),
        state.skills.len(),
        state.decisions.len(),
    );
    match &state.index {
        Some(idx) => eprintln!(
            "vector index: {} row(s), model {} — `retrieve` uses semantic search",
            idx.rows.len(),
            idx.model_id
        ),
        None => eprintln!("vector index: absent — `retrieve` degrades to keyword search"),
    }
    if let Some(indexed) = &state.indexed_commit {
        let head = head_commit_short(&root);
        if *indexed != head {
            eprintln!(
                "warning: HEAD ({head}) has advanced past the indexed commit ({indexed}) — \
                 serving the stale index; run `yidam index-build` to refresh"
            );
        }
    }
    eprintln!("serving MCP over stdio");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    run_loop(&state, stdin.lock(), stdout.lock())
}

/// Read newline-delimited JSON-RPC messages from `input`, write responses to
/// `output`. Notifications (no `id`) are consumed without a response.
fn run_loop(state: &ServerState, input: impl BufRead, mut output: impl Write) -> Result<()> {
    for line in input.lines() {
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let msg: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")}
                });
                writeln!(output, "{resp}")?;
                output.flush()?;
                continue;
            }
        };
        let Some(id) = msg.get("id").filter(|v| !v.is_null()).cloned() else {
            continue; // notification — nothing to send back
        };
        let method = msg["method"].as_str().unwrap_or("");
        let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));

        let resp = match handle(state, method, &params) {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err(e) => json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {"code": e.code, "message": e.message}
            }),
        };
        writeln!(output, "{resp}")?;
        output.flush()?;
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub(crate) fn invalid_params(message: impl Into<String>) -> Self {
        RpcError {
            code: -32602,
            message: message.into(),
        }
    }
}

fn handle(state: &ServerState, method: &str, params: &Value) -> Result<Value, RpcError> {
    match method {
        "initialize" => {
            let requested = params["protocolVersion"].as_str().unwrap_or("2024-11-05");
            Ok(json!({
                "protocolVersion": requested,
                "capabilities": {
                    "resources": {},
                    "tools": {},
                    // What this server backs, declared rather than discovered. See
                    // `prelude/sdks/parity/mcp/tools.json`.
                    "yidam": tools::capabilities(state),
                },
                "serverInfo": {"name": "yidam", "version": env!("CARGO_PKG_VERSION")}
            }))
        }
        "ping" => Ok(json!({})),
        "resources/list" => Ok(resources::list(state)),
        "resources/read" => {
            let uri = params["uri"]
                .as_str()
                .ok_or_else(|| RpcError::invalid_params("missing uri"))?;
            resources::read(state, uri)
        }
        "resources/templates/list" => Ok(json!({"resourceTemplates": []})),
        "prompts/list" => Ok(json!({"prompts": []})),
        "tools/list" => Ok(tools::list(state)),
        "tools/call" => {
            let name = params["name"]
                .as_str()
                .ok_or_else(|| RpcError::invalid_params("missing tool name"))?;
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            Ok(tools::call(state, name, &args))
        }
        other => Err(RpcError {
            code: -32601,
            message: format!("method not found: {other}"),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn test_state() -> ServerState {
        ServerState {
            dep_nodes: vec![
                Node {
                    id: "concept/knowledge-graph".into(),
                    class: "concept".into(),
                    label: "Knowledge graph (upstream)".into(),
                    description: "The dependency's account of a knowledge graph.".into(),
                    content: "class: concept\n".into(),
                    links: vec![("concept/traversal".into(), "enables".into())],
                    origin: Some("upstream".into()),
                },
                // An id no local node shares. The colliding node above is rescued by ordering
                // if the boundary is removed; this one is not, so it is the node that actually
                // pins the rule.
                Node {
                    id: "concept/only-upstream".into(),
                    class: "concept".into(),
                    label: "Only upstream".into(),
                    description: "Exists in the dependency and nowhere else.".into(),
                    content: "class: concept\n".into(),
                    links: vec![],
                    origin: Some("upstream".into()),
                },
            ],
            domain: "test".into(),
            commit: "abc1234".into(),
            nodes: vec![
                Node {
                    id: "concept/knowledge-graph".into(),
                    class: "concept".into(),
                    label: "Knowledge graph".into(),
                    description: "A graph of knowledge nodes and typed edges.".into(),
                    content: "class: concept\nlabel: Knowledge graph\n".into(),
                    links: vec![("concept/traversal".into(), "enables".into())],
                    origin: None,
                },
                Node {
                    id: "concept/traversal".into(),
                    class: "concept".into(),
                    label: "? Traversal strategy".into(),
                    description: "How agents walk the graph.".into(),
                    content: "class: concept\nlabel: \"? Traversal strategy\"\n".into(),
                    links: vec![],
                    origin: None,
                },
            ],
            skills: vec![("my-skill".into(), "---\nname: my-skill\n---\n".into())],
            decisions: vec![("adr-1".into(), "id: adr-1\nsummary: choice\n".into())],
            index: None,
            indexed_commit: None,
            // No class declares a claim field here, so the predicate reads prose only —
            // which is what these fixtures are written in.
            claim_fields: Default::default(),
        }
    }

    #[test]
    fn initialize_echoes_protocol_version() {
        let state = test_state();
        let result = handle(
            &state,
            "initialize",
            &json!({"protocolVersion": "2025-03-26"}),
        )
        .unwrap();
        assert_eq!(result["protocolVersion"], "2025-03-26");
        assert_eq!(result["serverInfo"]["name"], "yidam");
    }

    #[test]
    fn unknown_method_is_not_found() {
        let state = test_state();
        let err = handle(&state, "bogus/method", &json!({})).unwrap_err();
        assert_eq!(err.code, -32601);
    }

    #[test]
    fn run_loop_answers_requests_and_skips_notifications() {
        let state = test_state();
        let input = concat!(
            r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
            "\n",
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            "\n",
            r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#,
            "\n",
        );
        let mut out = Vec::new();
        run_loop(&state, input.as_bytes(), &mut out).unwrap();
        let lines: Vec<&str> = std::str::from_utf8(&out).unwrap().lines().collect();
        assert_eq!(
            lines.len(),
            2,
            "one response per request, none for the notification"
        );
        let first: Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["id"], 1);
        assert!(first["result"]["serverInfo"].is_object());
    }
}
