//! End-to-end test of `yidam serve --mcp`: spawn the real binary against a
//! fixture corpus (no vector index → keyword-degraded retrieve), speak MCP
//! over its stdio, and assert on the responses.
//!
//! This file used to open `#![cfg(feature = "index")]`, which meant the shared MCP
//! conformance suite ran only in the full-feature job — on `main` and the weekly schedule,
//! never on a pull request. It first spoke up *after* the merge that broke it.
//!
//! The gate was never about what the test needs. Every case here runs against a fixture
//! with no vector index, on the keyword path; the feature was required only because
//! `serve` itself was behind it. Now that it is not, the conformance suite runs on every
//! PR in the light build, which is also the build most consumers actually have.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// Stage the contract's own fixture corpus as a git repository.
///
/// The corpus used to be written here, as heredocs in this file. That made the shared
/// conformance corpus a detail of one language's test: the case files assert counts over it
/// — `open_questions` returns 3, `neighbors` from `traversal` returns 1 — and the only way
/// to learn which corpus those counts describe was to read Rust. A consumer running these
/// cases against its own server did the reasonable thing and re-expressed every `count` and
/// `equals` against a corpus of its own, which is a conformance suite asserting that a
/// server agrees with itself.
///
/// So the corpus ships at `mcp/corpus/`, where `mcp/README.md` always said it was, and this
/// copies it. The staging — copy, `git init`, one commit — is the same recipe
/// `report_goldens.rs` uses, and is what another language's harness re-implements in ten
/// lines rather than transcribing the corpus out of this file by hand.
fn make_fixture_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    copy_dir(&contract_dir().join("corpus"), root);

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.co"]);
    git(root, &["config", "user.name", "Test"]);
    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-q", "-m", "chore: genesis — mcp fixture"],
    );
    tmp
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in walkdir::WalkDir::new(from)
        .into_iter()
        .filter_map(Result::ok)
    {
        let rel = entry.path().strip_prefix(from).unwrap();
        let dest = to.join(rel);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

struct McpClient {
    child: std::process::Child,
    stdin: std::process::ChildStdin,
    stdout: BufReader<std::process::ChildStdout>,
    next_id: u64,
}

impl McpClient {
    fn spawn(cwd: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args(["serve", "--mcp"])
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawning yidam serve --mcp");
        let stdin = child.stdin.take().unwrap();
        let stdout = BufReader::new(child.stdout.take().unwrap());
        McpClient {
            child,
            stdin,
            stdout,
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();

        let mut line = String::new();
        self.stdout.read_line(&mut line).unwrap();
        let resp: Value = serde_json::from_str(&line).expect("response is JSON");
        assert_eq!(resp["id"], id, "response id matches request");
        assert!(
            resp.get("error").is_none(),
            "{method} returned an error: {resp}"
        );
        resp["result"].clone()
    }

    fn notify(&mut self, method: &str) {
        let msg = json!({"jsonrpc": "2.0", "method": method});
        writeln!(self.stdin, "{msg}").unwrap();
        self.stdin.flush().unwrap();
    }

    /// Handshake, returning the `yidam` capability block.
    fn initialize(&mut self) -> Value {
        let init = self.request("initialize", json!({"protocolVersion": "2024-11-05"}));
        assert_eq!(init["serverInfo"]["name"], "yidam");
        self.notify("notifications/initialized");
        init["capabilities"]["yidam"].clone()
    }

    fn tool_json(&mut self, name: &str, arguments: Value) -> Value {
        let result = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        assert!(
            result["isError"].as_bool() != Some(true),
            "tool {name} errored: {result}"
        );
        serde_json::from_str(result["content"][0]["text"].as_str().unwrap()).unwrap()
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn mcp_server_end_to_end() {
    let repo = make_fixture_repo();
    let mut client = McpClient::spawn(repo.path());

    // Handshake. What this server declares it backs is read once, and everything below is
    // checked against it — a capability claimed and not served, or served and not claimed,
    // is the failure an agent would otherwise meet as a tool-not-found on the call it
    // cared about.
    let capabilities = client.initialize();
    assert_eq!(capabilities["contract"], contract()["contract"]);
    assert_eq!(capabilities["graph"], true);
    assert_eq!(capabilities["resources"], true);
    // No working git repository is read by this server, so both are honestly false.
    assert_eq!(capabilities["phases"], false);
    assert_eq!(capabilities["sangha"], false);
    // The fixture has no vector index, which is the same fact every `retrieve` reports —
    // and the handshake now says *why*, in the value the call will repeat verbatim.
    assert_eq!(capabilities["retrieve"]["vector"], false);
    assert_eq!(capabilities["retrieve"]["reason"], "no_index");

    // Resources: all five kinds listable, instance fetchable
    let listed = client.request("resources/list", json!({}));
    let uris: Vec<&str> = listed["resources"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["uri"].as_str().unwrap())
        .collect();
    assert!(uris.contains(&"yidam://graph/summary"));
    assert!(uris.contains(&"yidam://corpus/concept"));
    assert!(uris.contains(&"yidam://corpus/concept/knowledge-graph"));

    let node = client.request(
        "resources/read",
        json!({"uri": "yidam://corpus/concept/knowledge-graph"}),
    );
    assert!(node["contents"][0]["text"]
        .as_str()
        .unwrap()
        .contains("label: Knowledge graph"));

    // Tools list — checked against the frozen contract, not against a list written here.
    // This assertion used to be `assert_eq!(names, vec![...])`, which made this file a
    // second freeze: the contract could grow a tool and the only thing that noticed was a
    // test nobody edits when writing a server in another language.
    let tools = client.request("tools/list", json!({}));
    let names: Vec<String> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    assert_eq!(names, expected_tool_names(&capabilities), "{names:?}");

    // Every tool this server LISTS must answer a call. The two halves are independent:
    // `tools/list` is derived from the contract and `tools/call` is a hand-written match, so
    // a tool added to the contract and not to the match is advertised and errors — and the
    // assertion above still passes, because both sides of it read the same file. That is a
    // green build for a server that cannot do what it says it can.
    //
    // Arguments are deliberately empty. A tool that needs one answers "class is required",
    // which is a tool doing its job; the failure this catches says "unknown tool".
    for name in &names {
        let result = client.request("tools/call", json!({"name": name, "arguments": {}}));
        // `request` returns the `result` object, not the envelope. Reading `["result"]`
        // here yielded null, `unwrap_or("")` swallowed it, and the assertion below could
        // not fail — a check that looked exactly like one doing work. Caught by deleting a
        // dispatch arm and watching this pass, which is the only way that class of defect
        // ever announces itself.
        let text = result["content"][0]["text"].as_str().unwrap_or_default();
        assert!(
            !text.contains("unknown tool"),
            "{name} is listed and not implemented — the contract and the dispatch disagree \
             about what this server can do, and the list assertion above cannot see it \
             because both sides of it read the same file"
        );
    }

    // Every case in the contract's `cases/` directory, run against this server.
    for case in contract_cases() {
        let capability = case["capability"].as_str();
        if let Some(c) = capability {
            if !capabilities[c].as_bool().unwrap_or(false) {
                continue; // declared absent — its cases are skipped, not passed
            }
        }
        let tool = case["tool"].as_str().unwrap();
        let response = client.tool_json(tool, case["call"].clone());
        check_case(&case, &response);
    }

    // retrieve — no index in the fixture, so keyword-degraded. `no_index` and not
    // `no_vector_support` even when this binary has no vector support: the corpus is
    // missing the artefact, which is the repair under either build.
    let retrieved = client.tool_json("retrieve", json!({"query": "knowledge graph"}));
    assert_eq!(retrieved["degraded"], true);
    assert_eq!(retrieved["degraded_reason"], "no_index");
    let results = retrieved["results"].as_array().unwrap();
    assert!(!results.is_empty(), "retrieve returns at least one result");
    assert_eq!(results[0]["label"], "Knowledge graph");

    // get_node + neighbors + open_questions
    let node = client.tool_json("get_node", json!({"id": "concept/knowledge-graph"}));
    assert_eq!(node["links"][0]["target"], "concept/traversal");

    let neigh = client.tool_json("neighbors", json!({"id": "concept/knowledge-graph"}));
    assert_eq!(neigh["neighbors"][0]["id"], "concept/traversal");

    let open = client.tool_json("open_questions", json!({}));
    let flagged: Vec<&str> = open["open_questions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| q["id"].as_str().unwrap())
        .collect();
    // One node per arm, named. The case file pins the count; this pins *which* node each
    // arm is supposed to have found, so a server that gains one arm and loses another still
    // fails here rather than passing on an accidental total.
    assert!(flagged.contains(&"concept/traversal"), "{flagged:?}"); // ? label
    assert!(flagged.contains(&"concept/retrieval"), "{flagged:?}"); // [open] in the body
    assert!(
        flagged.contains(&"concept/embedding-space"), // claim_tag: open, declared
        "the declared-claim-field arm found nothing: {flagged:?}"
    );
}

/// A build with no vector support, pointed at a corpus that *has* an index.
///
/// This is the case the `index` gate used to make unreachable: before `serve` moved into
/// the light set there was no such binary, so there was nothing to observe. Now it is the
/// build most people run, and it must not tell them to build an index they already built.
///
/// Light-only by necessity rather than by preference. The staged `corpus.arrow` is not real
/// Arrow — under `--features index` the server would try to decode it and refuse to start,
/// which is the correct behaviour there and a different test. What matters is that the
/// light build reads `meta.json`, notices the artefact, and says `no_vector_support`.
#[test]
#[cfg(not(feature = "index"))]
fn an_index_this_build_cannot_read_is_not_reported_as_a_missing_index() {
    let tmp = make_fixture_repo();
    let index_dir = tmp.path().join(".yidam/index");
    std::fs::create_dir_all(&index_dir).unwrap();
    std::fs::write(index_dir.join("corpus.arrow"), b"not really arrow").unwrap();
    std::fs::write(
        index_dir.join("meta.json"),
        br#"{"model_name": "Xenova/all-MiniLM-L6-v2", "indexed_commit": "deadbee"}"#,
    )
    .unwrap();

    let mut client = McpClient::spawn(tmp.path());
    let capabilities = client.initialize();
    assert_eq!(capabilities["retrieve"]["vector"], false);
    assert_eq!(capabilities["retrieve"]["reason"], "no_vector_support");

    let retrieved = client.tool_json("retrieve", json!({"query": "knowledge graph"}));
    assert_eq!(retrieved["degraded"], true);
    assert_eq!(
        retrieved["degraded_reason"], "no_vector_support",
        "an index is on disk; telling the user to build one is the wrong repair"
    );
    // Every other tool is unaffected — that is the claim that made this move safe.
    assert!(!retrieved["results"].as_array().unwrap().is_empty());
    let node = client.tool_json("get_node", json!({"id": "concept/knowledge-graph"}));
    assert_eq!(node["links"][0]["target"], "concept/traversal");
}

// ── conformance ──────────────────────────────────────────────────────────────

/// The MCP `query` tool and `yidam query` must answer the same thing (#263).
///
/// They are two front doors onto one function, and the case files can only ever see one of
/// them. The risk is specific rather than theoretical: the server answers from a corpus it
/// parsed at startup and the CLI parses one per invocation, so a divergence would look like
/// staleness on whichever surface was read less — and an agent asking through MCP would get a
/// confidently wrong answer with a cost block attached.
///
/// `results`, `matched` and `cost` only. The envelope differs by design: the CLI wraps the
/// payload in the RFC-0016 report envelope with a root and a build commit, and the tool
/// returns the payload bare.
#[test]
fn the_mcp_tool_and_the_cli_answer_the_same_query() {
    let repo = make_fixture_repo();
    let mut client = McpClient::spawn(repo.path());

    for query in [
        "concept -enables-> concept",
        "concept <-enables- concept",
        "*[claim_tag=open]",
        "concept~\"embedding space\"",
        // A rejection too: both surfaces must reject it, and for the same reason. The CLI
        // exits 1 here and the tool does not error — the difference is in how the verdict
        // travels, never in what it is.
        "concpet",
    ] {
        let served = client.tool_json("query", json!({"query": query}));
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .args(["query", query, "--format", "json"])
            .current_dir(repo.path())
            .output()
            .unwrap();
        let envelope: Value = serde_json::from_slice(&out.stdout).unwrap();
        for field in [
            "results",
            "matched",
            "cost",
            "rejected",
            "anchor",
            "diagnostics",
        ] {
            assert_eq!(
                served[field], envelope[field],
                "`{query}`: the MCP tool and the CLI disagree about `{field}`"
            );
        }
    }
}

fn contract_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../prelude/sdks/parity/mcp")
}

fn contract() -> Value {
    serde_json::from_str(&std::fs::read_to_string(contract_dir().join("tools.json")).unwrap())
        .unwrap()
}

/// The tools a server declaring these capabilities must serve, in contract order.
fn expected_tool_names(capabilities: &Value) -> Vec<String> {
    contract()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| {
            let tier = t["tier"].as_str().unwrap();
            tier == "core" || capabilities[tier].as_bool().unwrap_or(false)
        })
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect()
}

fn contract_cases() -> Vec<Value> {
    let mut cases = Vec::new();
    for tool_dir in walkdir::WalkDir::new(contract_dir().join("cases"))
        .into_iter()
        .filter_map(Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
    {
        cases.push(
            serde_json::from_str(&std::fs::read_to_string(tool_dir.path()).unwrap())
                .unwrap_or_else(|e| panic!("{}: {e}", tool_dir.path().display())),
        );
    }
    assert!(!cases.is_empty(), "no cases found — the scan is broken");
    cases
}

/// Follow a dotted path — `neighbors.0.direction` — through a response.
fn at<'a>(value: &'a Value, path: &str) -> &'a Value {
    let mut current = value;
    for part in path.split('.') {
        current = match part.parse::<usize>() {
            Ok(i) => &current[i],
            Err(_) => &current[part],
        };
    }
    current
}

/// An array at a dotted path, or a failure naming the case rather than `Option::unwrap`.
fn array<'a>(response: &'a Value, path: &str, tool: &str, why: &str) -> &'a Vec<Value> {
    at(response, path)
        .as_array()
        .unwrap_or_else(|| panic!("{tool}.{path} is not an array\n{why}\n{response}"))
}

/// Assert one case's expectations.
///
/// Shape and invariants only: never a score. A score is a property of a model, and a
/// contract that pinned one would fail on every server that legitimately embeds differently
/// — which is the thing `degraded` exists to report rather than forbid.
///
/// **Every name here is a dotted path**, not just `equalsAt`'s. `query` is the first tool
/// whose response has structure below the top level — `cost.nodes_read`, `anchor.entries`,
/// `steps.0.classes` — and a harness that could only assert on top-level keys would have had
/// its cases written against a flattened response, which is a second shape for one answer. A
/// single segment resolves to exactly what `response[name]` used to, so the existing cases
/// mean what they meant.
fn check_case(case: &Value, response: &Value) {
    let tool = case["tool"].as_str().unwrap();
    let expect = &case["expect"];
    let why = case["why"].as_str().unwrap_or("");

    if let Some(fields) = expect["fields"].as_array() {
        for f in fields {
            let name = f.as_str().unwrap();
            assert!(
                !at(response, name).is_null() || response.get(name).is_some(),
                "{tool}: response has no `{name}`.\n{why}\n{response}"
            );
        }
    }
    if let Some(equals) = expect["equals"].as_object() {
        for (k, v) in equals {
            assert_eq!(&response[k], v, "{tool}.{k}\n{why}");
        }
    }
    if let Some(equals) = expect["equalsAt"].as_object() {
        for (path, v) in equals {
            assert_eq!(at(response, path), v, "{tool}.{path}\n{why}");
        }
    }
    if let Some(names) = expect["nonEmpty"].as_array() {
        for n in names {
            let name = n.as_str().unwrap();
            // A string counts. `pack.text` is the first response field whose emptiness is the
            // thing worth asserting and which is not a list — and a case forced to say
            // `count: {text: …}` about prose would be pinning the renderer's byte layout on
            // every server, which is exactly what that case's `why` says it must not do.
            let empty = match at(response, name) {
                Value::String(s) => s.is_empty(),
                _ => array(response, name, tool, why).is_empty(),
            };
            assert!(!empty, "{tool}.{name} is empty\n{why}");
        }
    }
    if let Some(counts) = expect["count"].as_object() {
        for (name, n) in counts {
            assert_eq!(
                array(response, name, tool, why).len(),
                n.as_u64().unwrap() as usize,
                "{tool}.{name}\n{why}\n{response}"
            );
        }
    }
    if let Some(each) = expect["each"].as_object() {
        for (name, fields) in each {
            for item in array(response, name, tool, why) {
                for f in fields.as_array().unwrap() {
                    let field = f.as_str().unwrap();
                    assert!(
                        item.get(field).is_some(),
                        "{tool}.{name}[] has no `{field}`\n{why}\n{item}"
                    );
                }
            }
        }
    }
    if let Some(every) = expect["everyItemHas"].as_object() {
        for (name, pairs) in every {
            for item in array(response, name, tool, why) {
                for (k, v) in pairs.as_object().unwrap() {
                    assert_eq!(&item[k], v, "{tool}.{name}[].{k}\n{why}");
                }
            }
        }
    }
}

/// Every frozen tool has a row in the document that tells an agent when to reach for it.
///
/// The same procedural hole RFC-0017 closed one layer out. There, a tool added to
/// `tools.json` and not to `tools/call` was advertised by the server and errored on
/// invocation, and the E2E list assertion still passed because both halves read the same
/// file. Here, a tool added to the contract and not to `docs/mcp-server.md` is served
/// correctly, conforms fully, and is invisible to the only reader who decides whether to call
/// it. `query` shipped at contract 0.6.0 and spent a release exactly like that.
///
/// The row is the assertion and the prose is not: the table is what an agent scans, and a
/// tool that has one is discoverable whatever else the page does or does not say about it.
#[test]
fn every_tool_in_the_contract_is_in_the_document_an_agent_reads() {
    let doc = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/mcp-server.md"),
    )
    .expect("docs/mcp-server.md is readable");
    let table: Vec<&str> = doc.lines().filter(|l| l.starts_with("| `")).collect();
    let missing: Vec<String> = contract()["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .filter(|name| {
            !table
                .iter()
                .any(|row| row.starts_with(&format!("| `{name}` |")))
        })
        .collect();
    assert!(
        missing.is_empty(),
        "docs/mcp-server.md's tool table has no row for {missing:?} — a conforming server \
         serves them and no agent reading the documentation knows they exist"
    );
}

/// Spanning is `query`-only, and that is a decision rather than an omission (#333).
///
/// `pack` and `estimate` take no `across`, for the reason `cmd::pack::run` states: a pack is
/// context an agent writes *from*, and one mixing two corpora would put a dependency's prose
/// in a context window under this repository's class names — where `omitted_by_class` would
/// report `concept: 12` over two corpora that each mean something different by `concept`. The
/// receipt would be arithmetic over a category nobody declared.
///
/// The danger is not that someone argues against that. It is that `across` looks like an
/// input three sibling tools obviously ought to share, and adding it to `pack` is a one-line
/// diff that reads as consistency. This makes the omission deliberate in a place a diff has to
/// touch, and the contract carries the reason.
#[test]
fn only_query_spans_the_dependency_set() {
    let contract = contract();
    let tools = contract["tools"].as_array().unwrap();

    let spanning: Vec<&str> = tools
        .iter()
        .filter(|t| !t["inputSchema"]["properties"]["across"].is_null())
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        spanning,
        vec!["query"],
        "`across` belongs to `query` alone. A pack or a quote that spanned would be a receipt \
         over a category nobody declared — if that changed, it changed in the contract's notes \
         first, and this test with it"
    );

    let notes = tools
        .iter()
        .find(|t| t["name"] == "query")
        .and_then(|t| t["response"]["notes"].as_str())
        .expect("query documents its response");
    assert!(
        notes.contains("SPANNING IS `query`-ONLY"),
        "the contract must say why `pack` and `estimate` have no `across`; an omission nobody \
         wrote down is indistinguishable from one nobody thought of, which is what #333 found"
    );
}
