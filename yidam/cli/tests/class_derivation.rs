//! Every indexed record carries the class its own path derives.
//!
//! A node's class is its parent directory. That rule is applied twice, at different times and
//! by different code paths: `yidam embed` stamps it onto every record — and so into every
//! index row and every vector pushed to a remote bucket — while the query path recomputes it
//! from the file on disk whenever anybody asks a question.
//!
//! RFC-0033 §4.5 declined to push an anchored step's class filter to a vector service on
//! exactly that premise: *"the two derivations agree today and nothing holds them to it."*
//! `paths::class_of_path` is now the only derivation and both callers go through it, which
//! holds them to each other in the source. This holds them to each other in the **output** —
//! by running the real command over the real example corpora and reading what it wrote.
//!
//! The distinction matters because the source-level guarantee is the weaker of the two. It
//! says the two call sites agree; it says nothing about the records already on disk, which is
//! the population a query actually narrows over.

mod common;

use std::collections::BTreeSet;
use std::path::Path;

/// One record as `yidam embed` wrote it. Only the two fields under test.
#[derive(serde::Deserialize)]
struct Record {
    path: String,
    class: String,
    kind: String,
}

fn records(root: &Path) -> Vec<Record> {
    let dir = root.join(".yidam/embeddings");
    let mut found = Vec::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "json") {
                let text = std::fs::read_to_string(&p).unwrap();
                found.push(
                    serde_json::from_str(&text)
                        .unwrap_or_else(|e| panic!("{} is not a record: {e}", p.display())),
                );
            }
        }
    }
    found
}

/// The derivation, spelled out rather than borrowed from the library.
///
/// `yidam::paths::class_of_path` is the thing under test. A guard that called it would be
/// comparing the function against itself and would survive any change to it — which is the
/// whole failure mode this file exists to rule out.
fn parent_directory_of(path: &str) -> &str {
    let mut parts = path.rsplitn(3, ['/', '\\']);
    parts.next(); // the file
    parts.next().unwrap_or_default()
}

/// Run `yidam embed` over every example corpus and read back what it stamped.
///
/// Every example, discovered from git rather than listed here: a corpus added next year is a
/// corpus this guard covers, and a hardcoded list is a guard that stops growing with the
/// repository without ever going red.
#[test]
fn every_embedded_node_carries_the_class_its_path_derives() {
    let examples = common::examples();
    assert!(
        !examples.is_empty(),
        "no example corpora were discovered — this guard would pass having checked nothing"
    );

    let mut checked = 0usize;
    let mut classes: BTreeSet<String> = BTreeSet::new();
    for name in &examples {
        let example = common::Example::materialize(name);
        let (_out, err, code) = example.run(&["embed"]);
        assert_eq!(code, 0, "`yidam embed` failed in {name}:\n{err}");

        let records = records(&example.path());
        assert!(
            !records.is_empty(),
            "`yidam embed` wrote no records in {name}"
        );

        for r in &records {
            // Catalog sources are excluded on purpose, and not because they are awkward: a
            // source's `class` is its catalog `type` (`paper`, `dataset`, `api`) and its path
            // lives under `.yidam/catalog/`, so the two were never meant to agree. The query
            // path's ownership residual rejects those rows before anything compares them.
            if r.kind != "node" {
                continue;
            }
            assert_eq!(
                r.class,
                parent_directory_of(&r.path),
                "{name}: the record for {} was stamped `{}`, which is not the directory it \
                 sits in. An anchored step narrows a remote search on the recorded value and \
                 resolves the node from the path — a disagreement drops the node.",
                r.path,
                r.class
            );
            classes.insert(r.class.clone());
            checked += 1;
        }
    }

    // A count, because the assertion above is vacuous over an empty set and the loop has three
    // ways to reach one. Measured on 2026-09-20: 45 node records across 4 examples, in 14
    // distinct classes. Both floors sit well under that, so this fails on a corpus that
    // stopped being walked rather than on one that merely changed size.
    assert!(
        checked >= 30,
        "only {checked} node records were checked across {} examples",
        examples.len()
    );
    assert!(
        classes.len() >= 8,
        "only {} distinct classes appeared: {classes:?}",
        classes.len()
    );
}
