//! End-to-end test of `yidam serve --mcp --http`: spawn the real binary against the contract's
//! own fixture corpus, speak HTTP to it, and assert on what comes back.
//!
//! `http.rs`'s unit tests cover the *policy* — which origins pass, which methods are refused,
//! what counts as a protocol version — and none of them binds a socket. That is deliberate and
//! it is also not enough: a policy that is right and a server that never reaches it look
//! identical from inside the crate. Everything here goes over a real TCP connection, through
//! hyper, into the same `handle` the stdio transport uses.
//!
//! The port is `0`. The server reports the address it actually bound, and this parses it out of
//! stderr — which is why that line prints `local_addr()` and not the argument it was given.
#![cfg(feature = "serve-http")]

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract_dir() -> PathBuf {
    repo_root().join("yidam/prelude/sdks/parity/mcp")
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .unwrap();
    assert!(status.success(), "git {args:?} failed");
}

/// The same staging recipe `mcp_serve.rs` uses, against the same shipped corpus.
///
/// Copied rather than shared because the two files ask different questions of it — that one
/// runs the conformance cases, this one runs the transport — and a helper crate for six lines
/// of `cp` would couple them for no benefit.
fn fixture_repo() -> tempfile::TempDir {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    let from = contract_dir().join("corpus");
    assert!(from.is_dir(), "no fixture corpus at {}", from.display());

    let status = Command::new("cp")
        .arg("-R")
        .arg(from.join(".yidam"))
        .arg(root.join(".yidam"))
        .status()
        .unwrap();
    assert!(status.success(), "copying the fixture corpus failed");

    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "t"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "fixture"]);
    tmp
}

/// A server on a port the OS chose, and the address it chose.
struct Server {
    child: Child,
    addr: String,
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn start(repo: &Path, extra: &[&str]) -> Server {
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(repo)
        .args(["serve", "--mcp", "--http", "--port", "0"])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary starts");

    // The banner names the bound address. Reading it is also how this waits for the socket to
    // be listening — a sleep would be a race whichever number was chosen.
    let stderr = child.stderr.take().expect("stderr is piped");
    let mut reader = BufReader::new(stderr);
    let mut addr = None;
    for _ in 0..40 {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        if let Some(rest) = line.split_once("http://") {
            let host = rest.1.split('/').next().unwrap_or("").trim().to_string();
            if !host.is_empty() {
                addr = Some(host);
                break;
            }
        }
    }
    let addr = addr.unwrap_or_else(|| {
        let _ = child.kill();
        panic!("the server never announced an address")
    });
    Server { child, addr }
}

/// One HTTP request, hand-written. No client dependency: the point is to send exactly the bytes
/// a platform would, including the headers whose handling is the thing under test.
fn request(addr: &str, method: &str, path: &str, headers: &[(&str, &str)], body: &str) -> String {
    let mut stream = TcpStream::connect(addr).expect("the server is listening");
    let mut req = format!(
        "{method} {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nContent-Length: {}\r\n",
        body.len()
    );
    for (k, v) in headers {
        req.push_str(&format!("{k}: {v}\r\n"));
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();

    let mut response = String::new();
    stream.read_to_string(&mut response).unwrap();
    response
}

fn status_of(response: &str) -> u16 {
    response
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse().ok())
        .unwrap_or_else(|| panic!("no status line in:\n{response}"))
}

fn body_of(response: &str) -> &str {
    response.split_once("\r\n\r\n").map(|p| p.1).unwrap_or("")
}

fn rpc(addr: &str, body: Value) -> (u16, String) {
    let r = request(
        addr,
        "POST",
        "/mcp",
        &[
            ("content-type", "application/json"),
            ("accept", "application/json, text/event-stream"),
        ],
        &body.to_string(),
    );
    (status_of(&r), body_of(&r).to_string())
}

/// The handshake arrives over HTTP, carrying the capability block the contract requires.
///
/// This is the assertion that the transport is a transport and not a second server: the block
/// is built by `tools::capabilities` from the corpus on disk, and nothing in `http.rs` knows it
/// exists.
#[test]
fn initialize_answers_over_http_with_the_capability_block() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);

    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18"}}),
    );
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).expect("a JSON body");
    assert_eq!(v["id"], 1);
    assert_eq!(v["result"]["serverInfo"]["name"], "yidam");
    let caps = &v["result"]["capabilities"]["yidam"];
    assert!(caps["contract"].is_string(), "no contract version: {v}");
    assert_eq!(caps["ontology"], true, "the fixture corpus declares one");
}

/// The tool list over HTTP is the tool list, and `tools/call` answers from the same corpus.
#[test]
fn the_tools_are_the_frozen_ones_and_they_answer() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);

    let (_, body) = rpc(
        &server.addr,
        json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}),
    );
    let v: Value = serde_json::from_str(&body).unwrap();
    let names: Vec<&str> = v["result"]["tools"]
        .as_array()
        .expect("a tool array")
        .iter()
        .filter_map(|t| t["name"].as_str())
        .collect();
    for expected in ["retrieve", "get_node", "list_nodes", "open_questions"] {
        assert!(
            names.contains(&expected),
            "{expected} missing from {names:?}"
        );
    }

    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
               "params":{"name":"get_node","arguments":{"id":"concept/knowledge-graph"}}}),
    );
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    let text = v["result"]["content"][0]["text"].as_str().expect("text");
    let node: Value = serde_json::from_str(text).expect("the node is JSON, not a render");
    assert_eq!(node["id"], "concept/knowledge-graph");
}

/// A notification gets 202 and no body, which the spec requires in those words.
#[test]
fn a_notification_is_accepted_and_not_answered() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);

    let response = request(
        &server.addr,
        "POST",
        "/mcp",
        &[("content-type", "application/json")],
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}).to_string(),
    );
    assert_eq!(status_of(&response), 202, "{response}");
    assert!(
        body_of(&response).trim().is_empty(),
        "202 must carry no body: {response}"
    );
}

/// GET is 405 rather than a stream, and says so in the frozen token.
#[test]
fn get_is_405_because_this_server_streams_nothing() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);

    let response = request(
        &server.addr,
        "GET",
        "/mcp",
        &[("accept", "text/event-stream")],
        "",
    );
    assert_eq!(status_of(&response), 405, "{response}");
    assert!(body_of(&response).contains("no-sse-stream"), "{response}");
}

/// A browser origin nobody allowed is refused, over a real connection.
///
/// The unit tests decide the policy; this proves the header reaches it. A `vet` that were
/// never called would pass every one of them.
#[test]
fn an_unnamed_origin_is_refused_and_a_named_one_is_not() {
    let repo = fixture_repo();
    let server = start(repo.path(), &["--allow-origin", "https://chat.example"]);
    let call = json!({"jsonrpc":"2.0","id":1,"method":"ping"}).to_string();

    let refused = request(
        &server.addr,
        "POST",
        "/mcp",
        &[
            ("content-type", "application/json"),
            ("origin", "http://evil.test"),
        ],
        &call,
    );
    assert_eq!(status_of(&refused), 403, "{refused}");
    assert!(
        body_of(&refused).contains("origin-not-allowed"),
        "{refused}"
    );

    let allowed = request(
        &server.addr,
        "POST",
        "/mcp",
        &[
            ("content-type", "application/json"),
            ("origin", "https://chat.example"),
        ],
        &call,
    );
    assert_eq!(status_of(&allowed), 200, "{allowed}");

    // And the case that would make the transport unusable if it were wrong.
    let no_origin = request(
        &server.addr,
        "POST",
        "/mcp",
        &[("content-type", "application/json")],
        &call,
    );
    assert_eq!(
        status_of(&no_origin),
        200,
        "a server-to-server client sends no Origin: {no_origin}"
    );
}

/// Defaulting to loopback is the spec's SHOULD, and it is a default a flag can change.
///
/// Asserted on the announced address rather than by probing an external interface, which no
/// test can do portably.
#[test]
fn the_default_bind_is_loopback() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);
    assert!(
        server.addr.starts_with("127.0.0.1:"),
        "bound {} rather than loopback",
        server.addr
    );
}

/// A path that is not the endpoint is told which one is.
#[test]
fn another_path_names_the_endpoint() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);
    let response = request(&server.addr, "POST", "/", &[], "");
    assert_eq!(status_of(&response), 404, "{response}");
    assert!(body_of(&response).contains("/mcp"), "{response}");
}
