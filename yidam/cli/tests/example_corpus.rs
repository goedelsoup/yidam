//! Every worked example must pass the gates it is teaching.
//!
//! `examples/` answers "what does a good corpus look like" for someone who has no access to
//! a real one — every live derived repository is private or is the divergence canary, and a
//! canary is by definition the corpus that violates things.
//!
//! A teaching example that has drifted is worse than none: it is read as authoritative and
//! copied. So they are gated here rather than by eye. The standard is `derived_repo_smoke`'s
//! and for the same reason — **`graph-check` clean and `lint` empty at every severity**, not
//! merely `gate.passed`, which ignores warn and info. The finding that motivated that rule
//! was info-severity, and a permanently non-empty report is where a real finding gets lost.
//!
//! # Discovered, not listed
//!
//! Every check below runs over [`examples`], which reads the set from `git ls-files`. This
//! file used to name `examples/streamflow/` in four places, and a second example added
//! beside it would have been **ungated and green** — `graph-check` never running against it,
//! `lint` never running against it, and nothing failing to say so (#448). That is the
//! guard-list shape: a hardcoded name stops covering new cases without ever going red.
//!
//! Discovery has its own failure, one level up: a predicate that matches nothing passes
//! everything. [`every_example_on_disk_is_discovered`] is what closes it, and it is the test
//! to fix first if this suite ever goes quiet.
//!
//! Two tests here are **deliberately still pinned to streamflow**, and say so at their own
//! definitions. Neither is an example check: one is a regression test for #336 that asserts
//! by mutating a corpus whose exact edge shape it knows, and the other runs the quickstart's
//! own instructions, which name that corpus by name.
//!
//! Each corpus is copied to a temp directory and `git init`-ed rather than checked in place.
//! `repo_root()` resolves through `git rev-parse --show-toplevel`, so running the binary
//! inside `examples/<name>/` finds *this* repository — which has no `.yidam/` and would fail
//! for a reason that has nothing to do with the example.

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use common::{examples, examples_on_disk, repo_root, tracked_under};

/// The example this repository ships to teach from, and the one two tests below know the
/// internals of. Named once, so the pinned tests are greppable and the loops are not.
const STREAMFLOW: &str = "streamflow";

/// The manifest naming the order an example was written in, if it ships one.
const HISTORY: &str = "history.toml";

/// One commit in that order: what it says, when, and which path prefixes it introduces.
#[derive(serde::Deserialize)]
struct HistoryCommit {
    message: String,
    date: Option<String>,
    paths: Vec<String>,
}

#[derive(serde::Deserialize)]
struct History {
    commit: Vec<HistoryCommit>,
}

/// The order `examples/<name>` was written in, or `None` where it does not say.
///
/// `yidam replay` reconstructs corpus health at every commit that touched the corpus, so a
/// corpus materialised with one genesis commit gives it nothing to reconstruct, and a
/// walkthrough's replay section would be a description of the feature rather than a run of it
/// (#452). An example that ships a manifest gets that history; every other example gets the
/// single genesis commit it always had, which is why this returns an option rather than a
/// default.
fn history(root: &Path, name: &str) -> Option<Vec<HistoryCommit>> {
    let p = root.join(format!("examples/{name}/{HISTORY}"));
    let text = std::fs::read_to_string(&p).ok()?;
    let parsed: History = toml::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not a usable history manifest: {e}", p.display()));
    assert!(
        !parsed.commit.is_empty(),
        "{} declares no commits; delete it rather than shipping an empty history",
        p.display()
    );
    Some(parsed.commit)
}

fn git(dir: &Path, args: &[&str], name: &str) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed for {name}");
}

/// Build the repository one commit at a time, in the order the manifest gives.
///
/// Paths are **prefixes**, so a commit names a directory or a file and picks up whatever is
/// under it. A commit that stages nothing is a stale manifest entry and fails here rather than
/// producing an empty commit nobody would notice — that is the failure mode this whole file
/// exists to prevent, one layer down.
///
/// Whatever the manifest does not name is swept into a final commit rather than dropped: a
/// file added to an example without a thought about its history must still reach the corpus,
/// or the gate would be checking a subset of what ships and saying nothing.
fn replay_history(dir: &Path, name: &str, commits: &[HistoryCommit], copied: &[String]) {
    for c in commits {
        let mut staged = false;
        for rel in copied {
            if c.paths
                .iter()
                .any(|p| rel == p || rel.starts_with(&format!("{p}/")))
            {
                git(dir, &["add", "--", rel], name);
                staged = true;
            }
        }
        assert!(
            staged,
            "{name}: history entry {:?} names no file that exists — the manifest has drifted \
             from the corpus",
            c.message
        );
        // Already-committed files stage as no-ops, so an entry can still be empty here.
        let nothing_new = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(
            !nothing_new,
            "{name}: history entry {:?} adds nothing not already committed",
            c.message
        );

        let mut cmd = Command::new("git");
        cmd.args(["commit", "-q", "-m", &c.message])
            .current_dir(dir);
        if let Some(date) = &c.date {
            // Both, because a reader comparing `replay` to this manifest is comparing dates,
            // and git takes the two from different places.
            cmd.arg(format!("--date={date}"));
            cmd.env("GIT_COMMITTER_DATE", date);
        }
        assert!(
            cmd.status().unwrap().success(),
            "{name}: commit {:?} failed",
            c.message
        );
    }

    git(dir, &["add", "-A"], name);
    let nothing_left = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    if !nothing_left {
        git(
            dir,
            &[
                "commit",
                "-q",
                "-m",
                "establish: the remainder of the corpus",
            ],
            name,
        );
    }
}

/// How many classes an example declares, counted from the corpus rather than asserted.
///
/// A class is a `<name>.ont.yml` directly under `.yidam/corpus/`. This replaces a literal
/// `across 3 classes`: what is worth checking is that the number `graph-check` reports and
/// the number the corpus was authored with are the same, and that claim holds for an example
/// nobody has written yet.
fn declared_classes(example: &str) -> usize {
    let prefix = format!("examples/{example}/.yidam/corpus/");
    tracked_under(&repo_root(), &prefix)
        .iter()
        .filter_map(|p| p.strip_prefix(&prefix))
        .filter(|rest| !rest.contains('/') && rest.ends_with(".ont.yml"))
        .count()
}

struct Example {
    dir: tempfile::TempDir,
}

impl Example {
    /// Materialize `examples/<name>` as a standalone repository.
    ///
    /// From `git ls-files`, matching how every other suite here builds a tree: a directory
    /// walk would pick up `.DS_Store` and any local scratch, and this test would then be
    /// measuring the maintainer's working directory.
    fn materialize(name: &str) -> Self {
        let root = repo_root();
        let dir = tempfile::tempdir().unwrap();
        let prefix = format!("examples/{name}/");

        let files = tracked_under(&root, &prefix);
        assert!(
            !files.is_empty(),
            "no tracked files under {prefix} — `{name}` is missing or unstaged. An example \
             directory that git does not know about is invisible to every check here"
        );
        let mut copied = Vec::new();
        for tracked in &files {
            let rel = tracked.strip_prefix(&prefix).unwrap();
            // The manifest describes the repository being built; it is not part of it.
            if rel == HISTORY {
                continue;
            }
            let to = dir.path().join(rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(tracked), &to)
                .unwrap_or_else(|e| panic!("copy {tracked}: {e}"));
            copied.push(rel.to_string());
        }

        // A real repository: `lint` reads history for `orphan-in` dating, and every path in
        // the corpus resolves from the toplevel.
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "example@yidam.test"],
            vec!["config", "user.name", "Example"],
        ] {
            let ok = Command::new("git")
                .args(&args)
                .current_dir(dir.path())
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed for {name}");
        }

        match history(&root, name) {
            Some(commits) => replay_history(dir.path(), name, &commits, &copied),
            None => {
                let genesis = format!("genesis: the {name} example");
                git(dir.path(), &["add", "-A"], name);
                git(dir.path(), &["commit", "-q", "-m", &genesis], name);
            }
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

/// **Fix this one first if the suite goes quiet.**
///
/// Every other check here iterates [`examples`], so a discovery predicate that matches
/// nothing makes all of them pass while checking nothing — the same silent hole one level up
/// from the hardcoded `examples/streamflow/` they replaced.
///
/// Non-emptiness alone would not catch it: the set could still be missing an example. So the
/// git-derived set is compared against a filesystem listing, which is the *other* way this
/// question can be answered. A disagreement is real either way round — an untracked example
/// directory is one git-based discovery silently skips, and a tracked path with no directory
/// means the working tree is not what the test believes it is.
#[test]
fn every_example_on_disk_is_discovered() {
    let discovered = examples();
    assert!(
        !discovered.is_empty(),
        "no examples discovered under examples/ — every check in this file is now vacuous"
    );
    assert_eq!(
        discovered,
        examples_on_disk(),
        "git and the filesystem disagree about what the examples are. A directory holding a \
         `.yidam/` that git does not track is ungated: nothing here runs against it, and \
         nothing else would have said so"
    );
}

#[test]
fn every_example_passes_graph_check() {
    for name in examples() {
        let ex = Example::materialize(&name);
        let (stdout, stderr, code) = ex.run(&["graph-check"]);
        assert_eq!(code, 0, "graph-check failed for {name}\n{stdout}\n{stderr}");
        assert!(
            stdout.contains("all clean"),
            "graph-check must report a clean graph for {name}, not merely exit 0: {stdout}"
        );
    }
}

#[test]
fn every_example_lints_clean_at_every_severity() {
    for name in examples() {
        let ex = Example::materialize(&name);
        let (stdout, stderr, code) = ex.run(&["lint"]);
        assert_eq!(code, 0, "lint failed for {name}\n{stdout}\n{stderr}");
        // Not `gate.passed`: warn and info do not gate, and an example must carry neither.
        assert!(
            stdout.contains("0 finding(s)"),
            "{name} must have nothing to report at any severity: {stdout}"
        );
    }
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
///
/// **Deliberately pinned to streamflow, and not generalised over [`examples`] (#448).** It
/// is not an example check. It is a regression test for #336 that works by cutting a
/// specific edge in a corpus whose exact shape it knows — `gage` declares
/// `sources-from → concept, direction: out`, which is what makes an uncited concept a
/// finding at all. Run against an example with a different ontology it would cut nothing,
/// hit the `cut > 0` assertion, and read as a broken test rather than as what it is: a
/// question that corpus cannot answer.
#[test]
fn an_uncited_concept_is_reported_here_at_all() {
    let ex = Example::materialize(STREAMFLOW);
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

/// An example has to be *worth* reading, not only valid — a corpus of four disconnected
/// stubs passes both gates above.
#[test]
fn every_example_is_substantial_enough_to_teach_from() {
    let root = repo_root();
    for name in examples() {
        let ex = Example::materialize(&name);
        let (stdout, _, code) = ex.run(&["graph-check"]);
        assert_eq!(code, 0, "graph-check failed for {name}");

        let count =
            |dir: &str| tracked_under(&root, &format!("examples/{name}/.yidam/{dir}")).len();

        assert!(
            count("catalog") >= 1,
            "{name} has no catalog entry, so it cannot teach provenance"
        );
        assert!(
            count("decisions") >= 2,
            "{name} has fewer than two decision records, and decision records are where \
             the ontology's reasoning lives"
        );
        assert!(
            count("skills") >= 1,
            "{name} has no skill, so it does not show what a skill is for"
        );

        // Read from the corpus, not written down here. The literal this replaces was
        // `across 3 classes`, which is a fact about streamflow rather than about examples,
        // and which a second example would have had to satisfy by coincidence.
        let declared = declared_classes(&name);
        assert!(
            declared >= 2,
            "{name} declares {declared} class(es); a single-class corpus has no edges \
             between classes and cannot show what the ontology is for"
        );
        assert!(
            stdout.contains(&format!("across {declared} classes")),
            "{name} ships {declared} `*.ont.yml` files and graph-check reports something \
             else — a class is declared and unread, or read and undeclared: {stdout}"
        );
    }
}

/// `[open]` is the tag the rest of the apparatus is built around, and an example whose
/// every claim is settled teaches the wrong lesson about what a corpus is for.
///
/// The bar is *any*, where streamflow's was three. Three was a fact about how much of
/// streamflow happens to be unsettled, and holding an example nobody has written yet to it
/// would be the constant this file is removing, one level down. What is worth requiring is
/// that the habit is demonstrated at all.
#[test]
fn every_example_has_live_open_questions() {
    for name in examples() {
        let ex = Example::materialize(&name);
        let (stdout, stderr, code) = ex.run(&["open-questions"]);
        assert_eq!(code, 0, "open-questions failed for {name}\n{stderr}");
        let listed = stdout.lines().filter(|l| l.starts_with("- [")).count();
        assert!(
            listed >= 1,
            "{name} has no open questions; a corpus where every claim is settled teaches \
             the wrong lesson about what one is for: {stdout}"
        );
    }
}

/// The quickstart's central loop, run end to end. If this breaks, the document is lying.
///
/// Renaming a node by hand severs every edge into it — the failure `information-architecture`
/// warns about in one line and which nobody believes until they have seen the gate go red.
/// `yidam rename` is the repair, and it is the reason the warning is survivable.
///
/// **Deliberately pinned to streamflow, and not generalised over [`examples`] (#448).** This
/// checks a *document*, not an example: `docs/quickstart.md` names `concept/low-flow` and
/// tells the reader to rename it. Running the same steps against another corpus would assert
/// nothing about the page, and the page is the thing that can be wrong.
#[test]
fn the_quickstart_break_and_fix_loop_behaves_as_documented() {
    let ex = Example::materialize(STREAMFLOW);

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
