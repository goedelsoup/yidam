//! The worked example must pass the gates it is teaching.
//!
//! `examples/streamflow/` exists to answer "what does a good corpus look like" for someone
//! who has no access to a real one — every live derived repository is private or is the
//! divergence canary, and a canary is by definition the corpus that violates things.
//!
//! A teaching example that has drifted is worse than none: it is read as authoritative and
//! copied. So it is gated here rather than by eye. The standard is
//! `derived_repo_smoke`'s and for the same reason — **`graph-check` clean and `lint` empty
//! at every severity**, not merely `gate.passed`, which ignores warn and info. The finding
//! that motivated that rule was info-severity, and a permanently non-empty report is where
//! a real finding gets lost.
//!
//! The corpus is copied to a temp directory and `git init`-ed rather than checked in place.
//! `repo_root()` resolves through `git rev-parse --show-toplevel`, so running the binary
//! inside `examples/streamflow/` finds *this* repository — which has no `.yidam/` and would
//! fail for a reason that has nothing to do with the example.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{repo_root, tracked_under};

struct Example {
    dir: tempfile::TempDir,
}

impl Example {
    /// Materialize `examples/streamflow` as a standalone repository.
    ///
    /// From `git ls-files`, matching how every other suite here builds a tree: a directory
    /// walk would pick up `.DS_Store` and any local scratch, and this test would then be
    /// measuring the maintainer's working directory.
    fn materialize() -> Self {
        let root = repo_root();
        let dir = tempfile::tempdir().unwrap();
        let prefix = "examples/streamflow/";

        let files = tracked_under(&root, prefix);
        assert!(
            !files.is_empty(),
            "no tracked files under {prefix} — the example corpus is missing or unstaged"
        );
        for tracked in &files {
            let rel = tracked.strip_prefix(prefix).unwrap();
            let to = dir.path().join(rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(tracked), &to)
                .unwrap_or_else(|e| panic!("copy {tracked}: {e}"));
        }

        // A real repository: `lint` reads history for `orphan-in` dating, and every path in
        // the corpus resolves from the toplevel.
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "example@yidam.test"],
            vec!["config", "user.name", "Example"],
            vec!["add", "-A"],
            vec![
                "commit",
                "-q",
                "-m",
                "genesis: streamflow on regulated rivers",
            ],
        ] {
            let ok = Command::new("git")
                .args(&args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed");
        }
        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn run(&self, args: &[&str]) -> (String, String, i32) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.path())
            .args(args)
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().unwrap_or(-1),
        )
    }
}

#[test]
fn the_example_corpus_passes_graph_check() {
    let ex = Example::materialize();
    let (stdout, stderr, code) = ex.run(&["graph-check"]);
    assert_eq!(code, 0, "graph-check failed\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("all clean"),
        "graph-check must report a clean graph, not merely exit 0: {stdout}"
    );
}

#[test]
fn the_example_corpus_lints_clean_at_every_severity() {
    let ex = Example::materialize();
    let (stdout, stderr, code) = ex.run(&["lint"]);
    assert_eq!(code, 0, "lint failed\n{stdout}\n{stderr}");
    // Not `gate.passed`: warn and info do not gate, and this example must carry neither.
    assert!(
        stdout.contains("0 finding(s)"),
        "the teaching example must have nothing to report at any severity: {stdout}"
    );
}

/// The example has to be *worth* reading, not only valid — a corpus of four disconnected
/// stubs passes both gates above.
/// `orphan-in` can actually fire here, which it could not before #336.
///
/// The check reads the *whole* ontology to decide which classes are exempt. Reading only a
/// class's own edge list made all three of this corpus's classes derive as source classes —
/// so `orphan-in` was structurally silent across the entire worked example, and the corpus
/// yidam ships to teach people taught nothing about the one check with a residence clock.
///
/// `gage` declares `sources-from → concept, direction: out` and always did. That is a
/// statement that gages point at concepts, so an uncited concept is a finding, and it took
/// reading the far end of the edge to see it.
///
/// Asserted by breaking the corpus rather than by inspecting the derivation: this is about
/// what the gate reports, and a unit test on the rule would have passed throughout the
/// period the gate was silent.
#[test]
fn an_uncited_concept_is_reported_here_at_all() {
    let ex = Example::materialize();
    let target = "low-flow.yml";

    // Cut every inbound link to one concept, leaving the ontology untouched.
    let mut cut = 0;
    for entry in walkdir(&ex.path().join(".yidam/corpus")) {
        let text = std::fs::read_to_string(&entry).unwrap_or_default();
        if !text.contains(target) || entry.ends_with(target) {
            continue;
        }
        let kept: Vec<&str> = {
            let lines: Vec<&str> = text.split('\n').collect();
            let mut out = Vec::new();
            let mut i = 0;
            while i < lines.len() {
                if lines[i].contains(target) && lines[i].trim_start().starts_with("- target:") {
                    i += 1;
                    while i < lines.len()
                        && lines[i].starts_with("    ")
                        && !lines[i].trim_start().starts_with("- ")
                    {
                        i += 1;
                    }
                    cut += 1;
                    continue;
                }
                out.push(lines[i]);
                i += 1;
            }
            out
        };
        std::fs::write(&entry, kept.join("\n")).unwrap();
    }
    assert!(
        cut > 0,
        "no inbound link to {target} to cut — has the example changed?"
    );

    let (stdout, _, _) = ex.run(&["lint", "--warn"]);
    assert!(
        stdout.contains("orphan-in") && stdout.contains(target),
        "an uncited concept is not reported — every class is deriving as a source class \
         again (#336):\n{stdout}"
    );
}

/// Every `.yml` under `dir`, recursively.
fn walkdir(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "yml") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn the_example_corpus_is_substantial_enough_to_teach_from() {
    let ex = Example::materialize();
    let (stdout, _, code) = ex.run(&["graph-check"]);
    assert_eq!(code, 0);

    let root = repo_root();
    let count =
        |dir: &str| tracked_under(&root, &format!("examples/streamflow/.yidam/{dir}")).len();

    assert!(
        count("catalog") >= 1,
        "an example with no catalog entry cannot teach provenance"
    );
    assert!(
        count("decisions") >= 2,
        "decision records are where the ontology's reasoning lives"
    );
    assert!(
        count("skills") >= 1,
        "a corpus with no skill does not show what a skill is for"
    );
    // Three classes and eight instances, per the corpus as authored. The assertion is a
    // floor rather than an equality so the example can grow without a test edit.
    assert!(
        stdout.contains("across 3 classes"),
        "expected the three-class ontology: {stdout}"
    );
}

/// `[open]` is the tag the rest of the apparatus is built around, and an example whose
/// every claim is settled teaches the wrong lesson about what a corpus is for.
#[test]
fn the_example_corpus_has_live_open_questions() {
    let ex = Example::materialize();
    let (stdout, stderr, code) = ex.run(&["open-questions"]);
    assert_eq!(code, 0, "open-questions failed\n{stderr}");
    let listed = stdout.lines().filter(|l| l.starts_with("- [")).count();
    assert!(
        listed >= 3,
        "expected several open questions, found {listed}: {stdout}"
    );
}

/// The quickstart's central loop, run end to end. If this breaks, the document is lying.
///
/// Renaming a node by hand severs every edge into it — the failure `information-architecture`
/// warns about in one line and which nobody believes until they have seen the gate go red.
/// `yidam rename` is the repair, and it is the reason the warning is survivable.
#[test]
fn the_quickstart_break_and_fix_loop_behaves_as_documented() {
    let ex = Example::materialize();

    let (_, _, code) = ex.run(&["graph-check"]);
    assert_eq!(code, 0, "the example must start clean");

    // Break it exactly the way the quickstart says to.
    let corpus = ex.path().join(".yidam/corpus/concept");
    std::fs::rename(
        corpus.join("low-flow.yml"),
        corpus.join("low-flow-statistics.yml"),
    )
    .unwrap();

    let (stdout, _, code) = ex.run(&["graph-check"]);
    assert_ne!(code, 0, "a severed edge must fail the gate: {stdout}");
    assert!(
        stdout.contains("broken link"),
        "the failure must name the broken links: {stdout}"
    );

    // Undo, and do it the supported way.
    std::fs::rename(
        corpus.join("low-flow-statistics.yml"),
        corpus.join("low-flow.yml"),
    )
    .unwrap();
    let (stdout, stderr, code) =
        ex.run(&["rename", "concept/low-flow", "concept/low-flow-statistics"]);
    assert_eq!(code, 0, "rename failed\n{stdout}\n{stderr}");
    assert!(
        stdout.contains("link(s) rewritten"),
        "rename must report the edges it repaired: {stdout}"
    );

    let (stdout, _, code) = ex.run(&["graph-check"]);
    assert_eq!(code, 0, "the gate must pass again after rename: {stdout}");
}

/// The example must not be inherited.
///
/// `yidam clone` copies the template wholesale, excluding only what the caller names. The
/// example is a whole corpus — its own `.yidam/corpus`, catalog, decisions and skills — so
/// without an exclusion a brand-new repository is born holding another domain's nodes,
/// before it has an ontology of its own. Its `graph-check` would then pass on eight
/// instances nobody in that repository wrote.
///
/// End to end through the real command, because the bug this guards against is a one-word
/// edit to an argument list and a test on the list itself would restate it rather than
/// check it.
#[test]
fn clone_does_not_copy_the_example_into_a_derived_repository() {
    let src = repo_root();
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("derived");

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(&src)
        .args(["clone", target.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    assert!(
        !target.join("examples").exists(),
        "examples/ was copied into the derived repository"
    );
    // The paired assertion: `docs` stays out and `sadhana/docs` ships. Naming both here
    // means an exclusion widened to match at every depth — the exact regression the
    // `EXCLUDE_DIRS` comment in copy.rs records — fails this test rather than silently
    // stripping the scaffold the bootstrap skill reads in step 3.
    assert!(
        !target.join("docs").exists(),
        "yidam's own docs/ was copied"
    );
    assert!(
        target.join("sadhana/docs").is_dir(),
        "sadhana/docs/ must ship: the bootstrap skill reads it"
    );
}
