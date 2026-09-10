//! MCP resources — the corpus, skills, and decisions exposed as `yidam://` URIs.
//!
//! # One reading of `yidam://`
//!
//! This surface used to answer *what does this URI name?* with its own chain of
//! `strip_prefix` calls, and [`yidam_core::uri`] answers the same question for every other
//! caller in the repository. Two readers of one scheme is the defect RFC-0032 §1 is about,
//! reproduced inside the fix — so [`parse_resource`] reads the grammar's parse and adds only
//! what the grammar deliberately cannot name.
//!
//! **Two of RFC-0005's five frozen URIs are not references (#779).**
//! `yidam://corpus/<class>` names a *set* of things and `yidam://graph/summary` a *computed
//! view*; the grammar addresses things with identity, and forcing either into a kind would make
//! `reference_conforms` false for a URI the contract declares valid. So they are named here,
//! where they are served, as [`Resource::Collection`] and [`Resource::Report`] — one parser, two
//! stated exceptions, rather than a second vocabulary that happens to overlap.
//!
//! # What this surface does not gain
//!
//! **A corpus slot.** The addressing plan had this step add one, so a client could fetch a node
//! from a named dependency. Measured before building it: `ReadMcpResourceTool` was invoked 0
//! times across 1,638 local sessions, for any server, and 0 of the 16 derived corpora write
//! `cites:` or vendor a dependency. It would have been a slot for an address nobody writes,
//! reachable by a mechanism nobody calls, and it spends a contract minor bump to ship. So a
//! parsed reference naming *any* corpus is refused here, in one arm that says so — the decision
//! is in the code rather than in its absence.
//!
//! # Reads through the grammar, writes the frozen spelling
//!
//! [`list`] keeps emitting `yidam://corpus/<class>/<name>`, which is not what
//! `render_reference` would emit for the same node. RFC-0005 froze the spelling and this step
//! spends no contract bump, so the freeze wins; what is unified is the *reading*. A test asserts
//! the two halves agree — every URI `list` emits, `read` resolves.

use serde_json::{json, Value};
use std::collections::BTreeMap;

use yidam_core::uri::{parse_reference, Kind, Reference};

use super::{RpcError, ServerState};

/// What a `yidam://` URI names, in the one reading this repository has.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Resource {
    /// A thing with identity — a node, a skill, a decision — as the grammar reads it.
    Thing(Reference),
    /// Every instance of a class. A set of things, and so not a reference (#779).
    Collection(String),
    /// A computed view. Not a reference either (#779).
    Report(String),
}

/// Which of the three a URI names, or `None` when this surface does not serve it.
///
/// **Arity and kind decide, not [`yidam_core::uri::reference_conforms`].** Conformance answers
/// *may this be written into a URI*, which is a different question from *does this corpus
/// contain it*: two of the sixteen measured corpora name classes `NonConformance` and
/// `DischargePoint` (#777), and gating on conformance here would stop serving their nodes. The
/// grammar's job is to parse the string; the corpus decides what exists.
fn parse_resource(uri: &str) -> Option<Resource> {
    if !uri.starts_with("yidam://") {
        return None;
    }
    let r = parse_reference(uri)?;
    // A pin and a property path are both meaningful in a reference and neither is servable here:
    // this surface answers for the working corpus at HEAD and holds no revision machinery.
    if r.rev.is_some() || r.fragment.is_some() {
        return None;
    }
    match r.corpus.as_deref() {
        // RFC-0005 spends the authority slot on `graph` for its one report, which costs a corpus
        // actually named `graph` its address on this surface — a cost the freeze already imposed
        // and this only makes visible.
        Some("graph") => Some(Resource::Report(r.path)),
        // The corpus slot this step deliberately did not add.
        //
        // This arm also swallows `yidam://catalog/x` and `yidam://issue/1`, which
        // `split_authority` reads as corpora named `catalog` and `issue` because it unfreezes
        // only three authority words. Neither is in RFC-0005's five, so refusing them is the
        // right outcome by the wrong route; the day one becomes a resource, the authority slot
        // is where to look.
        Some(_) => None,
        None => match (r.kind, r.segments().len()) {
            (Kind::Node, 2) => Some(Resource::Thing(r)),
            (Kind::Node, 1) => Some(Resource::Collection(r.path)),
            (Kind::Skill | Kind::Decision, 1) => Some(Resource::Thing(r)),
            _ => None,
        },
    }
}

fn text_contents(uri: &str, text: String) -> Value {
    json!({"contents": [{"uri": uri, "mimeType": "text/plain", "text": text}]})
}

/// True when the node counts as an open question, by any of the three arms the frozen
/// contract names: a `?`-prefixed label, an `[open]` claim in the body, or an open tag in a
/// property the class declared `type: claim`. Shared with the `open_questions` tool.
///
/// One predicate, shared with the reports.
///
/// This was a second copy of `label.starts_with('?') || content.contains("[open]")`, so a
/// corpus whose tags are structured was under-reported by the MCP server and by
/// `open-questions` in exactly the same way — which is the kind of agreement that looks like
/// correctness.
pub(crate) fn is_open_question(state: &ServerState, node: &super::Node) -> bool {
    crate::claims::is_open_question(
        &node.label,
        &node.content,
        state.claim_fields.for_class(&node.class),
    )
}

fn classes(state: &ServerState) -> BTreeMap<String, usize> {
    let mut map = BTreeMap::new();
    for node in &state.nodes {
        *map.entry(node.class.clone()).or_insert(0) += 1;
    }
    map
}

pub(crate) fn list(state: &ServerState) -> Value {
    let mut resources = vec![json!({
        "uri": "yidam://graph/summary",
        "name": "graph summary",
        "description": "Class list, node count, and open-question count",
        "mimeType": "text/plain"
    })];

    for (class, count) in classes(state) {
        resources.push(json!({
            "uri": format!("yidam://corpus/{class}"),
            "name": class,
            "description": format!("All {count} instance(s) of class {class:?}"),
            "mimeType": "text/plain"
        }));
    }
    for node in &state.nodes {
        resources.push(json!({
            "uri": format!("yidam://corpus/{}", node.id),
            "name": node.label,
            "description": node.description,
            "mimeType": "text/plain"
        }));
    }
    for (name, _) in &state.skills {
        resources.push(json!({
            "uri": format!("yidam://skills/{name}"),
            "name": name,
            "mimeType": "text/plain"
        }));
    }
    for (name, _) in &state.decisions {
        resources.push(json!({
            "uri": format!("yidam://decisions/{name}"),
            "name": name,
            "mimeType": "text/plain"
        }));
    }

    json!({"resources": resources})
}

pub(crate) fn read(state: &ServerState, uri: &str) -> Result<Value, RpcError> {
    let not_found = || RpcError::invalid_params(format!("unknown resource: {uri}"));
    let text = match parse_resource(uri).ok_or_else(not_found)? {
        Resource::Report(name) if name == "summary" => graph_summary(state),
        Resource::Report(_) => return Err(not_found()),
        Resource::Collection(class) => {
            let mut lines: Vec<String> = state
                .nodes
                .iter()
                .filter(|n| n.class == class)
                .map(|n| format!("- {} ({}): {}", n.label, n.id, n.description))
                .collect();
            if lines.is_empty() {
                return Err(not_found());
            }
            lines.insert(0, format!("Instances of class {class:?}:"));
            lines.join("\n")
        }
        Resource::Thing(r) => match r.kind {
            Kind::Node => state
                .nodes
                .iter()
                .find(|n| n.id == r.path)
                .ok_or_else(not_found)?
                .content
                .clone(),
            Kind::Skill => named(&state.skills, &r.path).ok_or_else(not_found)?,
            Kind::Decision => named(&state.decisions, &r.path).ok_or_else(not_found)?,
            // `parse_resource` admits no other kind as a `Thing`; this arm exists so adding one
            // to the grammar is a compile error here rather than a resource that silently 404s.
            Kind::Crate | Kind::Catalog | Kind::Issue => return Err(not_found()),
        },
    };
    Ok(text_contents(uri, text))
}

fn named(pairs: &[(String, String)], name: &str) -> Option<String> {
    pairs
        .iter()
        .find(|(n, _)| n == name)
        .map(|(_, content)| content.clone())
}

fn graph_summary(state: &ServerState) -> String {
    let open = state
        .nodes
        .iter()
        .filter(|n| is_open_question(state, n))
        .count();
    let class_lines: Vec<String> = classes(state)
        .iter()
        .map(|(class, count)| format!("  {class}: {count} instance(s)"))
        .collect();
    format!(
        "Domain: {}\nCommit: {}\nNodes: {}\nOpen questions: {}\nSkills: {}\nDecisions: {}\n\
         Vector index: {}\nIndex freshness: {}\nClasses:\n{}",
        state.domain,
        state.commit,
        state.nodes.len(),
        open,
        state.skills.len(),
        state.decisions.len(),
        match &state.retrieval {
            #[cfg(feature = "vector-read")]
            super::Retrieval::Vector(idx) =>
                format!("{} row(s), model {}", idx.rows.len(), idx.model_id),
            super::Retrieval::NoIndex => "absent (no_index)".to_string(),
            #[cfg(not(feature = "vector-read"))]
            super::Retrieval::NoVectorSupport =>
                "present, unreadable by this build (no_vector_support)".to_string(),
        },
        // The same fact the handshake reports, from the same `stale_index()`. This resource
        // and `capabilities.corpus` are two renderings of one answer to "which corpus is
        // this", and the point of #424 is that they stop being two different answers.
        match (state.stale_index(), &state.indexed_commit) {
            (None, _) => "unknown — no working repository to compare against".to_string(),
            (Some(true), Some(indexed)) => {
                format!("STALE — index built at {indexed}, HEAD is {}", state.commit)
            }
            (Some(_), None) => "no index".to_string(),
            (Some(false), Some(_)) => "current".to_string(),
        },
        class_lines.join("\n"),
    )
}

#[cfg(test)]
mod tests {
    use super::super::tests::test_state;
    use super::*;

    #[test]
    fn list_includes_all_five_resource_kinds() {
        let state = test_state();
        let result = list(&state);
        let uris: Vec<String> = result["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["uri"].as_str().unwrap().to_string())
            .collect();
        assert!(uris.contains(&"yidam://graph/summary".to_string()));
        assert!(uris.contains(&"yidam://corpus/concept".to_string()));
        assert!(uris.contains(&"yidam://corpus/concept/knowledge-graph".to_string()));
        assert!(uris.contains(&"yidam://skills/my-skill".to_string()));
        assert!(uris.contains(&"yidam://decisions/adr-1".to_string()));
    }

    #[test]
    fn read_instance_returns_yaml_source() {
        let state = test_state();
        let result = read(&state, "yidam://corpus/concept/knowledge-graph").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("label: Knowledge graph"));
    }

    #[test]
    fn read_class_lists_instances() {
        let state = test_state();
        let result = read(&state, "yidam://corpus/concept").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("Knowledge graph"));
        assert!(text.contains("concept/traversal"));
    }

    #[test]
    fn read_graph_summary_counts_open_questions() {
        let state = test_state();
        let result = read(&state, "yidam://graph/summary").unwrap();
        let text = result["contents"][0]["text"].as_str().unwrap();
        assert!(text.contains("Nodes: 2"));
        assert!(text.contains("Open questions: 1"));
    }

    #[test]
    fn read_unknown_uri_errors() {
        let state = test_state();
        assert!(read(&state, "yidam://corpus/nope/nothing").is_err());
        assert!(read(&state, "other://x").is_err());
    }

    /// The two halves of this surface, held to each other.
    ///
    /// `list` writes the frozen spelling and `read` parses through the grammar, which is only
    /// safe while they agree about every URI. Nothing asserted that before: the chain of
    /// `strip_prefix` calls this replaces was written to match what `list` emitted, and a
    /// divergence between them would have shown up as a client 404 rather than as a red test.
    #[test]
    fn every_uri_the_listing_offers_is_one_the_reader_resolves() {
        let state = test_state();
        let listed = list(&state);
        let uris: Vec<String> = listed["resources"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["uri"].as_str().unwrap().to_string())
            .collect();
        assert!(
            uris.len() >= 5,
            "the listing has to offer something: {uris:?}"
        );
        for uri in &uris {
            assert!(read(&state, uri).is_ok(), "listed but unreadable: {uri}");
        }
    }

    /// The five frozen URIs, each landing in the case that serves it.
    #[test]
    fn each_frozen_uri_lands_in_the_case_that_serves_it() {
        assert!(matches!(
            parse_resource("yidam://graph/summary"),
            Some(Resource::Report(ref n)) if n == "summary"
        ));
        assert!(matches!(
            parse_resource("yidam://corpus/concept"),
            Some(Resource::Collection(ref c)) if c == "concept"
        ));
        for (uri, kind, path) in [
            (
                "yidam://corpus/concept/knowledge-graph",
                Kind::Node,
                "concept/knowledge-graph",
            ),
            ("yidam://skills/my-skill", Kind::Skill, "my-skill"),
            ("yidam://decisions/adr-1", Kind::Decision, "adr-1"),
        ] {
            match parse_resource(uri) {
                Some(Resource::Thing(r)) => {
                    assert_eq!((r.kind, r.path.as_str()), (kind, path), "{uri}")
                }
                other => panic!("{uri} read as {other:?}"),
            }
        }
    }

    /// The decision not to add a corpus slot, as a refusal rather than an absence.
    #[test]
    fn a_uri_naming_another_corpus_is_refused_rather_than_served_from_this_one() {
        let state = test_state();
        assert_eq!(parse_resource("yidam://producer/node/concept/thing"), None);
        assert!(read(&state, "yidam://producer/node/concept/knowledge-graph").is_err());
        // …and the same node without the corpus is served, so the refusal is about the slot.
        assert!(read(&state, "yidam://corpus/concept/knowledge-graph").is_ok());
    }

    /// A class whose name the grammar would refuse to *write* is still served.
    #[test]
    fn a_class_outside_the_slug_rule_is_still_addressable() {
        let uri = "yidam://corpus/NonConformance/DIS-2024-0447";
        match parse_resource(uri) {
            Some(Resource::Thing(r)) => {
                assert_eq!(r.path, "NonConformance/DIS-2024-0447");
                assert!(
                    !yidam_core::uri::reference_conforms(&r),
                    "the point of this test is that it does not conform and is served anyway"
                );
            }
            other => panic!("{uri} read as {other:?}"),
        }
    }

    /// A pin and a property path are meaningful in a reference and not on this surface.
    #[test]
    fn a_revision_or_a_property_path_is_not_a_resource() {
        assert_eq!(
            parse_resource("yidam://corpus/concept/knowledge-graph@abc123"),
            None
        );
        assert_eq!(
            parse_resource("yidam://corpus/concept/knowledge-graph#label"),
            None
        );
    }
}
