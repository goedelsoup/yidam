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
//! `yidam/sdks/parity/mcp/` answer for both transports without being run twice.
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
//! Everything that decides whether a request is served — [`vet`], [`authorized`], [`classify`] — takes plain
//! values and returns an [`Outcome`]. None of it needs a socket, so all of it is tested in this
//! file's own unit tests rather than behind an integration harness that binds a port.
//!
//! # Two probes, and nothing from the corpus in either (#1238)
//!
//! A kubelet asks two questions a person never does: is the process up, and may traffic be
//! sent to it. [`HEALTHZ`] answers the first and [`READYZ`] the second, so the socket is bound
//! *before* the corpus loads — a probe asked during a slow load gets "loading" rather than a
//! refused connection, which a kubelet cannot tell from a dead process. Until the load
//! finishes, [`ENDPOINT`] answers 503 as well.
//!
//! They are not MCP. They carry no JSON-RPC, are absent from `tools.json`, and are answered
//! before [`vet`], so neither `--allow-origin` nor a token applies to them: a kubelet sends no
//! `Origin` and holds no token, and what the two answer says nothing about the corpus — not
//! its domain, its size or its commit — so there is nothing for either check to protect.

use std::cell::{OnceCell, RefCell};
use std::convert::Infallible;
use std::io::Write;
use std::net::{IpAddr, SocketAddr};

use anyhow::{Context, Result};
use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::header::{HeaderValue, AUTHORIZATION, CONTENT_TYPE, WWW_AUTHENTICATE};
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use super::ServerState;

/// The one endpoint path. The spec asks for a single one, so there is a single one; a request
/// to any other path is told which it wanted rather than 404'd blankly.
pub(crate) const ENDPOINT: &str = "/mcp";

/// Liveness: the process is up and its accept loop is running.
pub(crate) const HEALTHZ: &str = "/healthz";

/// Readiness: the corpus is loaded and a trivial read of it answers.
pub(crate) const READYZ: &str = "/readyz";

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
    /// A token is configured and this request did not carry it. 401.
    Unauthorized,
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
            Self::OriginNotAllowed => StatusCode::FORBIDDEN,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::MalformedProtocolVersion => StatusCode::BAD_REQUEST,
            Self::NoSseStream | Self::NoSessionToDelete | Self::MethodNotAllowed => {
                StatusCode::METHOD_NOT_ALLOWED
            }
            Self::UnknownEndpoint => StatusCode::NOT_FOUND,
        }
    }

    /// The frozen token, and then a sentence saying what to do about it.
    pub(crate) fn token(self) -> &'static str {
        match self {
            Self::OriginNotAllowed => "origin-not-allowed",
            Self::Unauthorized => "unauthorized",
            Self::MalformedProtocolVersion => "malformed-protocol-version",
            Self::NoSseStream => "no-sse-stream",
            Self::NoSessionToDelete => "no-session-to-delete",
            Self::MethodNotAllowed => "method-not-allowed",
            Self::UnknownEndpoint => "unknown-endpoint",
        }
    }

    pub(crate) fn message(self) -> String {
        match self {
            Self::OriginNotAllowed => format!(
                "{}: this request carried an Origin this server was not started with. Pass \
                 `--allow-origin <url>` for each browser origin that may reach it.",
                self.token()
            ),
            // Names where the token comes from and never what it is: the caller who lacks it
            // learns which header to send, and the operator learns which setting to look up.
            Self::Unauthorized => format!(
                "{}: this server requires `Authorization: Bearer <token>`, carrying the token \
                 its operator set in {TOKEN_VAR} or `--token-file`.",
                self.token()
            ),
            Self::MalformedProtocolVersion => format!(
                "{}: MCP-Protocol-Version must be a dated version such as 2025-06-18.",
                self.token()
            ),
            Self::NoSseStream => format!(
                "{}: this server sends no messages a client did not ask for, so it opens no \
                 event stream. POST {ENDPOINT} instead.",
                self.token()
            ),
            Self::NoSessionToDelete => format!(
                "{}: this server assigns no Mcp-Session-Id, so there is no session to end.",
                self.token()
            ),
            Self::MethodNotAllowed => format!(
                "{}: {ENDPOINT} answers POST, carrying one JSON-RPC message as its body.",
                self.token()
            ),
            Self::UnknownEndpoint => {
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
/// `yidam/sdks/parity/mcp/tools.json`, and `initialize` echoes whatever version it was asked
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

/// The environment variable a bearer token is read from (#939).
pub(crate) const TOKEN_VAR: &str = "YIDAM_SERVE_TOKEN";

/// A token's SHA-256. What the server keeps instead of the token itself.
pub(crate) type TokenDigest = [u8; 32];

fn digest(token: &str) -> TokenDigest {
    Sha256::digest(token.as_bytes()).into()
}

/// The bearer token this server requires, or `None` when it requires none (#939).
///
/// From [`TOKEN_VAR`] or from `--token-file`, and never from a literal flag: argv is in `ps`
/// and in shell history, and an environment variable and a file are both places an operator
/// already keeps a secret. `env` is a parameter so this is testable without mutating the
/// process environment.
///
/// **Both at once is refused**, not resolved by precedence. Either one alone is a complete
/// answer, so a second one is a setting somebody forgot, and whichever lost would be the one
/// its author believed was in force.
///
/// **An empty token is refused**, not read as "off". `YIDAM_SERVE_TOKEN=` is what an unset
/// secret in a template expands to, and a server that started open from it would be the
/// failure this exists to prevent, delivered silently.
///
/// Surrounding whitespace is trimmed, which is the trailing newline `echo > file` writes. A
/// token that cannot travel in a header — a space or a control byte inside it — is refused,
/// because no client could ever send it and every request would be a 401.
pub(crate) fn token_source(
    env: impl Fn(&str) -> Option<String>,
    token_file: Option<&std::path::Path>,
) -> Result<Option<TokenDigest>> {
    let from_env = env(TOKEN_VAR);
    let (raw, origin) = match (from_env, token_file) {
        (None, None) => return Ok(None),
        (Some(_), Some(path)) => anyhow::bail!(
            "{TOKEN_VAR} is set and `--token-file {}` was passed. Use one: either alone names \
             the token, and with both, one of them is a setting someone believes is in force.",
            path.display()
        ),
        (Some(v), None) => (v, TOKEN_VAR.to_string()),
        (None, Some(path)) => (
            std::fs::read_to_string(path)
                .with_context(|| format!("cannot read `--token-file {}`", path.display()))?,
            format!("`--token-file {}`", path.display()),
        ),
    };
    let token = raw.trim();
    if token.is_empty() {
        anyhow::bail!(
            "{origin} is empty. An empty token is not \"no token\": unset it to serve without \
             authentication, or set it to a secret such as `openssl rand -hex 32` prints."
        );
    }
    if !token.bytes().all(|b| b.is_ascii_graphic()) {
        anyhow::bail!(
            "{origin} holds a space or a non-printing character, which no client can send in \
             an `Authorization` header. Use printable ASCII, such as `openssl rand -hex 32` \
             prints."
        );
    }
    Ok(Some(digest(token)))
}

/// Whether a request's `Authorization` header carries the configured token.
///
/// With no token configured, everything passes and the header is not read: a server started
/// without one behaves exactly as it did before #939.
///
/// The scheme is matched without regard to case, as RFC 7235 asks. The comparison is between
/// SHA-256 digests, which are always 32 bytes, so its time says nothing about the token's
/// length; and every byte is compared, so it says nothing about where they first differ.
pub(crate) fn authorized(authorization: Option<&str>, required: Option<&TokenDigest>) -> bool {
    let Some(required) = required else {
        return true;
    };
    let Some((scheme, token)) = authorization.and_then(|h| h.trim().split_once(' ')) else {
        return false;
    };
    if !scheme.eq_ignore_ascii_case("bearer") {
        return false;
    }
    let offered = digest(token.trim());
    offered
        .iter()
        .zip(required.iter())
        .fold(0u8, |acc, (a, b)| acc | (a ^ b))
        == 0
}

/// What this server admits, set once at startup: the browser origins it names and the token
/// it requires. One value rather than two arguments, so a third check does not grow [`vet`]'s
/// signature again.
#[derive(Debug, Default)]
pub(crate) struct Policy {
    pub(crate) origins: Vec<String>,
    pub(crate) token: Option<TokenDigest>,
}

/// The address `--bind` names, or a refusal saying which to write instead (#1259).
///
/// An IP address and nothing else, and the two halves of that are separate reasons.
///
/// **IPv6 is parsed, not formatted.** Gluing `{bind}:{port}` into one string and parsing a
/// `SocketAddr` from it cannot work for a bare IPv6 address, which needs brackets to carry a
/// port — so `::1` and `::` both failed. The port is joined with [`SocketAddr::new`] instead,
/// and a bracketed `[::1]` is accepted too, since that is how a URL spells it.
///
/// **A name is refused, not resolved.** [`super::is_loopback`] decides what a bind may serve:
/// `act` only on loopback, and the warning off it. It can only judge what it is shown, and a
/// name resolves to whatever the resolver says at start-up — `localhost` included, which is an
/// `/etc/hosts` line and not a guarantee. Refusing names keeps the address the gate judged and
/// the address the socket bound the same address.
pub(crate) fn bind_address(bind: &str) -> Result<IpAddr> {
    let bare = bind
        .strip_prefix('[')
        .and_then(|b| b.strip_suffix(']'))
        .unwrap_or(bind);
    bare.parse::<IpAddr>().map_err(|_| {
        anyhow::anyhow!(
            "`--bind {bind}` is not an IP address. `--bind` takes an address, not a name, so \
             the loopback rule judges the address that is bound: 127.0.0.1 or ::1 for this \
             machine, 0.0.0.0 or :: for every interface."
        )
    })
}

/// Which question a probe asks.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Probe {
    /// [`HEALTHZ`].
    Live,
    /// [`READYZ`].
    Ready,
}

/// What a request is, decided before its body is read.
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub(crate) enum Admission {
    /// A probe, answered without any other check — see the module header.
    Probe(Probe),
    /// Refused, for the reason given.
    Refused(Refusal),
    /// A JSON-RPC message for [`ENDPOINT`].
    Mcp,
}

/// Admit one request: a probe first, and then everything [`vet`] decides.
///
/// A probe is a GET or a HEAD and nothing else, so a POST to [`READYZ`] is a request for an
/// endpoint that does not exist and meets every check that request would.
pub(crate) fn admit(
    method: &Method,
    path: &str,
    origin: Option<&str>,
    authorization: Option<&str>,
    protocol_version: Option<&str>,
    policy: &Policy,
) -> Admission {
    if matches!(*method, Method::GET | Method::HEAD) {
        match path {
            HEALTHZ => return Admission::Probe(Probe::Live),
            READYZ => return Admission::Probe(Probe::Ready),
            _ => {}
        }
    }
    match vet(
        method,
        path,
        origin,
        authorization,
        protocol_version,
        policy,
    ) {
        Some(refusal) => Admission::Refused(refusal),
        None => Admission::Mcp,
    }
}

/// The answer to a probe: a status and a body naming no corpus content.
///
/// `state` is `None` until the load finishes. Ready means more than loaded: the summary
/// resource is computed from the loaded corpus, and only its success reaches the caller —
/// never the summary itself.
pub(crate) fn probe_answer(
    probe: Probe,
    state: Option<&ServerState>,
) -> (StatusCode, &'static str) {
    match (probe, state) {
        (Probe::Live, _) => (StatusCode::OK, "live\n"),
        (Probe::Ready, None) => (StatusCode::SERVICE_UNAVAILABLE, LOADING),
        (Probe::Ready, Some(state)) => {
            match super::resources::read(state, "yidam://graph/summary") {
                Ok(_) => (StatusCode::OK, "ready\n"),
                Err(_) => (
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unready: the corpus loaded and a read of it failed\n",
                ),
            }
        }
    }
}

/// What [`READYZ`] and [`ENDPOINT`] both answer while the corpus loads. A 503 and not a
/// [`Refusal`]: nothing about the request is wrong, and the same request will be served once
/// the load is done.
const LOADING: &str = "loading: the corpus is not loaded yet; /readyz answers 200 once it is\n";

/// The transport's decision about one request, before the body is parsed.
pub(crate) fn vet(
    method: &Method,
    path: &str,
    origin: Option<&str>,
    authorization: Option<&str>,
    protocol_version: Option<&str>,
    policy: &Policy,
) -> Option<Refusal> {
    // Origin first: it is the check that must not be reachable around, so it runs before the
    // request is classified at all — including on the paths that are refused anyway.
    if !origin_allowed(origin, &policy.origins) {
        return Some(Refusal::OriginNotAllowed);
    }
    // Then the token, before anything that would tell a caller about the endpoint. A caller
    // without it gets the same 401 on every path and method, so it cannot learn which path
    // exists by comparing a 404 against a 405.
    if !authorized(authorization, policy.token.as_ref()) {
        return Some(Refusal::Unauthorized);
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
fn answer(state: &mut ServerState, msg: &Value) -> Value {
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

/// Read and discard a refused request's body, before answering it.
///
/// A server that responds to a `Connection: close` request without consuming the body
/// closes a socket with unread data in its receive queue, and Linux answers that with an RST
/// rather than a FIN — so a client that has the whole response still sees "connection reset
/// by peer" on its last read. This is why nginx has `lingering_close`. The refusal is already
/// decided; draining only makes the goodbye graceful.
///
/// Measured, and stated precisely because it is NOT what fixed the Linux test failure this
/// was first written for: bisected in a container, this change alone took that failure from
/// three to two. It removes real resets; it was not the cause.
///
/// Bounded, because this runs before any check that the caller is welcome: hyper's `Limited`
/// stops reading past the cap, and a body larger than that is an oversized request being
/// refused, which has no claim on politeness.
async fn drain(req: Request<hyper::body::Incoming>) {
    let _ = http_body_util::Limited::new(req.into_body(), DRAIN_LIMIT)
        .collect()
        .await;
}

/// Serve one HTTP request. The only function here that touches hyper types.
///
/// `state` is empty until the load finishes, and is set once.
async fn serve_one(
    state: &OnceCell<RefCell<ServerState>>,
    policy: &Policy,
    req: Request<hyper::body::Incoming>,
) -> Response<Full<Bytes>> {
    let header = |name: &str| {
        req.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
    };
    let origin = header("origin");
    let authorization = header(AUTHORIZATION.as_str());
    let version = header("mcp-protocol-version");
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    match admit(
        &method,
        &path,
        origin.as_deref(),
        authorization.as_deref(),
        version.as_deref(),
        policy,
    ) {
        Admission::Probe(probe) => {
            // The borrow ends inside the call, which is synchronous; see `Outcome::Answer`.
            let (status, body) = probe_answer(probe, state.get().map(|s| s.borrow()).as_deref());
            return text(status, body.to_string());
        }
        Admission::Refused(refusal) => {
            drain(req).await;
            let mut response = text(refusal.status(), refusal.message());
            // A 401 must name its scheme (RFC 7235 §3.1). No `resource_metadata`: this server
            // serves no OAuth metadata, and pointing a client at a document that 404s would
            // start a discovery it cannot finish. #427 is where that changes.
            if refusal == Refusal::Unauthorized {
                response.headers_mut().insert(
                    WWW_AUTHENTICATE,
                    HeaderValue::from_static("Bearer realm=\"yidam\""),
                );
            }
            return response;
        }
        Admission::Mcp => {}
    }
    // After the policy, so a caller the policy refuses learns nothing — not even that the
    // server is still loading.
    let Some(state) = state.get() else {
        drain(req).await;
        return text(StatusCode::SERVICE_UNAVAILABLE, LOADING.to_string());
    };

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
        // The borrow opens and closes inside this expression, and `answer` is synchronous —
        // so no `RefMut` is ever held across an `.await`, which is the one way a `RefCell`
        // behind a shared `Rc` panics on a second connection. The reload RFC-0029 requires
        // is what makes the state mutable at all; see `ServerState::reload`.
        Outcome::Answer => json_response(StatusCode::OK, &answer(&mut state.borrow_mut(), &msg)),
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
/// The socket is bound first and the corpus loaded after, by `load` on a blocking thread, so
/// [`HEALTHZ`] answers from the moment the port is held. `announce` runs once the load
/// finishes, before the address line, which is where the banner and the exposure warning
/// belong: both describe the loaded corpus. A load that fails ends the process, with the
/// load's own error.
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
/// — a build no pull request compiled until #922 added `ci (cli · feature check)`, which
/// clippies that feature set and `--all-features` on every pull request. The
/// alternative to a `LocalSet` is a lock on the embedder, which buys parallelism this server
/// has no use for: the work is JSON dispatch over an in-memory corpus, and connections
/// interleave on one thread perfectly well.
///
/// The load is the one thing that leaves the thread, and it can: the state is `Send` — it is
/// `Sync` it is not — and it crosses exactly once, before any connection task can see it.
pub(crate) fn serve(
    ip: IpAddr,
    port: u16,
    policy: Policy,
    load: impl FnOnce() -> Result<ServerState> + Send + 'static,
    announce: impl FnOnce(&ServerState),
) -> Result<()> {
    use std::rc::Rc;

    let addr = SocketAddr::new(ip, port);

    // The crate's one runtime (#930), not a private one. The `LocalSet` runs on this thread
    // inside its `block_on`, which is where the `Rc`s above need it to; a `retrieve` over
    // `[index.remote]` from a connection task reaches the S3 Vectors transport, whose
    // `block_on` knows it is inside the runtime and does not panic on it.
    let local = tokio::task::LocalSet::new();
    crate::runtime::block_on_local(local.run_until(async move {
        let state: Rc<OnceCell<RefCell<ServerState>>> = Rc::new(OnceCell::new());
        let policy = Rc::new(policy);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .with_context(|| format!("cannot bind {addr}"))?;

        // The address the socket actually got, not the one that was asked for. They differ
        // exactly when `--port 0` was passed, which is how a caller says "any free port" —
        // and a caller who does that has no other way to learn the answer.
        let bound = listener.local_addr().unwrap_or(addr);

        // No `http://` in this line: the address line below is the one that says the MCP
        // endpoint is served, and it is not yet.
        eprintln!("yidam: bound {bound}, loading the corpus; {HEALTHZ} answers now");

        let mut loading = tokio::task::spawn_blocking(load);
        let mut announce = Some(announce);
        loop {
            tokio::select! {
                loaded = &mut loading, if state.get().is_none() => {
                    let loaded = loaded.context("loading the corpus panicked")??;
                    if let Some(announce) = announce.take() {
                        announce(&loaded);
                    }
                    // Set once, here, and never again: the cell is empty only until this line.
                    let _ = state.set(RefCell::new(loaded));
                    // stderr, not stdout: an HTTP client has no stderr to read, but a person
                    // running the command in a terminal does, and stdout carries no protocol
                    // here to pollute. #424 is the issue for the connect-time facts a remote
                    // client cannot see at all.
                    eprintln!("yidam MCP over HTTP on http://{bound}{ENDPOINT}");
                    if policy.origins.is_empty() {
                        eprintln!(
                            "  no --allow-origin: a request carrying an Origin header will be \
                             refused, which is every browser and no server-to-server client"
                        );
                    }
                }
                accepted = listener.accept() => {
                    let (stream, _peer) = accepted?;
                    let state = Rc::clone(&state);
                    let policy = Rc::clone(&policy);
                    tokio::task::spawn_local(async move {
                        let service = service_fn(move |req| {
                            let state = Rc::clone(&state);
                            let policy = Rc::clone(&policy);
                            async move {
                                Ok::<_, Infallible>(serve_one(&state, &policy, req).await)
                            }
                        });
                        // A connection that fails is that client's problem, not the server's:
                        // report it and keep serving, or one malformed request ends the
                        // process.
                        //
                        // `writeln!` and not `eprintln!`, and the result deliberately dropped.
                        // A client can provoke this line, and `eprintln!` PANICS if the write
                        // fails — so with stderr closed or a full pipe, a request from outside
                        // could take the server down through its logging. A server whose log
                        // can kill it is worse than one that loses a log line.
                        if let Err(e) = hyper::server::conn::http1::Builder::new()
                            .serve_connection(TokioIo::new(stream), service)
                            .await
                        {
                            let _ = writeln!(std::io::stderr(), "connection error: {e}");
                        }
                    });
                }
            }
        }
    }))
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
        let allowed = Policy {
            origins: origins(&["https://chat.example"]),
            token: Some(digest("s3cret")),
        };
        for (method, path) in [
            (Method::POST, ENDPOINT),
            (Method::GET, ENDPOINT),
            (Method::POST, "/somewhere-else"),
            (Method::PUT, "/somewhere-else"),
        ] {
            assert_eq!(
                vet(
                    &method,
                    path,
                    Some("http://evil.test"),
                    None,
                    None,
                    &allowed
                ),
                Some(Refusal::OriginNotAllowed),
                "{method} {path} leaked a different refusal to a disallowed origin, even one \
                 that would also have been unauthorized"
            );
        }
    }

    // ── method and path ───────────────────────────────────────────────────────

    /// GET is answered 405, which the spec names as the alternative to opening a stream.
    #[test]
    fn get_is_refused_because_there_is_no_stream_to_open() {
        assert_eq!(
            vet(&Method::GET, ENDPOINT, None, None, None, &Policy::default()),
            Some(Refusal::NoSseStream)
        );
    }

    /// DELETE ends a session, and this server assigns none.
    #[test]
    fn delete_is_refused_because_there_is_no_session() {
        assert_eq!(
            vet(
                &Method::DELETE,
                ENDPOINT,
                None,
                None,
                None,
                &Policy::default()
            ),
            Some(Refusal::NoSessionToDelete)
        );
    }

    #[test]
    fn post_to_the_endpoint_is_the_one_thing_that_proceeds() {
        assert_eq!(
            vet(
                &Method::POST,
                ENDPOINT,
                None,
                None,
                None,
                &Policy::default()
            ),
            None
        );
    }

    #[test]
    fn another_path_is_told_which_one_it_wanted() {
        assert_eq!(
            vet(&Method::POST, "/", None, None, None, &Policy::default()),
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
            assert_eq!(
                vet(
                    &Method::POST,
                    ENDPOINT,
                    None,
                    None,
                    Some(v),
                    &Policy::default()
                ),
                None
            );
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
                vet(
                    &Method::POST,
                    ENDPOINT,
                    None,
                    None,
                    Some(v),
                    &Policy::default()
                ),
                Some(Refusal::MalformedProtocolVersion),
                "{v} was served"
            );
        }
    }

    /// An absent header is not a malformed one — the spec says to assume 2025-03-26.
    #[test]
    fn an_absent_protocol_version_is_not_a_refusal() {
        assert_eq!(
            vet(
                &Method::POST,
                ENDPOINT,
                None,
                None,
                None,
                &Policy::default()
            ),
            None
        );
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
        // The list is held to the enum by an exhaustive match: a variant added later fails to
        // compile here until it is listed, rather than going untested without a failure.
        let all = [
            Refusal::OriginNotAllowed,
            Refusal::Unauthorized,
            Refusal::MalformedProtocolVersion,
            Refusal::NoSseStream,
            Refusal::NoSessionToDelete,
            Refusal::MethodNotAllowed,
            Refusal::UnknownEndpoint,
        ];
        for r in all {
            match r {
                Refusal::OriginNotAllowed
                | Refusal::Unauthorized
                | Refusal::MalformedProtocolVersion
                | Refusal::NoSseStream
                | Refusal::NoSessionToDelete
                | Refusal::MethodNotAllowed
                | Refusal::UnknownEndpoint => {}
            }
        }
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

    // ── bearer token ──────────────────────────────────────────────────────────

    fn with_token(token: &str) -> Policy {
        Policy {
            origins: vec![],
            token: Some(digest(token)),
        }
    }

    /// A missing header, a wrong token and another scheme are refused; the right token is not.
    #[test]
    fn only_the_configured_token_is_authorized() {
        let required = digest("s3cret");
        assert!(authorized(Some("Bearer s3cret"), Some(&required)));
        for header in [
            None,
            Some(""),
            Some("Bearer"),
            Some("Bearer "),
            Some("Bearer s3cre"),
            Some("Bearer s3cret2"),
            Some("Basic s3cret"),
            Some("s3cret"),
        ] {
            assert!(
                !authorized(header, Some(&required)),
                "{header:?} was authorized"
            );
        }
    }

    /// RFC 7235: the scheme is case-insensitive. The token is not.
    #[test]
    fn the_scheme_is_matched_without_case_and_the_token_with_it() {
        let required = digest("s3cret");
        for header in ["bearer s3cret", "BEARER s3cret", "Bearer  s3cret "] {
            assert!(authorized(Some(header), Some(&required)), "{header}");
        }
        assert!(!authorized(Some("Bearer S3CRET"), Some(&required)));
    }

    /// With no token configured the header is never read: a server started without one
    /// behaves exactly as it did before #939.
    #[test]
    fn with_no_token_configured_any_header_passes() {
        for header in [None, Some("Bearer anything"), Some("garbage")] {
            assert!(authorized(header, None), "{header:?}");
            assert_eq!(
                vet(
                    &Method::POST,
                    ENDPOINT,
                    None,
                    header,
                    None,
                    &Policy::default()
                ),
                None
            );
        }
    }

    /// Auth runs before the version, path and method checks, so a caller without the token
    /// gets one answer everywhere and learns nothing about the endpoint.
    #[test]
    fn auth_is_checked_before_the_version_the_path_or_the_method() {
        let policy = with_token("s3cret");
        for (method, path, version) in [
            (Method::POST, ENDPOINT, None),
            (Method::GET, ENDPOINT, None),
            (Method::DELETE, ENDPOINT, None),
            (Method::PUT, ENDPOINT, None),
            (Method::POST, "/somewhere-else", None),
            (Method::POST, ENDPOINT, Some("latest")),
        ] {
            assert_eq!(
                vet(&method, path, None, None, version, &policy),
                Some(Refusal::Unauthorized),
                "{method} {path} {version:?} answered something other than 401 without a token"
            );
        }
        assert_eq!(
            vet(
                &Method::POST,
                ENDPOINT,
                None,
                Some("Bearer s3cret"),
                None,
                &policy
            ),
            None
        );
        assert_eq!(
            vet(
                &Method::GET,
                ENDPOINT,
                None,
                Some("Bearer s3cret"),
                None,
                &policy
            ),
            Some(Refusal::NoSseStream),
            "with the token, the other checks still run"
        );
    }

    /// The 401's message names where the token comes from, and the token itself never appears.
    #[test]
    fn the_401_names_the_setting_and_not_the_secret() {
        let m = Refusal::Unauthorized.message();
        assert!(m.contains(TOKEN_VAR) && m.contains("--token-file"), "{m}");
        assert_eq!(Refusal::Unauthorized.status(), StatusCode::UNAUTHORIZED);
    }

    // ── where the token comes from ────────────────────────────────────────────

    fn env_with(value: Option<&str>) -> impl Fn(&str) -> Option<String> + '_ {
        move |k| {
            (k == TOKEN_VAR)
                .then(|| value.map(str::to_string))
                .flatten()
        }
    }

    #[test]
    fn no_source_means_no_token() {
        assert_eq!(token_source(env_with(None), None).unwrap(), None);
    }

    #[test]
    fn the_variable_is_read_and_trimmed() {
        assert_eq!(
            token_source(env_with(Some("s3cret\n")), None).unwrap(),
            Some(digest("s3cret"))
        );
    }

    /// `echo s3cret > file` writes a trailing newline, and it is not part of the token.
    #[test]
    fn a_trailing_newline_in_the_file_is_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "s3cret\n").unwrap();
        assert_eq!(
            token_source(env_with(None), Some(&path)).unwrap(),
            Some(digest("s3cret"))
        );
    }

    /// An empty value is not "auth off": it is refused, in both sources.
    #[test]
    fn an_empty_token_is_refused_rather_than_read_as_off() {
        for value in ["", "   ", "\n"] {
            let err = token_source(env_with(Some(value)), None).unwrap_err();
            assert!(err.to_string().contains("is empty"), "{value:?}: {err}");
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "\n").unwrap();
        let err = token_source(env_with(None), Some(&path)).unwrap_err();
        assert!(err.to_string().contains("is empty"), "{err}");
    }

    /// Two sources is a setting someone forgot, and neither silently wins.
    #[test]
    fn both_sources_at_once_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "other").unwrap();
        let err = token_source(env_with(Some("s3cret")), Some(&path)).unwrap_err();
        assert!(err.to_string().contains("Use one"), "{err}");
    }

    /// A token no client can put in a header would make every request a 401.
    #[test]
    fn a_token_no_header_can_carry_is_refused() {
        for value in ["two words", "tab\there", "caf\u{e9}"] {
            assert!(
                token_source(env_with(Some(value)), None).is_err(),
                "{value:?} accepted"
            );
        }
    }

    #[test]
    fn a_missing_token_file_is_an_error_not_no_token() {
        let err =
            token_source(env_with(None), Some(std::path::Path::new("/nonexistent/t"))).unwrap_err();
        assert!(err.to_string().contains("cannot read"), "{err}");
    }

    // ── the bind address ──────────────────────────────────────────────────────

    /// Every IPv6 spelling binds, which `"{bind}:{port}".parse()` refused (#1259).
    #[test]
    fn an_ipv6_bind_is_an_address_and_takes_a_port() {
        for (bind, want) in [
            ("127.0.0.1", "127.0.0.1:8787"),
            ("0.0.0.0", "0.0.0.0:8787"),
            ("::1", "[::1]:8787"),
            ("::", "[::]:8787"),
            ("[::1]", "[::1]:8787"),
            ("fe80::1", "[fe80::1]:8787"),
        ] {
            let ip = bind_address(bind).unwrap_or_else(|e| panic!("{bind}: {e}"));
            assert_eq!(SocketAddr::new(ip, 8787).to_string(), want, "{bind}");
        }
    }

    /// A name is refused, and the refusal names the addresses to write instead.
    #[test]
    fn a_name_is_refused_and_told_the_address_to_use() {
        for bind in ["localhost", "yidam.example", "", "127.0.0.1:8787", "[::1"] {
            let err = bind_address(bind).unwrap_err().to_string();
            assert!(
                err.contains("127.0.0.1") && err.contains("::1"),
                "{bind}: {err}"
            );
        }
    }

    // ── probes (#1238) ────────────────────────────────────────────────────────

    /// A kubelet sends no Origin and holds no token, and its probes are answered anyway.
    #[test]
    fn a_probe_needs_no_origin_and_no_token() {
        let strict = Policy {
            origins: origins(&["https://chat.example"]),
            token: Some(digest("s3cret")),
        };
        for method in [Method::GET, Method::HEAD] {
            for (path, probe) in [(HEALTHZ, Probe::Live), (READYZ, Probe::Ready)] {
                for origin in [None, Some("http://evil.test")] {
                    assert_eq!(
                        admit(&method, path, origin, None, None, &strict),
                        Admission::Probe(probe),
                        "{method} {path} {origin:?}"
                    );
                }
            }
        }
        // The same request to the MCP endpoint is still refused: the exemption is the probes'.
        assert_eq!(
            admit(&Method::POST, ENDPOINT, None, None, None, &strict),
            Admission::Refused(Refusal::Unauthorized)
        );
    }

    /// A probe is a GET or a HEAD; anything else to its path meets every other check.
    #[test]
    fn a_post_to_a_probe_path_is_not_a_probe() {
        assert_eq!(
            admit(&Method::POST, READYZ, None, None, None, &Policy::default()),
            Admission::Refused(Refusal::UnknownEndpoint)
        );
        assert_eq!(
            admit(
                &Method::POST,
                HEALTHZ,
                None,
                None,
                None,
                &with_token("s3cret")
            ),
            Admission::Refused(Refusal::Unauthorized)
        );
    }

    /// Live from the start; ready only once the corpus is loaded and a read of it answers.
    #[test]
    fn ready_is_unready_before_the_load_and_ready_after_it() {
        assert_eq!(probe_answer(Probe::Live, None).0, StatusCode::OK);
        assert_eq!(
            probe_answer(Probe::Ready, None).0,
            StatusCode::SERVICE_UNAVAILABLE
        );

        let state = super::super::tests::test_state();
        assert_eq!(probe_answer(Probe::Live, Some(&state)).0, StatusCode::OK);
        let (status, body) = probe_answer(Probe::Ready, Some(&state));
        assert_eq!(status, StatusCode::OK, "{body}");
    }

    /// No probe answer carries anything from the corpus: they are the two answers that skip
    /// every check, so they must have nothing to give away.
    #[test]
    fn a_probe_answer_names_nothing_in_the_corpus() {
        let state = super::super::tests::test_state();
        for probe in [Probe::Live, Probe::Ready] {
            for loaded in [None, Some(&state)] {
                let (_, body) = probe_answer(probe, loaded);
                for leak in [state.domain.as_str(), state.commit.as_str()] {
                    assert!(!body.contains(leak), "{probe:?} answered {body:?}");
                }
            }
        }
    }

    // ── the seam ──────────────────────────────────────────────────────────────

    /// Both transports answer from the same `handle`, so a tool answers identically over each.
    ///
    /// This is the property RFC-0005 froze payloads rather than framing for. If it ever fails,
    /// the parity cases in `yidam/sdks/parity/mcp/` stop answering for the HTTP surface and
    /// would have to be run twice.
    #[test]
    fn the_http_answer_is_the_stdio_answer() {
        let mut state = super::super::tests::test_state();
        let msg = json!({"jsonrpc":"2.0","id":7,"method":"tools/list","params":{}});
        let over_http = answer(&mut state, &msg);

        let direct = super::super::handle(&mut state, "tools/list", &json!({})).unwrap();
        assert_eq!(over_http["result"], direct);
        assert_eq!(over_http["id"], 7);
        assert_eq!(over_http["jsonrpc"], "2.0");
    }
}
