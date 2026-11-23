//! `yidam signal` — a computed value quoted in prose, refreshed by `regen` (#1503, RFC-0058).
//!
//! Against `examples/streamflow`, for the reason `count.rs` gives, with one signal table
//! committed beside it. The table is written by hand rather than computed, so every value
//! below is one this file states. The last case is the exception: it lets the example's own
//! `travel-tier` calculator write its table, because what it checks is that a run carries the
//! block it moved.
//!
//! What the cases are for, in one line each: a node's own row is read by name and the same
//! block in markdown is refused; both reference spellings reach the row; each modifier writes
//! its documented form; a block that does not resolve fails and names itself while the others
//! are still written; a value that would change its YAML scalar is refused in a node and
//! written in markdown; the three gates that read blocks read them in node files; and a run
//! folds a block whose only home is a node.

use std::path::Path;
use std::process::Command;

use crate::common;

use common::{repo_root, tracked_under};

const EXAMPLE: &str = "examples/streamflow/";
const NODE: &str = ".yidam/corpus/gage/canyon-outlet.yml";

/// A signal table on two nodes, seeded with one value of each kind a block may quote.
const FIGURES: &str = "\
format_version: 1
signals:
  - node: gage/canyon-outlet
    share: 0.18234
    total: 1234567
    level: high
    quoted: 'say \"hi\"'
  - node: gage/valley-bridge
    share: 0.5
";

/// `examples/streamflow` as its own repository, plus `files`, committed.
fn stage(files: &[(&str, &str)]) -> tempfile::TempDir {
    let root = repo_root();
    let dir = tempfile::tempdir().unwrap();
    for tracked in tracked_under(&root, EXAMPLE) {
        let to = dir.path().join(tracked.strip_prefix(EXAMPLE).unwrap());
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(&tracked), &to).unwrap();
    }
    for (rel, text) in files {
        let to = dir.path().join(rel);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(to, text).unwrap();
    }
    let git = |args: &[&str]| common::git::git_at(dir.path(), args, common::git::FIXTURE_DATE);
    git(&["init", "-q", "-b", "main"]);
    git(&["config", "user.email", "signal@yidam.test"]);
    git(&["config", "user.name", "Signal"]);
    git(&["add", "-A"]);
    git(&[
        "commit",
        "-q",
        "-m",
        "genesis: streamflow and a signal table",
    ]);
    dir
}

/// `stage` with the figures table and `node_line` appended to canyon-outlet's description.
fn with_node_block(node_line: &str, more: &[(&str, &str)]) -> tempfile::TempDir {
    let original = std::fs::read_to_string(repo_root().join(EXAMPLE).join(NODE)).unwrap();
    let node = original.replacen(
        "description: |\n",
        &format!("description: |\n  {node_line}\n\n"),
        1,
    );
    assert_ne!(
        node, original,
        "the node's description is not a block scalar"
    );
    let mut files = vec![
        (".yidam/computed/figures.yml", FIGURES),
        (NODE, node.as_str()),
    ];
    files.extend_from_slice(more);
    stage(&files)
}

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

impl Run {
    fn both(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn run(root: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

fn read(root: &Path, rel: &str) -> String {
    std::fs::read_to_string(root.join(rel)).unwrap()
}

#[test]
fn a_node_reads_its_own_row_by_name_and_markdown_may_not() {
    let dir = with_node_block(
        "Share <!-- REGEN: yidam signal share -->0<!-- /REGEN -->.",
        &[(
            "NOTES.md",
            "Share <!-- REGEN: yidam signal share -->0<!-- /REGEN -->.\n",
        )],
    );
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 1, "{}", r.both());
    assert!(
        read(dir.path(), NODE).contains("<!-- REGEN: yidam signal share -->0.18234<!-- /REGEN -->"),
        "{}",
        read(dir.path(), NODE)
    );
    assert!(r.stderr.contains("NOTES.md"), "{}", r.stderr);
    assert!(
        r.stderr.contains("only a node file has a row of its own"),
        "{}",
        r.stderr
    );
    assert!(read(dir.path(), "NOTES.md").contains("-->0<!--"));
}

#[test]
fn both_reference_spellings_reach_the_row() {
    let dir = stage(&[
        (".yidam/computed/figures.yml", FIGURES),
        (
            "NOTES.md",
            "a <!-- REGEN: yidam signal gage/valley-bridge share -->0<!-- /REGEN -->\n\
             b <!-- REGEN: yidam signal yidam://corpus/node/gage/valley-bridge share -->0<!-- /REGEN -->\n",
        ),
    ]);
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 0, "{}", r.both());
    let notes = read(dir.path(), "NOTES.md");
    assert_eq!(notes.matches("-->0.5<!--").count(), 2, "{notes}");

    let r = run(
        dir.path(),
        &["signal", "gage/valley-bridge", "share", "percent"],
    );
    assert_eq!((r.code, r.stdout.as_str()), (0, "50.0%\n"), "{}", r.stderr);
}

#[test]
fn each_modifier_writes_its_documented_form_in_any_order() {
    let dir = stage(&[
        (".yidam/computed/figures.yml", FIGURES),
        (
            "NOTES.md",
            "a <!-- REGEN: yidam signal gage/canyon-outlet share places=1 percent -->0<!-- /REGEN -->\n\
             b <!-- REGEN: yidam signal gage/canyon-outlet share percent places=1 -->0<!-- /REGEN -->\n\
             c <!-- REGEN: yidam signal gage/canyon-outlet total grouped -->0<!-- /REGEN -->\n\
             d <!-- REGEN: yidam signal gage/canyon-outlet share places=2 -->0<!-- /REGEN -->\n",
        ),
    ]);
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 0, "{}", r.both());
    let notes = read(dir.path(), "NOTES.md");
    assert_eq!(notes.matches("-->18.2%<!--").count(), 2, "{notes}");
    assert!(notes.contains("-->1,234,567<!--"), "{notes}");
    assert!(notes.contains("-->0.18<!--"), "{notes}");
}

#[test]
fn a_block_that_does_not_resolve_is_named_and_the_others_are_written() {
    let dir = stage(&[
        (".yidam/computed/figures.yml", FIGURES),
        (
            "NOTES.md",
            "a <!-- REGEN: yidam signal gage/canyon-outlet nosuch -->0<!-- /REGEN -->\n\
             b <!-- REGEN: yidam signal gage/valley-bridge total -->0<!-- /REGEN -->\n\
             c <!-- REGEN: yidam signal gage/canyon-outlet level percent -->0<!-- /REGEN -->\n\
             d <!-- REGEN: yidam signal gage/canyon-outlet share percent percent -->0<!-- /REGEN -->\n\
             e <!-- REGEN: yidam signal gage/canyon-outlet total -->0<!-- /REGEN -->\n",
        ),
    ]);
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 1, "{}", r.both());
    for (block, why) in [
        ("gage/canyon-outlet nosuch", "no computed file declares"),
        ("gage/valley-bridge total", "has no `total` row"),
        ("gage/canyon-outlet level percent", "applies to a number"),
        ("gage/canyon-outlet share percent percent", "twice"),
    ] {
        assert!(
            r.stderr.contains(block),
            "{block} is not named:\n{}",
            r.stderr
        );
        assert!(r.stderr.contains(why), "{why}:\n{}", r.stderr);
    }
    let notes = read(dir.path(), "NOTES.md");
    assert!(
        notes.contains("e <!-- REGEN: yidam signal gage/canyon-outlet total -->1234567<!--"),
        "{notes}"
    );
    assert_eq!(notes.matches("-->0<!--").count(), 4, "{notes}");
}

#[test]
fn a_collision_is_reported_by_the_block_that_names_it() {
    let other = "format_version: 1\nsignals:\n  - node: gage/canyon-outlet\n    share: 0.9\n";
    let dir = stage(&[
        (".yidam/computed/figures.yml", FIGURES),
        (".yidam/computed/other.yml", other),
        (
            "NOTES.md",
            "a <!-- REGEN: yidam signal gage/canyon-outlet share -->0<!-- /REGEN -->\n",
        ),
    ]);
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 1, "{}", r.both());
    assert!(
        r.stderr.contains("other.yml") && r.stderr.contains("figures.yml"),
        "{}",
        r.stderr
    );
}

#[test]
fn a_quote_is_refused_in_a_node_and_written_in_markdown() {
    let dir = with_node_block(
        "Says <!-- REGEN: yidam signal quoted -->x<!-- /REGEN -->.",
        &[(
            "NOTES.md",
            "Says <!-- REGEN: yidam signal gage/canyon-outlet quoted -->x<!-- /REGEN -->.\n",
        )],
    );
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 1, "{}", r.both());
    assert!(r.stderr.contains(NODE), "{}", r.stderr);
    assert!(r.stderr.contains("quote or a backslash"), "{}", r.stderr);
    assert!(read(dir.path(), NODE).contains("-->x<!--"));
    assert!(read(dir.path(), "NOTES.md").contains("-->say \"hi\"<!--"));
}

/// A string that would turn a plain scalar into a mapping, or cut it at a comment, is refused
/// in a node and leaves the node as it was.
#[test]
fn a_plain_scalar_breaker_is_refused_in_a_node() {
    let original = std::fs::read_to_string(repo_root().join(EXAMPLE).join(NODE)).unwrap();
    let node =
        format!("{original}notes: Level <!-- REGEN: yidam signal noted -->x<!-- /REGEN -->.\n");
    let figures =
        "format_version: 1\nsignals:\n  - node: gage/canyon-outlet\n    noted: 'high: see #1'\n";
    let dir = stage(&[
        (".yidam/computed/figures.yml", figures),
        (NODE, node.as_str()),
    ]);
    let r = run(dir.path(), &["signal"]);
    assert_eq!(r.code, 1, "{}", r.both());
    assert!(r.stderr.contains("plain YAML scalar"), "{}", r.stderr);
    assert_eq!(read(dir.path(), NODE), node);
}

/// `count` reads node files as `signal` does: a block there is refreshed, and the line-shaped
/// one beside it, which no reader reads in a node, is left as written.
#[test]
fn a_count_block_in_a_node_is_refreshed_and_a_line_shaped_one_is_not() {
    let dir = with_node_block(
        "Of <!-- REGEN: yidam count concept -->99<!-- /REGEN --> concepts.\n  \
         <!-- REGEN: yidam count concept -->\n  99\n  <!-- /REGEN -->",
        &[],
    );
    let r = run(dir.path(), &["regen", "--check"]);
    assert_ne!(r.code, 0, "{}", r.both());
    assert!(r.both().contains(NODE), "{}", r.both());
    let r = run(dir.path(), &["count"]);
    assert_eq!(r.code, 0, "{}", r.both());
    let after = read(dir.path(), NODE);
    assert!(
        after.contains("Of <!-- REGEN: yidam count concept -->4<!-- /REGEN --> concepts."),
        "{after}"
    );
    assert!(
        after.contains("<!-- REGEN: yidam count concept -->\n  99\n  <!-- /REGEN -->"),
        "{after}"
    );
}

#[test]
fn the_gates_read_blocks_in_node_files() {
    // Stale: `regen --check` reports the node, and `regen` refreshes it.
    let dir = with_node_block(
        "Level <!-- REGEN: yidam signal level -->low<!-- /REGEN -->.",
        &[],
    );
    let r = run(dir.path(), &["regen", "--check"]);
    assert_ne!(r.code, 0, "{}", r.both());
    assert!(r.both().contains(NODE), "{}", r.both());
    let r = run(dir.path(), &["regen"]);
    assert_eq!(r.code, 0, "{}", r.both());
    assert!(read(dir.path(), NODE).contains("-->high<!--"));
    let r = run(dir.path(), &["regen", "--check"]);
    assert_eq!(r.code, 0, "{}", r.both());

    // Unclaimed: a misspelt generator in a node is reported.
    let dir = with_node_block(
        "Level <!-- REGEN: yidam signl level -->low<!-- /REGEN -->.",
        &[],
    );
    let r = run(dir.path(), &["regen", "--check"]);
    assert_ne!(r.code, 0, "{}", r.both());
    assert!(r.both().contains("signl"), "{}", r.both());

    // Malformed: a block opened at the start of a line and never closed is a lint finding.
    let dir = with_node_block("<!-- REGEN: yidam signal level -->", &[]);
    let r = run(dir.path(), &["lint", "--format", "json"]);
    assert!(r.stdout.contains("malformed-regen-block"), "{}", r.both());
    assert!(r.stdout.contains("canyon-outlet.yml"), "{}", r.both());
}

#[test]
fn a_reader_of_node_prose_sees_the_value_and_not_the_tags() {
    let dir = with_node_block(
        "Level <!-- REGEN: yidam signal level -->high<!-- /REGEN -->.",
        &[],
    );
    let r = run(dir.path(), &["export", "--format", "llms"]);
    assert_eq!(r.code, 0, "{}", r.both());
    assert!(r.stdout.contains("Level high."), "{}", r.stdout);
    assert!(!r.stdout.contains("yidam signal"), "{}", r.stdout);
    let r = run(dir.path(), &["export", "--format", "graphml"]);
    assert_eq!(r.code, 0, "{}", r.both());
    assert!(
        !r.stdout.contains("yidam signal"),
        "graphml carries the tags"
    );
    let r = run(dir.path(), &["export", "--format", "rdf"]);
    assert_eq!(r.code, 0, "{}", r.both());
    assert!(!r.stdout.contains("yidam signal"), "rdf carries the tags");
}

/// The fold widened with the reader: a repository whose only block sits in a node still has
/// it carried by the `compute:` commit that moved it (RFC-0058 §2.4).
#[test]
fn a_run_folds_a_block_whose_only_home_is_a_node() {
    let original = std::fs::read_to_string(repo_root().join(EXAMPLE).join(NODE)).unwrap();
    let node = original.replacen(
        "description: |\n",
        "description: |\n  Travels as <!-- REGEN: yidam signal travels_as -->unknown<!-- /REGEN -->.\n\n",
        1,
    );
    let dir = stage(&[(NODE, node.as_str())]);
    let root = dir.path();
    let r = run(root, &["run", "travel-tier"]);
    assert_eq!(r.code, 0, "{}", r.both());
    assert!(
        !r.stderr.contains("landed without"),
        "the fold was skipped: {}",
        r.stderr
    );
    let files = common::git::out(root, &["show", "--name-only", "--format=", "HEAD"]);
    assert!(
        files.lines().any(|f| f == NODE),
        "the compute commit does not carry the node:\n{files}"
    );
    let landed = common::git::out(root, &["show", &format!("HEAD:{NODE}")]);
    assert!(!landed.contains("-->unknown<!--"), "{landed}");
    assert!(
        landed.contains("Travels as <!-- REGEN: yidam signal travels_as -->"),
        "{landed}"
    );
}
