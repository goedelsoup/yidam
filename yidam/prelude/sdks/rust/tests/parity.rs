use yidam_core::{corpus, git, graph, markers};

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

// ── parse_node ────────────────────────────────────────────────────────────────

#[test]
fn parity_parse_node() {
    let fixtures = load_fixtures("parse_node");
    assert!(!fixtures.is_empty(), "no parse_node fixtures found");

    for fx in &fixtures {
        let input = &fx["input"];
        let expected = &fx["expected"];

        let node = corpus::parse_node(
            input["path"].as_str().unwrap(),
            input["content"].as_str().unwrap(),
        );

        assert_eq!(node.path, expected["path"].as_str().unwrap(), "path");
        assert_eq!(node.title, expected["title"].as_str().unwrap(), "title");
        assert_eq!(
            node.kind.as_str(),
            expected["kind"].as_str().unwrap(),
            "kind"
        );

        let exp_claims = expected.get("claims").and_then(|v| v.as_array());
        let exp_claims = exp_claims.map(|a| a.as_slice()).unwrap_or(&[]);
        assert_eq!(node.claims.len(), exp_claims.len(), "claim count");
        for (claim, exp) in node.claims.iter().zip(exp_claims.iter()) {
            assert_eq!(claim.text, exp["text"].as_str().unwrap(), "claim.text");
            assert_eq!(
                claim.tag.as_str(),
                exp["tag"].as_str().unwrap(),
                "claim.tag"
            );
        }

        let exp_links = expected.get("links").and_then(|v| v.as_array());
        let exp_links = exp_links.map(|a| a.as_slice()).unwrap_or(&[]);
        assert_eq!(node.links.len(), exp_links.len(), "link count");
        for (link, exp) in node.links.iter().zip(exp_links.iter()) {
            assert_eq!(link.label, exp["label"].as_str().unwrap(), "link.label");
            assert_eq!(link.target, exp["target"].as_str().unwrap(), "link.target");
            let exp_anchor = exp.get("anchor").and_then(|v| v.as_str());
            assert_eq!(link.anchor.as_deref(), exp_anchor, "link.anchor");
        }
    }
}

// ── extract_claims ────────────────────────────────────────────────────────────

#[test]
fn parity_extract_claims() {
    let fixtures = load_fixtures("extract_claims");
    assert!(!fixtures.is_empty(), "no extract_claims fixtures found");

    for fx in &fixtures {
        let text = fx["input"]["content"].as_str().unwrap();
        let claims = corpus::extract_claims(text);

        let expected = fx["expected"].as_array().unwrap();
        assert_eq!(claims.len(), expected.len(), "claim count");
        for (claim, exp) in claims.iter().zip(expected.iter()) {
            assert_eq!(claim.text, exp["text"].as_str().unwrap(), "claim.text");
            assert_eq!(
                claim.tag.as_str(),
                exp["tag"].as_str().unwrap(),
                "claim.tag"
            );
        }
    }
}

// ── extract_links ─────────────────────────────────────────────────────────────

#[test]
fn parity_extract_links() {
    let fixtures = load_fixtures("extract_links");
    assert!(!fixtures.is_empty(), "no extract_links fixtures found");

    for fx in &fixtures {
        let text = fx["input"]["content"].as_str().unwrap();
        let links = corpus::extract_links(text);

        let expected = fx["expected"].as_array().unwrap();
        assert_eq!(links.len(), expected.len(), "link count");
        for (link, exp) in links.iter().zip(expected.iter()) {
            assert_eq!(link.label, exp["label"].as_str().unwrap(), "link.label");
            assert_eq!(link.target, exp["target"].as_str().unwrap(), "link.target");
            let exp_anchor = exp.get("anchor").and_then(|v| v.as_str());
            assert_eq!(link.anchor.as_deref(), exp_anchor, "link.anchor");
        }
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
        let parsed = markers::parse_markers(text);

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
