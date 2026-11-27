//! `.yidam/embeddings/` holds a record for exactly the nodes and sources the corpus has now.
//!
//! `index-build` turns every `.json` under that directory into an index row. So a record left
//! behind by a node that was deleted, renamed or moved to another class is a row for a node
//! the corpus no longer has, and `retrieve` returns it. Before #1625 `embed` only ever wrote
//! records and never removed one, and every rename left one behind.
//!
//! Run over a real example rather than a hand-made tree, because the population that matters
//! is what `embed` writes for a corpus, by the same path rules the corpus is read with.

use crate::common;

use std::collections::BTreeSet;
use std::path::Path;

#[derive(serde::Deserialize)]
struct Record {
    path: String,
}

/// Every record under `.yidam/embeddings/`, by the `path` it names.
fn recorded(root: &Path) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![root.join(".yidam/embeddings")];
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
                let r: Record = serde_json::from_str(&text)
                    .unwrap_or_else(|e| panic!("{} is not a record: {e}", p.display()));
                found.insert(r.path);
            }
        }
    }
    found
}

/// Every file with extension `ext` under `dir`, as a path relative to `root`.
fn files_under(root: &Path, dir: &str, ext: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut stack = vec![root.join(dir)];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == ext) {
                let rel = p.strip_prefix(root).unwrap();
                found.insert(rel.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found
}

/// Every corpus node: a `.yml` inside a class directory. The `<class>.ont.yml` definitions sit
/// at the top of `.yidam/corpus/` and are not nodes.
fn nodes(root: &Path) -> BTreeSet<String> {
    files_under(root, ".yidam/corpus", "yml")
        .into_iter()
        .filter(|p| p.matches('/').count() > 2)
        .collect()
}

fn embed(example: &common::Example, args: &[&str]) -> String {
    let mut all = vec!["embed"];
    all.extend_from_slice(args);
    let (out, err, code) = example.run(&all);
    assert_eq!(code, 0, "`yidam {}` failed:\n{err}", all.join(" "));
    out
}

#[test]
fn a_deleted_renamed_or_moved_node_leaves_no_record_behind() {
    let example = common::Example::materialize("journalism");
    let root = example.path();
    embed(&example, &[]);

    let corpus = root.join(".yidam/corpus");
    std::fs::rename(
        corpus.join("entity/lorne-vasquez.yml"),
        corpus.join("entity/l-vasquez.yml"),
    )
    .unwrap();
    std::fs::rename(
        corpus.join("thread/terminal-conditions.yml"),
        corpus.join("finding/terminal-conditions.yml"),
    )
    .unwrap();
    std::fs::remove_file(corpus.join("document/maintenance-memo.yml")).unwrap();
    let catalog = root.join(".yidam/catalog");
    std::fs::rename(
        catalog.join("edgar-filings.md"),
        catalog.join("sec-edgar.md"),
    )
    .unwrap();

    let out = embed(&example, &[]);
    assert!(
        out.contains("removed 4 record(s)"),
        "the second run should say what it removed:\n{out}"
    );

    let mut expected = nodes(&root);
    expected.extend(files_under(&root, ".yidam/catalog", "md"));
    assert_eq!(
        recorded(&root),
        expected,
        "`.yidam/embeddings/` should hold one record per node and source on disk"
    );
    assert!(
        !root
            .join(".yidam/embeddings/thread/terminal-conditions.json")
            .exists()
            && !root
                .join(".yidam/embeddings/entity/lorne-vasquez.json")
                .exists()
    );
}

/// `--no-catalog` says the catalog is not to be retrievable. A source record an earlier run
/// wrote would still be indexed, so it goes too.
#[test]
fn no_catalog_removes_the_catalog_records_an_earlier_run_wrote() {
    let example = common::Example::materialize("journalism");
    let root = example.path();
    embed(&example, &[]);
    assert!(root.join(".yidam/embeddings/_catalog").is_dir());

    embed(&example, &["--no-catalog"]);
    assert!(!root.join(".yidam/embeddings/_catalog").exists());
    assert_eq!(recorded(&root), nodes(&root));
}

/// A dry run writes nothing, and removes nothing either.
#[test]
fn a_dry_run_removes_nothing() {
    let example = common::Example::materialize("journalism");
    let root = example.path();
    embed(&example, &[]);
    let before = recorded(&root);
    std::fs::remove_file(root.join(".yidam/corpus/document/maintenance-memo.yml")).unwrap();

    embed(&example, &["--dry-run"]);
    assert_eq!(recorded(&root), before);
}
