//! The artifact's links into its corpus, end to end — RFC-0028 §6, #577.
//!
//! The unit tests in `coupling.rs` and `cmd/lint/mod.rs` hold the rules. This file holds the
//! *wiring*: that `[object] paths` reaches both readers through a real binary over a real
//! `git ls-files`, and that the populated `kuten.coupling` a report emits is the shape
//! `report.schema.json` declares. The shared golden fixture declares no object, so this is the
//! only place that shape is held against a document (see `UNREACHED` in `report_goldens.rs`).
//!
//! # The shape is measured, not invented
//!
//! #577 asks for a demonstration "against a real object-coupled shape, not a fixture built to
//! pass". The tree here is allen-county-ohio's, read from its clone on 2026-09-28: it holds
//! `inquiry` at the shipped revision, declares `web/**`, `crates/**` and `design/**`, and cites
//! its corpus from a crate's README, a crate's doc comments, the web app's TypeScript and
//! `design/README.md`. It declares `design/upstream/` imported. Its `web/src/feeds/` holds the
//! JSON the site is built from, which carries corpus prose and about three thousand markdown
//! links written relative to the node they came from, not to the JSON file. That is why JSON
//! is not read, and why `graph.json` below carries a link that must not count. The names are
//! cut down; the kinds of file and where the links sit are not. The one addition is a `.md`
//! under the imported region, whose real contents are stylesheets, so that the region's
//! exclusion has something to exclude.
//!
//! # Why the same repository twice
//!
//! One tree, two configs, for the reason `commit_registers.rs` gives: whatever differs between
//! the runs is the declaration's doing, because nothing else differs.

use std::path::Path;
use std::process::Command;

mod common;

use common::repo_root;

fn write(root: &Path, rel: &str, text: &str) {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().expect("a parent")).expect("mkdir");
    std::fs::write(path, text).expect("write");
}

fn git(root: &Path, args: &[&str]) {
    common::git::git_at(root, args, common::git::FIXTURE_DATE);
}

const OBJECT: &str = "[object]\npaths = [\"web/**\", \"crates/**\", \"design/**\"]\n";

/// The object-coupled repository, committed. `object` decides whether `.yidam/config.toml`
/// declares the artifact; nothing else about the tree depends on it.
fn stage(object: bool) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path();

    let profile =
        std::fs::read_to_string(repo_root().join("yidam/prelude/kuten/inquiry/kuten.yml"))
            .expect("the shipped profile");
    let revision = yidam::kuten::Profile::parse(&profile)
        .expect("the binary can read the shipped profile")
        .revision;
    write(
        root,
        ".yidam/.vendor/prelude/kuten/inquiry/kuten.yml",
        &profile,
    );
    write(
        root,
        ".yidam/decisions/kuten.yml",
        &format!("kuten: inquiry\nrevision: {revision}\n"),
    );
    write(root, ".yidam/config.toml", if object { OBJECT } else { "" });

    // The corpus. One node links into the artifact; one is cited by it; one is cited only
    // from a file the reader does not open.
    write(root, ".yidam/corpus/site.ont.yml", "class: site\n");
    write(
        root,
        ".yidam/corpus/site/allen-county-courthouse.yml",
        "class: site\nlabel: Allen County Courthouse\ndescription: |\n  Scored by \
         [the scorer](../../../crates/nrhp/src/score.rs).\n",
    );
    write(
        root,
        ".yidam/corpus/site/old-jail.yml",
        "class: site\nlabel: Old Jail\ndescription: Listed 1974.\n",
    );
    write(
        root,
        ".yidam/corpus/site/lima-station.yml",
        "class: site\nlabel: Lima Station\ndescription: Demolished.\n",
    );
    write(
        root,
        ".yidam/decisions/0001-score-by-significance.yml",
        "title: Score sites by significance\nstatus: accepted\n",
    );
    write(
        root,
        ".yidam/catalog/nrhp.md",
        "---\nobtained: true\n---\n# National Register of Historic Places\n",
    );

    // The artifact.
    write(
        root,
        "crates/nrhp/README.md",
        "# nrhp\n\nScores [the courthouse](../../.yidam/corpus/site/allen-county-courthouse.yml)\n\
         under [the scoring decision](../../.yidam/decisions/0001-score-by-significance.yml).\n",
    );
    write(
        root,
        "crates/nrhp/src/lib.rs",
        "//! Reads the [NRHP](../../../.yidam/catalog/nrhp.md) export.\npub mod score;\n",
    );
    write(
        root,
        "crates/nrhp/src/score.rs",
        "pub fn score() -> u32 {\n    0\n}\n",
    );
    write(
        root,
        "web/src/app.ts",
        "// See [the jail](../../.yidam/corpus/site/old-jail.yml).\nexport const app = 1;\n",
    );
    write(
        root,
        "design/README.md",
        "# Design\n\nScored as [the decision](../.yidam/decisions/0001-score-by-significance.yml) \
         says.\n",
    );
    write(
        root,
        ".yidam/authorship.yml",
        "imported:\n  - path: design/upstream/\n    from: the county atlas design system\n",
    );
    // Neither of these is the artifact's citation: one is someone else's text, the other is
    // generated. Each names the one node nothing else cites, so either being read shows up.
    write(
        root,
        "design/upstream/README.md",
        "[station](../../.yidam/corpus/site/lima-station.yml)\n",
    );
    write(
        root,
        "web/src/feeds/graph.json",
        "{\"source\": \"[station](../../../.yidam/corpus/site/lima-station.yml)\"}\n",
    );

    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "fixture@yidam.test"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "genesis: the corpus"]);
    tmp
}

struct Run {
    code: i32,
    stdout: String,
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("yidam runs");
    Run {
        code: out.status.code().expect("an exit code"),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
    }
}

fn json(root: &Path, args: &[&str]) -> serde_json::Value {
    let mut a = args.to_vec();
    a.extend_from_slice(&["--format", "json"]);
    let r = run(root, &a);
    serde_json::from_str(&r.stdout)
        .unwrap_or_else(|e| panic!("`yidam {}` ({e}): {}", a.join(" "), r.stdout))
}

/// The locations one lint check reports, and its severity.
fn lint_check(doc: &serde_json::Value, id: &str) -> (String, Vec<String>) {
    let check = doc["checks"]
        .as_array()
        .expect("the report lists checks")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("`lint` runs `{id}`"));
    let nodes = check["violations"]
        .as_array()
        .map(|vs| {
            vs.iter()
                .filter_map(|v| v["node"].as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    (check["severity"].as_str().unwrap_or("").to_string(), nodes)
}

fn strings(v: &serde_json::Value) -> Vec<&str> {
    v.as_array()
        .expect("an array")
        .iter()
        .map(|s| s.as_str().expect("a string"))
        .collect()
}

#[test]
fn kuten_check_counts_the_links_both_ways() {
    let tmp = stage(true);
    let doc = json(tmp.path(), &["kuten", "check"]);
    let c = &doc["kuten"]["coupling"];

    assert_eq!(strings(&c["object"]), ["web/**", "crates/**", "design/**"]);
    // The crate's README, lib.rs and score.rs, app.ts and design/README.md. Not graph.json,
    // which is generated, and not design/upstream/README.md, which is imported.
    assert_eq!(c["files"], 5);
    assert_eq!(c["inbound"], serde_json::json!({"total": 5, "dead": 0}));
    assert_eq!(c["outbound"], serde_json::json!({"total": 1, "dead": 0}));

    assert_eq!(c["nodes"]["total"], 3);
    assert_eq!(c["nodes"]["cited"], 2);
    assert_eq!(c["nodes"]["citing"], 1);
    assert_eq!(
        strings(&c["nodes"]["uncited"]),
        [".yidam/corpus/site/lima-station.yml"]
    );
    // The kuten record is a decision too, and nothing in an artifact has a reason to cite it.
    assert_eq!(c["decisions"]["cited"], 1);
    assert_eq!(
        strings(&c["decisions"]["uncited"]),
        [".yidam/decisions/kuten.yml"]
    );
    assert_eq!(c["catalog"]["total"], 1);
    assert_eq!(c["catalog"]["cited"], 1);

    let text = run(tmp.path(), &["kuten", "check"]);
    assert_eq!(text.code, 0, "{}", text.stdout);
    assert!(
        text.stdout
            .contains("Coupling: 5 artifact file(s) read from web/**, crates/**, design/**."),
        "{}",
        text.stdout
    );
    assert!(text.stdout.contains("artifact → corpus  5 link(s), 0 dead"));
}

/// The case #577 was measured on: a node the artifact cites is removed. Before this, twenty
/// findings in a derived corpus and none of them named the README.
#[test]
fn deleting_a_cited_node_is_named_at_the_line_that_cited_it() {
    let tmp = stage(true);
    let root = tmp.path();
    let before = run(root, &["lint"]);
    let (severity, nodes) = lint_check(&json(root, &["lint"]), "broken-object-link");
    assert_eq!(severity, "warn");
    assert!(nodes.is_empty(), "{nodes:?}");

    git(
        root,
        &["rm", "-q", ".yidam/corpus/site/allen-county-courthouse.yml"],
    );
    git(root, &["commit", "-q", "-m", "retract: the courthouse"]);

    let doc = json(root, &["lint"]);
    let (_, nodes) = lint_check(&doc, "broken-object-link");
    assert_eq!(nodes, ["crates/nrhp/README.md:3"]);
    let (_, prose) = lint_check(&doc, "broken-prose-link");
    assert!(!prose.iter().any(|n| n.starts_with("crates/")), "{prose:?}");
    // A warning, and the gate does not move for it.
    assert_eq!(run(root, &["lint"]).code, before.code);

    let c = &json(root, &["kuten", "check"])["kuten"]["coupling"];
    assert_eq!(c["inbound"], serde_json::json!({"total": 5, "dead": 1}));
    assert_eq!(c["nodes"]["total"], 2);
    assert!(run(root, &["kuten", "check"])
        .stdout
        .contains("`broken-object-link` from the artifact"));
}

/// The mutation of the two tests above: the same tree and the same deletion with no object
/// declared. Every path is corpus, so there is no boundary for a link to cross.
#[test]
fn without_an_object_the_same_tree_has_no_coupling() {
    let tmp = stage(false);
    let root = tmp.path();
    git(
        root,
        &["rm", "-q", ".yidam/corpus/site/allen-county-courthouse.yml"],
    );
    git(root, &["commit", "-q", "-m", "retract: the courthouse"]);

    let (_, nodes) = lint_check(&json(root, &["lint"]), "broken-object-link");
    assert!(nodes.is_empty(), "{nodes:?}");
    let doc = json(root, &["kuten", "check"]);
    assert!(doc["kuten"]["coupling"].is_null(), "{}", doc["kuten"]);
    assert!(!run(root, &["kuten", "check"]).stdout.contains("Coupling"));
}

/// Every path the populated report emits is declared in the committed schema.
///
/// The golden matrix reaches `kuten.coupling` only as `null`, so this is the one place its
/// members are held against a document. Both arms run: the dead count is the one a clean tree
/// emits as zero.
#[test]
fn every_emitted_field_is_declared_in_the_schema() {
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repo_root().join("yidam/sdks/parity/fixtures/reports/report.schema.json"),
        )
        .expect("the schema"),
    )
    .expect("the schema parses");

    let tmp = stage(true);
    let doc = json(tmp.path(), &["kuten", "check"]);
    let mut emitted = std::collections::BTreeSet::new();
    paths_of(&doc, "", &mut emitted);
    for path in &emitted {
        assert!(
            declares(&schema, path),
            "`kuten check` emits `{path}`, which report.schema.json does not declare"
        );
    }

    // Every member the schema requires of the coupling and of each shape inside it is emitted.
    let coupling = resolve(
        &schema,
        &schema["properties"]["kuten"]["properties"]["coupling"],
    );
    for field in coupling["required"].as_array().expect("required") {
        let field = field.as_str().unwrap();
        let value = &doc["kuten"]["coupling"][field];
        assert!(!value.is_null(), "`kuten.coupling.{field}` is required");
        let shape = resolve(&schema, &coupling["properties"][field]);
        for inner in shape["required"].as_array().into_iter().flatten() {
            let inner = inner.as_str().unwrap();
            assert!(
                !value[inner].is_null(),
                "`kuten.coupling.{field}.{inner}` is required"
            );
        }
    }

    // `declares` says no to what the schema does not carry, at the depth of the new shapes
    // and through a `$ref`, or the loop above holds nothing.
    for absent in [
        "kuten.coupling.nonesuch",
        "kuten.coupling.nodes.nonesuch",
        "kuten.coupling.inbound.nonesuch",
    ] {
        assert!(
            !declares(&schema, absent),
            "`{absent}` is not in the schema and the walk said it was"
        );
    }
    // And the walk reached the members behind each `$ref`, not only the envelope.
    for witness in [
        "kuten.coupling.object",
        "kuten.coupling.inbound.dead",
        "kuten.coupling.outbound.total",
        "kuten.coupling.nodes.uncited",
        "kuten.coupling.decisions.citing",
        "kuten.coupling.catalog.cited",
    ] {
        assert!(
            emitted.contains(witness),
            "the run never emitted `{witness}`: {emitted:?}"
        );
    }
}

/// Every path a document carries a value at, in the schema's own notation.
fn paths_of(node: &serde_json::Value, path: &str, out: &mut std::collections::BTreeSet<String>) {
    match node {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let here = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                out.insert(here.clone());
                paths_of(child, &here, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                paths_of(item, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// A schema node with its local `$ref` followed. The coupling's two repeated shapes live in
/// `$defs`, so a walk that stopped at the reference would find no members under it.
fn resolve<'s>(
    schema: &'s serde_json::Value,
    node: &'s serde_json::Value,
) -> &'s serde_json::Value {
    match node.get("$ref").and_then(|r| r.as_str()) {
        Some(r) => {
            let name = r.strip_prefix("#/$defs/").expect("a local $defs reference");
            &schema["$defs"][name]
        }
        None => node,
    }
}

/// Does the schema declare `path`? Walks `properties` and `items` the way [`paths_of`] builds
/// them, following `$ref` at every step.
fn declares(schema: &serde_json::Value, path: &str) -> bool {
    let mut node = schema;
    for segment in path.split('.') {
        let (key, arrays) = match segment.split_once("[]") {
            Some((key, rest)) => (key, rest.matches("[]").count() + 1),
            None => (segment, 0),
        };
        node = match resolve(schema, node)
            .get("properties")
            .and_then(|p| p.get(key))
        {
            Some(child) => child,
            None => return false,
        };
        for _ in 0..arrays {
            node = match resolve(schema, node).get("items") {
                Some(items) => items,
                None => return false,
            };
        }
    }
    true
}
