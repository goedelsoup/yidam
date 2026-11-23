//! A table is a catalog entry that declares its columns (RFC-0059).
//!
//! Each test adds one `dataset` entry to `examples/streamflow`, declaring a small TSV of gage
//! readings, and runs the real binary over it: `lint` holds the file to the declaration, a
//! `#row=` link resolves against it, and `yidam run` checks it on the way in and on the way out.

use crate::common;

use common::git::out as git;
use common::Example;

const ENTRY: &str = "---
name: gage-readings
description: Illustrative daily discharge readings at the two gages.
type: dataset
obtained: true
location:
  - kind: file
    value: data/gage-readings.tsv
columns:
  - { name: reading, type: string }
  - { name: gage, type: ref }
  - { name: discharge, type: number, unit: cfs }
key: reading
---

# Gage readings

Illustrative, as the gages are.
";

const GOOD: &str = "reading\tgage\tdischarge\n\
                    R1\tgage/canyon-outlet\t412.5\n\
                    R2\tgage/valley-bridge\t\n";

/// A step that reads the declared table through the kit, and writes what it read as a report.
const READER: &str = "import json, os, sys
sys.path.insert(0, os.path.join(os.environ['YIDAM_IN'], '.yidam/capabilities'))
import yidam_kit as k
rows = k.table('data/gage-readings.tsv')
out = os.path.join(os.environ['YIDAM_OUT'], '.yidam/computed/reports')
os.makedirs(out)
with open(os.path.join(out, 'readings.json'), 'w') as f:
    json.dump(rows, f)
";

/// `examples/streamflow` with the entry, `table` as its file, and `extra` written and committed.
fn with_table(table: &str, extra: &[(&str, &str)]) -> Example {
    let e = Example::materialize("streamflow");
    let root = e.path();
    let mut files = vec![
        (".yidam/catalog/gage-readings.md", ENTRY),
        ("data/gage-readings.tsv", table),
    ];
    files.extend_from_slice(extra);
    for (path, text) in files {
        let at = root.join(path);
        std::fs::create_dir_all(at.parent().unwrap()).unwrap();
        std::fs::write(at, text).unwrap();
    }
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fix: declare the gage readings"]);
    e
}

/// The same, with one more step, `probe`, declared with `reads`, `writes` and its script.
fn with_step(table: &str, writes: &str, script: (&str, &str)) -> Example {
    let manifest = std::fs::read_to_string(
        Example::materialize("streamflow")
            .path()
            .join(".yidam/capabilities.toml"),
    )
    .unwrap();
    let (name, body) = script;
    let manifest = format!(
        "{manifest}\n[capability.probe]\nkind   = \"calculator\"\n\
         run    = [{}\".yidam/capabilities/{name}\"]\n\
         reads  = [\".yidam/capabilities/**\", \"data/**\"]\nwrites = {writes}\n\
         verb   = \"compute\"\n",
        if name.ends_with(".py") {
            "\"python3\", \"-I\", "
        } else {
            "\"sh\", "
        },
    );
    with_table(
        table,
        &[
            (".yidam/capabilities.toml", &manifest),
            (&format!(".yidam/capabilities/{name}"), body),
        ],
    )
}

fn lint(e: &Example) -> serde_json::Value {
    let (out, err, _) = e.run(&["lint", "--format", "json"]);
    serde_json::from_str(&out).unwrap_or_else(|x| panic!("lint's report ({x}): {out}{err}"))
}

/// Each finding one check reports, as `node: detail`.
fn findings(doc: &serde_json::Value, id: &str) -> Vec<String> {
    let check = doc["checks"]
        .as_array()
        .expect("the report lists checks")
        .iter()
        .find(|c| c["id"] == id)
        .unwrap_or_else(|| panic!("`lint` runs `{id}`"));
    check["violations"]
        .as_array()
        .map(|vs| {
            vs.iter()
                .map(|v| format!("{}: {}", v["node"].as_str().unwrap(), v["detail"]))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_declared_table_that_holds_its_rows_reports_nothing() {
    let e = with_table(GOOD, &[]);
    let doc = lint(&e);
    for id in [
        "table-declaration-invalid",
        "table-shape",
        "table-ref-unresolved",
    ] {
        assert!(
            findings(&doc, id).is_empty(),
            "{id}: {:?}",
            findings(&doc, id)
        );
    }
}

#[test]
fn a_table_that_departs_from_its_declaration_is_reported_by_line_and_column() {
    let e = with_table(
        "reading\tgage\tdischarge\nR1\tgage/canyon-outlet\tlots\nR1\tgage/nowhere\t3\n",
        &[],
    );
    let doc = lint(&e);
    let shape = findings(&doc, "table-shape");
    assert!(
        shape
            .iter()
            .any(|f| f.starts_with("data/gage-readings.tsv: ")
                && f.contains("line 2")
                && f.contains("discharge")),
        "{shape:?}"
    );
    assert!(shape.iter().any(|f| f.contains("line 3")), "{shape:?}");
    let refs = findings(&doc, "table-ref-unresolved");
    assert_eq!(refs.len(), 1, "{refs:?}");
    assert!(refs[0].contains("gage/nowhere"), "{refs:?}");
}

#[test]
fn a_declaration_it_cannot_be_held_to_is_reported_on_the_entry() {
    let e = with_table(GOOD, &[]);
    let root = e.path();
    let entry = root.join(".yidam/catalog/gage-readings.md");
    let text = std::fs::read_to_string(&entry).unwrap();
    std::fs::write(&entry, text.replace("type: ref", "type: claim")).unwrap();
    let invalid = findings(&lint(&e), "table-declaration-invalid");
    assert_eq!(invalid.len(), 1, "{invalid:?}");
    assert!(
        invalid[0].starts_with(".yidam/catalog/gage-readings.md: ") && invalid[0].contains("claim"),
        "{invalid:?}"
    );
}

#[test]
fn a_row_link_resolves_and_a_key_no_row_holds_is_a_broken_link() {
    let e = with_table(GOOD, &[]);
    let node = e.path().join(".yidam/corpus/gage/valley-bridge.yml");
    let text = std::fs::read_to_string(&node).unwrap();
    let cite = |key: &str| {
        text.replace(
            "  Illustrative, as above.\n",
            &format!(
                "  Illustrative, as above. One [reading](../../catalog/gage-readings.md#row={key}) \
                 has no discharge. [verified]\n"
            ),
        )
    };
    std::fs::write(&node, cite("R2")).unwrap();
    let held = findings(&lint(&e), "broken-prose-link");
    assert!(
        !held.iter().any(|f| f.contains("gage-readings")),
        "{held:?}"
    );
    std::fs::write(&node, cite("R9")).unwrap();
    let broken = findings(&lint(&e), "broken-prose-link");
    assert!(
        broken
            .iter()
            .any(|f| f.contains("valley-bridge") && f.contains("R9")),
        "{broken:?}"
    );
}

#[test]
fn a_step_reads_a_declared_table_through_the_kit_typed() {
    let e = with_step(GOOD, "[\".yidam/computed/**\"]", ("reader.py", READER));
    let (out, err, code) = e.run(&["run", "probe"]);
    assert_eq!(code, 0, "{out}{err}");
    let root = e.path();
    let report = git(
        &root,
        &["show", "HEAD:.yidam/computed/reports/readings.json"],
    );
    let rows: serde_json::Value = serde_json::from_str(&report).unwrap();
    assert_eq!(rows[0]["discharge"], serde_json::json!(412.5), "{report}");
    assert_eq!(rows[1]["discharge"], serde_json::Value::Null, "{report}");
    assert_eq!(rows[1]["gage"], "gage/valley-bridge", "{report}");
    let receipt = git(&root, &["show", "HEAD:.yidam/runs/probe.yml"]);
    assert!(receipt.contains("tables_sha256:"), "{receipt}");
}

#[test]
fn a_step_is_not_run_on_a_declared_table_that_departs() {
    let e = with_step(
        "reading\tgage\tdischarge\nR1\tgage/canyon-outlet\tlots\n",
        "[\".yidam/computed/**\"]",
        ("reader.py", READER),
    );
    let before = git(&e.path(), &["rev-parse", "HEAD"]);
    let (out, err, code) = e.run(&["run", "probe"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        err.contains("`data/gage-readings.tsv` departs from the table `gage-readings` declares"),
        "{err}"
    );
    assert!(err.contains("the step was not run"), "{err}");
    assert_eq!(git(&e.path(), &["rev-parse", "HEAD"]), before);
}

#[test]
fn a_step_that_writes_a_declared_table_that_departs_commits_nothing() {
    let e = with_step(
        GOOD,
        "[\"data/**\"]",
        (
            "writer.sh",
            "set -eu\nmkdir -p \"$YIDAM_OUT/data\"\n\
             printf 'reading\\tgage\\tdischarge\\nR1\\tgage/canyon-outlet\\tlots\\n' \
             > \"$YIDAM_OUT/data/gage-readings.tsv\"\n",
        ),
    );
    let before = git(&e.path(), &["rev-parse", "HEAD"]);
    let (out, err, code) = e.run(&["run", "probe"]);
    assert_ne!(code, 0, "{out}{err}");
    assert!(
        err.contains("the step wrote `data/gage-readings.tsv`"),
        "{err}"
    );
    assert!(err.contains("nothing was committed"), "{err}");
    assert_eq!(git(&e.path(), &["rev-parse", "HEAD"]), before);
}

#[test]
fn an_undeclared_table_is_read_as_before() {
    let e = with_step(
        GOOD,
        "[\".yidam/computed/**\"]",
        (
            "plain.sh",
            "set -eu\n[ -z \"${YIDAM_TABLES:-}\" ] || exit 3\nmkdir -p \"$YIDAM_OUT/.yidam/computed/reports\"\n\
             wc -l < data/other.tsv > \"$YIDAM_OUT/.yidam/computed/reports/count.txt\"\n",
        ),
    );
    let root = e.path();
    // The step reads only the undeclared file, so it is handed no declarations.
    let manifest = root.join(".yidam/capabilities.toml");
    let text = std::fs::read_to_string(&manifest).unwrap();
    std::fs::write(
        &manifest,
        text.replace("\"data/**\"]\nwrites", "\"data/other.tsv\"]\nwrites"),
    )
    .unwrap();
    std::fs::write(root.join("data/other.tsv"), "a\tb\nnot\ta\ttable\n").unwrap();
    git(&root, &["add", "-A"]);
    git(&root, &["commit", "-qm", "fix: read an undeclared file"]);
    let (out, err, code) = e.run(&["run", "probe"]);
    assert_eq!(code, 0, "{out}{err}");
    let receipt = git(&root, &["show", "HEAD:.yidam/runs/probe.yml"]);
    assert!(!receipt.contains("tables_sha256"), "{receipt}");
}
