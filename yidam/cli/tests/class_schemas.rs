//! The compiled per-class schemas must accept exactly what the gate accepts.
//!
//! This is the whole reason `.ont.yml` is compiled rather than described. A consumer that
//! validates a corpus for itself and a gate that validates the same corpus can disagree in
//! one direction that really hurts: the schema rejecting an instance `yidam lint` is happy
//! with, so a file that passes CI fails in somebody's editor and the ontology gets blamed.
//!
//! RFC-0002 documents that failure at the node-model layer and RFC-0005 at the MCP layer.
//! This is the same shape one level down, and the example corpus is where it would show up
//! first — it is the corpus people copy.

use std::path::Path;
use std::process::Command;

mod common;

use common::{examples, repo_root, tracked_under};

/// The example whose `gage` class the compiler-behaviour tests below are written against.
const STREAMFLOW: &str = "streamflow";

/// Materialize `examples/<name>` as a standalone repository.
///
/// From `git ls-files`, matching how every other suite here builds a tree: a directory walk
/// would pick up local scratch and this test would be measuring the working directory.
fn example(name: &str) -> tempfile::TempDir {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    let prefix = format!("examples/{name}/");
    let files = tracked_under(&root, &prefix);
    assert!(!files.is_empty(), "no tracked files under {prefix}");
    for tracked in &files {
        let to = dir.path().join(tracked.strip_prefix(&prefix).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(tracked), &to).unwrap();
    }
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "example@yidam.test"],
        vec!["config", "user.name", "Example"],
        vec!["add", "-A"],
        vec!["commit", "-q", "-m", "genesis: the example corpus"],
    ] {
        assert!(Command::new("git")
            .current_dir(dir.path())
            .args(&args)
            .status()
            .unwrap()
            .success());
    }
    dir
}

fn compiled(root: &Path) -> Vec<(String, serde_json::Value)> {
    yidam::class_schemas_at(root)
        .into_iter()
        .map(|(file, _, body)| (file, body))
        .collect()
}

/// Every instance in every example corpus validates against its own class's schema.
///
/// Each example is gated in `example_corpus.rs` as `graph-check` clean and `lint` empty at
/// every severity. So every node here is one the gate accepts, and any rejection below is
/// the compiler being stricter than the checks it publishes.
///
/// Over [`examples`] rather than one name (#448): this is a *gate*, and a second corpus
/// added beside streamflow would otherwise have its instances validated by nothing.
#[test]
fn every_instance_the_gate_accepts_is_accepted_by_its_class_schema() {
    for name in examples() {
        every_instance_matches_its_schema(&name);
    }
}

fn every_instance_matches_its_schema(name: &str) {
    let dir = example(name);
    let root = dir.path();

    let mut checked = 0;
    for (file, schema) in compiled(root) {
        let class = file
            .trim_start_matches("class/")
            .trim_end_matches(".json")
            .to_string();
        let validator = jsonschema::validator_for(&schema)
            .unwrap_or_else(|e| panic!("{file} is not a usable JSON Schema: {e}"));

        let class_dir = root.join(".yidam/corpus").join(&class);
        for entry in std::fs::read_dir(&class_dir).expect("class directory") {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|x| x != "yml") {
                continue;
            }
            let text = std::fs::read_to_string(&path).unwrap();
            let instance: serde_json::Value =
                serde_yaml::from_str(&text).expect("instance parses as YAML");

            let errors: Vec<String> = validator
                .iter_errors(&instance)
                .map(|e| format!("  {} at {}", e, e.instance_path()))
                .collect();
            assert!(
                errors.is_empty(),
                "{} is accepted by the gate and rejected by {file}:\n{}",
                path.display(),
                errors.join("\n")
            );
            checked += 1;
        }
    }
    assert!(
        checked >= 1,
        "no instances checked for {name} — this scan is looking at nothing"
    );
}

/// The agreement that is easiest to break, and silent when broken.
///
/// The compiled schema must be **no stricter than the gate**. Since #301 the ontology can
/// say `required: true`, and one declaration decides both: `missing-property` gates on
/// exactly those properties, and the compiled schema lists exactly those as JSON Schema
/// `required`. This asserts the second half against the first — not that the list is empty,
/// which was the old claim and stopped being true the moment a class could say otherwise.
///
/// Read off the corpus rather than hardcoded, so it keeps holding as an example corpus
/// grows a required property or loses one — and over [`examples`] rather than one name
/// (#448), because it is the other half of the gate above.
#[test]
fn the_compiled_schema_requires_exactly_what_the_ontology_declares_required() {
    for name in examples() {
        the_schema_matches_the_ontology(&name);
    }
}

fn the_schema_matches_the_ontology(name: &str) {
    let dir = example(name);
    let mut saw_a_class_with_properties = false;

    for (file, schema) in compiled(dir.path()) {
        assert_eq!(
            schema["required"],
            serde_json::json!(["class"]),
            "{file} requires more than the class it is for at the top level"
        );
        let Some(bag) = schema["properties"].get("properties") else {
            continue;
        };
        saw_a_class_with_properties = true;

        // What the class file itself declares, read back from the corpus this schema was
        // compiled from. Going through the ontology parser rather than re-reading YAML here
        // is the point: a second reading of `required:` could disagree with the compiler's,
        // and then this test would be pinning its own opinion.
        let class = file
            .strip_prefix("class/")
            .and_then(|f| f.strip_suffix(".json"))
            .expect("compiled schemas are named class/<name>.json");
        let ont = dir.path().join(format!(".yidam/corpus/{class}.ont.yml"));
        let text = std::fs::read_to_string(&ont).unwrap_or_default();
        let parsed = yidam_core::ontology::parse_class(class, &text);
        let declared: Vec<&str> = parsed
            .properties
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.as_str())
            .collect();

        match declared.is_empty() {
            // Omitted, not written as an empty list: a schema saying `required: []` is a
            // different document for the same meaning, and three languages compile these.
            true => assert!(
                bag.get("required").is_none(),
                "{file} requires properties the ontology never declared required — a schema \
                 stricter than the gate underlines, in the editor, an omission the build \
                 accepts"
            ),
            false => assert_eq!(
                bag["required"],
                serde_json::json!(declared),
                "{file} and {class}.ont.yml disagree about which properties are required"
            ),
        }
    }
    assert!(
        saw_a_class_with_properties,
        "no class in {name} declares a property, so this test proved nothing about it"
    );
}

/// An instance omitting every declared property still validates, stated directly rather
/// than inferred from the absence of a `required` key.
///
/// **Pinned to streamflow, and not generalised over [`examples`] (#448).** The three tests
/// from here down are about what the schema *compiler* does, demonstrated against a class
/// whose shape they state inline — `gage`, its declared properties, and the catalog file it
/// cites. Run against a corpus that has no `gage` they would not fail, they would panic on
/// the lookup, which reads as a broken test rather than as a question that corpus cannot
/// answer. The two gates above are the ones every example must pass.
#[test]
fn an_instance_carrying_no_properties_at_all_validates() {
    let dir = example(STREAMFLOW);
    let (file, schema) = compiled(dir.path())
        .into_iter()
        .find(|(f, _)| f == "class/gage.json")
        .expect("gage is a class in the example corpus");

    let validator = jsonschema::validator_for(&schema).unwrap();
    let bare = serde_json::json!({
        "class": "gage",
        "label": "A station with nothing filled in",
        "description": "Authored this morning.",
        "links": [{ "target": "../concept/x.yml", "relationship": "sources-from" }],
    });
    assert!(
        validator.is_valid(&bare),
        "{file} rejects a node the gate reports at warn severity"
    );
}

/// The other half of the same agreement, in the direction that gates.
///
/// `undeclared-property` is an error, so a property the class never declared must be
/// rejected here too — otherwise the schema is *more permissive* than the gate and an
/// editor shows green on a file CI will fail.
#[test]
fn a_property_the_class_never_declared_is_rejected() {
    let dir = example(STREAMFLOW);
    let (_, schema) = compiled(dir.path())
        .into_iter()
        .find(|(f, _)| f == "class/gage.json")
        .unwrap();

    let validator = jsonschema::validator_for(&schema).unwrap();
    let invented = serde_json::json!({
        "class": "gage",
        "properties": { "parameter": "00060", "vendor": "acme" },
    });
    assert!(
        !validator.is_valid(&invented),
        "the schema accepts what `undeclared-property` gates on"
    );
}

/// A relationship the class does not declare must **not** be rejected here.
///
/// The gate licenses a relationship only for edges landing on another instance — a link to
/// `../<class>.ont.yml` or into the catalog is a citation, and no class declares those.
/// JSON Schema cannot resolve a path, so constraining `relationship` would reject the
/// `instance-of` link every instance is required to carry.
#[test]
fn the_structural_links_every_instance_carries_are_not_rejected() {
    let dir = example(STREAMFLOW);
    let (_, schema) = compiled(dir.path())
        .into_iter()
        .find(|(f, _)| f == "class/gage.json")
        .unwrap();

    let validator = jsonschema::validator_for(&schema).unwrap();
    let structural = serde_json::json!({
        "class": "gage",
        "links": [
            { "target": "../gage.ont.yml", "relationship": "instance-of" },
            { "target": "../../catalog/usgs-nwis.md", "relationship": "sourced-from" },
        ],
    });
    assert!(
        validator.is_valid(&structural),
        "the schema rejects the citation links the bootstrap requires"
    );
    // …and the declarations are still published, for completion rather than as a rule.
    assert_eq!(schema["x-yidam-edges"][0]["relationship"], "sources-from");
}

/// A class declaring nothing constrains nothing — the rule that must live in one place.
#[test]
fn a_class_that_declares_nothing_accepts_anything() {
    let class = yidam_core::ontology::parse_class("reach", "class: reach\nlabel: Reach\n");
    let schema = yidam_core::ontology::compile_class_schema(&class);
    let validator = jsonschema::validator_for(&schema).unwrap();

    assert!(validator.is_valid(&serde_json::json!({
        "class": "reach",
        "properties": { "anything": "at all", "nobody": "declared" },
    })));
}
