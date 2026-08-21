//! Build the repository bootstrap produces, and run its own gate against it.
//!
//! Almost everything under `sadhana/`, plus `mise.yidam.toml`, has **no consumer inside this
//! repository**. It is copied into a derived repo by an agent following
//! `prelude/skills/bootstrap.md`, and nothing here ever loads the result. That is the
//! property that lets it ship broken: `mise.yidam.toml` sat unparseable on `main` while
//! every job stayed green, and a derived repo taking the update got zero inherited tasks.
//!
//! The other tests over this material check it a piece at a time — the task file parses, the
//! links resolve in the installed layout. This one assembles the whole thing and asks the
//! question those cannot: **does a repository that just came out of bootstrap work?**
//!
//! Three properties, in the order a new repository meets them:
//!
//! 1. mise loads the inherited task layer, and `.yidam/bin` is what answers.
//! 2. `mise run ci` — the gate the derived README tells a new repo to run first — passes.
//! 3. `yidam lint` reports **nothing at any severity**.
//!
//! The third is the strict one and it is deliberate. `gate.passed` is not the assertion:
//! it ignores warn and info, and the defect that motivated this was info-severity. A derived
//! repository reported that `yidam lint` in a single-elector repo could never return a clean
//! report, because the vendored `GRAPH.md` linked a `.yidam/sangha/` that single-elector
//! repos never create. Nothing gated, and that was the complaint — a permanently non-empty
//! report is where a real finding gets lost. Zero-at-every-severity is the only assertion
//! that would have caught it.
//!
//! Materialized from `git ls-files`, matching the production vendor step, which copies out
//! of a `git clone` and so never carries build output. See [`common::tracked_under`].

mod common;

use std::collections::BTreeSet;
use std::path::Path;
use std::process::Command;

use common::{materialize, repo_root};

/// A derived repository, freshly bootstrapped in single-elector mode.
struct Derived {
    dir: tempfile::TempDir,
}

impl Derived {
    fn bootstrap() -> Self {
        let root = repo_root();
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path();

        let files = materialize(&root, target, &BTreeSet::new());
        assert!(
            files > 100,
            "only {files} files materialized — the mapping or the ls-files walk is broken"
        );

        // The provenance pin. `yidam clone`/`overlay` write it; bootstrap requires it to
        // exist and carry a real commit before the vendor step is considered done.
        let commit = String::from_utf8(
            Command::new("git")
                .current_dir(&root)
                .args(["rev-parse", "HEAD"])
                .output()
                .expect("git rev-parse")
                .stdout,
        )
        .unwrap();
        std::fs::write(
            target.join(".yidam.toml"),
            format!(
                "[yidam]\norigin = \"git@github.com:goedelsoup/yidam.git\"\n\
                 commit = \"{}\"\ntemplate = \"untagged\"\ncommitted = \"2026-01-01\"\n",
                commit.trim()
            ),
        )
        .unwrap();

        // The binary this repository pins. Installing it here is not scaffolding — it is
        // what makes property 1 meaningful: `mise run graph-check` has to resolve THIS
        // binary and not whichever `yidam` the machine has lying around.
        let bin = target.join(".yidam/bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::copy(env!("CARGO_BIN_EXE_yidam"), bin.join("yidam")).expect("install the pin");

        git(target, &["init", "-q", "-b", "main"]);
        git(target, &["config", "user.email", "bootstrap@yidam.test"]);
        git(target, &["config", "user.name", "Bootstrap"]);
        git(target, &["add", "-A"]);
        git(target, &["commit", "-qm", "genesis: smoke"]);

        Self { dir }
    }

    fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Run a mise command in the derived repository.
    ///
    /// `MISE_TRUSTED_CONFIG_PATHS` rather than `mise trust`: the latter writes to the user's
    /// own trust store, and a test that mutates the machine it runs on is one nobody wants
    /// to run twice.
    fn mise(&self, args: &[&str]) -> std::process::Output {
        Command::new("mise")
            .current_dir(self.path())
            .env("MISE_TRUSTED_CONFIG_PATHS", self.path())
            .env("MISE_YES", "1")
            .args(args)
            .output()
            .expect(
                "running mise — this test loads the inherited task layer the way a derived \
                 repository does, so mise must be on PATH. `mise run ci-cli` provides it.",
            )
    }

    /// Run the repository's own pinned binary.
    fn yidam(&self, args: &[&str]) -> std::process::Output {
        Command::new(self.path().join(".yidam/bin/yidam"))
            .current_dir(self.path())
            .args(args)
            .output()
            .expect("running the pinned binary")
    }
}

fn git(dir: &Path, args: &[&str]) {
    let ok = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git")
        .success();
    assert!(ok, "git {args:?} failed");
}

fn text(out: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

/// mise loads the inherited layer, and the pinned binary is the one that answers.
///
/// Both halves failed at once when `mise.yidam.toml` did not parse: no tasks, and the
/// `[env] _.path` that was supposed to put `.yidam/bin` first was in the file that could
/// not hold it.
#[test]
fn the_inherited_tasks_load_and_the_pinned_binary_answers() {
    let repo = Derived::bootstrap();

    let out = repo.mise(&["tasks", "--no-header"]);
    let listed = text(&out);
    assert!(
        out.status.success(),
        "mise could not load the task layer:\n{listed}"
    );
    for task in [
        "graph-check",
        "regen",
        "yidam-build",
        "yidam-vendor-update",
        "rename",
        "neighbors",
    ] {
        assert!(
            listed
                .lines()
                .any(|l| l.split_whitespace().next() == Some(task)),
            "`{task}` is not among the tasks a derived repository loads:\n{listed}"
        );
    }

    // The PATH guarantee, resolved rather than asserted from the config file.
    let which = repo.mise(&["exec", "--", "sh", "-c", "command -v yidam"]);
    let resolved = text(&which);
    assert!(
        resolved.contains(".yidam/bin/yidam"),
        "mise must resolve the repository's pinned binary first, got:\n{resolved}"
    );
}

/// The gate a new repository is told to run must work on the day it is created.
///
/// `mise run ci` failed at genesis: its crate steps ran `cargo --manifest-path
/// crates/Cargo.toml` against a file that does not exist yet, which is a usage error. The
/// comment above those tasks had claimed they no-op until `crates/` appears for as long as
/// they did not.
#[test]
fn the_derived_repos_own_gate_passes_at_genesis() {
    let repo = Derived::bootstrap();
    let out = repo.mise(&["run", "ci"]);
    assert!(
        out.status.success(),
        "`mise run ci` fails in a freshly bootstrapped repository:\n{}",
        text(&out)
    );
}

/// A fresh derived repository lints clean — at *every* severity, not just the gating one.
///
/// This is the assertion the reported defect needed. It was info-severity, so every gate in
/// the repository passed while `yidam lint` could not return an empty report.
#[test]
fn a_fresh_derived_repository_lints_completely_clean() {
    let repo = Derived::bootstrap();
    let out = repo.yidam(&["lint", "--format", "json"]);
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    let report: serde_json::Value = serde_json::from_str(&stdout)
        .unwrap_or_else(|e| panic!("lint --format json: {e}\n{}", text(&out)));

    let mut findings = Vec::new();
    for check in report["checks"].as_array().expect("checks") {
        for v in check["violations"].as_array().expect("violations") {
            findings.push(format!(
                "  {:5} {}: {} — {}",
                check["severity"].as_str().unwrap_or("?"),
                check["id"].as_str().unwrap_or("?"),
                v["node"].as_str().unwrap_or("?"),
                v["detail"].as_str().unwrap_or("?")
            ));
        }
    }
    assert!(
        findings.is_empty(),
        "a repository that has just been bootstrapped reports {} finding(s):\n{}\n\n\
         Every one of these is addressed to somebody who has written nothing yet, and \
         several cannot be acted on at all — material under `.yidam/.vendor/` is fixed \
         upstream. A report that is never empty is one whose next real finding gets lost.",
        findings.len(),
        findings.join("\n")
    );
}

/// The corpus gate answers from the genesis commit onward.
///
/// `graph-check` is the one job the derived CI runs unconditionally, on the argument that it
/// works before a single crate exists. That has to be true of an empty corpus too.
#[test]
fn the_corpus_gate_answers_on_an_empty_corpus() {
    let repo = Derived::bootstrap();
    let out = repo.mise(&["run", "graph-check"]);
    assert!(
        out.status.success(),
        "`mise run graph-check` must succeed on a corpus with no nodes yet:\n{}",
        text(&out)
    );
}

/// What bootstrap installs is what a derived repo has — no build output, no template leftovers.
///
/// A directory walk of the working tree carries `target/`, `__pycache__` and `.pytest_cache`
/// from the prelude's parity SDKs: ~38,000 files against the ~500 a derived repository
/// actually receives. The production vendor step copies out of a `git clone` and never sees
/// them, so a test that walks the tree measures the maintainer's build cache.
#[test]
fn the_materialized_repository_carries_no_build_output() {
    let repo = Derived::bootstrap();
    let mut junk = Vec::new();
    for entry in walkdir::WalkDir::new(repo.path())
        .into_iter()
        .filter_map(Result::ok)
    {
        let p = entry.path().to_string_lossy().to_string();
        if p.contains("/.git/") {
            continue;
        }
        if [
            "/target/",
            "/__pycache__/",
            "/.pytest_cache/",
            "/node_modules/",
            "/.venv/",
        ]
        .iter()
        .any(|bad| p.contains(bad))
            || p.ends_with("/.DS_Store")
        {
            junk.push(p);
        }
    }
    assert!(
        junk.is_empty(),
        "{} build artifact(s) reached the derived repository, e.g. {}",
        junk.len(),
        junk[0]
    );
}

/// Single-elector is the default, and it installs no sangha.
///
/// The conditional row exists so that the layout under test is the one most derived repos
/// get. If materialization ignored the condition, every assertion above would be made
/// against a collective repository nobody asked for.
#[test]
fn the_default_bootstrap_installs_no_sangha() {
    let repo = Derived::bootstrap();
    assert!(
        !repo.path().join(".yidam/sangha").exists(),
        "single-elector is the documented default and creates no .yidam/sangha/"
    );
    // …and the prelude must not link into it. This is the defect, from the other side.
    let graph = std::fs::read_to_string(repo.path().join(".yidam/.vendor/prelude/GRAPH.md"))
        .expect("the vendored GRAPH.md");
    assert!(
        !graph.contains("](../../sangha/README.md)"),
        "GRAPH.md links a directory this repository does not have"
    );
}
