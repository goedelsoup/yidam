//! Coverage, and the difference between *untested* and *not compiled here*.
//!
//! 35,806 lines of Rust had no coverage measurement of any kind (#464). The obvious fix
//! publishes one percentage, and it would be false in the specific way this repository cares
//! about: pull-request CI compiles the light default, `--features index` code is never built
//! there (a deliberate latency trade argued at `ci.yml`), and a coverage run under that build
//! sees every gated file at **zero**. Published as a number, that says *untested* where the
//! truth is *not compiled here*.
//!
//! `report.rs`'s `features` field already states the principle for the binary's own report:
//!
//! > Cargo features compiled in. `reports` names the base and is always present; the rest gate
//! > whole subcommands, so a consumer can tell "this binary cannot do that" from "that failed".
//!
//! The same field on a coverage report separates unmeasured from untested. This module makes
//! that separation by *discovery* rather than by a list: a file compiled into the measured
//! build appears in the LCOV; a file the feature flags excluded does not appear at all.
//!
//! Absence has more than one cause, so it is classified rather than assumed — and the fourth
//! class is the one worth having. A production file that the build did not compile, with no
//! feature gate to explain it and functions inside it, is an orphaned module: reachable by no
//! build, measured by nothing, and invisible to a report that called it "unmeasured" along
//! with the legitimate ones.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// Per-file line coverage, as LCOV records it.
#[derive(Debug, Default, Clone)]
pub struct Lcov {
    /// file path (as LCOV spells it) → line number → execution count
    pub files: BTreeMap<String, BTreeMap<u32, u64>>,
}

impl Lcov {
    /// Does any record end with this repository-relative path?
    ///
    /// LCOV paths are absolute on the machine that produced them, so matching is by suffix.
    fn record_for(&self, rel: &str) -> Option<&BTreeMap<u32, u64>> {
        self.files
            .iter()
            .find(|(f, _)| f.ends_with(rel))
            .map(|(_, v)| v)
    }

    pub fn covers(&self, rel: &str) -> bool {
        self.record_for(rel).is_some()
    }
}

/// Parse LCOV. Only `SF:` and `DA:` matter here — the branch and function records describe
/// the same lines from another angle and nothing below asks about them.
pub fn parse_lcov(text: &str) -> Lcov {
    let mut out = Lcov::default();
    let mut current: Option<String> = None;
    for line in text.lines() {
        if let Some(path) = line.strip_prefix("SF:") {
            current = Some(path.trim().to_string());
            out.files.entry(path.trim().to_string()).or_default();
        } else if let Some(da) = line.strip_prefix("DA:") {
            let (Some(file), Some((num, count))) = (current.as_ref(), da.trim().split_once(','))
            else {
                continue;
            };
            if let (Ok(num), Ok(count)) = (num.parse::<u32>(), count.parse::<u64>()) {
                out.files
                    .entry(file.clone())
                    .or_default()
                    .insert(num, count);
            }
        } else if line.starts_with("end_of_record") {
            current = None;
        }
    }
    out
}

pub fn read_lcov(path: &Path) -> Result<Lcov> {
    let text = std::fs::read_to_string(path)
        .with_context(|| format!("reading LCOV at {}", path.display()))?;
    Ok(parse_lcov(&text))
}

// ── why a source file might be absent from the report ────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Absence {
    /// A `#[cfg(feature = "…")]` on its `mod` declaration kept it out of this build.
    /// Unmeasured, and legitimately so.
    Gated(String),
    /// Test code. Not production, and not this report's subject.
    TestOnly,
    /// No function outside a test module — nothing for an instrumenter to record.
    NoCoverableCode,
    /// None of the above: production code the measured build did not compile, and nothing
    /// in the tree says why.
    Unexplained,
}

/// Where each `mod` declaration sits behind a feature, discovered from the tree.
///
/// The map is module *name* to feature, because a gate is written on the declaration
/// (`#[cfg(feature = "index")] mod index_build;`) and not in the file it admits. A file
/// therefore cannot say on its own whether it is gated, which is why this walks the parents.
fn feature_gates(src_root: &Path) -> BTreeMap<String, String> {
    let mut gates = BTreeMap::new();
    for path in rust_files(src_root) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let trimmed = line.trim_start();
            let Some(rest) = trimmed
                .strip_prefix("pub(crate) mod ")
                .or_else(|| trimmed.strip_prefix("pub mod "))
                .or_else(|| trimmed.strip_prefix("mod "))
            else {
                continue;
            };
            let Some(name) = rest.split(&[';', ' '][..]).next().filter(|n| !n.is_empty()) else {
                continue;
            };
            // Look back over the attributes and comments attached to this declaration.
            for prev in lines[..i].iter().rev().take(6) {
                let prev = prev.trim_start();
                if let Some(feat) = prev
                    .strip_prefix("#[cfg(feature = \"")
                    .and_then(|r| r.split('"').next())
                {
                    gates.insert(name.to_string(), feat.to_string());
                    break;
                }
                if !prev.starts_with("//") && !prev.starts_with("#[") && !prev.is_empty() {
                    break;
                }
            }
        }
    }
    gates
}

/// Every `.rs` file under a source root.
pub fn rust_files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "rs") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

/// Is this file test code — either named `tests.rs` or declared `#[cfg(test)] mod`?
fn is_test_file(rel: &str, cfg_test_mods: &BTreeSet<String>) -> bool {
    let stem = Path::new(rel).file_stem().and_then(|s| s.to_str());
    stem == Some("tests") || stem.is_some_and(|s| cfg_test_mods.contains(s))
}

/// Modules declared `#[cfg(test)] mod x;` anywhere in the tree.
fn cfg_test_mods(src_root: &Path) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for path in rust_files(src_root) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            let t = line.trim_start();
            if !t.starts_with("mod ") {
                continue;
            }
            if lines[..i]
                .iter()
                .rev()
                .take(2)
                .any(|p| p.trim_start().starts_with("#[cfg(test)]"))
            {
                if let Some(name) = t.trim_start_matches("mod ").split(';').next() {
                    out.insert(name.trim().to_string());
                }
            }
        }
    }
    out
}

/// Does this file declare a function outside a test module?
fn has_coverable_code(text: &str) -> bool {
    text.lines()
        .map(|l| l.split("//").next().unwrap_or("").trim_start())
        .any(|l| {
            l.starts_with("fn ")
                || l.starts_with("pub fn ")
                || l.starts_with("pub(crate) fn ")
                || l.starts_with("async fn ")
                || l.starts_with("pub async fn ")
        })
}

/// Classify every source file the measured build did not report on.
pub fn absences(src_root: &Path, repo_root: &Path, lcov: &Lcov) -> BTreeMap<String, Absence> {
    let gates = feature_gates(src_root);
    let test_mods = cfg_test_mods(src_root);
    let mut out = BTreeMap::new();

    for path in rust_files(src_root) {
        let rel = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        if lcov.covers(&rel) {
            continue;
        }
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        let text = std::fs::read_to_string(&path).unwrap_or_default();

        let why = if is_test_file(&rel, &test_mods) {
            Absence::TestOnly
        } else if let Some(feature) = gates.get(&stem) {
            Absence::Gated(feature.clone())
        } else if !has_coverable_code(&text) {
            Absence::NoCoverableCode
        } else {
            Absence::Unexplained
        };
        out.insert(rel, why);
    }
    out
}

// ── diff coverage ────────────────────────────────────────────────────────────

/// One file's added lines, and what the measurement can say about them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    pub path: String,
    pub added: Vec<u32>,
    /// `None` when the file was not compiled into the measured build — which is *not* the
    /// same as every added line being uncovered, and is the distinction this whole module
    /// exists to keep.
    pub uncovered: Option<Vec<u32>>,
    pub absence: Option<Absence>,
}

/// Added line numbers per file, from `git diff --unified=0`.
///
/// `-U0` so a hunk header's line range is exactly the added lines: with context, the range
/// covers lines the change did not touch and diff coverage starts grading a neighbour's code.
pub fn parse_added_lines(diff: &str) -> BTreeMap<String, Vec<u32>> {
    let mut out: BTreeMap<String, Vec<u32>> = BTreeMap::new();
    let mut file = String::new();
    let mut next_line = 0u32;
    for line in diff.lines() {
        if let Some(rest) = line.strip_prefix("+++ b/") {
            file = rest.trim().to_string();
        } else if line.starts_with("@@") {
            // @@ -a,b +c,d @@
            if let Some(plus) = line.split('+').nth(1) {
                let head = plus.split(&[' ', ','][..]).next().unwrap_or("0");
                next_line = head.parse().unwrap_or(0);
            }
        } else if line.starts_with('+') && !line.starts_with("+++") {
            if !file.is_empty() {
                out.entry(file.clone()).or_default().push(next_line);
            }
            next_line += 1;
        }
    }
    out
}

/// What the measured run can say about the lines this change added.
///
/// `src_prefix` is the source root the measurement covers, and files outside it are dropped
/// rather than classified. This is not a detail: run against a real pull request the first
/// version reported six files as "not compiled, and nothing says why" — the alarming class
/// reserved for orphaned modules — when they were a different workspace's sources and this
/// crate's own integration tests. A coverage run has no opinion about code it was never
/// pointed at, and saying nothing is the correct thing to say about it.
pub fn diff_coverage(
    diff: &str,
    lcov: &Lcov,
    absences: &BTreeMap<String, Absence>,
    src_prefix: &str,
) -> Vec<FileDiff> {
    let mut out = Vec::new();
    for (path, added) in parse_added_lines(diff) {
        if !path.ends_with(".rs") || !path.starts_with(src_prefix) {
            continue;
        }
        let record = lcov.record_for(&path);
        let uncovered = record.map(|lines| {
            added
                .iter()
                .filter(|l| lines.get(l).is_some_and(|count| *count == 0))
                .copied()
                .collect::<Vec<u32>>()
        });
        out.push(FileDiff {
            absence: absences.get(&path).cloned(),
            path,
            added,
            uncovered,
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const LCOV: &str = "\
SF:/runner/work/yidam/yidam/cli/src/parse.rs
DA:10,3
DA:11,0
DA:12,1
end_of_record
SF:/runner/work/yidam/yidam/cli/src/report.rs
DA:5,7
end_of_record
";

    #[test]
    fn it_reads_per_line_execution_counts() {
        let lcov = parse_lcov(LCOV);
        assert_eq!(lcov.files.len(), 2);
        assert!(lcov.covers("yidam/cli/src/parse.rs"));
        assert!(!lcov.covers("yidam/cli/src/embedding.rs"));
    }

    #[test]
    fn added_lines_come_from_the_hunk_headers() {
        let diff = "\
diff --git a/yidam/cli/src/parse.rs b/yidam/cli/src/parse.rs
--- a/yidam/cli/src/parse.rs
+++ b/yidam/cli/src/parse.rs
@@ -9,0 +10,3 @@ fn thing() {
+    let a = 1;
+    let b = 2;
+    let c = 3;
";
        let added = parse_added_lines(diff);
        assert_eq!(added["yidam/cli/src/parse.rs"], vec![10, 11, 12]);
    }

    /// The assertion #464 names as the one a naive integration will miss.
    ///
    /// A file the build did not compile has no LCOV record, and every added line in it looks
    /// uncovered to anything that treats "no record" as "count zero". Reported that way, a
    /// change to the index path would show as 0% covered — *untested* — when the truth is
    /// that this build cannot compile it.
    #[test]
    fn a_file_the_build_did_not_compile_is_unmeasured_rather_than_uncovered() {
        let diff = "\
+++ b/yidam/cli/src/embedding.rs
@@ -0,0 +1,2 @@
+    let model = resolve();
+    model.embed(text)
";
        let lcov = parse_lcov(LCOV);
        let mut absences = BTreeMap::new();
        absences.insert(
            "yidam/cli/src/embedding.rs".to_string(),
            Absence::Gated("vector-read".into()),
        );

        let files = diff_coverage(diff, &lcov, &absences, "yidam/cli/src");
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.added.len(), 2);
        assert_eq!(
            f.uncovered, None,
            "a gated file must report NO uncovered lines, not two — it was never compiled"
        );
        assert_eq!(f.absence, Some(Absence::Gated("vector-read".into())));
    }

    #[test]
    fn a_compiled_file_reports_the_added_lines_no_test_executed() {
        let diff = "\
+++ b/yidam/cli/src/parse.rs
@@ -9,0 +10,3 @@
+    a
+    b
+    c
";
        let files = diff_coverage(diff, &parse_lcov(LCOV), &BTreeMap::new(), "yidam/cli/src");
        assert_eq!(files[0].uncovered, Some(vec![11]), "line 11 has DA count 0");
    }

    /// Found by running the report against a real pull request rather than a fixture.
    ///
    /// A change touching another workspace's sources, or this crate's own integration tests,
    /// is not something the CLI's coverage run can speak to. Classified rather than skipped,
    /// they landed in `Unexplained` — the class that means "an orphaned module nobody
    /// compiles" — and six of them appeared under a heading in bold.
    #[test]
    fn files_outside_the_measured_source_root_are_not_graded_at_all() {
        let diff = "\
+++ b/yidam/cli/tests/coverage_reporting.rs
@@ -0,0 +1,2 @@
+    let a = 1;
+    let b = 2;
+++ b/yidam/tests/harness/ci-report/src/coverage.rs
@@ -0,0 +1,1 @@
+    let c = 3;
";
        let files = diff_coverage(diff, &parse_lcov(LCOV), &BTreeMap::new(), "yidam/cli/src");
        assert!(
            files.is_empty(),
            "a coverage run pointed at yidam/cli/src graded {} file(s) outside it",
            files.len()
        );
    }

    #[test]
    fn non_rust_files_are_not_graded() {
        let diff = "+++ b/README.md\n@@ -1,0 +2,1 @@\n+a line\n";
        assert!(
            diff_coverage(diff, &parse_lcov(LCOV), &BTreeMap::new(), "yidam/cli/src").is_empty()
        );
    }

    #[test]
    fn a_feature_gate_on_a_mod_declaration_is_found() {
        let dir = std::env::temp_dir().join(format!("ci-report-gates-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("mod.rs"),
            "mod plain;\n#[cfg(feature = \"index\")]\npub(crate) mod index_build;\n",
        )
        .unwrap();
        let gates = feature_gates(&dir);
        assert_eq!(gates.get("index_build"), Some(&"index".to_string()));
        assert_eq!(
            gates.get("plain"),
            None,
            "an ungated mod must not be listed"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// A comment between the attribute and the declaration must not break the association —
    /// `cmd/mod.rs` has exactly that shape, with a four-line note under the `#[cfg]`.
    #[test]
    fn a_comment_between_the_gate_and_the_mod_does_not_hide_it() {
        let dir = std::env::temp_dir().join(format!("ci-report-cmt-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("mod.rs"),
            "#[cfg(feature = \"index\")]\n// Widened for retrieval::vector, which resolves\n// the model by name.\npub(crate) mod index_build;\n",
        )
        .unwrap();
        assert_eq!(
            feature_gates(&dir).get("index_build"),
            Some(&"index".to_string())
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
