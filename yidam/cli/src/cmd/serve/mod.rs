//! `yidam serve --mcp` — expose the domain computer as an MCP server.
//!
//! The protocol layer is hand-rolled stdio JSON-RPC rather than an MCP SDK
//! dependency: the surface we need (initialize, resources, tools) is small,
//! and the available crates require a newer toolchain than the repo pins.
//!
//! All reads come from the already-built corpus and index on disk via
//! [`load_domain_model`] — no live git operations. If HEAD has advanced past
//! the indexed commit the startup banner warns but the server keeps serving.
//!
//! # What the `index` feature does and does not gate
//!
//! It gates one tool's *quality*, not the server. Everything here compiles in the light
//! default build; only [`crate::retrieval::vector`] — the embedder and the index decode —
//! needs the ML stack. Without it `retrieve` answers from keyword search and reports
//! `degraded: true` with a [`Retrieval::degraded_reason`], and every other tool is
//! byte-identical. The command that makes a corpus reachable by an agent should not be the
//! one command a collaborator cannot install.

mod absence;
mod resources;
pub(crate) mod tools;

use anyhow::Result;
use serde_json::{json, Value};
use std::io::{BufRead, Write};
use std::path::Path;

use crate::git::head_commit_short;
use crate::model::{corpus_nodes, file_stem as stem, load_domain_model};
use crate::paths::repo_root;

/// One corpus instance, parsed for serving. The id (`<class>/<name>`) is
/// what `get_node` and `neighbors` accept.
pub(crate) use crate::model::NodeView as Node;

/// How `retrieve` will answer, and — when it will answer badly — why.
///
/// Lifted to [`crate::retrieval`] by #263 and re-exported here, so `serve` and `query` can
/// never come to disagree about why retrieval is degraded. RFC-0018 asked for exactly this
/// refactor and nothing else.
pub(crate) use crate::retrieval::Retrieval;

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
    pub retrieval: Retrieval,
    /// Commit the index was built at, from `index/meta.json`.
    ///
    /// Read from the raw metadata, so the staleness warning still fires in a build that
    /// cannot decode the index itself. An index too old to trust is worth saying either way.
    pub indexed_commit: Option<String>,
    /// Which properties of each class carry an evidence tag.
    ///
    /// Loaded here so the MCP server's open-question answer is the reports' answer. It was a
    /// second copy of the predicate, and a corpus with structured tags was under-reported by
    /// both in the same way — which is the kind of agreement that looks like correctness.
    pub claim_fields: crate::claims::ClaimFields,
    /// The class contract, keyed by the `<class>.ont.yml` **stem**.
    ///
    /// Keyed by the filename and not by the file's `class:` field, because the filename is
    /// what the gate keys on: `load_classes` uses the stem always, and `unknown-class`
    /// compares an instance's `class:` against the set of stems. Where the two disagree,
    /// answering from the field would license edges for a class no instance belongs to.
    pub classes: Vec<(String, yidam_core::ontology::OntologyClass)>,
    /// The corpus as `yidam query` reads it, loaded once.
    ///
    /// A second parse of the same directory, and deliberately not a projection of `nodes`.
    /// The query surface resolves edges with the *gate's* own resolver
    /// ([`crate::cmd::lint::checks::instance_links`]), which is narrower than what
    /// `neighbors` reports — it drops the link to the class file and citations into the
    /// catalog. Deriving one from the other would mean re-implementing that rule here, and
    /// `cmd/graph.rs` already states what happens then: the two would disagree about which
    /// edges exist, silently, in the direction of "looks fine here".
    ///
    /// Loaded at startup rather than per call: the server answers many queries against one
    /// corpus, and re-walking `.yidam/corpus` on each would make the cheapest query the most
    /// expensive thing the server does.
    ///
    /// **`query --select body` answers from here too**, and since #324 that is a startup
    /// snapshot rather than a read of the file at request time. It is the same snapshot every
    /// other tool answers from — `nodes`, `retrieval`, `classes` and `citations` are all built
    /// once above — so the change made `body` consistent with the server rather than stale
    /// against it. A `body` that reached for the working tree would be one field, on one tool,
    /// answering about a different corpus than every field beside it, including the `at` key
    /// that would still read null. The contract states it (`parity/mcp/tools.json`, 0.9.1) and
    /// the connect-time staleness banner reports the same fact. Freshness is a restart.
    pub graph: crate::cmd::query::Graph,
    /// Catalog entries each node cites, keyed by node id.
    ///
    /// Resolved once at startup with the gate's own resolver rather than from
    /// [`Node::links`], which cannot answer it: a prose citation —
    /// `[Pearl 2009](../../catalog/pearl-2009.md)`, the form the conventions prescribe —
    /// never enters that list, and a catalog target is carried verbatim rather than
    /// resolved. Not by slug, either: that read failed an error-severity gate on a node
    /// containing no citation, because connector crates are named after what they fetch.
    pub citations: std::collections::HashMap<String, Vec<String>>,
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

        let (retrieval, indexed_commit) = crate::retrieval::load(&model)?;

        let claim_fields = crate::claims::ClaimFields::load(&crate::paths::yidam_corpus_dir(root));
        let classes = model
            .classes
            .iter()
            .map(|c| {
                let name = stem(&c.filename).replace(".ont", "");
                let text = String::from_utf8_lossy(&c.content);
                let class = yidam_core::ontology::parse_class(&name, &text);
                (name, class)
            })
            .collect();
        let citations = load_citations(root, &nodes);
        let graph = crate::cmd::query::Graph::load(root);
        Ok(ServerState {
            domain: model.provenance.domain,
            commit: model.provenance.commit,
            nodes,
            dep_nodes,
            skills,
            decisions,
            retrieval,
            indexed_commit,
            claim_fields,
            classes,
            citations,
            graph,
        })
    }
}

/// Which catalog entries each node cites.
///
/// `linked_paths` is the resolver `catalog-uncited` gates on — `links:` targets and prose
/// markdown links both, resolved against the node's own directory. Reusing it is what keeps
/// the agent surface and the gate from disagreeing about what a citation is.
///
/// `walk_md_files` for the catalog, not a recursive walk: it is `max_depth(1)` and skips
/// `README.md`, which is a REGEN target rather than a source. A deeper walk would invent
/// catalog entries no check knows about.
fn load_citations(root: &Path, nodes: &[Node]) -> std::collections::HashMap<String, Vec<String>> {
    let catalog_dir = crate::paths::yidam_catalog_dir(root);
    let corpus_dir = crate::paths::yidam_corpus_dir(root);
    let sources: Vec<(String, std::path::PathBuf)> = crate::walk::walk_md_files(&catalog_dir)
        .into_iter()
        .map(|p| {
            let rel = p
                .strip_prefix(root)
                .unwrap_or(&p)
                .to_string_lossy()
                .replace('\\', "/");
            (rel, crate::cmd::lint::checks::normalize(&p))
        })
        .collect();
    if sources.is_empty() {
        return Default::default();
    }

    nodes
        .iter()
        .filter(|n| n.is_local())
        .map(|n| {
            let path = corpus_dir.join(format!("{}.yml", n.id));
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let linked = crate::cmd::lint::checks::linked_paths(&path, &rel, &n.content);
            let cited = sources
                .iter()
                .filter(|(_, p)| linked.contains(p))
                .map(|(rel, _)| rel.clone())
                .collect();
            (n.id.clone(), cited)
        })
        .collect()
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
    match &state.retrieval {
        #[cfg(feature = "index")]
        Retrieval::Vector(idx) => eprintln!(
            "vector index: {} row(s), model {} — `retrieve` uses semantic search",
            idx.rows.len(),
            idx.model_id
        ),
        Retrieval::NoIndex => eprintln!(
            "vector index: absent (no_index) — `retrieve` degrades to keyword search; \
             run `yidam embed && yidam index-build` to build one"
        ),
        #[cfg(not(feature = "index"))]
        Retrieval::NoVectorSupport => eprintln!(
            "vector index: present but unreadable by this build (no_vector_support) — \
             `retrieve` degrades to keyword search; reinstall with `--features index` for \
             semantic search"
        ),
    }
    if let Some(indexed) = &state.indexed_commit {
        let head = head_commit_short(&root);
        if *indexed != head {
            // "serving the stale index" is only true of a build that is serving it. A
            // build that cannot read the index still owes the warning — the staleness is
            // real and worth knowing before installing one that can — but must not claim
            // to be answering from it.
            let consequence = match state.retrieval.degraded_reason() {
                None => "serving the stale index; run `yidam index-build` to refresh",
                Some(_) => "`retrieve` is not reading it; run `yidam index-build` to refresh",
            };
            eprintln!(
                "warning: HEAD ({head}) has advanced past the indexed commit ({indexed}) — \
                 {consequence}"
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
            retrieval: Retrieval::NoIndex,
            indexed_commit: None,
            // No class declares a claim field here, so the predicate reads prose only —
            // which is what these fixtures are written in.
            claim_fields: Default::default(),
            // A class that declares edges and one that declares none. The second is what
            // makes `licensed_edges`' "has said nothing" arm reachable, and that arm is the
            // one a client is most likely to read backwards.
            classes: vec![
                (
                    "concept".to_string(),
                    yidam_core::ontology::parse_class(
                        "concept",
                        "class: concept\nedges:\n  - relationship: enables\n    target: concept\n    direction: out\n",
                    ),
                ),
                (
                    "silent".to_string(),
                    yidam_core::ontology::parse_class("silent", "class: silent\n"),
                ),
            ],
            citations: Default::default(),
            // Empty, and the reason is worth stating: every query assertion worth making is
            // an assertion about a *corpus*, and this fixture is a hand-built `ServerState`
            // with no directory behind it. The query tool's cases live in the contract's
            // `cases/query/`, against the shared fixture corpus, where another language's
            // server runs the same ones.
            graph: crate::cmd::query::Graph::load(std::path::Path::new(
                "/nonexistent/query-fixture",
            )),
        }
    }

    #[test]
    #[cfg(not(feature = "index"))]
    fn a_light_build_says_why_it_cannot_use_an_index_that_exists() {
        // The whole point of the third state. Before this, a binary that could not read an
        // index and a corpus that had none both answered `degraded: true` and nothing else,
        // and the advice for one ("run index-build") is wasted on the other.
        let mut state = test_state();
        state.retrieval = Retrieval::NoVectorSupport;
        let capabilities = tools::capabilities(&state);
        assert_eq!(capabilities["retrieve"]["vector"], false);
        assert_eq!(capabilities["retrieve"]["reason"], "no_vector_support");
    }

    #[test]
    fn the_capability_block_and_the_call_report_the_same_reason() {
        // They are one fact stated at two moments, and a client is entitled to assume they
        // agree — the contract says the handshake *promises* what every call will say.
        #[cfg(not(feature = "index"))]
        let states = [Retrieval::NoIndex, Retrieval::NoVectorSupport];
        #[cfg(feature = "index")]
        let states = [Retrieval::NoIndex];
        for retrieval in states {
            let mut state = test_state();
            state.retrieval = retrieval;
            let declared = tools::capabilities(&state)["retrieve"]["reason"].clone();
            let answered = handle(
                &state,
                "tools/call",
                &json!({"name": "retrieve", "arguments": {"query": "graph"}}),
            )
            .unwrap();
            let body: Value =
                serde_json::from_str(answered["content"][0]["text"].as_str().unwrap()).unwrap();
            assert_eq!(body["degraded"], true);
            assert_eq!(body["degraded_reason"], declared);
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
