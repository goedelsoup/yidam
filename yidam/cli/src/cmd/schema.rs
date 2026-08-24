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
        // Permissive, because nothing that reads a node enforces otherwise and a real corpus
        // does not look like the closed shape this used to declare.
        //
        // `CorpusInstance` has no `deny_unknown_fields`: unknown keys are ignored by design,
        // which is what makes a projecting consumer's provenance lossless. `claims.rs` walks
        // the whole document for the fields a class declared, explicitly not requiring them
        // to be nested. And nothing reads a nested `properties:` on an instance at all —
        // `properties` is a conventional home, not the only one.
        //
        // Measured: with `false`, one derived repository was rejected 117 nodes of 117
        // (`summary`, `findings`, `revisions`, `unfilled` at the top level), a projecting
        // consumer 199 of 199, and this repository's own recommendation contradicted itself
        // — `class-asserts-purpose` tells an author to move prose to `analytic_note`, which
        // the schema then called invalid.
        //
        // The harm was never a red squiggle. It is that the first thing a consumer does is
        // nest their data to make the squiggle stop, reshaping a corpus to satisfy a
        // validator no runtime consults — the same move `claims.rs` argues against.
        //
        // The sub-objects below stay closed: they are yidam's own vocabulary, they carry
        // enums, and no corpus measured violates one. `required` already catches a
        // misspelled key, so strictness here was buying almost nothing to begin with.
        "additionalProperties": true
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
            "edge_policy": {
                "type": "string",
                "enum": ["characteristic", "exhaustive"],
                "description": "Whether `edges` bounds the vocabulary or describes it. \
                                `exhaustive` closes it: a relationship outside the list is \
                                an error, because the class said it would be. \
                                `characteristic` says `edges` names what the class is \
                                *defined by*, and a relationship outside it is a deliberate \
                                coinage — none are reported. Omitting the field is neither: \
                                an undeclared relationship is reported and does not gate, \
                                because naming the relationships a class enters into never \
                                claimed the list was complete. Declared here so a misspelled \
                                policy is underlined as it is typed — `unlicensed-edge` \
                                reads an unrecognized value as absent rather than gating on \
                                a typo."
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
        // Permissive for the same reason as the node schema, and measured the same way: a
        // derived repository carries `analytic_note` — the field `class-asserts-purpose`
        // recommends by name. `edge_policy` was the other half of this argument until the
        // gate started reading it; a field the checks depend on belongs in the schema, where
        // a typo in it is underlined rather than silently read as absent.
        // The `properties[]`, `edges[]` and `foundational_type` shapes below stay closed.
        "additionalProperties": true
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

/// Schema for the authorship manifest — `.yidam/authorship.yml`.
///
/// Each kind requires the field that names who can act on a finding inside it, and
/// `additionalProperties: false` makes `why:` under `generated:` an error while typing
/// rather than at the gate. That is the whole weight of the mechanism: a region declared
/// without an addressee is a request for silence wearing a provenance label.
pub fn authorship_schema() -> Value {
    let region = |field: &str, description: &str| {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "minLength": 1,
                    "description": "Repo-root-relative. Covers itself and everything under \
                                    it, matched on path components — `docs/ref` does not \
                                    cover `docs/reference/`."
                },
                field: { "type": "string", "minLength": 1, "description": description }
            },
            "required": ["path", field],
            "additionalProperties": false
        })
    };
    json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "yidam authorship manifest",
        "description": "What in this repository is not authored here. Checks that read prose \
                        skip or re-address these regions. `.yidam/.vendor/` is built in and \
                        is not declared here.",
        "type": "object",
        "properties": {
            "generated": {
                "type": "array",
                "default": [],
                "description": "Written by this repository's own tooling. A defect here is \
                                the generator's; it is reported at info severity so it can \
                                be routed, not silenced.",
                "items": region("by", "The command that writes this region.")
            },
            "imported": {
                "type": "array",
                "default": [],
                "description": "Copied from elsewhere and not modified. A defect here is \
                                upstream's, and editing the file to satisfy a linter \
                                falsifies the record it exists to keep.",
                "items": region("from", "What this is a copy of, and as of when.")
            },
            "excluded": {
                "type": "array",
                "default": [],
                "description": "Neither generated nor imported. The escape hatch, named as \
                                one: the only kind that is not read at all.",
                "items": region("why", "Why this region is not linted.")
            }
        },
        "additionalProperties": false
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
        (
            "authorship.json",
            ".yidam/authorship.yml",
            authorship_schema(),
        ),
    ]
}

/// The `yaml.schemas` mapping an editor needs to apply these while typing.
///
/// Read by `yaml-language-server`, which ships inside the Red Hat YAML extension for VS
/// Code and is reachable from Neovim and Helix over LSP. Editors using none of these still
/// get the check from `yidam lint`.
pub fn editor_settings() -> Value {
    editor_settings_for(&repo_root().unwrap_or_default())
}

/// The mapping, including one entry per compiled class.
///
/// A per-class schema is worth exactly as much as the editor's willingness to apply it, and
/// an editor applies what a glob names. `.yidam/corpus/<class>/*.yml` is the glob, which is
/// also how `graph-check` decides which class governs an instance — so the validation while
/// typing and the validation at the gate are keyed the same way.
pub fn editor_settings_for(root: &Path) -> Value {
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
    for (file, glob, _) in class_schemas(root) {
        schemas.insert(format!("./.yidam/schemas/{file}"), json!(glob));
    }
    json!({
        "yaml.schemas": schemas,
        "files.associations": { "*.ont.yml": "yaml" }
    })
}

/// One compiled schema per class, from the corpus's own ontology.
///
/// The four schemas above describe what yidam defines — what *a* node is. These describe
/// what this corpus declared: that a `gage` carries a `parameter` typed `string` and a
/// `claim_tag` typed `claim`. Until they existed, `.ont.yml` was a schema written in a
/// format only yidam's own walker read, and a consumer wanting to validate an instance had
/// to either link against yidam or re-derive the rules — which is how a mirror drifts.
///
/// **Compiled by `yidam_core::ontology`, not here.** The CLI is a consumer of the
/// compiler like any other, so the schema the gate publishes and the schema an SDK
/// produces cannot come apart. That is the whole point of compiling the ontology rather
/// than describing it.
pub fn class_schemas(root: &Path) -> Vec<(String, String, Value)> {
    let corpus = crate::paths::yidam_corpus_dir(root);
    crate::walk::walk_ont_files(&corpus)
        .iter()
        .map(|path| {
            let stem = path
                .file_name()
                .and_then(|n| n.to_str())
                .and_then(|n| n.strip_suffix(".ont.yml"))
                .unwrap_or_default();
            let text = std::fs::read_to_string(path).unwrap_or_default();
            let mut class = yidam_core::ontology::parse_class(stem, &text);
            // **The filename governs, not the `class:` field.** `parse_class` prefers the
            // field and falls back to the stem, which is right for the SDK; the *gate* does
            // the opposite — `load_classes` keys by filename always, and `unknown-class`
            // compares an instance's `class:` against the set of stems.
            //
            // Where the two disagree this compiler followed the field, and emitted
            // `class/station.json` mapped to `.yidam/corpus/station/*.yml` for a class
            // whose instances live in `gage/` — a schema the editor applies to nothing,
            // asserting a `const` no instance carries. Keyed by the stem it agrees with the
            // gate, which is the only agreement that matters.
            class.name = stem.to_string();
            let schema = yidam_core::ontology::compile_class_schema(&class);
            (
                format!("class/{}.json", class.name),
                format!(".yidam/corpus/{}/*.yml", class.name),
                schema,
            )
        })
        .collect()
}

pub fn schema(print_settings: bool) -> Result<()> {
    let root = repo_root()?;

    if print_settings {
        println!("{}", serde_json::to_string_pretty(&editor_settings())?);
        return Ok(());
    }

    let dir = schemas_dir(&root);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating {}", dir.display()))?;

    let fixed = all()
        .into_iter()
        .map(|(f, g, b)| (f.to_string(), g.to_string(), b));
    let mut wrote_a_class = false;
    for (file, glob, body) in fixed.chain(class_schemas(&root)) {
        let path = dir.join(&file);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating {}", parent.display()))?;
        }
        wrote_a_class |= file.starts_with("class/");
        let text = format!("{}\n", serde_json::to_string_pretty(&body)?);
        std::fs::write(&path, text).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote .yidam/schemas/{file}  ({glob})");
    }

    if !wrote_a_class {
        println!();
        println!("no .ont.yml files found — no per-class schemas to compile");
    }

    println!();
    println!("To validate while typing, add to .vscode/settings.json:");
    println!("  yidam schema --settings");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A corpus may carry its own fields at the top level of a node or a class.
    ///
    /// These were closed, and nothing that reads either enforced it. A derived repository was
    /// rejected 117 nodes of 117 and 18 classes of 18; a projecting consumer, 199 of 199; and
    /// `class-asserts-purpose` recommends `analytic_note`, which the ontology schema then
    /// called invalid. `catalog-entry.json` was already permissive at the top level — the
    /// other two were the ones out of step with it.
    #[test]
    fn a_corpus_may_carry_its_own_fields() {
        for (file, body) in [
            ("corpus-node", corpus_node_schema()),
            ("corpus-ontology", corpus_ontology_schema()),
            ("catalog-entry", catalog_entry_schema()),
        ] {
            assert_eq!(
                body["additionalProperties"], true,
                "{file} must not reject a field no reader rejects"
            );
        }
    }

    /// What stays closed, and why: these are yidam's own vocabulary rather than a corpus's
    /// data, they carry enums, and no corpus measured violates one. Relaxing them too would
    /// be the same error as closing the top level — a rule set wider than the evidence.
    #[test]
    fn yidam_s_own_shapes_stay_closed() {
        let node = corpus_node_schema();
        assert_eq!(
            node["properties"]["links"]["items"]["additionalProperties"],
            false
        );

        let ont = corpus_ontology_schema();
        for key in ["properties", "edges"] {
            assert_eq!(
                ont["properties"][key]["items"]["additionalProperties"], false,
                "{key}[] is a declared shape, not a place for corpus data"
            );
        }
        assert_eq!(
            ont["properties"]["foundational_type"]["additionalProperties"],
            false
        );
    }

    /// The invariants that make the schema worth wiring into an editor at all. None of them
    /// depends on `additionalProperties`, which is why closing it bought so little: a
    /// misspelled `lable:` still fails, on `required`.
    #[test]
    fn the_checks_that_earn_the_schema_survive() {
        let node = corpus_node_schema();
        assert_eq!(
            node["properties"]["links"]["minItems"], 1,
            "the central invariant"
        );
        let req: Vec<String> = serde_json::from_value(node["required"].clone()).unwrap();
        assert!(
            req.contains(&"label".to_string()),
            "a typo'd key fails here"
        );
        assert_eq!(
            node["properties"]["links"]["items"]["required"],
            serde_json::json!(["target", "relationship"])
        );
    }

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
            3,
            "catalog frontmatter lives in .md and is not mapped"
        );
        assert!(m.contains_key("./.yidam/schemas/corpus-node.json"));
    }

    /// The requirement is the mechanism: a region with no addressee is a request for
    /// silence, and the editor should say so while it is being typed.
    #[test]
    fn every_authorship_kind_requires_the_field_that_names_who_can_act() {
        let s = authorship_schema();
        for (kind, field) in [
            ("generated", "by"),
            ("imported", "from"),
            ("excluded", "why"),
        ] {
            let items = &s["properties"][kind]["items"];
            let req: Vec<String> = serde_json::from_value(items["required"].clone()).unwrap();
            assert_eq!(req, vec!["path".to_string(), field.to_string()], "{kind}");
            assert_eq!(items["additionalProperties"], false, "{kind}");
        }
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
