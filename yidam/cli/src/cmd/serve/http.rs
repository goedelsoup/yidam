//! `yidam serve --mcp --http` — the same contract over a transport a URL can reach.
//!
//! Everything a platform in #420 reaches is reached by a URL, and none of them can spawn a
//! subprocess on someone else's machine. `--mcp` alone is stdio, so the corpus was reachable
//! only by someone who could already run a binary inside the checkout.
//!
//! # What this is not
//!
//! Not a second contract. [`super::handle`] is the seam RFC-0005 left for exactly this — it
//! takes a method and params and returns a result, and knows nothing about framing. `run_loop`
//! frames it as newline-delimited JSON on stdio; this frames it as HTTP. The payloads are
//! identical by construction, which is what makes the parity cases in
//! `prelude/sdks/parity/mcp/` answer for both transports without being run twice.
//!
//! # Why hyper directly and not a framework
//!
//! `hyper` 1.x and `hyper-util` are already in the default dependency tree — `reqwest` pulls
//! them for its client — so turning on their server features costs one crate to compile
//! (`httpdate`, already pinned in `Cargo.lock`) and no new entry in the audited set. `axum`
//! measured at five. Neither is expensive; hyper is what is already here, and what is written
//! on top of it is a match on method and path, not an HTTP implementation.
//!
//! # The policy is pure, and that is deliberate
//!
//! Everything that decides whether a request is served — [`vet`], [`classify`] — takes plain
//! values and returns an [`Outcome`]. None of it needs a socket, so all of it is tested in this
//! file's own unit tests rather than behind an integration harness that binds a port.

use std::convert::Infallible;
use std::io::Write;
use std::net::SocketAddr;

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderValue, CONTENT_TYPE};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};

use super::ServerState;

/// The one endpoint path. The spec asks for a single one, so there is a single one; a request
/// to any other path is told which it wanted rather than 404'd blankly.
pub(crate) const ENDPOINT: &str = "/mcp";

/// How much of a refused request's body is read before closing, so the close is a FIN and not
/// an RST. 64 KiB — larger than any legitimate JSON-RPC call this server takes, and small
/// enough that reading it costs an unwelcome caller more than it costs the server.
const DRAIN_LIMIT: usize = 64 * 1024;

/// Why a request was refused, frozen the way the MCP contract's own reason strings are.
///
/// A token rather than a sentence because a client acts on it: `origin-not-allowed` is a
/// deployment fix and `no-sse-stream` is not a fix at all — it is the server saying it has no
/// server-initiated messages to stream, which the spec allows in as many words. A value outside
/// this set is a divergence; something needing a new one should add it here first.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Refusal {
    /// The `Origin` header names a site that was not allowed. 403.
    OriginNotAllowed,
    /// `MCP-Protocol-Version` was not a protocol version. 400.
    MalformedProtocolVersion,
    /// GET: this server offers no server-initiated SSE stream. 405, and the spec's own answer.
    NoSseStream,
    /// DELETE: this server assigns no session, so there is none to end. 405.
    NoSessionToDelete,
    /// Any other method on the endpoint. 405.
    MethodNotAllowed,
    /// A path that is not the endpoint. 404.
    UnknownEndpoint,
}

impl Refusal {
    /// The typed status, not a `u16`. `StatusCode::from_u16` is fallible and every call to it
    /// here would be an `expect` on a constant — a panic path in production code to express
    /// something the type system already knows.
    pub(crate) fn status(self) -> StatusCode {
        match self {
            Refusal::OriginNotAllowed => StatusCode::FORBIDDEN,
            Refusal::MalformedProtocolVersion => StatusCode::BAD_REQUEST,
            Refusal::NoSseStream | Refusal::NoSessionToDelete | Refusal::MethodNotAllowed => {
                StatusCode::METHOD_NOT_ALLOWED
            }
            Refusal::UnknownEndpoint => StatusCode::NOT_FOUND,
        }
    }

    /// The frozen token, and then a sentence saying what to do about it.
    pub(crate) fn token(self) -> &'static str {
        match self {
            Refusal::OriginNotAllowed => "origin-not-allowed",
            Refusal::MalformedProtocolVersion => "malformed-protocol-version",
            Refusal::NoSseStream => "no-sse-stream",
            Refusal::NoSessionToDelete => "no-session-to-delete",
            Refusal::MethodNotAllowed => "method-not-allowed",
            Refusal::UnknownEndpoint => "unknown-endpoint",
        }
    }

    pub(crate) fn message(self) -> String {
        match self {
            Refusal::OriginNotAllowed => format!(
                "{}: this request carried an Origin this server was not started with. Pass \
                 `--allow-origin <url>` for each browser origin that may reach it.",
                self.token()
            ),
            Refusal::MalformedProtocolVersion => format!(
                "{}: MCP-Protocol-Version must be a dated version such as 2025-06-18.",
                self.token()
            ),
            Refusal::NoSseStream => format!(
                "{}: this server sends no messages a client did not ask for, so it opens no \
                 event stream. POST {ENDPOINT} instead.",
                self.token()
            ),
            Refusal::NoSessionToDelete => format!(
                "{}: this server assigns no Mcp-Session-Id, so there is no session to end.",
                self.token()
            ),
            Refusal::MethodNotAllowed => format!(
                "{}: {ENDPOINT} answers POST, carrying one JSON-RPC message as its body.",
                self.token()
            ),
            Refusal::UnknownEndpoint => {
                format!("{}: the MCP endpoint is {ENDPOINT}.", self.token())
            }
        }
    }
}

/// Whether a well-formed JSON-RPC message expects an answer.
///
/// Two variants and not three: a *refusal* is [`vet`]'s answer and is decided before a body is
/// read at all, so folding it in here would make one type model two different moments.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Outcome {
    /// A JSON-RPC request: dispatch it and answer with one JSON object.
    Answer,
    /// A JSON-RPC notification or response: 202, no body. The spec requires exactly this.
    Accepted,
}

/// Whether an `Origin` may reach this server.
///
/// **The absence of a header is not an origin.** `curl`, the OpenAI Responses API and every
/// other server-to-server client sends none, and refusing those would make the transport
/// unusable without a flag while defending against nothing: the attack the spec's MUST is
/// about is DNS rebinding, where a *browser* on some other site is made to talk to a server
/// bound to localhost — and a browser always sends `Origin`.
///
/// So: no header passes, and a header must be named. That is the whole rule.
pub(crate) fn origin_allowed(origin: Option<&str>, allowed: &[String]) -> bool {
    match origin {
        None => true,
        Some(o) => allowed.iter().any(|a| a == o),
    }
}

/// Whether a `MCP-Protocol-Version` header is well-formed.
///
/// Shape only, and the reason is worth stating because a stricter check reads as the safer
/// one. This server's payloads do not vary by protocol version — they are frozen in
/// `prelude/sdks/parity/mcp/tools.json`, and `initialize` echoes whatever version it was asked
/// for rather than negotiating one. So there is no well-formed version it *cannot* serve, and
/// a hardcoded list of the three that exist today would refuse the fourth on the day it ships,
/// asserting an incompatibility that does not exist.
///
/// What is left to reject is a header that is not a version at all, which is the other half of
/// the spec's "invalid or unsupported".
pub(crate) fn protocol_version_well_formed(v: &str) -> bool {
    let mut parts = v.split('-');
    let (Some(y), Some(m), Some(d), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    y.len() == 4
        && m.len() == 2
        && d.len() == 2
        && [y, m, d]
            .iter()
            .all(|p| p.bytes().all(|b| b.is_ascii_digit()))
}

/// The transport's decision about one request, before the body is parsed.
pub(crate) fn vet(
    method: &Method,
    path: &str,
    origin: Option<&str>,
    protocol_version: Option<&str>,
    allowed_origins: &[String],
) -> Option<Refusal> {
    // Origin first: it is the check that must not be reachable around, so it runs before the
    // request is classified at all — including on the paths that are refused anyway.
    if !origin_allowed(origin, allowed_origins) {
        return Some(Refusal::OriginNotAllowed);
    }
    if let Some(v) = protocol_version {
        if !protocol_version_well_formed(v) {
            return Some(Refusal::MalformedProtocolVersion);
        }
    }
    if path != ENDPOINT {
        return Some(Refusal::UnknownEndpoint);
    }
    match *method {
        Method::POST => None,
        Method::GET => Some(Refusal::NoSseStream),
        Method::DELETE => Some(Refusal::NoSessionToDelete),
        _ => Some(Refusal::MethodNotAllowed),
    }
}

/// Whether a JSON-RPC message expects an answer.
///
/// The same rule `run_loop` applies on stdio, and deliberately the same code shape: a message
/// with a non-null `id` is a request, and everything else is a notification or a response that
/// this server has nothing to say back to.
pub(crate) fn classify(msg: &Value) -> Outcome {
    match msg.get("id").filter(|v| !v.is_null()) {
        Some(_) => Outcome::Answer,
        None => Outcome::Accepted,
    }
}

/// Dispatch one JSON-RPC request body and render the JSON-RPC response.
///
/// Identical in every respect to the stdio loop's arm, because it calls the same [`super::handle`].
fn answer(state: &ServerState, msg: &Value) -> Value {
    let id = msg.get("id").cloned().unwrap_or(Value::Null);
    let method = msg["method"].as_str().unwrap_or("");
    let params = msg.get("params").cloned().unwrap_or_else(|| json!({}));
    match super::handle(state, method, &params) {
        Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
        Err(e) => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {"code": e.code, "message": e.message}
        }),
    }
}

/// One response constructor, and no fallible step in it.
///
/// `Response::builder()` returns a `Result` because a header name or value can be invalid, so
/// every use of it here would end in an `expect` over a literal. Setting the parts directly with
/// `HeaderValue::from_static` moves that check to compile time and leaves no panic path in a
/// network-facing loop.
fn respond(status: StatusCode, content_type: &'static str, body: Bytes) -> Response<Full<Bytes>> {
    let mut response = Response::new(Full::new(body));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
    response
}

fn text(status: StatusCode, body: String) -> Response<Full<Bytes>> {
    respond(status, "text/plain; charset=utf-8", Bytes::from(body))
}

fn json_response(status: StatusCode, value: &Value) -> Response<Full<Bytes>> {
    respond(status, "application/json", Bytes::from(value.to_string()))
}

/// Serve one HTTP request. The only function here that touches hyper types.
async fn serve_one(
    state: &ServerState,
    allowed_origins: &[String],
    req: Request<hyper::body::Incoming>,
) -> Response<Full<Bytes>> {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let origin = header("origin");
    let version = header("mcp-protocol-version");
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    if let Some(refusal) = vet(
        &method,
        &path,
        origin.as_deref(),
        version.as_deref(),
        allowed_origins,
    ) {
        // Read the body before answering, even though the answer does not depend on it.
        //
        // A server that responds to a `Connection: close` request without consuming the body
        // closes a socket with unread data in its receive queue, and Linux answers that with
        // an RST rather than a FIN — so a client that has the whole response still sees
        // "connection reset by peer" on its last read. This is why nginx has
        // `lingering_close`. The refusal is already decided; draining only makes the goodbye
        // graceful.
        //
        // Measured, and stated precisely because it is NOT what fixed the Linux test failure
        // this was first written for: bisected in a container, this change alone took that
        // failure from three to two. It removes real resets; it was not the cause.
        //
        // Bounded, because this runs before any check that the caller is welcome: hyper's
        // `Limited` stops reading past the cap, and a body larger than that is an oversized
        // request being refused, which has no claim on politeness.
        let _ = http_body_util::Limited::new(req.into_body(), DRAIN_LIMIT)
            .collect()
            .await;
        return text(refusal.status(), refusal.message());
    }

    let body = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(e) => return text(StatusCode::BAD_REQUEST, format!("body-unreadable: {e}")),
    };
    let msg: Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => {
            // A parse error is a JSON-RPC concern, not an HTTP one: the request arrived
            // intact, so it is answered with the same -32700 the stdio loop sends.
            return json_response(
                StatusCode::OK,
                &json!({
                    "jsonrpc": "2.0", "id": null,
                    "error": {"code": -32700, "message": format!("parse error: {e}")}
                }),
            );
        }
    };

    match classify(&msg) {
        Outcome::Answer => json_response(StatusCode::OK, &answer(state, &msg)),
        // 202 with no body, which the spec requires in those words for a notification or a
        // response. The stdio loop's equivalent is writing nothing at all.
        Outcome::Accepted => {
            let mut response = Response::new(Full::new(Bytes::new()));
            *response.status_mut() = StatusCode::ACCEPTED;
            response
        }
    }
}

/// Serve MCP over HTTP until the process is stopped.
///
/// # One thread, and not by accident
///
/// [`ServerState`] is not `Sync` in every build: under `vector-read` the retrieval state holds
/// the embedder and its space verdict in a [`std::cell::RefCell`], lazily initialised on the
/// first query. So the connection tasks are spawned onto a [`tokio::task::LocalSet`] with
/// [`std::rc::Rc`], which needs neither `Send` nor `Sync`, rather than `tokio::spawn` with
/// `Arc`, which needs both.
///
/// That is not a detail to discover later. `tokio::spawn` **compiles in the light default
/// build**, where no `RefCell` is in the state, and fails only under `--features vector-read`
/// — a build no pull request compiles (`ci (cli · full features)` is `main`-push only). The
/// alternative to a `LocalSet` is a lock on the embedder, which buys parallelism this server
/// has no use for: the work is JSON dispatch over an in-memory corpus, and connections
/// interleave on one thread perfectly well.
pub(crate) fn serve(
    state: ServerState,
    bind: &str,
    port: u16,
    allow_origin: Vec<String>,
) -> Result<()> {
    use std::rc::Rc;

    let addr: SocketAddr = format!("{bind}:{port}")
        .parse()
        .with_context(|| format!("`{bind}:{port}` is not an address to bind"))?;

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_io()
        .build()?;
    let local = tokio::task::LocalSet::new();

    local.block_on(&runtime, async move {
        let state = Rc::new(state);
        let allowed = Rc::new(allow_origin);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("cannot bind {addr}"))?;

        // The address the socket actually got, not the one that was asked for. They differ
        // exactly when `--port 0` was passed, which is how a caller says "any free port" —
        // and a caller who does that has no other way to learn the answer.
        let bound = listener.local_addr().unwrap_or(addr);

        // stderr, not stdout: an HTTP client has no stderr to read, but a person running the
        // command in a terminal does, and stdout carries no protocol here to pollute. #424 is
        // the issue for the connect-time facts a remote client cannot see at all.
        eprintln!("yidam MCP over HTTP on http://{bound}{ENDPOINT}");
        if allowed.is_empty() {
            eprintln!(
                "  no --allow-origin: a request carrying an Origin header will be refused, \
                 which is every browser and no server-to-server client"
            );
        }

        loop {
            let (stream, _peer) = listener.accept().await?;
            let state = Rc::clone(&state);
            let allowed = Rc::clone(&allowed);
            tokio::task::spawn_local(async move {
                let service = service_fn(move |req| {
                    let state = Rc::clone(&state);
                    let allowed = Rc::clone(&allowed);
                    async move { Ok::<_, Infallible>(serve_one(&state, &allowed, req).await) }
                });
                // A connection that fails is that client's problem, not the server's: report
                // it and keep serving, or one malformed request ends the process.
                //
                // `writeln!` and not `eprintln!`, and the result deliberately dropped. A
                // client can provoke this line, and `eprintln!` PANICS if the write fails —
                // so with stderr closed or a full pipe, a request from outside could take the
                // server down through its logging. A server whose log can kill it is worse
                // than one that loses a log line.
                if let Err(e) = hyper::server::conn::http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .await
                {
                    let _ = writeln!(std::io::stderr(), "connection error: {e}");
                }
            });
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn origins(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // ── Origin ────────────────────────────────────────────────────────────────

    /// A server-to-server client sends no Origin, and must not be refused for it.
    ///
    /// The OpenAI Responses API and `curl` are both in this case. Refusing them would make
    /// `--http` unusable without a flag while defending against nothing, because the attack
    /// the check exists for is a browser one and a browser always sends the header.
    #[test]
    fn a_request_with_no_origin_is_not_a_cross_origin_request() {
        assert!(origin_allowed(None, &[]));
        assert!(origin_allowed(None, &origins(&["https://chat.example"])));
    }

    /// An unnamed browser origin is refused by default, which is the whole DNS-rebinding rule.
    #[test]
    fn an_origin_nobody_allowed_is_refused() {
        assert!(!origin_allowed(Some("http://evil.test"), &[]));
        assert!(!origin_allowed(
            Some("http://evil.test"),
            &origins(&["https://chat.example"])
        ));
        assert!(origin_allowed(
            Some("https://chat.example"),
            &origins(&["https://chat.example"])
        ));
    }

    /// The origin check runs before anything else, including on requests refused anyway.
    ///
    /// Otherwise the reason a caller is told depends on which other thing was also wrong, and
    /// a probe could learn the endpoint path by comparing 404 against 403.
    #[test]
    fn origin_is_checked_before_the_path_or_the_method() {
        let allowed = origins(&["https://chat.example"]);
        for (method, path) in [
            (Method::POST, ENDPOINT),
            (Method::GET, ENDPOINT),
            (Method::POST, "/somewhere-else"),
            (Method::PUT, "/somewhere-else"),
        ] {
            assert_eq!(
                vet(&method, path, Some("http://evil.test"), None, &allowed),
                Some(Refusal::OriginNotAllowed),
                "{method} {path} leaked a different refusal to a disallowed origin"
            );
        }
    }

    // ── method and path ───────────────────────────────────────────────────────

    /// GET is answered 405, which the spec names as the alternative to opening a stream.
    #[test]
    fn get_is_refused_because_there_is_no_stream_to_open() {
        assert_eq!(
            vet(&Method::GET, ENDPOINT, None, None, &[]),
            Some(Refusal::NoSseStream)
        );
    }

    /// DELETE ends a session, and this server assigns none.
    #[test]
    fn delete_is_refused_because_there_is_no_session() {
        assert_eq!(
            vet(&Method::DELETE, ENDPOINT, None, None, &[]),
            Some(Refusal::NoSessionToDelete)
        );
    }

    #[test]
    fn post_to_the_endpoint_is_the_one_thing_that_proceeds() {
        assert_eq!(vet(&Method::POST, ENDPOINT, None, None, &[]), None);
    }

    #[test]
    fn another_path_is_told_which_one_it_wanted() {
        assert_eq!(
            vet(&Method::POST, "/", None, None, &[]),
            Some(Refusal::UnknownEndpoint)
        );
        assert!(Refusal::UnknownEndpoint.message().contains(ENDPOINT));
    }

    // ── protocol version ──────────────────────────────────────────────────────

    /// A version this server has never heard of is still served.
    ///
    /// The payloads are frozen in `tools.json` and do not vary by protocol version, so there
    /// is no well-formed version this server cannot answer. A hardcoded list of the three
    /// that exist today would refuse the fourth on the day it ships.
    #[test]
    fn a_future_protocol_version_is_not_an_unsupported_one() {
        for v in ["2024-11-05", "2025-03-26", "2025-06-18", "2031-01-01"] {
            assert!(protocol_version_well_formed(v), "{v} rejected");
            assert_eq!(vet(&Method::POST, ENDPOINT, None, Some(v), &[]), None);
        }
    }

    #[test]
    fn a_header_that_is_not_a_version_is_refused() {
        for v in [
            "",
            "latest",
            "2025",
            "2025-06",
            "2025-6-18",
            "2025-06-18-1",
            "x025-06-18",
        ] {
            assert!(!protocol_version_well_formed(v), "{v} accepted");
            assert_eq!(
                vet(&Method::POST, ENDPOINT, None, Some(v), &[]),
                Some(Refusal::MalformedProtocolVersion),
                "{v} was served"
            );
        }
    }

    /// An absent header is not a malformed one — the spec says to assume 2025-03-26.
    #[test]
    fn an_absent_protocol_version_is_not_a_refusal() {
        assert_eq!(vet(&Method::POST, ENDPOINT, None, None, &[]), None);
    }

    // ── request classification ────────────────────────────────────────────────

    /// The same rule the stdio loop applies, so the two transports cannot come to disagree
    /// about what deserves an answer.
    #[test]
    fn a_request_is_answered_and_a_notification_is_only_accepted() {
        assert_eq!(
            classify(&json!({"jsonrpc":"2.0","id":1,"method":"ping"})),
            Outcome::Answer
        );
        assert_eq!(
            classify(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})),
            Outcome::Accepted
        );
        assert_eq!(
            classify(&json!({"jsonrpc":"2.0","id":null,"method":"ping"})),
            Outcome::Accepted
        );
    }

    // ── the reason vocabulary ─────────────────────────────────────────────────

    /// Every refusal has its own token and its own status, and says what to do next.
    ///
    /// Two refusals sharing a token would be two different repairs a client cannot tell
    /// apart, which is the failure `degraded_reason` exists to prevent one layer up.
    #[test]
    fn the_refusal_tokens_are_distinct_and_each_says_its_repair() {
        let all = [
            Refusal::OriginNotAllowed,
            Refusal::MalformedProtocolVersion,
            Refusal::NoSseStream,
            Refusal::NoSessionToDelete,
            Refusal::MethodNotAllowed,
            Refusal::UnknownEndpoint,
        ];
        let mut seen = std::collections::BTreeSet::new();
        for r in all {
            assert!(seen.insert(r.token()), "duplicate token: {}", r.token());
            assert!(
                r.message().starts_with(r.token()),
                "{} does not lead with its token",
                r.token()
            );
            assert!(
                r.message().len() > r.token().len() + 20,
                "{} states no repair",
                r.token()
            );
            assert!(r.status().is_client_error(), "{}", r.token());
        }
    }

    // ── the seam ──────────────────────────────────────────────────────────────

    /// Both transports answer from the same `handle`, so a tool answers identically over each.
    ///
    /// This is the property RFC-0005 froze payloads rather than framing for. If it ever fails,
    /// the parity cases in `prelude/sdks/parity/mcp/` stop answering for the HTTP surface and
    /// would have to be run twice.
    #[test]
    fn the_http_answer_is_the_stdio_answer() {
        let state = super::super::tests::test_state();
        let msg = json!({"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}});
        let over_http = answer(&state, &msg);

        let direct = super::super::handle(&state, "tools/list", &json!({})).unwrap();
        assert_eq!(over_http["result"], direct);
        assert_eq!(over_http["id"], 7);
        assert_eq!(over_http["jsonrpc"], "2.0");
    }
}
