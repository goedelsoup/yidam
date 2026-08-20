//! `yidam schema` — emit JSON Schema for the corpus shapes.
//!
//! Yidam defines what a node and a class definition are; a derived repo should not have to
//! restate that in order to get validation while typing. One repository derived from this
//! template wrote its own generator against a hand-maintained zod mirror of these shapes,
//! which works exactly until the mirror drifts.
//!
//! The schemas are written into the repo rather than served, because an editor mapping has
//! to name a path. They are generated output: regenerate rather than edit.

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

use crate::paths::repo_root;

pub fn schemas_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("schemas")
}

fn non_empty_string() -> Value {
    json!({ "type": "string", "minLength": 1 })
}

/// Schema for a corpus instance — `.yidam/corpus/<class>/<instance>.yml`.
pub fn corpus_node_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "yidam corpus node",
        "description": "One node in .yidam/corpus/<class>/. An instance of its directory's \
                        ontology class, carrying at least one outgoing link — a node with no \
                        outgoing edge is not in the graph.",
        "type": "object",
        "properties": {
            "class": non_empty_string(),
            "label": non_empty_string(),
            "description": non_empty_string(),
            "properties": {
                "type": "object",
                "default": {},
                "additionalProperties": true
            },
            "links": {
                "type": "array",
                // The corpus's central invariant, stated where an editor can enforce it
                // as you type rather than when the gate runs.
                "minItems": 1,
                "items": {
                    "type": "object",
                    "properties": {
                        "target": non_empty_string(),
                        "relationship": non_empty_string(),
                        "note": { "type": "string" }
                    },
                    "required": ["target", "relationship"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["class", "label", "description", "links"],
        "additionalProperties": false
    })
}

/// Schema for a class definition — `.yidam/corpus/<class>.ont.yml`.
pub fn corpus_ontology_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "yidam corpus ontology class",
        "description": "One class definition, .yidam/corpus/<class>.ont.yml. Declares what a \
                        kind of node is, the properties it may carry, and the relationships it \
                        may enter into.",
        "type": "object",
        "properties": {
            "class": non_empty_string(),
            "label": non_empty_string(),
            "description": non_empty_string(),
            "foundational_type": {
                "type": "object",
                "description": "Omitted entirely when the corpus chose no foundational alignment.",
                "properties": {
                    "ontology": { "type": "string", "enum": ["bfo", "ufo"] },
                    "type": non_empty_string()
                },
                "required": ["ontology", "type"],
                "additionalProperties": false
            },
            "properties": {
                "type": "array",
                "default": [],
                "items": {
                    "type": "object",
                    "properties": {
                        "name": non_empty_string(),
                        "type": {
                            "type": "string",
                            "enum": ["string", "date", "ref", "text", "claim"],
                            "description": "`claim` marks a property whose value IS an \
                                            evidence tag — `verified`, `inference`, or \
                                            `open` — rather than prose that mentions one. \
                                            Declaring it is what lets a corpus with a typed \
                                            claim vocabulary be counted at all: without it, \
                                            only the bracketed token in serialized text is \
                                            seen, and a consumer storing `claim_tag: open` \
                                            had 2 of its 26 open questions found."
                        },
                        "description": non_empty_string()
                    },
                    "required": ["name", "type", "description"],
                    "additionalProperties": false
                }
            },
            "edges": {
                "type": "array",
                "default": [],
                "items": {
                    "type": "object",
                    "properties": {
                        "relationship": non_empty_string(),
                        "target": non_empty_string(),
                        "direction": { "type": "string", "enum": ["out", "in"] },
                        "description": non_empty_string()
                    },
                    "required": ["relationship", "target", "direction"],
                    "additionalProperties": false
                }
            }
        },
        "required": ["class", "label", "description"],
        "additionalProperties": false
    })
}

/// Schema for a catalog entry's frontmatter — `.yidam/catalog/<slug>.md`.
pub fn catalog_entry_schema() -> Value {
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "yidam catalog entry frontmatter",
        "description": "The YAML frontmatter of a source in .yidam/catalog/. Describes where \
                        knowledge came from, not the knowledge itself.",
        "type": "object",
        "properties": {
            "name": non_empty_string(),
            "description": non_empty_string(),
            "type": { "type": "string", "enum": ["paper", "dataset", "api", "database", "other"] },
            "obtained": {
                "type": "boolean",
                "description": "Absent means yes. `false` declares a source registered ahead of \
                                the extraction that will use it — which exempts it from \
                                catalog-uncited and makes citing it an error."
            },
            "location": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "kind": {
                            "type": "string",
                            "enum": crate::parse::CATALOG_LOCATION_KINDS
                        },
                        "value": non_empty_string(),
                        "description": { "type": "string" }
                    },
                    "required": ["kind", "value"],
                    "additionalProperties": false
                }
            },
            "used-by": {
                "type": "array",
                "description": "Optional. Corpus nodes known to draw on this source. Declaring \
                                one asserts it is current; the citations are authoritative.",
                "items": non_empty_string()
            }
        },
        "required": ["name", "description"],
        "additionalProperties": true
    })
}

/// Every schema yidam emits, as `(filename, glob it validates, body)`.
pub fn all() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        (
            "corpus-node.json",
            ".yidam/corpus/*/*.yml",
            corpus_node_schema(),
        ),
        (
            "corpus-ontology.json",
            ".yidam/corpus/*.ont.yml",
            corpus_ontology_schema(),
        ),
        (
            "catalog-entry.json",
            ".yidam/catalog/*.md",
            catalog_entry_schema(),
        ),
    ]
}

/// The `yaml.schemas` mapping an editor needs to apply these while typing.
///
/// Read by `yaml-language-server`, which ships inside the Red Hat YAML extension for VS
/// Code and is reachable from Neovim and Helix over LSP. Editors using none of these still
/// get the check from `yidam lint`.
pub fn editor_settings() -> Value {
    let mut schemas = serde_json::Map::new();
    for (file, glob, _) in all() {
        // The catalog schema describes frontmatter inside markdown, which
        // yaml-language-server cannot apply to a .md file; it is emitted for use by
        // other tooling and deliberately not mapped here.
        if file == "catalog-entry.json" {
            continue;
        }
        schemas.insert(format!("./.yidam/schemas/{file}"), json!(glob));
    }
    json!({
        "yaml.schemas": schemas,
        "files.associations": { "*.ont.yml": "yaml" }
    })
}

pub fn schema(print_settings: bool) -> Result<()> {
    let root = repo_root()?;

    if print_settings {
        println!("{}", serde_json::to_string_pretty(&editor_settings())?);
        return Ok(());
    }

    let dir = schemas_dir(&root);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    for (file, glob, body) in all() {
        let path = dir.join(file);
        let text = format!("{}\n", serde_json::to_string_pretty(&body)?);
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote .yidam/schemas/{file}  ({glob})");
    }

    println!();
    println!("To validate while typing, add to .vscode/settings.json:");
    println!("  yidam schema --settings");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_schema_is_valid_json_schema_draft_2020_12() {
        for (file, _, body) in all() {
            assert_eq!(
                body["$schema"], "https://json-schema.org/draft/2020-12/schema",
                "{file}"
            );
            assert!(body["title"].is_string(), "{file} needs a title");
            assert_eq!(body["type"], "object", "{file}");
        }
    }

    #[test]
    fn a_node_must_declare_at_least_one_link() {
        // The corpus's central invariant. If this drifts, the editor stops enforcing the
        // one rule that keeps the graph connected.
        assert_eq!(corpus_node_schema()["properties"]["links"]["minItems"], 1);
    }

    #[test]
    fn a_node_requires_class_label_description_and_links() {
        let req = corpus_node_schema()["required"].clone();
        let req: Vec<String> = serde_json::from_value(req).unwrap();
        for field in ["class", "label", "description", "links"] {
            assert!(req.contains(&field.to_string()), "{field} must be required");
        }
    }

    #[test]
    fn location_kinds_come_from_the_same_constant_the_lint_uses() {
        let kinds = catalog_entry_schema()["properties"]["location"]["items"]["properties"]["kind"]
            ["enum"]
            .clone();
        let kinds: Vec<String> = serde_json::from_value(kinds).unwrap();
        assert_eq!(kinds, crate::parse::CATALOG_LOCATION_KINDS);
    }

    #[test]
    fn editor_settings_map_only_what_yaml_language_server_can_apply() {
        let s = editor_settings();
        let m = s["yaml.schemas"].as_object().unwrap();
        assert_eq!(
            m.len(),
            2,
            "catalog frontmatter lives in .md and is not mapped"
        );
        assert!(m.contains_key("./.yidam/schemas/corpus-node.json"));
    }

    #[test]
    fn schemas_are_written_where_the_editor_mapping_points() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dir = schemas_dir(tmp.path());
        std::fs::create_dir_all(&dir).unwrap();
        for (file, _, body) in all() {
            std::fs::write(dir.join(file), serde_json::to_string(&body).unwrap()).unwrap();
        }
        let settings = editor_settings();
        for key in settings["yaml.schemas"].as_object().unwrap().keys() {
            let rel = key.trim_start_matches("./");
            assert!(
                tmp.path().join(rel).exists(),
                "{key} is mapped but not emitted"
            );
        }
    }
}
