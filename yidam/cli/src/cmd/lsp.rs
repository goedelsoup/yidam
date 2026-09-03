//! `yidam serve --lsp` — the checks and the graph, spoken over LSP.
//!
//! # Why this exists when a VS Code extension already does most of it
//!
//! Two reasons, and the second is the larger one.
//!
//! **Rename.** `textDocument/rename` is [`crate::cmd::rename`]'s natural trigger: F2 on a
//! node, every inbound `target:` rewritten in one transaction, refused outright if anything
//! would dangle. That is a judgement, and RFC-0016 puts judgements on this side of the
//! process boundary.
//!
//! **Neovim and Helix.** `yidam schema --settings` has named them as targets since before any
//! editor surface existed. They have no extension and are not going to get one; over LSP they
//! get diagnostics, navigation, and rename without anybody writing a line for them.
//!
//! # Live, not on-save
//!
//! The checks read the working tree, which is right for a gate and wrong for an editor: the
//! file you are typing into is the one whose findings you want, and it is the one on disk
//! that is stale. [`Overlay`] closes that — every check reads through it without knowing it
//! exists, so the diagnostics are about the buffer and are still computed by the same
//! functions `yidam lint` runs.
//!
//! # Not gated behind `index`
//!
//! An LSP that required the ML stack would be one nobody could install, so this one never
//! did. `serve --mcp` has since joined it in the light default — only the *semantic*
//! retrieval path inside it still needs fastembed, lancedb and protoc. Neither transport is
//! gated any more; see `cmd::serve::vector`.

use anyhow::Result;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use super::graph::{graph_data, GraphReport};
use super::lint::{run_checks_with, Options, Overlay};
use super::rename;
use crate::paths::yidam_corpus_dir;

/// LSP `DiagnosticSeverity`.
const ERROR: u8 = 1;
const WARNING: u8 = 2;
const INFORMATION: u8 = 3;
const HINT: u8 = 4;

struct Server {
    root: PathBuf,
    corpus: PathBuf,
    overlay: Overlay,
    /// Open buffers by URI. The overlay is keyed by path; this keeps the reverse.
    docs: HashMap<String, String>,
    /// URIs we last published diagnostics for, so they can be cleared when they go clean.
    /// Without it a finding that is fixed stays on screen until the client is restarted.
    published: HashSet<String>,
    graph: Option<GraphReport>,
    shutdown: bool,
}

// ── uris ─────────────────────────────────────────────────────────────────────

/// `file:///a/b%20c.yml` → `/a/b c.yml`.
///
/// Hand-rolled rather than a `url` dependency: the only scheme an editor sends here is
/// `file:`, and the only escaping that appears in a corpus path in practice is `%20`.
pub(crate) fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    let mut out = String::with_capacity(rest.len());
    let bytes = rest.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(byte) = u8::from_str_radix(&rest[i + 1..i + 3], 16) {
                out.push(byte as char);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    Some(PathBuf::from(out))
}

pub(crate) fn path_to_uri(path: &Path) -> String {
    let mut out = String::from("file://");
    for ch in path.to_string_lossy().chars() {
        match ch {
            ' ' => out.push_str("%20"),
            other => out.push(other),
        }
    }
    out
}

// ── framing ──────────────────────────────────────────────────────────────────

/// Read one `Content-Length`-framed message.
///
/// LSP frames differ from the MCP stdio transport's line-delimited JSON, which is why this
/// is not shared with `serve/mod.rs`: same idea, different wire.
pub(crate) fn read_message<R: BufRead>(input: &mut R) -> Option<Value> {
    let mut length: Option<usize> = None;
    loop {
        let mut line = String::new();
        if input.read_line(&mut line).ok()? == 0 {
            return None;
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if trimmed.is_empty() {
            break;
        }
        if let Some(v) = trimmed.strip_prefix("Content-Length:") {
            length = v.trim().parse().ok();
        }
    }
    let mut body = vec![0u8; length?];
    input.read_exact(&mut body).ok()?;
    serde_json::from_slice(&body).ok()
}

pub(crate) fn frame(value: &Value) -> String {
    let body = serde_json::to_string(value).unwrap_or_default();
    format!("Content-Length: {}\r\n\r\n{body}", body.len())
}

// ── mapping ──────────────────────────────────────────────────────────────────

/// RFC-0016's severity table, in one place.
///
/// **Baseline membership outranks check severity in both directions.** `yidam lint` does not
/// ask *is the corpus clean?* — it asks *did this change make it less clean?* — so inherited
/// debt is a Hint however severe the check is, and a fresh `info` is still only Information.
///
/// The VS Code extension carries the same table in TypeScript for its own providers, because
/// the alternative is an editor that cannot render a diagnostic without a subprocess per
/// keystroke. Two transcriptions of four rows, both pinned to
/// `prelude/sdks/parity/fixtures/diagnostic_severity/` — each was previously pinned only by
/// its own hand-written expectations, which leaves the two free to be independently right
/// about different tables.
pub(crate) fn severity_of(severity: &str, in_baseline: bool) -> u8 {
    if in_baseline {
        return HINT;
    }
    match severity {
        "error" => ERROR,
        "warn" => WARNING,
        _ => INFORMATION,
    }
}

/// A whole-line range, 0-based, from a 1-based line number.
fn line_range(line: usize) -> Value {
    let line = line.saturating_sub(1);
    json!({
        "start": {"line": line, "character": 0},
        "end": {"line": line, "character": 0},
    })
}

impl Server {
    fn new(root: PathBuf) -> Self {
        let corpus = yidam_corpus_dir(&root);
        Self {
            root,
            corpus,
            overlay: Overlay::default(),
            docs: HashMap::new(),
            published: HashSet::new(),
            graph: None,
            shutdown: false,
        }
    }

    fn text_of(&self, uri: &str) -> String {
        self.docs.get(uri).cloned().unwrap_or_default()
    }

    /// Corpus-relative id of a URI, or none when it is outside the corpus.
    fn node_id(&self, uri: &str) -> Option<String> {
        let path = uri_to_path(uri)?;
        let rel = path.strip_prefix(&self.corpus).ok()?;
        Some(rel.to_string_lossy().replace('\\', "/"))
    }

    fn graph(&mut self) -> &GraphReport {
        if self.graph.is_none() {
            self.graph = Some(graph_data(&self.root, &self.corpus));
        }
        self.graph.as_ref().expect("just built")
    }

    /// The `target:` value under a cursor, resolved to a corpus id.
    fn target_at(
        &self,
        uri: &str,
        line: usize,
        character: usize,
    ) -> Option<(String, usize, usize)> {
        let id = self.node_id(uri)?;
        let text = self.text_of(uri);
        let line_text = text.lines().nth(line)?;
        let (start, end, value) = rename::target_on(line_text)?;
        if character < start || character > end {
            return None;
        }
        let dir = Path::new(&id).parent().unwrap_or(Path::new(""));
        let resolved = rename::normalize(&dir.join(&value))
            .to_string_lossy()
            .replace('\\', "/");
        (!resolved.is_empty()).then_some((resolved, start, end))
    }

    /// Run every check through the overlay and publish, clearing what has gone clean.
    fn publish(&mut self, out: &mut dyn Write) {
        let checks = run_checks_with(&self.root, &Options::default(), &self.overlay);
        let baseline = super::lint::load_baseline(&self.root);
        let report = super::lint::build_report(&self.root, &checks, &baseline);

        let mut by_uri: HashMap<String, Vec<Value>> = HashMap::new();
        for check in &report.checks {
            for v in &check.violations {
                // `node` is repo-relative, and for three checks it carries `:line`.
                let path_part = v.node.rsplit_once(':').map_or(v.node.as_str(), |(p, _)| p);
                let path = self.root.join(path_part);
                let line = v.span.map(|s| s.line).unwrap_or(1);
                by_uri.entry(path_to_uri(&path)).or_default().push(json!({
                    "range": line_range(line),
                    "severity": severity_of(check.severity, v.in_baseline),
                    "code": check.id,
                    "source": if v.in_baseline { "yidam (baseline)" } else { "yidam" },
                    "message": format!("{}\n\n{}", v.detail, check.rationale),
                }));
            }
        }

        // Clear what was published before and no longer has findings. A finding that stays
        // on screen after it is fixed is worse than one that never appeared.
        for uri in self.published.clone() {
            by_uri.entry(uri).or_default();
        }
        self.published = by_uri.keys().cloned().collect();
        for (uri, diagnostics) in by_uri {
            let _ = write!(
                out,
                "{}",
                frame(&json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/publishDiagnostics",
                    "params": {"uri": uri, "diagnostics": diagnostics},
                }))
            );
        }
        let _ = out.flush();
    }

    fn handle(&mut self, message: &Value, out: &mut dyn Write) -> bool {
        let method = message["method"].as_str().unwrap_or("");
        let id = message.get("id").cloned();
        let params = &message["params"];

        let reply = |out: &mut dyn Write, id: Value, result: Value| {
            let _ = write!(
                out,
                "{}",
                frame(&json!({"jsonrpc": "2.0", "id": id, "result": result}))
            );
            let _ = out.flush();
        };

        match method {
            "initialize" => {
                if let Some(root) = params["rootUri"].as_str().and_then(uri_to_path) {
                    // The client's workspace wins over the cwd: an editor opened elsewhere
                    // still means the folder it was pointed at.
                    if root.join(".yidam").is_dir() {
                        *self = Server::new(root);
                    }
                }
                reply(out, id.unwrap_or(Value::Null), self.capabilities());
            }
            "shutdown" => {
                self.shutdown = true;
                reply(out, id.unwrap_or(Value::Null), Value::Null);
            }
            "exit" => return false,
            "textDocument/didOpen" | "textDocument/didChange" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let text = match method {
                    "textDocument/didOpen" => params["textDocument"]["text"].as_str().unwrap_or(""),
                    // Full sync only: the capability declares it, so a range change never
                    // arrives. Applying an incremental change wrongly is worse than not
                    // offering the mode.
                    _ => params["contentChanges"][0]["text"].as_str().unwrap_or(""),
                }
                .to_string();
                if let Some(path) = uri_to_path(&uri) {
                    self.overlay.set(path, text.clone());
                }
                self.docs.insert(uri, text);
                self.graph = None;
                self.publish(out);
            }
            "textDocument/didSave" => {
                self.graph = None;
                self.publish(out);
            }
            "textDocument/didClose" => {
                let uri = params["textDocument"]["uri"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                if let Some(path) = uri_to_path(&uri) {
                    self.overlay.clear(&path);
                }
                self.docs.remove(&uri);
                self.graph = None;
                self.publish(out);
            }
            "textDocument/definition" => {
                reply(out, id.unwrap_or(Value::Null), self.definition(params));
            }
            "textDocument/hover" => {
                reply(out, id.unwrap_or(Value::Null), self.hover(params));
            }
            "textDocument/references" => {
                reply(out, id.unwrap_or(Value::Null), self.references(params));
            }
            "textDocument/prepareRename" => {
                reply(out, id.unwrap_or(Value::Null), self.prepare_rename(params));
            }
            "textDocument/rename" => match self.rename(params) {
                Ok(edit) => reply(out, id.unwrap_or(Value::Null), edit),
                Err(message) => {
                    let _ = write!(
                        out,
                        "{}",
                        frame(&json!({
                            "jsonrpc": "2.0",
                            "id": id.unwrap_or(Value::Null),
                            // A refusal is an error, not an empty edit: an editor that
                            // showed "renamed" and changed nothing would be the worst
                            // outcome available.
                            "error": {"code": -32803, "message": message},
                        }))
                    );
                    let _ = out.flush();
                }
            },
            _ if id.is_some() => reply(out, id.unwrap(), Value::Null),
            _ => {}
        }
        true
    }

    fn capabilities(&self) -> Value {
        json!({
            "capabilities": {
                // 1 = full. See the didChange arm for why not 2.
                "textDocumentSync": {"openClose": true, "change": 1, "save": true},
                "definitionProvider": true,
                "referencesProvider": true,
                "hoverProvider": true,
                "renameProvider": {"prepareProvider": true},
            },
            "serverInfo": {"name": "yidam", "version": env!("CARGO_PKG_VERSION")},
        })
    }

    fn position(params: &Value) -> (String, usize, usize) {
        (
            params["textDocument"]["uri"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            params["position"]["line"].as_u64().unwrap_or(0) as usize,
            params["position"]["character"].as_u64().unwrap_or(0) as usize,
        )
    }

    /// Offered without an existence check, exactly as the extension does it.
    ///
    /// `exists` is the gate's answer. A second one here could disagree with `lint` about
    /// which edges are broken, and an editor that quietly declined to jump would be the
    /// least informative way to say so.
    fn definition(&mut self, params: &Value) -> Value {
        let (uri, line, character) = Self::position(params);
        let Some((target, _, _)) = self.target_at(&uri, line, character) else {
            return Value::Null;
        };
        json!({
            "uri": path_to_uri(&self.corpus.join(target)),
            "range": line_range(1),
        })
    }

    fn hover(&mut self, params: &Value) -> Value {
        let (uri, line, character) = Self::position(params);
        let Some((target, start, end)) = self.target_at(&uri, line, character) else {
            return Value::Null;
        };
        let graph = self.graph();
        let Some(node) = graph.nodes.iter().find(|n| n.node == target) else {
            return Value::Null;
        };
        let inbound = graph
            .nodes
            .iter()
            .flat_map(|n| n.links.iter())
            .filter(|l| l.resolved == target)
            .count();
        let text = format!(
            "**{}** — `{}` · {} out · {inbound} in\n\n{}",
            if node.label.is_empty() {
                &node.node
            } else {
                &node.label
            },
            node.class,
            node.links.len(),
            node.description
        );
        json!({
            "contents": {"kind": "markdown", "value": text},
            "range": {
                "start": {"line": line, "character": start},
                "end": {"line": line, "character": end},
            },
        })
    }

    /// Inbound edges — to the target under the cursor, or to the file itself.
    fn references(&mut self, params: &Value) -> Value {
        let (uri, line, character) = Self::position(params);
        let subject = match self.target_at(&uri, line, character) {
            Some((target, _, _)) => Some(target),
            None => self.node_id(&uri),
        };
        let Some(subject) = subject else {
            return json!([]);
        };
        let corpus = self.corpus.clone();
        let graph = self.graph();
        let mut out = Vec::new();
        for n in &graph.nodes {
            for l in &n.links {
                if l.resolved != subject {
                    continue;
                }
                let path = corpus.join(&n.node);
                let line = std::fs::read_to_string(&path)
                    .ok()
                    .and_then(|t| t.lines().position(|x| x.contains(&l.target)))
                    .map(|i| i + 1)
                    .unwrap_or(1);
                out.push(json!({"uri": path_to_uri(&path), "range": line_range(line)}));
            }
        }
        Value::Array(out)
    }

    /// F2 is offered on a corpus node and nowhere else, with its stem pre-filled.
    fn prepare_rename(&mut self, params: &Value) -> Value {
        let (uri, _, _) = Self::position(params);
        let Some(id) = self.node_id(&uri) else {
            return Value::Null;
        };
        // Instance only. A `<class>.ont.yml` is a class definition, and renaming one without
        // its directory breaks every instance in the class — see `rename::plan`.
        if !self.corpus.join(&id).is_file() || !id.contains('/') || id.ends_with(".ont.yml") {
            return Value::Null;
        }
        let stem = Path::new(&id)
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        json!({"range": line_range(1), "placeholder": stem})
    }

    /// The whole rename as one `WorkspaceEdit`, refused outright if anything would dangle.
    ///
    /// The client applies it, which is what keeps undo working — and what keeps the server
    /// from writing to a tree the editor has unsaved changes in.
    fn rename(&mut self, params: &Value) -> Result<Value, String> {
        let (uri, _, _) = Self::position(params);
        let new_name = params["newName"].as_str().unwrap_or("").trim();
        let id = self.node_id(&uri).ok_or("not a corpus node")?;
        let class = Path::new(&id)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        // A bare stem stays in its class; a `class/name` moves between them.
        let target = match new_name.contains('/') {
            true => new_name.to_string(),
            false => format!("{class}/{new_name}"),
        };

        let plan = rename::plan(&self.root, &self.corpus, &id, &target);
        if !plan.blocked.is_empty() {
            return Err(plan.blocked.join("; "));
        }

        let mut changes: HashMap<String, Vec<Value>> = HashMap::new();
        for e in &plan.edits {
            let path = self.root.join(&e.file);
            let text = self.overlay.read(&path);
            let Some(line_text) = text.lines().nth(e.line - 1) else {
                continue;
            };
            let Some(column) = line_text.find(&e.from) else {
                continue;
            };
            changes.entry(path_to_uri(&path)).or_default().push(json!({
                "range": {
                    "start": {"line": e.line - 1, "character": column},
                    "end": {"line": e.line - 1, "character": column + e.from.chars().count()},
                },
                "newText": e.to,
            }));
        }

        // `documentChanges` rather than `changes`: only the former can carry the file
        // rename, and a rename whose edits landed without the move would be exactly the
        // broken state the gate forbids.
        let mut document_changes: Vec<Value> = changes
            .into_iter()
            .map(|(uri, edits)| json!({"textDocument": {"uri": uri, "version": null}, "edits": edits}))
            .collect();
        document_changes.push(json!({
            "kind": "rename",
            "oldUri": path_to_uri(&self.corpus.join(&plan.from)),
            "newUri": path_to_uri(&self.corpus.join(&plan.to)),
        }));

        Ok(json!({"documentChanges": document_changes}))
    }
}

/// Serve LSP over stdio until the client says `exit`.
pub fn serve_lsp(root: Option<&Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let stdin = std::io::stdin();
    let mut input = stdin.lock();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    run(&mut input, &mut out, Server::new(root));
    Ok(())
}

fn run<R: BufRead, W: Write>(input: &mut R, out: &mut W, mut server: Server) {
    while let Some(message) = read_message(input) {
        if !server.handle(&message, out) {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::TempDir;

    fn fixture() -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_path_buf();
        let corpus = root.join(".yidam/corpus");
        std::fs::create_dir_all(corpus.join("concept")).unwrap();
        std::fs::write(
            corpus.join("concept.ont.yml"),
            "class: concept\nlabel: Concept\ndescription: A unit of understanding.\n",
        )
        .unwrap();
        std::fs::write(
            corpus.join("concept/a.yml"),
            "class: concept\nlabel: A\ndescription: The first.\nlinks:\n  - target: ../concept/b.yml\n    relationship: relates-to\n",
        )
        .unwrap();
        std::fs::write(
            corpus.join("concept/b.yml"),
            "class: concept\nlabel: B\ndescription: The second.\nlinks:\n  - target: ../concept/a.yml\n    relationship: relates-to\n",
        )
        .unwrap();
        (tmp, root)
    }

    /// Drive the server the way an editor does: framed messages in, framed messages out.
    fn exchange(root: &Path, messages: Vec<Value>) -> Vec<Value> {
        let mut input = String::new();
        for m in &messages {
            input.push_str(&frame(m));
        }
        let mut reader = Cursor::new(input.into_bytes());
        let mut out: Vec<u8> = Vec::new();
        run(&mut reader, &mut out, Server::new(root.to_path_buf()));

        let text = String::from_utf8_lossy(&out).to_string();
        let mut parsed = Vec::new();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("\r\n\r\n") {
            let header = &rest[..at];
            let len: usize = header
                .rsplit("Content-Length:")
                .next()
                .unwrap()
                .trim()
                .parse()
                .unwrap();
            let body = &rest[at + 4..at + 4 + len];
            parsed.push(serde_json::from_str::<Value>(body).unwrap());
            rest = &rest[at + 4 + len..];
        }
        parsed
    }

    fn uri(root: &Path, id: &str) -> String {
        path_to_uri(&root.join(".yidam/corpus").join(id))
    }

    fn open(root: &Path, id: &str, text: &str) -> Value {
        json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {"textDocument": {"uri": uri(root, id), "text": text}},
        })
    }

    #[test]
    fn a_frame_round_trips() {
        let value = json!({"jsonrpc": "2.0", "id": 1});
        let mut reader = Cursor::new(frame(&value).into_bytes());
        assert_eq!(read_message(&mut reader).unwrap(), value);
    }

    /// Headers other than `Content-Length` are ignored, and a torn stream ends the loop.
    #[test]
    fn framing_tolerates_what_clients_actually_send() {
        let body = r#"{"jsonrpc":"2.0"}"#;
        let raw = format!(
            "Content-Type: application/vscode-jsonrpc\r\nContent-Length: {}\r\n\r\n{body}",
            body.len()
        );
        let mut reader = Cursor::new(raw.into_bytes());
        assert!(read_message(&mut reader).is_some());
        assert!(read_message(&mut reader).is_none(), "clean end of stream");
    }

    #[test]
    fn a_file_uri_round_trips_through_a_space() {
        let path = PathBuf::from("/a b/c.yml");
        assert_eq!(path_to_uri(&path), "file:///a%20b/c.yml");
        assert_eq!(uri_to_path("file:///a%20b/c.yml").unwrap(), path);
    }

    /// The level name the fixtures speak, mapped to this server's numbering.
    ///
    /// The fixtures carry a name rather than a number because neither side's numbering is
    /// shared: LSP counts from 1, and `vscode.DiagnosticSeverity` counts from 0. A fixture
    /// holding either would make one of the two transcriptions assert a translation it does
    /// not perform.
    fn level(name: &str) -> u8 {
        match name {
            "error" => ERROR,
            "warning" => WARNING,
            "information" => INFORMATION,
            "hint" => HINT,
            other => panic!("fixture names a level this server has no numbering for: {other}"),
        }
    }

    /// RFC-0016's table, read rather than restated.
    ///
    /// Baseline membership outranks check severity in both directions, and this rule is the
    /// one place RFC-0016 licenses a client to recompute a verdict — the alternative being an
    /// editor that cannot render a diagnostic without a subprocess per keystroke. So it lives
    /// in two languages, and the fixtures are what keep the two answering the same question.
    /// The VS Code extension's `diagnostics.test.ts` reads these same files.
    #[test]
    fn the_severity_table_is_the_shared_fixture() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../prelude/sdks/parity/fixtures/diagnostic_severity");
        let mut cases: Vec<std::path::PathBuf> = std::fs::read_dir(&dir)
            .unwrap_or_else(|e| panic!("reading {}: {e}", dir.display()))
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "toml"))
            .collect();
        cases.sort();
        assert!(!cases.is_empty(), "no fixtures in {}", dir.display());

        for path in &cases {
            let fx: toml::Value = toml::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let name = path.file_name().unwrap().to_string_lossy();
            let severity = fx["input"]["severity"].as_str().unwrap();
            let in_baseline = fx["input"]["in_baseline"].as_bool().unwrap();
            let expected = level(fx["expected"]["level"].as_str().unwrap());
            assert_eq!(
                severity_of(severity, in_baseline),
                expected,
                "{name}: {}",
                fx["description"].as_str().unwrap_or("")
            );
        }
    }

    #[test]
    fn initialize_declares_what_this_server_actually_does() {
        let (_t, root) = fixture();
        let out = exchange(
            &root,
            vec![json!({"jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {}})],
        );
        let caps = &out[0]["result"]["capabilities"];
        assert_eq!(caps["definitionProvider"], true);
        assert_eq!(caps["referencesProvider"], true);
        assert_eq!(caps["hoverProvider"], true);
        assert_eq!(caps["renameProvider"]["prepareProvider"], true);
        // Full sync, because that is what the didChange arm implements.
        assert_eq!(caps["textDocumentSync"]["change"], 1);
    }

    /// The point of the overlay: findings about the buffer, not about the file.
    #[test]
    fn a_broken_edge_typed_but_not_saved_is_reported() {
        let (_t, root) = fixture();
        let broken = "class: concept\nlabel: A\ndescription: The first.\nlinks:\n  - target: ../concept/gone.yml\n    relationship: relates-to\n";
        let out = exchange(&root, vec![open(&root, "concept/a.yml", broken)]);

        let published: Vec<&Value> = out
            .iter()
            .filter(|m| m["method"] == "textDocument/publishDiagnostics")
            .collect();
        let mine = published
            .iter()
            .find(|m| m["params"]["uri"] == uri(&root, "concept/a.yml"))
            .expect("diagnostics for the open file");
        let diagnostics = mine["params"]["diagnostics"].as_array().unwrap();
        assert!(
            diagnostics.iter().any(|d| d["code"] == "dangling-edge"),
            "{diagnostics:#?}"
        );
        // The file on disk is fine, so nothing but the buffer could have produced this.
        assert!(root.join(".yidam/corpus/concept/b.yml").exists());
    }

    /// A finding that stays on screen after it is fixed is worse than one that never
    /// appeared, so a clean file gets an explicit empty array.
    #[test]
    fn fixing_a_finding_clears_it() {
        let (_t, root) = fixture();
        let broken = "class: concept\nlabel: A\ndescription: d\nlinks:\n  - target: ../concept/gone.yml\n    relationship: r\n";
        let fixed = "class: concept\nlabel: A\ndescription: d\nlinks:\n  - target: ../concept/b.yml\n    relationship: r\n";
        let out = exchange(
            &root,
            vec![
                open(&root, "concept/a.yml", broken),
                json!({
                    "jsonrpc": "2.0",
                    "method": "textDocument/didChange",
                    "params": {
                        "textDocument": {"uri": uri(&root, "concept/a.yml")},
                        "contentChanges": [{"text": fixed}],
                    },
                }),
            ],
        );
        let last = out
            .iter()
            .filter(|m| {
                m["method"] == "textDocument/publishDiagnostics"
                    && m["params"]["uri"] == uri(&root, "concept/a.yml")
            })
            .next_back()
            .unwrap();
        let diagnostics = last["params"]["diagnostics"].as_array().unwrap();
        assert!(
            !diagnostics.iter().any(|d| d["code"] == "dangling-edge"),
            "{diagnostics:#?}"
        );
    }

    #[test]
    fn definition_resolves_the_target_under_the_cursor() {
        let (_t, root) = fixture();
        let text = std::fs::read_to_string(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        let out = exchange(
            &root,
            vec![
                open(&root, "concept/a.yml", &text),
                json!({
                    "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
                    "params": {
                        "textDocument": {"uri": uri(&root, "concept/a.yml")},
                        "position": {"line": 4, "character": 20},
                    },
                }),
            ],
        );
        let result = &out.iter().find(|m| m["id"] == 2).unwrap()["result"];
        assert_eq!(result["uri"], uri(&root, "concept/b.yml"));
    }

    #[test]
    fn the_cursor_has_to_be_on_the_value() {
        let (_t, root) = fixture();
        let text = std::fs::read_to_string(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        let out = exchange(
            &root,
            vec![
                open(&root, "concept/a.yml", &text),
                json!({
                    "jsonrpc": "2.0", "id": 2, "method": "textDocument/definition",
                    "params": {
                        "textDocument": {"uri": uri(&root, "concept/a.yml")},
                        "position": {"line": 4, "character": 5},
                    },
                }),
            ],
        );
        assert_eq!(
            out.iter().find(|m| m["id"] == 2).unwrap()["result"],
            Value::Null
        );
    }

    #[test]
    fn hover_names_the_node_and_its_degree() {
        let (_t, root) = fixture();
        let text = std::fs::read_to_string(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        let out = exchange(
            &root,
            vec![
                open(&root, "concept/a.yml", &text),
                json!({
                    "jsonrpc": "2.0", "id": 3, "method": "textDocument/hover",
                    "params": {
                        "textDocument": {"uri": uri(&root, "concept/a.yml")},
                        "position": {"line": 4, "character": 20},
                    },
                }),
            ],
        );
        let value = out.iter().find(|m| m["id"] == 3).unwrap()["result"]["contents"]["value"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(value.contains("**B**"), "{value}");
        assert!(value.contains("1 out · 1 in"), "{value}");
    }

    #[test]
    fn references_finds_the_inbound_edge() {
        let (_t, root) = fixture();
        let text = std::fs::read_to_string(root.join(".yidam/corpus/concept/b.yml")).unwrap();
        let out = exchange(
            &root,
            vec![
                open(&root, "concept/b.yml", &text),
                json!({
                    "jsonrpc": "2.0", "id": 4, "method": "textDocument/references",
                    "params": {
                        "textDocument": {"uri": uri(&root, "concept/b.yml")},
                        "position": {"line": 0, "character": 0},
                        "context": {"includeDeclaration": false},
                    },
                }),
            ],
        );
        let refs = out.iter().find(|m| m["id"] == 4).unwrap()["result"]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0]["uri"], uri(&root, "concept/a.yml"));
        assert_eq!(refs[0]["range"]["start"]["line"], 4);
    }

    /// F2 on a corpus node offers its stem; F2 anywhere else offers nothing.
    #[test]
    fn prepare_rename_is_offered_only_on_a_node() {
        let (_t, root) = fixture();
        let ask = |id: &str| {
            json!({
                "jsonrpc": "2.0", "id": 5, "method": "textDocument/prepareRename",
                "params": {
                    "textDocument": {"uri": uri(&root, id)},
                    "position": {"line": 0, "character": 0},
                },
            })
        };
        let out = exchange(&root, vec![ask("concept/a.yml")]);
        assert_eq!(
            out.iter().find(|m| m["id"] == 5).unwrap()["result"]["placeholder"],
            "a"
        );
        let out = exchange(&root, vec![ask("concept.ont.yml")]);
        assert_eq!(
            out.iter().find(|m| m["id"] == 5).unwrap()["result"],
            Value::Null,
            "a class definition is not a node"
        );
    }

    /// The whole rename as one edit: the inbound rewrite *and* the file move.
    ///
    /// A rename whose edits landed without the move would be exactly the broken state the
    /// gate forbids, which is why this uses `documentChanges` — `changes` cannot carry it.
    #[test]
    fn rename_produces_one_workspace_edit_carrying_the_move() {
        let (_t, root) = fixture();
        let out = exchange(
            &root,
            vec![json!({
                "jsonrpc": "2.0", "id": 6, "method": "textDocument/rename",
                "params": {
                    "textDocument": {"uri": uri(&root, "concept/b.yml")},
                    "position": {"line": 0, "character": 0},
                    "newName": "beta",
                },
            })],
        );
        let changes = out.iter().find(|m| m["id"] == 6).unwrap()["result"]["documentChanges"]
            .as_array()
            .unwrap()
            .clone();

        let mv = changes.iter().find(|c| c["kind"] == "rename").unwrap();
        assert_eq!(mv["oldUri"], uri(&root, "concept/b.yml"));
        assert_eq!(mv["newUri"], uri(&root, "concept/beta.yml"));

        let edit = changes
            .iter()
            .find(|c| c["textDocument"]["uri"] == uri(&root, "concept/a.yml"))
            .expect("a.yml points at b.yml");
        let range = &edit["edits"][0]["range"];
        assert_eq!(edit["edits"][0]["newText"], "../concept/beta.yml");
        assert_eq!(range["start"]["line"], 4);
        // The span covers the value and not the key.
        let text = std::fs::read_to_string(root.join(".yidam/corpus/concept/a.yml")).unwrap();
        let line = text.lines().nth(4).unwrap();
        let start = range["start"]["character"].as_u64().unwrap() as usize;
        let end = range["end"]["character"].as_u64().unwrap() as usize;
        assert_eq!(&line[start..end], "../concept/b.yml");

        // Nothing was written: the client applies it, which is what keeps undo working.
        assert!(root.join(".yidam/corpus/concept/b.yml").is_file());
    }

    /// A refusal is an error, not an empty edit.
    ///
    /// An editor that showed "renamed" and changed nothing is the worst outcome available.
    #[test]
    fn a_rename_onto_an_existing_node_is_refused_as_an_error() {
        let (_t, root) = fixture();
        let out = exchange(
            &root,
            vec![json!({
                "jsonrpc": "2.0", "id": 7, "method": "textDocument/rename",
                "params": {
                    "textDocument": {"uri": uri(&root, "concept/b.yml")},
                    "position": {"line": 0, "character": 0},
                    "newName": "a",
                },
            })],
        );
        let message = out.iter().find(|m| m["id"] == 7).unwrap();
        assert!(message["result"].is_null());
        assert!(message["error"]["message"]
            .as_str()
            .unwrap()
            .contains("already exists"));
    }

    /// `class/name` moves between classes; a bare stem stays put.
    #[test]
    fn a_new_name_with_a_slash_moves_the_node() {
        let (_t, root) = fixture();
        std::fs::write(
            root.join(".yidam/corpus/gauge.ont.yml"),
            "class: gauge\nlabel: Gauge\ndescription: An instrument.\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".yidam/corpus/gauge")).unwrap();
        let out = exchange(
            &root,
            vec![json!({
                "jsonrpc": "2.0", "id": 8, "method": "textDocument/rename",
                "params": {
                    "textDocument": {"uri": uri(&root, "concept/b.yml")},
                    "position": {"line": 0, "character": 0},
                    "newName": "gauge/moved",
                },
            })],
        );
        let changes = out.iter().find(|m| m["id"] == 8).unwrap()["result"]["documentChanges"]
            .as_array()
            .unwrap()
            .clone();
        let mv = changes.iter().find(|c| c["kind"] == "rename").unwrap();
        assert_eq!(mv["newUri"], uri(&root, "gauge/moved.yml"));
    }

    #[test]
    fn exit_ends_the_loop() {
        let (_t, root) = fixture();
        let out = exchange(
            &root,
            vec![
                json!({"jsonrpc": "2.0", "id": 9, "method": "shutdown"}),
                json!({"jsonrpc": "2.0", "method": "exit"}),
                json!({"jsonrpc": "2.0", "id": 10, "method": "initialize", "params": {}}),
            ],
        );
        assert!(out.iter().any(|m| m["id"] == 9));
        assert!(
            !out.iter().any(|m| m["id"] == 10),
            "nothing is served after exit"
        );
    }

    /// An unknown request gets a null result rather than silence.
    ///
    /// A client blocking forever on a method this server does not implement is a hang, and a
    /// hang reads as a broken install.
    #[test]
    fn an_unknown_request_is_answered() {
        let (_t, root) = fixture();
        let out = exchange(
            &root,
            vec![
                json!({"jsonrpc": "2.0", "id": 11, "method": "textDocument/codeLens", "params": {}}),
            ],
        );
        assert_eq!(
            out.iter().find(|m| m["id"] == 11).unwrap()["result"],
            Value::Null
        );
    }
}
