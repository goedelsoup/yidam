//! `yidam retrieve` — the retrieval an agent gets, in a terminal.
//!
//! # Why this exists as a command
//!
//! `retrieve` is the tool an agent reaches for before it knows enough to write a query, and
//! until now the only way to reach it was to be an MCP client. Every other reading surface has
//! a terminal route — `query`, `pack`, `estimate`, `neighbors`, `graph` — and this one did
//! not, which made the one retrieval surface a person could not try the one they were being
//! asked to trust. #835 is what made the gap worth closing rather than noting: a query that
//! can span several corpora needs somebody to be able to *ask* it, and the corpora it spans
//! are named in a config file a person edits.
//!
//! # It is the same path, not a second implementation
//!
//! Dispatched through [`crate::cmd::serve::tools::call`] by tool name, for the reason
//! `bench`'s flat arm gives: this must provably be the retrieval `serve --mcp` performs, not a
//! reimplementation that could quietly differ. So `--format json` emits the MCP contract's own
//! response shape inside the report envelope — one shape for one answer, rather than a second
//! rendering of it that a consumer would have to know which it was reading.

use anyhow::Result;

use crate::report::Format;

/// What a caller asked for.
pub struct Options {
    pub k: usize,
    pub class: Option<String>,
    /// Other corpora sharing the vector index, by a name `[index.remote.corpora]` declares or
    /// by a genesis hash. Empty is this corpus alone.
    pub corpora: Vec<String>,
    pub format: Format,
}

pub fn retrieve(root: Option<&std::path::Path>, query: &str, opts: Options) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let mut state = crate::cmd::serve::ServerState::load(&root)?;

    let args = serde_json::json!({
        "query": query,
        "k": opts.k,
        "class": opts.class,
        "corpora": opts.corpora,
    });
    let envelope = crate::cmd::serve::tools::call(&mut state, "retrieve", &args);
    // A tool error is a failure of the call rather than an answer — the same distinction the
    // contract draws for `isError` — so it leaves as an error rather than as a report saying
    // nothing was found.
    if envelope["isError"].as_bool().unwrap_or(false) {
        anyhow::bail!(
            "{}",
            envelope["content"][0]["text"]
                .as_str()
                .unwrap_or("retrieve failed")
        );
    }
    let payload: serde_json::Value = envelope["content"][0]["text"]
        .as_str()
        .and_then(|t| serde_json::from_str(t).ok())
        .ok_or_else(|| anyhow::anyhow!("the retrieve envelope carried no JSON payload"))?;

    // **Emitted, then failed.** A rejection is an answer and not a crash, so it is rendered
    // in whichever format was asked for — `cmd::query` states the rule in as many words: a
    // rejection propagated as an ordinary `Err` would print `Error: …` on stderr with an
    // empty stdout, which a `--format json` consumer cannot tell from a truncated pipe. It
    // still exits nonzero — as `report::GateFailed`, which prints nothing above the report
    // — because at a shell a misspelled `--class` or `--corpora` should fail a script;
    // the MCP tool must *not* set `isError` for the same state, and that asymmetry is the
    // contract's (`rejected` is an answer to a client) meeting the shell's (a wrong argument
    // is an error to a caller).
    let rejected = payload["rejected"].is_object();
    crate::report::gate(&root, opts.format, payload, !rejected, |p| {
        print!("{}", render(query, p, &state.corpus_aliases))
    })
}

/// The human rendering.
///
/// Pure, and taking the payload rather than producing it, so what a person sees can be held
/// against a fixture without a corpus, an index or a network — which is the only way the
/// foreign-row case is testable at all.
fn render(query: &str, payload: &serde_json::Value, aliases: &crate::s3vectors::Aliases) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();

    let scope = payload["scope"].as_str().unwrap_or("local");
    let _ = writeln!(out, "retrieve {query:?} — scope {scope}");

    if let Some(reason) = payload["degraded_reason"].as_str() {
        let _ = writeln!(out, "  degraded: {reason}");
    }
    if let Some(rejected) = payload["rejected"].as_object() {
        let _ = writeln!(
            out,
            "  rejected: {} — {}",
            rejected["code"].as_str().unwrap_or(""),
            rejected["message"].as_str().unwrap_or("")
        );
        return out;
    }

    for row in payload["results"].as_array().into_iter().flatten() {
        let score = row["score"].as_f64().unwrap_or_default();
        // The id where there is one, the path where there is not. A row that resolves to no
        // node still has to be nameable — a catalog source is the ordinary case — and the
        // path is what the index recorded.
        let name = row["id"]
            .as_str()
            .or_else(|| row["path"].as_str())
            .unwrap_or("");
        let label = row["label"].as_str().unwrap_or("");
        // Whose corpus, in the nickname this repository declared for it where it declared
        // one. The identifier above carries the hash regardless — a name is for the person
        // reading and the hash is the identity — see `Aliases::name_of`.
        let whose = match row["corpus"].as_str() {
            Some(id) => match aliases.name_of(id) {
                Some(name) => format!("[{name}] "),
                None => format!("[{id}] "),
            },
            None => String::new(),
        };
        let cut = match row["truncated"].as_bool() {
            Some(true) => "  (text cut to fit the index's per-row ceiling)",
            _ => "",
        };
        let _ = writeln!(out, "  {score:.3}  {whose}{name}  —  {label}{cut}");
    }

    if let Some(absence) = payload["absence"].as_object() {
        let _ = writeln!(
            out,
            "  nothing: {} — {}",
            absence["code"].as_str().unwrap_or(""),
            absence["message"].as_str().unwrap_or("")
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn aliases(pairs: &[(&str, &str)]) -> crate::s3vectors::Aliases {
        crate::s3vectors::Aliases::resolve(
            &pairs
                .iter()
                .map(|(n, h)| ((*n).to_string(), (*h).to_string()))
                .collect(),
        )
        .unwrap()
    }

    /// A local row and a foreign one, rendered side by side.
    ///
    /// The assertion that matters is that they are **distinguishable**: a renderer that
    /// dropped `corpus` would print two lines that look like two nodes in this repository,
    /// which is the whole failure #835's "every result says which corpus it came from" is
    /// about.
    #[test]
    fn a_foreign_row_says_whose_it_is_and_a_local_one_does_not() {
        let payload = json!({
            "scope": "across",
            "degraded": false,
            "degraded_reason": null,
            "rejected": null,
            "absence": null,
            "results": [
                {"id": "concept/funding", "path": ".yidam/corpus/concept/funding.yml",
                 "corpus": null, "class": "concept", "label": "Funding", "text": "",
                 "score": 0.8, "truncated": false},
                {"id": "yidam://3f2a9c4d1b70/node/concept/levy",
                 "path": ".yidam/corpus/concept/levy.yml", "corpus": "3f2a9c4d1b70",
                 "class": "concept", "label": "Levy", "text": "", "score": 0.7,
                 "truncated": true},
            ],
        });
        let out = render(
            "funding",
            &payload,
            &aliases(&[("ohio-budget", "3f2a9c4d1b70")]),
        );
        assert!(out.contains("scope across"), "{out}");
        assert!(
            out.contains("  0.800  concept/funding  —  Funding\n"),
            "{out}"
        );
        assert!(
            out.contains("[ohio-budget] yidam://3f2a9c4d1b70/node/concept/levy"),
            "{out}"
        );
        assert!(out.contains("text cut"), "{out}");
    }

    /// A corpus nobody named still renders — by its identity, which is what a result carries.
    #[test]
    fn an_undeclared_corpus_renders_as_its_identity() {
        let payload = json!({
            "scope": "across", "degraded_reason": null, "rejected": null, "absence": null,
            "results": [{"id": null, "path": ".yidam/catalog/x.md", "corpus": "0123456789ab",
                         "class": "paper", "label": "X", "text": "", "score": 0.5,
                         "truncated": false}],
        });
        let out = render("x", &payload, &aliases(&[]));
        assert!(out.contains("[0123456789ab] .yidam/catalog/x.md"), "{out}");
    }

    /// A rejection is the answer and the results are not rendered under it, because there are
    /// none and the reason is the point.
    #[test]
    fn a_rejection_is_what_a_reader_is_shown() {
        let payload = json!({
            "scope": "local", "degraded_reason": null,
            "rejected": {"code": "unknown-corpus", "message": "`ohi-budget` is neither"},
            "absence": null, "results": [],
        });
        let out = render("x", &payload, &aliases(&[]));
        assert!(out.contains("rejected: unknown-corpus"), "{out}");
    }
}
