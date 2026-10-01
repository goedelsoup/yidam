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
use std::fmt::Write as _;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use sha2::{Digest, Sha256};

mod common;

const TOKEN_VAR: &str = "YIDAM_SERVE_TOKEN";

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn contract_dir() -> PathBuf {
    repo_root().join("yidam/prelude/sdks/parity/mcp")
}

use common::git::git;

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
///
/// `stderr` is the reason this struct exists rather than a tuple. The banner is read from the
/// child's stderr to learn the port; dropping that reader would close the read end of the pipe,
/// and the child's next write to stderr would then fail. `eprintln!` panics when it does, so a
/// closed pipe here does not lose a log line — it kills the server, and the next request gets
/// ECONNRESET with nothing anywhere saying why. The thread keeps the pipe open and drained for
/// as long as the server is alive.
struct Server {
    child: Child,
    addr: String,
    /// Every stderr line before the address, which is everything said before the endpoint
    /// served: the bound line, the corpus banner, and the bind warning when there is one.
    banner: String,
    stderr: Option<std::thread::JoinHandle<()>>,
}

impl Drop for Server {
    fn drop(&mut self) {
        // Kill first, then join. Killing closes the child's stderr, which is what ends the
        // draining thread's read; joining is what stops nextest reporting the test as `leaky`,
        // which is its name for a handle still open when the test returned.
        let _ = self.child.kill();
        let _ = self.child.wait();
        if let Some(t) = self.stderr.take() {
            let _ = t.join();
        }
    }
}

fn start(repo: &Path, extra: &[&str]) -> Server {
    start_with_token(repo, extra, None)
}

/// [`start`], with `YIDAM_SERVE_TOKEN` set to `token` — or removed, so a token in the
/// environment running the suite cannot turn every other test here into a 401.
fn start_with_token(repo: &Path, extra: &[&str], token: Option<&str>) -> Server {
    let mut command = Command::new(env!("CARGO_BIN_EXE_yidam"));
    match token {
        Some(t) => command.env(TOKEN_VAR, t),
        None => command.env_remove(TOKEN_VAR),
    };
    let mut child = command
        .current_dir(repo)
        .args(["serve", "--mcp", "--http", "--port", "0"])
        .args(extra)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary starts");

    // The banner names the bound address. Reading it is also how this waits for the corpus to
    // be loaded — the line comes after the load — and a sleep would be a race whichever number
    // was chosen.
    let stderr = child.stderr.take().expect("stderr is piped");
    let mut reader = BufReader::new(stderr);
    let mut addr = None;
    let mut banner = String::new();
    for _ in 0..40 {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 {
            break;
        }
        banner.push_str(&line);
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

    // Keep reading, and keep the pipe open. See the note on `Server::stderr`.
    let stderr = std::thread::spawn(move || {
        let mut sink = String::new();
        let _ = reader.read_to_string(&mut sink);
    });

    Server {
        child,
        addr,
        banner,
        stderr: Some(stderr),
    }
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
        let _ = write!(req, "{k}: {v}\r\n");
    }
    req.push_str("\r\n");
    req.push_str(body);
    stream.write_all(req.as_bytes()).unwrap();
    stream.flush().unwrap();

    // A strict read, deliberately. This was relaxed to tolerate ECONNRESET while the Linux
    // failure was undiagnosed; bisecting it in a container showed the relaxation fixed nothing
    // — the resets were the server dying, so the responses were absent rather than merely
    // reset-terminated. A test that accepts a truncated response for no measured benefit is a
    // test that catches less.
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

/// The banner's content arrives over HTTP, where the banner itself cannot (#424).
///
/// This is the whole issue in one assertion. `serve` finds its corpus by walking up from
/// wherever the process started; a server pointed at the wrong directory does not fail, it
/// answers every tool with nothing, and `docs/mcp-server.md` §5 tells a reader to check the
/// banner's domain and node count first. Over HTTP the server is on another machine and there
/// is no stderr to check — so the fact has to be in the protocol or it is gone.
#[test]
fn the_handshake_names_the_corpus_because_there_is_no_stderr_to_read() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);

    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}),
    );
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    let corpus = &v["result"]["capabilities"]["yidam"]["corpus"];

    assert!(
        corpus["nodes"].as_u64().unwrap_or(0) > 0,
        "the tell for a server that found the wrong directory is `nodes: 0`, and this fixture \
         has nodes: {corpus}"
    );
    assert!(corpus["domain"].is_string(), "{corpus}");
    assert!(corpus["commit"].is_string(), "{corpus}");

    // Present even where the value is null, so a client never has to tell a thin server from
    // an old one.
    for key in [
        "domain",
        "commit",
        "nodes",
        "skills",
        "decisions",
        "indexed_commit",
        "stale",
    ] {
        assert!(
            corpus.get(key).is_some(),
            "`corpus` is missing `{key}` over HTTP: {corpus}"
        );
    }

    // The fixture repository has a git checkout and no index, so this is the determinate
    // `false` rather than the `null` a projected mirror would send.
    assert_eq!(corpus["indexed_commit"], Value::Null, "{corpus}");
    assert_eq!(corpus["stale"], false, "{corpus}");
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

/// A bare IPv6 address binds, and answers (#1259).
///
/// `::1` is loopback by `is_loopback`'s own rule and named in `docs/mcp-server.md`, and before
/// #1259 the server could not bind it: `::1:0` is not a socket address. Skipped where the host
/// has no IPv6 loopback, which some CI containers do not.
#[test]
fn an_ipv6_loopback_bind_serves() {
    if std::net::TcpListener::bind("[::1]:0").is_err() {
        ci_report::skipped("this host has no IPv6 loopback");
        return;
    }
    let repo = fixture_repo();
    let server = start(repo.path(), &["--bind", "::1"]);
    assert!(
        server.addr.starts_with("[::1]:"),
        "bound {} rather than [::1]",
        server.addr
    );
    assert!(
        !server.banner.contains("not loopback"),
        "::1 is loopback and warned:\n{}",
        server.banner
    );
    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{}}),
    );
    assert_eq!(status, 200, "{body}");
}

/// A name is refused at the command, naming the address to write instead (#1259).
///
/// `output()` is safe here where `the_http_transport_refuses_a_non_corpus_before_it_binds`
/// says it is not, because the refusal precedes the corpus load as well as the bind — but a
/// regression is a server that starts, so it is bounded all the same.
#[test]
fn a_name_is_refused_before_it_binds() {
    let repo = fixture_repo();
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .env_remove(TOKEN_VAR)
        .current_dir(repo.path())
        .args([
            "serve",
            "--mcp",
            "--http",
            "--bind",
            "localhost",
            "--port",
            "0",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        match child.try_wait().unwrap() {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("`--bind localhost` started a server");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    assert!(!status.success());
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    assert!(
        err.contains("not an IP address") && err.contains("127.0.0.1"),
        "{err}"
    );
    assert!(!err.contains("http://"), "announced an address: {err}");
}

/// A bind every interface can reach is said aloud, naming what it publishes (#939).
///
/// The warning is printed before the endpoint serves, so it is in the banner `start` collected
/// on its way to the address. `retrieve` is the tool named because it is the one the issue was
/// raised about: semantic search over the whole corpus, to anyone on the network.
#[test]
fn a_bind_off_loopback_warns_before_it_listens() {
    let repo = fixture_repo();
    let server = start(repo.path(), &["--bind", "0.0.0.0"]);
    assert!(
        server.banner.contains("`--bind 0.0.0.0` is not loopback"),
        "no warning for a bind every interface can reach:\n{}",
        server.banner
    );
    assert!(
        server.banner.contains("retrieve"),
        "the warning does not name what is exposed:\n{}",
        server.banner
    );
}

/// The converse: the default bind is loopback, and says nothing about exposure.
#[test]
fn the_default_bind_does_not_warn() {
    let repo = fixture_repo();
    let server = start(repo.path(), &[]);
    assert!(
        !server.banner.contains("not loopback"),
        "a loopback bind warned:\n{}",
        server.banner
    );
}

/// With a token set, a request without it is a 401 that names its scheme, and one with it is
/// served (#939).
#[test]
fn a_token_is_required_on_every_request_and_the_right_one_is_served() {
    let repo = fixture_repo();
    let server = start_with_token(repo.path(), &[], Some("s3cret"));
    assert!(
        server.banner.contains("auth: bearer token required"),
        "{}",
        server.banner
    );
    assert!(
        !server.banner.contains("s3cret"),
        "the banner printed the token:\n{}",
        server.banner
    );

    let init = json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}).to_string();
    for headers in [
        vec![],
        vec![("authorization", "Bearer wrong")],
        vec![("authorization", "Basic s3cret")],
    ] {
        let response = request(&server.addr, "POST", "/mcp", &headers, &init);
        assert_eq!(status_of(&response), 401, "{headers:?}:\n{response}");
        assert!(
            response
                .to_ascii_lowercase()
                .contains("www-authenticate: bearer realm=\"yidam\""),
            "a 401 must name its scheme:\n{response}"
        );
        assert!(body_of(&response).starts_with("unauthorized"), "{response}");
    }
    // Every path, not only the endpoint: a caller without the token learns nothing about it.
    let response = request(&server.addr, "GET", "/elsewhere", &[], "");
    assert_eq!(status_of(&response), 401, "{response}");

    let response = request(
        &server.addr,
        "POST",
        "/mcp",
        &[
            ("authorization", "Bearer s3cret"),
            ("content-type", "application/json"),
        ],
        &init,
    );
    assert_eq!(status_of(&response), 200, "{response}");
    let v: Value = serde_json::from_str(body_of(&response)).unwrap();
    assert!(
        v["result"]["capabilities"].is_object(),
        "initialize did not answer: {v}"
    );
}

/// A token is the repair the bind warning names, so with one set off loopback the banner says
/// auth is required and does not warn.
#[test]
fn a_token_off_loopback_says_auth_and_does_not_warn() {
    let repo = fixture_repo();
    let server = start_with_token(repo.path(), &["--bind", "0.0.0.0"], Some("s3cret"));
    assert!(
        server.banner.contains("auth: bearer token required"),
        "{}",
        server.banner
    );
    assert!(!server.banner.contains("not loopback"), "{}", server.banner);
}

/// An empty token is not "auth off". The server refuses to start rather than serving open.
///
/// Bounded, for the reason `an_act_declaring_server_refuses_a_non_loopback_bind` gives: the
/// failure this catches is a server that starts, which never exits on its own.
#[test]
fn an_empty_token_refuses_to_start() {
    let repo = fixture_repo();
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .env(TOKEN_VAR, "")
        .current_dir(repo.path())
        .args(["serve", "--mcp", "--http", "--port", "0"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(20);
    let status = loop {
        match child.try_wait().unwrap() {
            Some(status) => break status,
            None if std::time::Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                panic!("the server started with an empty {TOKEN_VAR} and was still serving");
            }
            None => std::thread::sleep(std::time::Duration::from_millis(50)),
        }
    };
    assert!(!status.success());
    let mut err = String::new();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_string(&mut err)
        .unwrap();
    assert!(err.contains(TOKEN_VAR) && err.contains("empty"), "{err}");
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

/// Over HTTP the refusal has to happen before the socket is bound, because there is no stderr
/// for the client to read (#549, and #424's reason).
///
/// The stdio half of this is pinned in `mcp_serve.rs`. It is asserted again here because the
/// two transports are two entry points, they *were* two places that both forgot the check, and
/// the consequence differs: on stdio the person who misconfigured the client is at a terminal
/// and can see the banner, while an HTTP client gets a handshake and nothing else. A server
/// that bound a port and served a fabricated domain over it is reachable by anyone who can
/// reach the port.
///
/// The assertion is that **nothing is listening**, not merely that the process failed. A
/// process that exits after binding leaves a window. The corpus itself loads after the bind,
/// so the probes answer during it (#1238); whether `root` is a corpus at all is decided before.
#[test]
fn the_http_transport_refuses_a_non_corpus_before_it_binds() {
    let tmp = tempfile::TempDir::new().unwrap();
    let root = tmp.path();
    std::fs::write(root.join("README.md"), "a repository\n").unwrap();
    git(root, &["init", "-q"]);
    git(root, &["config", "user.email", "t@example.com"]);
    git(root, &["config", "user.name", "t"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-qm", "genesis: not a corpus"]);

    // A fixed port, so that "nothing is listening" is a question with an address. `--port 0`
    // would make the refusal untestable: there would be no port to fail to connect to.
    let port = "8797";
    let mut child = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(["serve", "--mcp", "--http", "--port", port])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    // **Not `output()`.** A serving process never exits, so waiting for one turns the very
    // regression this test exists for into a hang — which is not a failing test, it is a job
    // that burns its timeout and reports nothing. Found by mutating the fix out and watching
    // this file hang instead of go red. Poll, then kill and fail on our own terms.
    let exit = {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
        loop {
            match child.try_wait().expect("polling the child") {
                Some(status) => break Some(status),
                None if std::time::Instant::now() >= deadline => {
                    let _ = child.kill();
                    break None;
                }
                None => std::thread::sleep(std::time::Duration::from_millis(50)),
            }
        }
    };
    let out = child.wait_with_output().expect("collecting output");

    let Some(status) = exit else {
        panic!(
            "the HTTP transport was still serving after 10s in a directory with no .yidam/ \
             — it bound a port and would have answered a fabricated domain over it. \
             stderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        )
    };
    assert!(
        !status.success(),
        "the HTTP transport started in a directory with no .yidam/ and exited {:?}",
        status.code()
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("not a yidam repository"),
        "the refusal does not say what is wrong:\n{stderr}"
    );
    assert!(
        !stderr.contains("http://"),
        "the server announced an address it should never have bound:\n{stderr}"
    );
    assert!(
        TcpStream::connect(format!("127.0.0.1:{port}")).is_err(),
        "something is listening on {port} after the refusal"
    );
}

// ── probes and bundles (#1238) ────────────────────────────────────────────────

/// A kubelet's request: no Origin, no token, and GET or HEAD.
///
/// The server requires a token here, so this is also the assertion that the probes are exempt
/// from it — and that the endpoint beside them is not.
#[test]
fn the_probes_answer_a_caller_with_no_origin_and_no_token() {
    let repo = fixture_repo();
    let server = start_with_token(repo.path(), &[], Some("s3cret"));

    for (path, body) in [("/healthz", "live"), ("/readyz", "ready")] {
        let response = request(&server.addr, "GET", path, &[], "");
        assert_eq!(status_of(&response), 200, "{path}:\n{response}");
        assert_eq!(body_of(&response).trim(), body, "{path}");

        let response = request(&server.addr, "HEAD", path, &[], "");
        assert_eq!(status_of(&response), 200, "HEAD {path}:\n{response}");

        // A browser's Origin does not change the answer: there is nothing in it to protect.
        let response = request(
            &server.addr,
            "GET",
            path,
            &[("origin", "http://evil.test")],
            "",
        );
        assert_eq!(
            status_of(&response),
            200,
            "{path} with an Origin:\n{response}"
        );
    }

    let response = request(&server.addr, "POST", "/readyz", &[], "");
    assert_eq!(
        status_of(&response),
        401,
        "a POST is not a probe:\n{response}"
    );
    let (status, _) = rpc(
        &server.addr,
        json!({"jsonrpc": "2.0", "id": 1, "method": "tools/list", "params": {}}),
    );
    assert_eq!(status, 401, "the endpoint lost its token");
}

/// `examples/streamflow` as a repository, bundled. The bytes `yidam bundle` wrote.
fn streamflow_bundle() -> Vec<u8> {
    const EXAMPLE: &str = "examples/streamflow/";
    let root = common::repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in common::tracked_under(&root, EXAMPLE) {
        let to = dir.path().join(tracked.strip_prefix(EXAMPLE).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    let git = |args: &[&str]| common::git::git_at(dir.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "t@example.com"]);
    git(&["config", "user.name", "t"]);
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "genesis: streamflow"]);
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir.path())
        .arg("bundle")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    std::fs::read(dir.path().join(".yidam/bundle.yiz")).unwrap()
}

/// A file vault holding `bytes` under `digest`, at the path `FileStore` reads it from.
fn file_vault(digest: &str, bytes: &[u8]) -> tempfile::TempDir {
    let vault = tempfile::tempdir().unwrap();
    let dir = vault.path().join("sha256").join(&digest[..2]);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join(digest), bytes).unwrap();
    vault
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            let _ = write!(s, "{b:02x}");
            s
        })
}

/// The deployment's shape: an exported bundle, pinned by digest in a vault, served from a
/// directory that is not a corpus and answering a `query` over HTTP.
#[test]
fn a_streamflow_bundle_pinned_by_digest_answers_a_query() {
    let bytes = streamflow_bundle();
    let digest = hex(&bytes);
    let vault = file_vault(&digest, &bytes);
    let url = format!("file://{}", vault.path().display());
    // Started somewhere with no corpus, so nothing it answers can have come from the cwd.
    let cwd = tempfile::tempdir().unwrap();
    let server = start(cwd.path(), &["--bundle", &digest, "--vault-url", &url]);

    let response = request(&server.addr, "GET", "/readyz", &[], "");
    assert_eq!(status_of(&response), 200, "{response}");

    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}}),
    );
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    let yidam = &v["result"]["capabilities"]["yidam"];
    // What the bundle says it is: scratch has no genesis commit to name the domain and no git
    // to name the commit.
    assert_eq!(yidam["corpus"]["domain"], "streamflow", "{yidam}");
    assert!(
        yidam["corpus"]["commit"]
            .as_str()
            .is_some_and(|c| c.len() >= 7 && c.bytes().all(|b| b.is_ascii_hexdigit())),
        "the commit is not the bundle's: {yidam}"
    );
    assert_eq!(yidam["act"], false, "a bundle is read-only by construction");

    let (status, body) = rpc(
        &server.addr,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/call",
               "params": {"name": "query", "arguments": {"query": "*", "limit": 1000}}}),
    );
    assert_eq!(status, 200, "{body}");
    let v: Value = serde_json::from_str(&body).unwrap();
    assert_ne!(v["result"]["isError"], true, "{v}");
    let text = v["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or_default();
    assert!(
        text.contains("canyon-outlet"),
        "the query did not answer from the bundle: {text}"
    );
}

/// A vault that answers a digest with other bytes is not served as the pin.
#[test]
fn a_bundle_whose_bytes_are_not_its_digest_is_refused() {
    let bytes = streamflow_bundle();
    let digest = hex(b"some other bundle");
    let vault = file_vault(&digest, &bytes);
    let url = format!("file://{}", vault.path().display());
    let cwd = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .env_remove(TOKEN_VAR)
        .current_dir(cwd.path())
        .args(["serve", "--mcp", "--http", "--port", "0"])
        .args(["--bundle", &digest, "--vault-url", &url])
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("refusing to serve"), "{stderr}");
    assert!(!stderr.contains("http://"), "{stderr}");
}
