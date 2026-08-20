//! End-to-end test of `yidam serve --mcp`: spawn the real binary against a
//! fixture corpus (no vector index → keyword-degraded retrieve), speak MCP
//! over its stdio, and assert on the responses.

#![cfg(feature = "index")]

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

fn make_fixture_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "t@t.co"]);
    git(root, &["config", "user.name", "Test"]);

    let corpus = root.join(".yidam").join("corpus");
    let class_dir = corpus.join("concept");
    std::fs::create_dir_all(&class_dir).unwrap();
    std::fs::write(corpus.join("concept.ont.yml"), "class: concept\n").unwrap();
    std::fs::write(
        class_dir.join("knowledge-graph.yml"),
        "class: concept\nlabel: Knowledge graph\n\
         description: A knowledge graph of typed nodes and edges.\n\
         links:\n  - target: traversal.yml\n    relationship: enables\n",
    )
    .unwrap();
    // Flagged by its LABEL only. The open-question predicate has two arms, and the fixture
    // used to carry one node flagged both ways — so a server implementing either arm alone
    // returned the same answer and the case could not tell them apart.
    std::fs::write(
        class_dir.join("traversal.yml"),
        "class: concept\nlabel: \"? Traversal strategy\"\n\
         description: How agents walk the graph.\nlinks: []\n",
    )
    .unwrap();
    // Flagged by its CLAIM only — the other arm.
    std::fs::write(
        class_dir.join("retrieval.yml"),
        "class: concept\nlabel: Retrieval\n\
         description: Finding a node without walking to it. [open]\n\
         links:\n  - target: knowledge-graph.yml\n    relationship: relates-to\n",
    )
    .unwrap();

    git(root, &["add", "."]);
    git(
        root,
        &["commit", "-q", "-m", "chore: genesis — mcp fixture"],
    );
    tmp
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

    // Handshake
    let init = client.request("initialize", json!({"protocolVersion": "2024-11-05"}));
    assert_eq!(init["serverInfo"]["name"], "yidam");
    client.notify("notifications/initialized");

    // What this server declares it backs. Read once, and everything below is checked
    // against it — a capability claimed and not served, or served and not claimed, is the
    // failure an agent would otherwise meet as a tool-not-found on the call it cared about.
    let capabilities = init["capabilities"]["yidam"].clone();
    assert_eq!(capabilities["contract"], contract()["contract"]);
    assert_eq!(capabilities["graph"], true);
    assert_eq!(capabilities["resources"], true);
    // No working git repository is read by this server, so both are honestly false.
    assert_eq!(capabilities["phases"], false);
    assert_eq!(capabilities["sangha"], false);
    // The fixture has no vector index, which is the same fact every `retrieve` reports.
    assert_eq!(capabilities["retrieve"]["vector"], false);

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

    // retrieve — no index in the fixture, so keyword-degraded
    let retrieved = client.tool_json("retrieve", json!({"query": "knowledge graph"}));
    assert_eq!(retrieved["degraded"], true);
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
    assert!(flagged.contains(&"concept/traversal"), "{flagged:?}");
    assert!(flagged.contains(&"concept/retrieval"), "{flagged:?}");
}

// ── conformance ──────────────────────────────────────────────────────────────

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

/// Assert one case's expectations.
///
/// Shape and invariants only: never a score. A score is a property of a model, and a
/// contract that pinned one would fail on every server that legitimately embeds differently
/// — which is the thing `degraded` exists to report rather than forbid.
fn check_case(case: &Value, response: &Value) {
    let tool = case["tool"].as_str().unwrap();
    let expect = &case["expect"];
    let why = case["why"].as_str().unwrap_or("");

    if let Some(fields) = expect["fields"].as_array() {
        for f in fields {
            let name = f.as_str().unwrap();
            assert!(
                response.get(name).is_some(),
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
            assert!(
                !response[name].as_array().unwrap().is_empty(),
                "{tool}.{name} is empty\n{why}"
            );
        }
    }
    if let Some(counts) = expect["count"].as_object() {
        for (name, n) in counts {
            assert_eq!(
                response[name].as_array().unwrap().len(),
                n.as_u64().unwrap() as usize,
                "{tool}.{name}\n{why}\n{response}"
            );
        }
    }
    if let Some(each) = expect["each"].as_object() {
        for (name, fields) in each {
            for item in response[name].as_array().unwrap() {
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
            for item in response[name].as_array().unwrap() {
                for (k, v) in pairs.as_object().unwrap() {
                    assert_eq!(&item[k], v, "{tool}.{name}[].{k}\n{why}");
                }
            }
        }
    }
}
