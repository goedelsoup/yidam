//! `yidam retrieve` — the retrieval an agent gets, answered at a shell.
//!
//! # What this can and cannot reach
//!
//! The light default build has no embedder, and no test in this repository can stand up a
//! vector bucket, so what runs here is the **keyword arm and the argument surface**: the
//! degraded reason, the rejection a misspelled name earns, the exit code, and the two fields
//! contract 0.24.0 added — `scope`, which a light build must report as `local` however it is
//! asked, and `corpus`, which it must report as null on every row rather than omitting.
//!
//! Those are exactly the halves a spanning call gets wrong without failing: a server that
//! echoed `across` because a span was *asked* for, and one that left `corpus` off because it
//! never has a foreign row to put in it. The foreign row itself is unit-tested where it can be
//! — `cmd::retrieve`'s renderer against a payload, and `s3vectors::response` against a
//! recorded page — because the thing that produces one is a network this suite has no access
//! to.
//!
//! It is `retrieve`'s own binary and not `serve --mcp`'s: `mcp_serve.rs` runs the contract's
//! cases against the server, and this asserts the command dispatches to the same answer.

mod common;

use common::Example;

fn payload(out: &str) -> serde_json::Value {
    serde_json::from_str(out).unwrap_or_else(|e| panic!("not JSON: {e}\n{out}"))
}

#[test]
fn the_command_answers_the_retrieval_the_server_would() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&["retrieve", "hydropeaking", "--k", "3"]);
    assert_eq!(code, 0, "{stderr}");
    // The example carries no index, so this is the keyword arm — the same reason
    // `serve --mcp` reports against it, and the one every build can reach.
    assert!(stdout.contains("degraded: no_index"), "{stdout}");
    assert!(stdout.contains("scope local"), "{stdout}");
    assert!(
        stdout.contains("concept/hydropeaking"),
        "the node whose label is the query did not come back:\n{stdout}"
    );
}

/// `scope` is `local` and every row's `corpus` is null — pinned by value, not by presence.
///
/// A server that reported `across` because a span was requested would pass a presence check
/// and be wrong about the one thing the field is for.
#[test]
fn a_build_that_cannot_span_says_local_and_names_no_corpus() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&["retrieve", "hydropeaking", "--format", "json"]);
    assert_eq!(code, 0, "{stderr}");
    let body = payload(&stdout);
    assert_eq!(body["scope"], "local");
    let rows = body["results"].as_array().expect("results is an array");
    assert!(!rows.is_empty(), "nothing came back:\n{stdout}");
    for row in rows {
        assert!(
            row.get("corpus").is_some(),
            "a result omitted `corpus`, which contract 0.24.0 requires present:\n{row}"
        );
        assert_eq!(row["corpus"], serde_json::Value::Null);
        assert_eq!(row["truncated"], false);
        // The id is the handle `get_node` resolves, and on this arm every row has one.
        assert!(row["id"].is_string(), "{row}");
    }
}

/// A corpus nobody declared is a rejection, before any search.
///
/// And the span that *was* asked for did not silently happen: `scope` reads `local` and no
/// rows came back at all, which is what distinguishes a refusal from an empty answer.
#[test]
fn a_corpus_this_repository_never_declared_is_rejected_rather_than_searched() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&[
        "retrieve",
        "hydropeaking",
        "--corpora",
        "ohio-budget",
        "--format",
        "json",
    ]);
    // Nonzero at a shell, where a misspelled argument should fail a script — and an answer
    // rather than a crash, which is why the report is on stdout and parses.
    assert_eq!(code, 1, "{stderr}");
    let body = payload(&stdout);
    assert_eq!(body["rejected"]["code"], "unknown-corpus");
    assert!(
        body["rejected"]["message"]
            .as_str()
            .unwrap_or_default()
            .contains("declares no"),
        "the message should say this corpus declares no names: {}",
        body["rejected"]["message"]
    );
    assert_eq!(body["scope"], "local");
    assert_eq!(body["results"].as_array().map(Vec::len), Some(0));
}

/// A genesis hash is a corpus name even where no nickname was declared, so asking about one
/// takes no config edit. It is accepted and then answers `local`, because a local index holds
/// one corpus whatever it was asked.
#[test]
fn a_genesis_hash_is_a_name_and_a_local_index_still_answers_local() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&[
        "retrieve",
        "hydropeaking",
        "--corpora",
        "0123456789ab",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let body = payload(&stdout);
    assert_eq!(body["rejected"], serde_json::Value::Null, "{stdout}");
    assert_eq!(body["scope"], "local");
}

/// A class this corpus does not declare is the older rejection, unchanged — and it is checked
/// here because both arguments now reject, and a refactor that broke one while fixing the
/// other would look green.
#[test]
fn a_class_this_corpus_does_not_declare_is_still_rejected() {
    let e = Example::materialize("streamflow");
    let (stdout, _, code) = e.run(&[
        "retrieve",
        "hydropeaking",
        "--class",
        "concpt",
        "--format",
        "json",
    ]);
    assert_eq!(code, 1);
    assert_eq!(payload(&stdout)["rejected"]["code"], "unknown-class");
}
