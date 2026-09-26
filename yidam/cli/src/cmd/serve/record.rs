//! What this server was asked, so a corpus can say whether it has ever been read.
//!
//! # The asymmetry this closes
//!
//! This repository can tell you, for every node, who asserted it, when, under what review, and
//! what it looked like at any commit. Until this module it could not tell you whether any node
//! had ever been *read*. `serve --mcp` wrote nothing at all — not a log, not a counter — and
//! thirteen tools dispatched, returned, and the process forgot.
//!
//! RFC-0015 exists specifically to make testimony visible rather than merely asserted, on the
//! argument that a philosophy you cannot see is a philosophy you cannot use. The same argument
//! applies unchanged to the consumption side, where nothing was visible at all. Four questions
//! a corpus could not ask about itself, and can once a record exists:
//!
//! - Which queries returned nothing? That set is the most direct empirical evidence of what a
//!   corpus is missing, and [`super::tools::call`] used to discard it.
//! - Did the degraded keyword path serve real traffic? `retrieve` reported `degraded: true`
//!   per call and nothing aggregated it, so *this corpus has been answering from keyword
//!   search for a month* was not a statement anyone could make.
//! - Did anything act on a clock? Four freshness clocks, and no closing evidence that a single
//!   one was ever discharged through this surface.
//! - How much of the corpus is the read surface actually reaching?
//!
//! # What it may author, which RFC-0026 already settled
//!
//! > A run authors **operational** commits directly. Every **epistemic** commit it produces
//! > goes to a proposal branch, and nothing merges itself.
//!
//! A record of what was retrieved is unambiguously operational — it is closest to `index:` in
//! the commit vocabulary's own table — so this lands on the permitted side of an invariant that
//! already exists and needs no new authority concept. It is the reason this is scoped against
//! RFC-0026 rather than built standalone.
//!
//! # Why a gitignored file and not a commit per call
//!
//! *The history is the graph* is this repository's premise, and an orchestrator that "ran steps
//! and wrote a log file would be the first component to contradict the premise" — #460's own
//! words. The premise is kept and the hot path is not: every call appends a line here, and
//! folding the file into a `refresh:`-class commit is a scheduled run rather than something a
//! `tools/call` does. `git commit-tree` on the read path would make the cheapest tool call the
//! most expensive thing the server does, and would give `serve` a git-write path that
//! RFC-0029's identity gate currently guards for the `act` tier alone.
//!
//! The file is therefore a staging buffer, not the record of last resort, and [`ensure_ignored`]
//! refuses to open it anywhere git would offer to commit it. The fold is #1018, not this.
//!
//! # Why no plaintext query
//!
//! [`digest`] and never the words. A corpus served over `--http` is answering callers it cannot
//! authenticate, and a plaintext record would be a file of third parties' questions accumulating
//! in a working tree. The digest answers every question above — *which queries came back empty*
//! is a set with counts either way — and answers them identically on both transports, so there
//! is no transport-conditional record shape for anyone to reason about.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{bail, Result};
use serde_json::{json, Value};

/// The file every `tools/call` appends one line to, relative to the corpus root.
///
/// Under a directory of its own rather than a bare `.yidam/calls.jsonl`, because `.yidam/` is
/// otherwise entirely committed content and a single ignored file in it is an exception a reader
/// has to know about. A directory is the shape `.yidam/vault/` already established for bytes
/// that are produced rather than asserted.
pub(crate) const PATH: &str = ".yidam/record/calls.jsonl";

/// [`PATH`]'s directory.
///
/// Spelled out rather than taken from `Path::new(PATH).parent()`, which is an `Option` this
/// module would have to `expect` its way out of. The invariant is real — a relative path with a
/// separator in it has a parent — but asserting it at runtime spends a panic path on something
/// two constants in the same file can be checked against each other for, and `panic_paths.rs`
/// is right that each one is a decision. [`the_two_constants_name_one_file`] is that check.
const DIR: &str = ".yidam/record";

/// An open, append-only record of what this server was asked.
///
/// `None` on [`ServerState`](super::ServerState) is the whole of the default behaviour: a corpus
/// that has not declared `[serve] record` gets a server that writes nothing, which is every
/// server this repository shipped before this module.
pub(crate) struct Record {
    file: std::fs::File,
    path: PathBuf,
    /// Set by the first failed write, so a record that cannot be written degrades to silence
    /// rather than to one line of stderr per tool call.
    ///
    /// **Silence after one warning, and not a refusal.** A server's job is to answer, and a
    /// full disk is not a reason to stop answering; but a record that has silently stopped
    /// recording is a file somebody will later read as evidence that nothing was retrieved.
    /// The warning is the difference, and it is on stderr where the startup banner is.
    broken: bool,
}

impl Record {
    /// Open the record this corpus declared, or `None` where it declared none.
    ///
    /// Called from [`ServerState::load`](super::ServerState::load) before the corpus is walked,
    /// for `act_declared`'s reason: a server told to record and unable to must fail at the
    /// command rather than at the first call, and loading ten thousand nodes first would be the
    /// same refusal, later and more expensively.
    pub(crate) fn open(root: &Path) -> Result<Option<Self>> {
        if !crate::config::load_yidam_config(root)?.serve.record {
            return Ok(None);
        }
        let path = root.join(PATH);
        let dir = root.join(DIR);
        ensure_ignored(root, &dir)?;
        std::fs::create_dir_all(&dir)
            .map_err(|e| anyhow::anyhow!("creating {} ({e})", dir.display()))?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|e| {
                anyhow::anyhow!(
                    "`[serve] record = true` in {}, and {} could not be opened for appending \
                     ({e}).\n  A server that was told to record and cannot is a deployment that \
                     believes something false about itself.",
                    root.join(".yidam/config.toml").display(),
                    path.display()
                )
            })?;
        Ok(Some(Self {
            file,
            path,
            broken: false,
        }))
    }

    /// Append one line for one `tools/call`.
    ///
    /// `commit` is read from the live snapshot rather than cached at [`Self::open`], because an
    /// `act` call reloads that snapshot and a record naming the commit the server *started* at
    /// would attribute a post-write read to the pre-write corpus.
    pub(crate) fn append(
        &mut self,
        commit: &str,
        tool: &str,
        args: &Value,
        outcome: &Result<Value, String>,
        elapsed: Duration,
    ) {
        if self.broken {
            return;
        }
        let mut line = entry(commit, tool, args, outcome, elapsed).to_string();
        line.push('\n');
        // One `write_all` of one line, on a handle opened `O_APPEND`, so two concurrent
        // appenders cannot interleave halves of a line. Today they cannot be concurrent at all
        // — `handle` takes `&mut ServerState` and the HTTP arm holds it behind a lock — and
        // that is a property of the caller rather than of the file.
        if let Err(e) = self.file.write_all(line.as_bytes()) {
            eprintln!(
                "warning: could not append to {} ({e}) — this server is still answering, and \
                 has stopped recording. The record below this point is not evidence that \
                 nothing was retrieved.",
                self.path.display()
            );
            self.broken = true;
        }
    }
}

/// One record line.
///
/// Every field is present on every line, including the ones that are null for the tool in hand.
/// That is the convention `degraded_reason`, `rejected` and `origin` already follow on the wire
/// and it exists for the same reason here: a reader testing a key must never have to
/// distinguish *nothing to report* from *a writer too old to report it*.
fn entry(
    commit: &str,
    tool: &str,
    args: &Value,
    outcome: &Result<Value, String>,
    elapsed: Duration,
) -> Value {
    let body = outcome.as_ref().ok();
    json!({
        "at": unix_now(),
        // WHICH CORPUS ANSWERED. A reading is attributable to a corpus state the way every
        // other claim in this repository is; without it the record says a query came back
        // empty and cannot say what it was asked of.
        "commit": commit,
        "tool": tool,
        "args_digest": digest(args),
        // A REFUSAL IS A READING. The failure path is the one that answers "which queries
        // returned nothing", so it is recorded on the same line shape as a success rather
        // than dropped — which is what `tools::call` did with it before this module.
        "outcome": match outcome {
            Ok(_) => "ok",
            Err(_) => "error",
        },
        "results": body.and_then(results),
        "degraded": body.and_then(|b| b.get("degraded").and_then(Value::as_bool)),
        // A REJECTION IS NOT AN ABSENCE — `retrieve`'s own distinction, carried here because
        // collapsing it would put a caller's typo and a hole in the corpus in the same bucket,
        // and the empty-result set is the reason this record exists.
        "rejected": body.and_then(|b| b.get("rejected").map(|r| !r.is_null())),
        "ms": elapsed.as_millis() as u64,
    })
}

/// How many rows a tool answered with, or `None` for a shape this cannot read.
///
/// **Exactly one top-level array, or nothing.** The tools do not share a key — `retrieve` and
/// `query` say `results`, `neighbors` says `neighbors`, `list_nodes` says `nodes`, `claims` says
/// `claims`, `licensed_edges` says `edges` — and a per-tool table here would be a second copy
/// of every response shape, drifting silently against the contract that froze them. The
/// structural rule needs no table and is honest about what it cannot count: a response with no
/// array, or with two, records `null` rather than a number somebody would read as a row count.
///
/// A count and not the ids, which is the gap #1020 names: this answers *which queries came back
/// empty* and not *which nodes were never returned*, though #719 led with the second.
fn results(body: &Value) -> Option<u64> {
    let mut arrays = body.as_object()?.values().filter_map(Value::as_array);
    let first = arrays.next()?;
    match arrays.next() {
        None => Some(first.len() as u64),
        Some(_) => None,
    }
}

/// A stable digest of the arguments, and never the arguments.
///
/// Two spellings of the same call digest alike, which is what makes *this query has been asked
/// before* and *these forty calls were the same question* answerable at all. It holds because
/// `serde_json::Map` is a `BTreeMap` in this build — `preserve_order` is off, and
/// `a_digest_ignores_key_order` is the case that goes red if it is ever turned on.
fn digest(args: &Value) -> String {
    format!(
        "sha256:{}",
        crate::deps::sha256_hex(args.to_string().as_bytes())
    )
}

/// Seconds since the epoch, the shape `index/meta.json` already writes its `generated_at` in.
fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Refuse to open the record anywhere git would offer to commit it.
///
/// [`crate::cmd::vault`]'s guard, applied to the other directory under `.yidam/` that holds
/// produced rather than asserted bytes, and for the reason that makes it worth a refusal rather
/// than a warning: this repository's own protocols prescribe `git add -A`, so a tracked
/// `calls.jsonl` would be swept into the next commit and would then dirty the working tree after
/// every session anyone served.
///
/// `git check-ignore` is the authority rather than a `.gitignore` grep, because ignoring can come
/// from `.git/info/exclude` or a global file and a grep would report a correctly configured
/// repository as broken. **The probe is a file inside the directory, not the directory itself**:
/// `.yidam/record/` is a directory-only pattern and git cannot tell that a path it is asked about
/// is a directory when the directory does not exist yet — which is the state the first recorded
/// call runs in.
fn ensure_ignored(root: &Path, dir: &Path) -> Result<()> {
    let probe = dir.join("probe");
    let out = crate::git::Git::new(root)
        .args(["check-ignore", "-q"])
        .paths([&probe])
        .output();
    match out.map(|o| o.status) {
        Ok(s) if s.success() => Ok(()),
        // 1 means "not ignored"; anything else means git could not answer, and an unanswered
        // question about whether these bytes would be committed is not a yes.
        Ok(_) => bail!(
            "{} is not ignored by git, and `[serve] record` appends to it on every tool call.\n  \
             Add `.yidam/record/` to `.gitignore`. A tracked record is a working tree that is \
             dirty after every session anyone served, arriving through `git add -A`.",
            dir.strip_prefix(root).unwrap_or(dir).display()
        ),
        Err(e) => bail!(
            "could not ask git whether {} is ignored ({e}).\n  \
             Refusing to append to a working tree without knowing that.",
            dir.display()
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The invariant `open` would otherwise have asserted with an `expect`.
    ///
    /// `DIR` exists so the one place that needs a directory does not spend a panic path on it.
    /// That trade is only sound while the two constants agree, and nothing in the type system
    /// makes them: they are two independent string literals. This is what holds them together.
    #[test]
    fn the_two_constants_name_one_file() {
        assert_eq!(
            Path::new(PATH).parent(),
            Some(Path::new(DIR)),
            "`DIR` must be `PATH`'s directory — `open` creates the first and appends to the \
             second, so a disagreement here is a file written outside the directory that was \
             checked against gitignore"
        );
    }

    fn ok(body: Value) -> Result<Value, String> {
        Ok(body)
    }

    #[test]
    fn a_digest_ignores_key_order() {
        // The property the digest is worth having: two spellings of the same call are the same
        // question, so "this query has been asked before" is answerable. It holds because
        // `serde_json::Map` is a `BTreeMap` here — this case is what goes red if
        // `preserve_order` is ever turned on for this workspace.
        let a = json!({"query": "traversal", "k": 5});
        let b = json!({"k": 5, "query": "traversal"});
        assert_eq!(digest(&a), digest(&b));
        assert_ne!(digest(&a), digest(&json!({"query": "traversal", "k": 6})));
    }

    #[test]
    fn a_digest_carries_none_of_the_words() {
        let d = digest(&json!({"query": "the incidence of latent tuberculosis"}));
        assert!(d.starts_with("sha256:"));
        assert!(!d.contains("tuberculosis"), "{d}");
        assert!(!d.contains("latent"), "{d}");
    }

    /// Every tool this server serves, counted by the structural rule rather than by a table.
    ///
    /// The keys differ per tool — `results`, `neighbors`, `nodes`, `claims`, `edges` — which is
    /// exactly why there is no table: one here would be a second copy of five response shapes,
    /// drifting against the contract that froze them.
    #[test]
    fn a_row_count_reads_whichever_key_the_tool_used() {
        assert_eq!(results(&json!({"results": [1, 2, 3]})), Some(3));
        assert_eq!(
            results(&json!({"id": "concept/x", "neighbors": []})),
            Some(0)
        );
        assert_eq!(results(&json!({"nodes": [1]})), Some(1));
        assert_eq!(
            results(&json!({"claims": [1, 2], "returned": 2, "total": 9})),
            Some(2)
        );
        // `retrieve`'s real shape: five scalar keys and one array.
        assert_eq!(
            results(&json!({
                "degraded": true,
                "scope": "local",
                "degraded_reason": "no_index",
                "rejected": null,
                "absence": null,
                "results": [1, 2],
            })),
            Some(2)
        );
    }

    /// Null, never a guess, for a shape this cannot count.
    ///
    /// A number here would be read as a row count by whoever folds this file, and a response
    /// with two arrays does not have one.
    #[test]
    fn a_shape_with_no_single_array_records_null() {
        assert_eq!(results(&json!({"edges": [1], "rejected": [2]})), None);
        assert_eq!(results(&json!({"ok": true})), None);
        assert_eq!(results(&json!("a string")), None);
    }

    #[test]
    fn an_entry_names_the_commit_it_answered_from() {
        let e = entry(
            "abc1234",
            "retrieve",
            &json!({"query": "x"}),
            &ok(json!({"degraded": false, "rejected": null, "results": []})),
            Duration::from_millis(12),
        );
        assert_eq!(e["commit"], "abc1234");
        assert_eq!(e["tool"], "retrieve");
        assert_eq!(e["outcome"], "ok");
        assert_eq!(e["results"], 0);
        assert_eq!(e["degraded"], false);
        assert_eq!(e["rejected"], false);
        assert_eq!(e["ms"], 12);
    }

    /// A refusal is a reading, and the record says which it was.
    #[test]
    fn a_refusal_records_the_outcome_and_no_rows() {
        let e = entry(
            "abc1234",
            "retrieve",
            &json!({}),
            &Err("missing required argument: query".to_string()),
            Duration::ZERO,
        );
        assert_eq!(e["outcome"], "error");
        // Null and not zero: "returned no rows" and "there was no answer to count rows in" are
        // different facts, and only the first is evidence about the corpus.
        assert!(e["results"].is_null());
        assert!(e["degraded"].is_null());
        assert!(e["rejected"].is_null());
        // And never the refusal's words either — a message can quote the argument it rejected.
        assert!(!e.to_string().contains("missing required argument"));
    }

    /// Every field on every line, including the ones that do not apply.
    ///
    /// The convention `degraded_reason` and `origin` already follow on the wire: a reader
    /// testing a key must never have to distinguish *nothing to report* from *a writer too old
    /// to report it*.
    #[test]
    fn every_line_carries_every_key() {
        let expected = [
            "at",
            "commit",
            "tool",
            "args_digest",
            "outcome",
            "results",
            "degraded",
            "rejected",
            "ms",
        ];
        for outcome in [ok(json!({"nodes": []})), Err("refused".to_string())] {
            let e = entry(
                "abc1234",
                "list_nodes",
                &json!({}),
                &outcome,
                Duration::ZERO,
            );
            let keys: Vec<&str> = e.as_object().unwrap().keys().map(String::as_str).collect();
            for key in expected {
                assert!(keys.contains(&key), "{key} is absent from {e}");
            }
            assert_eq!(keys.len(), expected.len(), "an unexpected key in {e}");
        }
    }
}
