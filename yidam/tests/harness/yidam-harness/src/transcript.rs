//! What the run actually did, read back off the event stream.
//!
//! The harness used to invoke the bootstrap agent with `.status()`, which returns an exit
//! code and discards everything else. That left three questions unanswerable after the fact:
//! what the agent did, what it cost, and — the one that matters most — whether it was ever
//! allowed to act at all.
//!
//! That last one is not hypothetical. `claude --print` runs under the default permission
//! mode, where a `Write` is denied and the process still exits 0 with `is_error: false`. A
//! bootstrap run under those conditions writes no files, and the structural checks then
//! report a corpus that is missing because the model was never permitted to create it —
//! indistinguishable, from the outside, from a model that produced nothing worth keeping.
//!
//! So the transcript is captured, and the denials are recorded beside the verdict. A result
//! whose `permission_denials` is non-empty is a broken harness run, not a model failure, and
//! the snapshot should be readable enough to say which.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// What produced a result, recorded beside it.
///
/// The argument for this is [RFC-0012](../../../../docs/rfcs/0012-elector-attestation.md),
/// made about electors and equally true here: recording what produced a position grants it
/// nothing — no extra weight, no priority — it makes the position auditable. The harness was
/// the one place in the system where an agent's output was kept and its provenance was not.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunRecord {
    /// What `--model` was asked for.
    pub model_requested: String,
    /// What the session actually resolved to. An alias can move under a fixed request, and a
    /// comparison between two runs of "the same model" is only meaningful against this one.
    pub model_resolved: Option<String>,
    pub session_id: Option<String>,
    pub num_turns: Option<u64>,
    pub duration_ms: Option<u64>,
    pub total_cost_usd: Option<f64>,
    /// The stream's own verdict — `success`, `error_max_turns`, and so on.
    pub subtype: Option<String>,
    pub is_error: Option<bool>,
    /// Tool calls the permission layer refused. Non-empty means the run was prevented, and
    /// no structural verdict taken from it describes the model.
    pub permission_denials: Vec<String>,
    /// Assistant turns before the agent first wrote to the tree.
    ///
    /// The evidence Q1 ("asked ≥2 clarifying questions before scaffolding") needs, and not
    /// yet the measurement: under the single-agent approximation the prompt tells the agent
    /// no domain owner is present, so it has nobody to ask and this reads 0 for reasons that
    /// have nothing to do with the model. It becomes Q1 when a responder exists.
    pub turns_before_first_write: Option<u64>,
}

/// Tools that change the repository. The first of these is where scaffolding begins.
const WRITING_TOOLS: [&str; 4] = ["Write", "Edit", "NotebookEdit", "MultiEdit"];

#[derive(Deserialize)]
struct Event {
    #[serde(rename = "type")]
    kind: String,
    subtype: Option<String>,
    // system/init
    model: Option<String>,
    // assistant
    message: Option<Message>,
    // result
    session_id: Option<String>,
    num_turns: Option<u64>,
    duration_ms: Option<u64>,
    total_cost_usd: Option<f64>,
    is_error: Option<bool>,
    #[serde(default)]
    permission_denials: Vec<Denial>,
}

#[derive(Deserialize)]
struct Message {
    #[serde(default)]
    content: Vec<Block>,
}

#[derive(Deserialize)]
struct Block {
    #[serde(rename = "type")]
    kind: String,
    name: Option<String>,
}

#[derive(Deserialize)]
struct Denial {
    tool_name: String,
}

/// Read a captured `transcript.jsonl` into a record.
///
/// Unparseable lines are skipped rather than fatal. The stream is written by another program
/// and a harness that refuses to report anything because one line was malformed is a harness
/// that loses a run it had already paid for.
pub fn read(path: &Path, model_requested: &str) -> Result<RunRecord> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading transcript: {}", path.display()))?;
    Ok(parse(&text, model_requested))
}

fn parse(text: &str, model_requested: &str) -> RunRecord {
    let mut record = RunRecord {
        model_requested: model_requested.to_string(),
        model_resolved: None,
        session_id: None,
        num_turns: None,
        duration_ms: None,
        total_cost_usd: None,
        subtype: None,
        is_error: None,
        permission_denials: vec![],
        turns_before_first_write: None,
    };

    let mut assistant_turns: u64 = 0;
    let mut wrote = false;

    for line in text.lines() {
        let Ok(event) = serde_json::from_str::<Event>(line) else {
            continue;
        };
        match event.kind.as_str() {
            "system" if event.subtype.as_deref() == Some("init") => {
                record.model_resolved = event.model;
            }
            "assistant" => {
                if wrote {
                    continue;
                }
                assistant_turns += 1;
                let blocks = event.message.map(|m| m.content).unwrap_or_default();
                if blocks.iter().any(|b| {
                    b.kind == "tool_use"
                        && b.name
                            .as_deref()
                            .is_some_and(|n| WRITING_TOOLS.contains(&n))
                }) {
                    // The turn that writes is not a turn before the write.
                    record.turns_before_first_write = Some(assistant_turns - 1);
                    wrote = true;
                }
            }
            "result" => {
                record.subtype = event.subtype;
                record.session_id = event.session_id;
                record.num_turns = event.num_turns;
                record.duration_ms = event.duration_ms;
                record.total_cost_usd = event.total_cost_usd;
                record.is_error = event.is_error;
                record.permission_denials = event
                    .permission_denials
                    .into_iter()
                    .map(|d| d.tool_name)
                    .collect();
            }
            _ => {}
        }
    }

    if !wrote {
        record.turns_before_first_write = Some(assistant_turns);
    }
    record
}

impl RunRecord {
    /// Was this run prevented from acting, rather than bad at it?
    pub fn was_denied(&self) -> bool {
        !self.permission_denials.is_empty()
    }

    /// One line for a person reading the run.
    pub fn summary(&self) -> String {
        let cost = self
            .total_cost_usd
            .map(|c| format!("${c:.4}"))
            .unwrap_or_else(|| "cost unknown".into());
        let turns = self
            .num_turns
            .map(|t| t.to_string())
            .unwrap_or_else(|| "?".into());
        let model = self
            .model_resolved
            .as_deref()
            .unwrap_or(&self.model_requested);
        format!("{model} · {turns} turns · {cost}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Shapes taken from a real `--output-format stream-json` run, trimmed to the fields the
    /// parser reads. Written out rather than fixtured because it is the contract with another
    /// program, and a fixture would hide which fields this code actually depends on.
    fn stream(assistant: &[&str], result: &str) -> String {
        let mut lines = vec![
            r#"{"type":"rate_limit_event"}"#.to_string(),
            r#"{"type":"system","subtype":"init","model":"claude-opus-5","tools":39}"#.to_string(),
        ];
        lines.extend(assistant.iter().map(|s| s.to_string()));
        lines.push(result.to_string());
        lines.join("\n")
    }

    fn text_turn() -> &'static str {
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"a question?"}]}}"#
    }

    fn write_turn() -> &'static str {
        r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Write"}]}}"#
    }

    fn ok_result() -> &'static str {
        r#"{"type":"result","subtype":"success","is_error":false,"num_turns":6,"duration_ms":41000,"total_cost_usd":0.42,"session_id":"abc"}"#
    }

    #[test]
    fn the_resolved_model_is_read_from_the_init_event() {
        let r = parse(&stream(&[text_turn()], ok_result()), "opus");
        assert_eq!(r.model_requested, "opus");
        assert_eq!(r.model_resolved.as_deref(), Some("claude-opus-5"));
    }

    #[test]
    fn cost_and_turns_come_off_the_result_event() {
        let r = parse(&stream(&[text_turn()], ok_result()), "opus");
        assert_eq!(r.num_turns, Some(6));
        assert_eq!(r.total_cost_usd, Some(0.42));
        assert_eq!(r.session_id.as_deref(), Some("abc"));
        assert!(!r.was_denied());
    }

    /// The case that motivated capturing anything: `--print` denies writes under the default
    /// permission mode, and the process still exits 0 saying it succeeded.
    #[test]
    fn a_denied_run_is_visible_even_though_it_reports_success() {
        let result = r#"{"type":"result","subtype":"success","is_error":false,"num_turns":2,"permission_denials":[{"tool_name":"Write","tool_use_id":"x"}]}"#;
        let r = parse(&stream(&[write_turn()], result), "opus");
        assert!(
            r.was_denied(),
            "a run that was never allowed to act must not read as a verdict"
        );
        assert_eq!(r.permission_denials, vec!["Write"]);
        assert_eq!(
            r.is_error,
            Some(false),
            "the stream itself calls this a success"
        );
    }

    #[test]
    fn turns_before_the_first_write_stop_counting_at_the_write() {
        let r = parse(
            &stream(
                &[text_turn(), text_turn(), write_turn(), text_turn()],
                ok_result(),
            ),
            "opus",
        );
        assert_eq!(r.turns_before_first_write, Some(2));
    }

    /// An agent that scaffolds immediately asked nothing.
    #[test]
    fn writing_on_the_first_turn_is_zero_turns_before() {
        let r = parse(&stream(&[write_turn()], ok_result()), "opus");
        assert_eq!(r.turns_before_first_write, Some(0));
    }

    /// A run that never wrote is not a run with an unknown count.
    #[test]
    fn a_run_that_never_wrote_counts_every_turn() {
        let r = parse(&stream(&[text_turn(), text_turn()], ok_result()), "opus");
        assert_eq!(r.turns_before_first_write, Some(2));
    }

    /// The parser against a real stream, not a hand-written approximation of one.
    ///
    /// Captured from `claude --print --verbose --output-format stream-json` and redacted;
    /// 29 lines carrying event types this parser ignores (`thinking_tokens`, `user`,
    /// `rate_limit_event`, `system/permission_denied`) as well as the three it reads. It is
    /// the run that proved the harness could not write a file: the model asked for `Write`,
    /// the permission layer refused, and the process exited 0 calling itself a success.
    ///
    /// Committed so that a change to the CLI's event shape fails here rather than silently
    /// producing a record of `None`s.
    #[test]
    fn a_real_stream_parses() {
        let fixture = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../fixtures/denied-run.jsonl");
        let r = read(&fixture, "claude-haiku-4-5-20251001").unwrap();

        assert_eq!(
            r.model_resolved.as_deref(),
            Some("claude-haiku-4-5-20251001")
        );
        assert_eq!(r.num_turns, Some(2));
        assert_eq!(r.subtype.as_deref(), Some("success"));
        assert_eq!(r.is_error, Some(false));
        assert!(r.total_cost_usd.is_some_and(|c| c > 0.0));
        assert!(r.duration_ms.is_some_and(|d| d > 0));

        assert_eq!(r.permission_denials, vec!["Write"]);
        assert!(
            r.was_denied(),
            "the exit code said success and no file was written; only the denial says why"
        );
    }

    #[test]
    fn a_malformed_line_does_not_lose_the_run() {
        let mut s = stream(&[text_turn()], ok_result());
        s = s.replace(r#"{"type":"rate_limit_event"}"#, "{not json");
        let r = parse(&s, "opus");
        assert_eq!(
            r.num_turns,
            Some(6),
            "one bad line must not discard the result event"
        );
    }
}
