use yidam_core::{corpus, git, graph, markers, ontology};

fn fixture_dir(function: &str) -> std::path::PathBuf {
    // CARGO_MANIFEST_DIR = prelude/sdks/rust/
    // ../parity/fixtures/<function>  →  prelude/sdks/parity/fixtures/<function>
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../parity/fixtures")
        .join(function)
}

fn load_fixtures(function: &str) -> Vec<toml::Value> {
    let dir = fixture_dir(function);
    if !dir.exists() {
        return vec![];
    }
    let mut out = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|x| x == "toml"))
        .collect();
    entries.sort_by_key(|e| e.path());
    for entry in entries {
        let raw = std::fs::read_to_string(entry.path()).unwrap();
        out.push(toml::from_str::<toml::Value>(&raw).unwrap());
    }
    out
}

// ── parse_instance ────────────────────────────────────────────────────────────

/// The instance is compared as **parsed JSON**, for `compile_class_schema`'s reason.
///
/// Key order is not part of the contract and three languages will not agree on it. What is
/// part of the contract is every string value, the key sets of `properties` and `extra`, and
/// the difference between a key that is absent (`null`) and one that is present and empty.
///
/// **Non-string scalar resolution is out of contract, and the fixtures avoid it.** Given
/// `010`, `serde_yaml` answers the string `"010"`, the `yaml` package answers the number
/// `10`, and PyYAML answers `8`; `no` is a string in the first two and `false` in the third.
/// That is YAML 1.1 against 1.2 plus one library's own reading, and no amount of fixture
/// writing makes three implementations of two specifications agree. The ontology is what
/// types these values, and every consumer reads them as text. The one such divergence that a
/// corpus actually hits — the timestamp, on 41% of nodes — is *in* contract and is pinned by
/// `unquoted-dates-stay-text.toml`.
#[test]
fn parity_parse_instance() {
    let fixtures = load_fixtures("parse_instance");
    assert!(!fixtures.is_empty(), "no parse_instance fixtures found");

    for fx in &fixtures {
        let description = fx["description"].as_str().unwrap_or("");
        let content = fx["input"]["content"].as_str().unwrap();
        let got = corpus::parse_instance(content).to_json();

        let want: serde_json::Value =
            serde_json::from_str(fx["expected"]["instance"].as_str().unwrap())
                .unwrap_or_else(|e| panic!("{description}: fixture instance is not JSON: {e}"));

        assert_eq!(got, want, "{description}");
    }
}

// ── classify_commit ───────────────────────────────────────────────────────────

#[test]
fn parity_classify_commit() {
    let fixtures = load_fixtures("classify_commit");
    assert!(!fixtures.is_empty(), "no classify_commit fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let expected = &fx["expected"];

        let event = git::classify_commit(
            input["hash"].as_str().unwrap(),
            input["message"].as_str().unwrap(),
        );

        assert_eq!(
            event.kind.as_str(),
            expected["kind"].as_str().unwrap(),
            "kind"
        );
        assert_eq!(event.verb, expected["verb"].as_str().unwrap(), "verb");
        assert_eq!(
            event.subject,
            expected["subject"].as_str().unwrap(),
            "subject"
        );
    }
}

// ── is_recognized_verb ────────────────────────────────────────────────────────

#[test]
fn parity_is_recognized_verb() {
    let fixtures = load_fixtures("is_recognized_verb");
    assert!(!fixtures.is_empty(), "no is_recognized_verb fixtures found");

    for fx in &fixtures {
        let verb = fx["input"]["verb"].as_str().unwrap();
        let expected = fx["expected"]["recognized"].as_bool().unwrap();
        assert_eq!(
            git::is_recognized_verb(verb),
            expected,
            "is_recognized_verb({verb:?})"
        );
    }
}

// ── parse_markers ─────────────────────────────────────────────────────────────

#[test]
fn parity_parse_markers() {
    let fixtures = load_fixtures("parse_markers");
    assert!(!fixtures.is_empty(), "no parse_markers fixtures found");

    for fx in &fixtures {
        let text = fx["input"]["content"].as_str().unwrap();
        let scan = markers::scan_markers(text);
        let parsed = &scan.markers;

        let expected = fx["expected"].as_array().unwrap();
        assert_eq!(parsed.len(), expected.len(), "marker count");

        for (marker, exp) in parsed.iter().zip(expected.iter()) {
            let exp_kind = exp["kind"].as_str().unwrap();
            assert_eq!(marker.kind_str(), exp_kind, "marker.kind");
            match marker {
                markers::Marker::Template { instruction } => {
                    assert_eq!(
                        instruction,
                        exp["instruction"].as_str().unwrap(),
                        "template.instruction"
                    );
                }
                markers::Marker::Regen { command, content } => {
                    assert_eq!(command, exp["command"].as_str().unwrap(), "regen.command");
                    assert_eq!(content, exp["content"].as_str().unwrap(), "regen.content");
                }
            }
        }

        // Absent means none, not "not checked". A fixture written before this field existed
        // is asserting that its input has no malformed block — which is what makes adding
        // the field to three runners safe, and what would catch a scan that started
        // reporting one on well-formed input.
        let empty: Vec<toml::Value> = Vec::new();
        let bad = fx
            .get("expected_malformed")
            .and_then(|v| v.as_array())
            .unwrap_or(&empty);
        assert_eq!(
            scan.malformed.len(),
            bad.len(),
            "malformed block count in {:?}: {:?}",
            fx["description"].as_str().unwrap_or(""),
            scan.malformed
        );
        for (got, exp) in scan.malformed.iter().zip(bad.iter()) {
            assert_eq!(
                got.command,
                exp["command"].as_str().unwrap(),
                "malformed.command"
            );
            assert_eq!(
                got.line as i64,
                exp["line"].as_integer().unwrap(),
                "malformed.line"
            );
            assert_eq!(
                got.fault.as_str(),
                exp["fault"].as_str().unwrap(),
                "malformed.fault"
            );
            assert_eq!(
                got.swallowed_lines as i64,
                exp["swallowed_lines"].as_integer().unwrap(),
                "malformed.swallowed_lines"
            );
            assert_eq!(
                got.swallowed_markers as i64,
                exp["swallowed_markers"].as_integer().unwrap(),
                "malformed.swallowed_markers"
            );
        }
    }
}

// ── find_reachable ────────────────────────────────────────────────────────────

#[test]
fn parity_find_reachable() {
    let fixtures = load_fixtures("find_reachable");
    assert!(!fixtures.is_empty(), "no find_reachable fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let node_path = input["node_path"].as_str().unwrap();
        let edges: Vec<graph::GraphEdge> = input["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| graph::GraphEdge {
                from: e["from"].as_str().unwrap().to_string(),
                to: e["to"].as_str().unwrap().to_string(),
            })
            .collect();

        let reachable = graph::find_reachable(&edges, node_path);

        let expected: Vec<&str> = fx["expected"]["reachable"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(reachable, expected, "find_reachable({node_path})");
    }
}

// ── find_citations ────────────────────────────────────────────────────────────

#[test]
fn parity_find_citations() {
    let fixtures = load_fixtures("find_citations");
    assert!(!fixtures.is_empty(), "no find_citations fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let node_path = input["node_path"].as_str().unwrap();
        let edges: Vec<graph::GraphEdge> = input["edges"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| graph::GraphEdge {
                from: e["from"].as_str().unwrap().to_string(),
                to: e["to"].as_str().unwrap().to_string(),
            })
            .collect();

        let citations = graph::find_citations(&edges, node_path);

        let expected: Vec<&str> = fx["expected"]["citations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap())
            .collect();
        assert_eq!(citations, expected, "find_citations({node_path})");
    }
}

// ── update_regen ──────────────────────────────────────────────────────────────

#[test]
fn parity_update_regen() {
    let fixtures = load_fixtures("update_regen");
    assert!(!fixtures.is_empty(), "no update_regen fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let result = markers::update_regen(
            input["content"].as_str().unwrap(),
            input["command"].as_str().unwrap(),
            input["new_content"].as_str().unwrap(),
        );
        assert_eq!(result, fx["expected"]["content"].as_str().unwrap());
    }
}

// ── compile_class_schema ──────────────────────────────────────────────────────

/// The compiled schema is compared as **parsed JSON**, not as text.
///
/// Key order and whitespace are not part of the contract — three languages will not agree
/// on either, and a fixture that demanded they did would be pinning serializer behaviour
/// while claiming to pin a schema.
#[test]
fn parity_compile_class_schema() {
    let fixtures = load_fixtures("compile_class_schema");
    assert!(
        !fixtures.is_empty(),
        "no compile_class_schema fixtures found"
    );

    for fx in &fixtures {
        let input = &fx["input"];
        let name = input["name"].as_str().unwrap();
        let class = ontology::parse_class(name, input["content"].as_str().unwrap());
        let got = ontology::compile_class_schema(&class);

        let want: serde_json::Value =
            serde_json::from_str(fx["expected"]["schema"].as_str().unwrap())
                .unwrap_or_else(|e| panic!("{name}: fixture schema is not JSON: {e}"));

        assert_eq!(
            got,
            want,
            "{name}: {}",
            fx["description"].as_str().unwrap_or("")
        );
    }
}
