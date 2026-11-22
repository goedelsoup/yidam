//! A step's outputs are read the way the next reader will read them, before they land
//! (RFC-0056 §2.1).
//!
//! `check_declared` asks whether a step wrote where it said it would. This asks whether what it
//! wrote can be read at all. Both run before the commit is built, and both refuse with *nothing
//! was committed*. `doctor`'s `computed` check already fails a file that does not parse, but it
//! reads the committed tree: by then the step has landed, its receipt says its outputs stand, and
//! a re-run with unchanged inputs writes nothing.
//!
//! Three readings, by where a file is and what it is called:
//!
//! - every `*.yml` and `*.yaml` parses as YAML, and every `*.json` as JSON;
//! - `.yidam/computed/*.yaml` is not written at all, because [`crate::computed::Signals`] lists
//!   that directory for `.yml` only and a table under the other spelling would be read by no one;
//! - `.yidam/computed/*.yml` passes every rule `Signals` applies, read over the tree the commit
//!   would have, so a collision with a committed file and a row keyed on a node the corpus does
//!   not hold are caught here.
//!
//! Anything else only has to be UTF-8, which `build_commit` already requires. A report artifact
//! keeps every form it has today.

use std::path::Path;

use anyhow::{bail, Result};

use super::exec::Scratch;
use crate::cmd::propose::write::{git, TempIndex};

const COMPUTED: &str = ".yidam/computed/";

/// A path directly under `.yidam/computed/` with the given extension. Not a subdirectory:
/// `Signals` does not descend, so a nested file is a report artifact like any other.
fn top_level_computed(path: &str, ext: &str) -> bool {
    path.strip_prefix(COMPUTED)
        .is_some_and(|rest| !rest.contains('/') && rest.ends_with(ext))
}

/// Refuse the step if any output it produced would not read. `parent` is the commit the
/// landing will sit on, which is the tree the signal-table rules are asked of.
pub fn check_readable(root: &Path, parent: &str, produced: &[(String, Vec<u8>)]) -> Result<()> {
    let mut refused: Vec<String> = Vec::new();
    for (path, bytes) in produced {
        if top_level_computed(path, ".yaml") {
            let spelled = format!("{}.yml", path.trim_end_matches(".yaml"));
            refused.push(format!(
                "{path}: a computed file is read as `.yml` only, so a table here would reach no \
                 reader. Write it as {spelled}"
            ));
            continue;
        }
        if let Some(problem) = parse_problem(path, bytes) {
            refused.push(problem);
        }
    }
    if !refused.is_empty() {
        bail!(
            "the step wrote output that does not read, and nothing was committed.\n  {}",
            refused.join("\n  ")
        );
    }

    let written: Vec<String> = produced
        .iter()
        .map(|(p, _)| p.clone())
        .filter(|p| top_level_computed(p, ".yml"))
        .collect();
    if written.is_empty() {
        return Ok(());
    }
    let tree = overlay(root, parent, produced)?;
    let signals = crate::computed::Signals::load(tree.path());
    let theirs: Vec<&str> = signals.problems_naming(&written).collect();
    if !theirs.is_empty() {
        bail!(
            "the step wrote a signal table that a later reader refuses, and nothing was \
             committed.\n  {}",
            theirs.join("\n  ")
        );
    }
    Ok(())
}

/// Why one output does not parse, or `None` when it does or is not a format this reads.
///
/// Names the parser's line and column and quotes that line, because the person reading the
/// refusal no longer has the file: the step's scratch output is gone when the run returns.
fn parse_problem(path: &str, bytes: &[u8]) -> Option<String> {
    let yaml = path.ends_with(".yml") || path.ends_with(".yaml");
    let json = path.ends_with(".json");
    if !yaml && !json {
        return None;
    }
    // Not UTF-8 is `build_commit`'s refusal, and it says so in its own words.
    let text = std::str::from_utf8(bytes).ok()?;
    let (what, message, line) = if yaml {
        let e = serde_yaml::from_str::<serde_yaml::Value>(text).err()?;
        let line = e.location().map(|l| l.line());
        ("YAML", e.to_string(), line)
    } else {
        let e = serde_json::from_str::<serde_json::Value>(text).err()?;
        ("JSON", e.to_string(), Some(e.line()))
    };
    let quoted = line
        .filter(|n| *n > 0)
        .and_then(|n| text.lines().nth(n - 1))
        .map(|l| format!("\n    line {}: {}", line.unwrap_or(0), l.trim_end()))
        .unwrap_or_default();
    Some(format!(
        "{path} does not parse as {what}: {message}{quoted}"
    ))
}

/// The parent's `.yidam/` as the signal-table reader needs it, with the step's outputs written
/// over it.
///
/// `Signals` reads three things: the manifest, the top-level computed files, and whether a node
/// file exists. So the manifest and the computed files are checked out, and each corpus file is
/// written empty: the reader asks only that it is there. A step that writes a node and a row
/// keyed on it in one landing sees both.
fn overlay(root: &Path, parent: &str, produced: &[(String, Vec<u8>)]) -> Result<Scratch> {
    let index = TempIndex::new(root, "outputs")?;
    let index_path = index.path().to_path_buf();
    git(root, Some(&index_path), &["read-tree", parent], None)?;
    let listed = git(
        root,
        Some(&index_path),
        &["ls-files", "-z", "--", ".yidam"],
        None,
    )?;
    let tree = Scratch::new("outputs")?;

    let mut wanted: Vec<&str> = Vec::new();
    for path in listed.split('\0').filter(|p| !p.is_empty()) {
        if path == crate::cmd::run::manifest::MANIFEST || top_level_computed(path, ".yml") {
            wanted.push(path);
        } else if path.starts_with(".yidam/corpus/") && path.ends_with(".yml") {
            write(tree.path(), path, b"")?;
        }
    }
    if !wanted.is_empty() {
        let prefix = format!("{}/", tree.path().display());
        git(
            root,
            Some(&index_path),
            &["checkout-index", "-f", "--stdin", "-z", "--prefix", &prefix],
            Some(&wanted.join("\0")),
        )?;
    }
    for (path, bytes) in produced {
        write(tree.path(), path, bytes)?;
    }
    Ok(tree)
}

fn write(base: &Path, rel: &str, bytes: &[u8]) -> Result<()> {
    let at = base.join(rel);
    if let Some(dir) = at.parent() {
        std::fs::create_dir_all(dir)?;
    }
    std::fs::write(at, bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_file_directly_under_computed_is_a_signal_table() {
        assert!(top_level_computed(".yidam/computed/a.yml", ".yml"));
        assert!(!top_level_computed(".yidam/computed/sub/a.yml", ".yml"));
        assert!(!top_level_computed(".yidam/corpus/a.yml", ".yml"));
        assert!(top_level_computed(".yidam/computed/a.yaml", ".yaml"));
    }

    #[test]
    fn a_sequence_dedented_under_a_mapping_is_refused_with_its_line() {
        // ngsim-arterial.yml's queue_discharge block, six commits running (RFC-0056 §1.2).
        let text = "format_version: 1\nqueue_discharge:\n  crossings: 4757\n  \
                    queues: 332\n  - {direction: 2, queues: 106}\n";
        let p = parse_problem(".yidam/computed/x.yml", text.as_bytes()).expect("refused");
        assert!(p.contains("does not parse as YAML"), "{p}");
        assert!(p.contains("line 5:   - {direction: 2, queues: 106}"), "{p}");
    }

    #[test]
    fn an_unquoted_time_in_a_flow_sequence_reads_for_yidam() {
        // travel-demand's line 105 is YAML 1.2 and reads here; PyYAML refuses it. The kit's
        // writer quotes every string, so a kit step cannot write it (RFC-0056 §1.2).
        assert!(parse_problem("x.yml", b"rows:\n- [00:00, 184, 0.5]\n").is_none());
    }

    #[test]
    fn json_and_other_files_are_read_by_extension() {
        assert!(parse_problem("a.json", b"{\"a\": 1,}").is_some());
        assert!(parse_problem("a.json", b"{\"a\": 1}").is_none());
        assert!(parse_problem("graph.tsv", b"- [00:00").is_none());
        assert!(parse_problem("a.yml", b"a: [1, 2]\n").is_none());
    }
}
