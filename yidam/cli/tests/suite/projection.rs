//! A projected corpus, end to end through the binary (RFC-0064, #1510).
//!
//! The unit tests in `src/projection.rs` hold the parsing and the refusal's wording. What
//! they cannot hold is the wiring: that `status`, `doctor` and the RDF export each ask the
//! mode on the corpus they were given, and that a writer asks it before touching anything.
//! Each of those is one call site that compiles just as well without the call, so each is
//! asserted here against the example corpus with `[corpus]` written into it.

use crate::common;

const PRODUCER: &str = "example-producer mirror";
const ID: &str = "0123456789ab";

/// streamflow, declared a projection with an id its producer wrote.
fn projected() -> common::Example {
    let example = common::Example::materialize("streamflow");
    let root = example.path();
    let config = root.join(".yidam/config.toml");
    let text = std::fs::read_to_string(&config).unwrap_or_default();
    let text = format!("{text}\n[corpus]\nmode = \"projection\"\ngenerated_by = \"{PRODUCER}\"\n");
    std::fs::write(&config, text).unwrap();
    std::fs::write(
        root.join(".yidam/corpus/corpus.toml"),
        format!("id = \"{ID}\"\n"),
    )
    .unwrap();
    example
}

#[test]
fn a_writer_refuses_and_names_the_producer() {
    let example = projected();
    let before = std::fs::read_to_string(
        example
            .path()
            .join(".yidam/corpus/concept/hydropeaking.yml"),
    )
    .unwrap();

    let (out, err, code) = example.run(&["rename", "concept/hydropeaking", "concept/peaking"]);
    assert_ne!(code, 0, "`rename` ran in a projection:\n{out}{err}");
    assert!(
        err.contains(PRODUCER) && err.contains("yidam rename"),
        "the refusal must name the producer and the command: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(
            example
                .path()
                .join(".yidam/corpus/concept/hydropeaking.yml")
        )
        .ok()
        .as_deref(),
        Some(before.as_str()),
        "`rename` touched the corpus before refusing"
    );
}

#[test]
fn status_reports_the_mode_and_no_genesis_date() {
    let example = projected();
    let (out, err, code) = example.run(&["status", "--format", "json"]);
    assert_eq!(code, 0, "`status` failed on a projection: {err}");
    let v: serde_json::Value = serde_json::from_str(&out).expect("status json");
    let body = v.get("data").unwrap_or(&v);
    assert_eq!(
        body.get("mode").and_then(|m| m.as_str()),
        Some("projection"),
        "`status` did not report the mode: {out:.600}"
    );
    assert!(
        body.get("genesis").is_some_and(|g| g.is_null()),
        "a projection's genesis date is the host's, so `status` reports none: {out:.600}"
    );
}

#[test]
fn doctor_reports_the_mode_and_names_the_producer() {
    let example = projected();
    let (out, err, _) = example.run(&["doctor", "--format", "json"]);
    assert!(
        out.contains("\"mode\"") && out.contains(PRODUCER),
        "`doctor` must carry a `mode` check naming the producer: {out:.800}{err:.400}"
    );
}

#[test]
fn the_rdf_export_names_the_corpus_by_its_declared_id() {
    let example = projected();
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("corpus.ttl");
    let (_, err, code) =
        example.run(&["export", "--format", "rdf", "--out", file.to_str().unwrap()]);
    assert_eq!(
        code, 0,
        "`export --format rdf` failed on a projection: {err}"
    );
    let ttl = std::fs::read_to_string(&file).unwrap();
    assert!(
        ttl.contains(&format!("urn:yidam:{ID}")),
        "the export must name the corpus by the id its producer declared, not the host's \
         genesis: {ttl:.600}"
    );

    std::fs::remove_file(example.path().join(".yidam/corpus/corpus.toml")).unwrap();
    let (_, err, code) =
        example.run(&["export", "--format", "rdf", "--out", file.to_str().unwrap()]);
    assert_ne!(code, 0, "a projection with no declared id exported anyway");
    assert!(
        err.contains(PRODUCER),
        "a missing id is the producer's to write, so the refusal names it: {err}"
    );
}
