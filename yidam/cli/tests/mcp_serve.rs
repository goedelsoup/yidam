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
    std::fs::write(
        class_dir.join("traversal.yml"),
        "class: concept\nlabel: \"? Traversal strategy\"\n\
         description: How agents walk the graph. [open]\nlinks: []\n",
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

    // Tools list
    let tools = client.request("tools/list", json!({}));
    let names: Vec<&str> = tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        vec!["retrieve", "get_node", "neighbors", "open_questions"]
    );

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
    assert_eq!(open["open_questions"][0]["id"], "concept/traversal");
}
