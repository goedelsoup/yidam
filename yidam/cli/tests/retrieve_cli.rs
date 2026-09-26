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

/// `--where` on the keyword arm: `reach.length_km` is declared `number` and holds 24 and 6
/// (#1029). The predicate runs before the `k` cut and reads the node `query` reads, so the
/// answer is the reach `query 'reach[length_km>10]'` returns and no other.
#[test]
fn a_where_keeps_only_the_rows_whose_typed_properties_satisfy_it() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&[
        "retrieve",
        "reach",
        "--where",
        "length_km>10",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stderr}");
    let p = payload(&stdout);
    assert_eq!(p["rejected"], serde_json::Value::Null, "{stdout}");
    let ids: Vec<&str> = p["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["reach/lower-canyon"], "{stdout}");

    // The complement, to pin that the number was compared as a number and not as text —
    // `"6" > "10"` lexically, and a lexical server returns the tailwater here.
    let (stdout, _, _) = e.run(&[
        "retrieve",
        "reach",
        "--where",
        "length_km<10",
        "--format",
        "json",
    ]);
    let p = payload(&stdout);
    let ids: Vec<&str> = p["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["reach/tailwater"], "{stdout}");
}

/// The rendering names the predicate beside the query, so `1 result` reads as one *of the
/// reaches over ten kilometres* and not one of everything.
#[test]
fn the_rendering_names_the_where() {
    let e = Example::materialize("streamflow");
    let (stdout, stderr, code) = e.run(&["retrieve", "reach", "--where", "length_km>10"]);
    assert_eq!(code, 0, "{stderr}");
    assert!(
        stdout.starts_with("retrieve \"reach\" where [length_km>10] — scope local"),
        "{stdout}"
    );
    assert!(stdout.contains("reach/lower-canyon"), "{stdout}");
    assert!(!stdout.contains("reach/tailwater"), "{stdout}");
}

/// A `where` that cannot be asked is rejected with `query`'s code and the shell's exit code,
/// the way a misspelled `--class` is.
#[test]
fn a_where_that_cannot_be_asked_is_rejected_as_query_would_reject_it() {
    let e = Example::materialize("streamflow");
    for (text, code) in [
        ("colour=red", "undeclared-property"),
        ("regulated>yes", "unordered-property"),
        ("length_km>ten", "unsatisfiable-predicate"),
        ("length_km>10,,", "parse"),
    ] {
        let (stdout, _, exit) = e.run(&["retrieve", "reach", "--where", text, "--format", "json"]);
        assert_eq!(exit, 1, "{text}: {stdout}");
        assert_eq!(
            payload(&stdout)["rejected"]["code"],
            code,
            "{text}: {stdout}"
        );
    }
}

/// A predicate nothing satisfies is `predicate-unsatisfied`, with the candidates it was
/// evaluated over as the denominator — not `no-term-match`, which would send a reader off to
/// rephrase a question the corpus answered.
#[test]
fn a_where_nothing_satisfies_is_its_own_kind_of_empty() {
    let e = Example::materialize("streamflow");
    let (stdout, _, code) = e.run(&[
        "retrieve",
        "reach",
        "--where",
        "length_km>100",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stdout}");
    let p = payload(&stdout);
    assert_eq!(p["absence"]["code"], "predicate-unsatisfied", "{stdout}");
    assert_eq!(p["absence"]["instances"], 2, "{stdout}");
}

/// A `where` that admits candidates the words then miss reports the candidates it searched,
/// not the class. Two reaches pass `length_km>0`; the query names neither; the count is two.
#[test]
fn a_where_narrows_what_no_term_match_counts() {
    let e = Example::materialize("streamflow");
    let (stdout, _, code) = e.run(&[
        "retrieve",
        "xylophone",
        "--where",
        "length_km>0",
        "--format",
        "json",
    ]);
    assert_eq!(code, 0, "{stdout}");
    let p = payload(&stdout);
    assert_eq!(p["absence"]["code"], "no-term-match", "{stdout}");
    assert_eq!(p["absence"]["instances"], 2, "{stdout}");
}

/// A `where` against an index built before the `properties` column is refused, not answered.
///
/// The alternative — evaluating the predicate over rows whose `properties` are all null and
/// reporting `predicate-unsatisfied` — is the wrong fact stated confidently. The index here
/// is one row with every column but that one, which is exactly what `index-build` wrote
/// before #1029; the rejection comes before the query is embedded, so no model is needed.
#[cfg(feature = "vector-read")]
#[test]
fn a_where_against_an_index_without_the_column_is_where_unindexed() {
    use arrow_array::{ArrayRef, FixedSizeListArray, Float32Array, RecordBatch, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    let e = Example::materialize("streamflow");
    let index_dir = e.path().join(".yidam").join("index");
    std::fs::create_dir_all(&index_dir).unwrap();

    let dim: i32 = 4;
    let item = Arc::new(Field::new("item", DataType::Float32, true));
    let schema = Arc::new(Schema::new(vec![
        Field::new("path", DataType::Utf8, false),
        Field::new("class", DataType::Utf8, false),
        Field::new("label", DataType::Utf8, false),
        Field::new("text", DataType::Utf8, false),
        Field::new("vector", DataType::FixedSizeList(item.clone(), dim), false),
    ]));
    let one = |s: &str| Arc::new(StringArray::from(vec![s])) as ArrayRef;
    let vector = FixedSizeListArray::new(
        item,
        dim,
        Arc::new(Float32Array::from(vec![0.0f32; dim as usize])),
        None,
    );
    let batch = RecordBatch::try_new(
        schema.clone(),
        vec![
            one(".yidam/corpus/reach/tailwater.yml"),
            one("reach"),
            one("Tailwater"),
            one("Tailwater reach"),
            Arc::new(vector),
        ],
    )
    .unwrap();
    let mut bytes = Vec::new();
    {
        let mut w = arrow_ipc::writer::FileWriter::try_new(&mut bytes, &schema).unwrap();
        w.write(&batch).unwrap();
        w.finish().unwrap();
    }
    std::fs::write(index_dir.join("corpus.arrow"), bytes).unwrap();
    std::fs::write(
        index_dir.join("meta.json"),
        r#"{"model_name":"m","indexed_commit":"abc123","node_count":1}"#,
    )
    .unwrap();

    let (stdout, _, code) = e.run(&[
        "retrieve",
        "reach",
        "--where",
        "length_km>10",
        "--format",
        "json",
    ]);
    assert_eq!(code, 1, "{stdout}");
    let p = payload(&stdout);
    assert_eq!(p["rejected"]["code"], "where-unindexed", "{stdout}");
    let message = p["rejected"]["message"].as_str().unwrap();
    assert!(
        message.contains("yidam embed && yidam index-build"),
        "the rejection must name the repair:\n{message}"
    );
}
