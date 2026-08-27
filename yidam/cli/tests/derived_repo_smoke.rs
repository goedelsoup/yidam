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
    /// A repository that has completed the whole protocol, step 8.5 included.
    fn bootstrap() -> Self {
        Self::build(true)
    }

    /// A repository stopped at the `vendor:` commit — everything written, nothing checked.
    ///
    /// This is what bootstrap produced for as long as it had no step 8.5, and it is the
    /// fixture [`the_scaffold_is_stale_on_arrival`] needs in order to show that the step is
    /// load-bearing rather than ceremonial.
    fn before_the_gate() -> Self {
        Self::build(false)
    }

    fn build(run_the_gate: bool) -> Self {
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

        // Step 8.5. Every REGEN marker the scaffold ships is stale on arrival — generated
        // from a corpus that did not exist when the template was written — so a repository
        // that skips this is one whose first push fails on content nobody wrote.
        if run_the_gate {
            let out = Command::new(target.join(".yidam/bin/yidam"))
                .current_dir(target)
                .arg("regen")
                .output()
                .expect("yidam regen");
            assert!(
                out.status.success(),
                "step 8.5's `yidam regen` failed:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            );
            git(target, &["add", "-A"]);
            git(
                target,
                &[
                    "commit",
                    "-qm",
                    "regen: blocks populated on the first run of the gate",
                ],
            );
        }

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

/// Every rooted rule in the derived `.gitignore` names a directory this repository has.
///
/// `.gitignore` shipped as yidam's own for as long as bootstrap called it generic, and it is
/// not. Yidam's ignores `yidam/tests/results/**/transcript.jsonl` — fifteen lines about the
/// bootstrap test harness — and the vendor step three paragraphs above the sentence calling
/// the file generic deletes `yidam/` entirely. The rule outlived the directory it named by
/// the length of the repository's life, and nothing could see it: a pattern matching nothing
/// ignores nothing and fails no gate, forever.
///
/// The check is the first path segment, and only for **rooted** patterns — ones naming a
/// directory component, like `.yidam/bin/`. A bare `target/` or `node_modules/` matches at
/// any depth and asserts nothing about layout, so it is not evidence of anything and is
/// skipped. What remains is exactly the class of rule that can name a directory that does
/// not exist.
///
/// Discovered from the file rather than listed here. A rule added tomorrow is checked
/// tomorrow; a list would have to be remembered, which is how the last one rotted.
#[test]
fn the_derived_gitignore_names_no_directory_this_repository_lacks() {
    let repo = Derived::bootstrap();
    let text = std::fs::read_to_string(repo.path().join(".gitignore")).expect("the .gitignore");

    let mut checked = 0;
    let mut absent = Vec::new();
    for line in text.lines() {
        let rule = line.trim();
        if rule.is_empty() || rule.starts_with('#') {
            continue;
        }
        let rule = rule.strip_prefix('!').unwrap_or(rule);
        // Rooted: names a directory component. `a/b` and `a/b/` qualify; `a/` does not.
        let Some((head, _)) = rule.trim_end_matches('/').split_once('/') else {
            continue;
        };
        // A glob in the first segment names no particular directory.
        if head.contains('*') || head.contains('?') || head.is_empty() {
            continue;
        }
        checked += 1;
        if !repo.path().join(head).exists() {
            absent.push(format!("  {rule}  (no `{head}/` in a derived repository)"));
        }
    }

    assert!(
        checked >= 2,
        "only {checked} rooted rule(s) found — the parse is broken, not the file"
    );
    assert!(
        absent.is_empty(),
        "the .gitignore a derived repository receives has {} rule(s) for paths it does not \
         have:\n{}\n\nA rule that matches nothing is not harmless: it is confident prose \
         about a layout this repository never had, in the file bootstrap hands over as its \
         own. Fix it in `sadhana/root/gitignore`, not in yidam's.",
        absent.len(),
        absent.join("\n")
    );
}

/// `git add -A` after `mise run yidam-build` stages no cargo bookkeeping.
///
/// `yidam-build` runs `cargo install --force --root "$PWD/.yidam"`, and
/// `cargo install --root DIR` writes three things: `DIR/bin/<binary>`, `DIR/.crates.toml`,
/// and `DIR/.crates2.json`. The inherited `.gitignore` covered one of the three. Both
/// bootstrap and `PROTOCOL.md` prescribe `git add -A`, so the window between the first build
/// and the next prescribed commit was one command wide — and nothing in it would look wrong:
/// two dotfiles under a directory already full of infrastructure.
///
/// `.yidam/` is not an ordinary build directory. It is the corpus root. Machine state
/// committed there is in every clone forever.
///
/// Asserted through git rather than by matching strings in the ignore file, because the
/// question is what git does with the rule and not what the rule says. The second half is
/// the over-reach check: a rule broad enough to swallow `.yidam/` wholesale would pass the
/// first assertion and take the corpus with it.
#[test]
fn a_prescribed_git_add_stages_no_cargo_install_bookkeeping() {
    let repo = Derived::bootstrap();
    let root = repo.path();

    // What `cargo install --root .yidam` leaves behind, plus a corpus node to prove the
    // rule is not simply ignoring the directory.
    std::fs::create_dir_all(root.join(".yidam/bin")).unwrap();
    std::fs::write(root.join(".yidam/.crates.toml"), "[v1]\n").unwrap();
    std::fs::write(root.join(".yidam/.crates2.json"), "{}\n").unwrap();
    std::fs::write(root.join(".yidam/bin/yidam"), "").unwrap();
    std::fs::create_dir_all(root.join(".yidam/corpus/thing")).unwrap();
    std::fs::write(root.join(".yidam/corpus/thing/one.yml"), "class: thing\n").unwrap();

    git(root, &["add", "-A"]);
    let staged = Command::new("git")
        .current_dir(root)
        .args(["diff", "--cached", "--name-only"])
        .output()
        .expect("git diff --cached");
    let staged = String::from_utf8_lossy(&staged.stdout).to_string();
    let staged: Vec<&str> = staged.lines().collect();

    let machine: Vec<&&str> = staged
        .iter()
        .filter(|p| p.starts_with(".yidam/.crates") || p.starts_with(".yidam/bin/"))
        .collect();
    assert!(
        machine.is_empty(),
        "the command bootstrap and PROTOCOL.md both prescribe staged cargo's install \
         bookkeeping into the corpus root: {machine:?}"
    );
    assert!(
        staged.contains(&".yidam/corpus/thing/one.yml"),
        "the ignore rule is too broad — it took a corpus node with it. Staged: {staged:?}"
    );
}

/// Step 8.5 is load-bearing, and the skill still prescribes it.
///
/// The bootstrap protocol used to end at the `vendor:` commit. It installed a CI workflow
/// that runs `yidam regen --check`, a scaffold carrying REGEN markers in seven files, and a
/// CLI that could refresh them — and ran none of it. The templates' markers are generated
/// from a corpus that did not exist when the template was written, so they are stale from
/// the moment they are copied: a repository with no nodes at all reports ten stale blocks.
/// Every derived repository's first push therefore failed on generated content nobody wrote.
///
/// Two halves, and both are needed. That a repository stopped before the gate *fails* it is
/// what makes the step necessary rather than ceremonial — if this ever passes, the staleness
/// was fixed somewhere better and step 8.5 can be reconsidered. That `bootstrap.md` still
/// names the command is what makes it happen. A step that is necessary and absent is the
/// state this test exists to prevent returning to.
#[test]
fn the_scaffold_is_stale_on_arrival_and_bootstrap_says_to_fix_it() {
    let stopped = Derived::before_the_gate();
    let out = stopped.yidam(&["regen", "--check"]);
    assert!(
        !out.status.success(),
        "a repository stopped at the `vendor:` commit passes `regen --check`.\n\
         If the scaffold no longer ships stale blocks, that is a better fix than step 8.5 \
         and this test should be retired deliberately rather than deleted:\n{}",
        text(&out)
    );

    let skill = std::fs::read_to_string(repo_root().join("yidam/prelude/skills/bootstrap.md"))
        .expect("the bootstrap skill");
    assert!(
        skill.contains("yidam regen"),
        "a repository that stops before the gate fails it, and bootstrap.md no longer tells \
         the agent to run `yidam regen`. Every derived repository's first push will fail on \
         generated content nobody wrote."
    );
    assert!(
        skill.contains("mise run yidam-build"),
        "bootstrap.md prescribes running the gate but never installs the binary that runs it"
    );
}

/// The local gate runs every yidam gate CI gates on.
///
/// `mise run ci` is what the derived README tells a new repository to run first, and
/// `.github/workflows/ci.yml`'s corpus job is what decides whether the push is green. They
/// were not the same set: the local gate ran `graph-check` and CI ran `graph-check`, `lint`
/// and `regen --check`. A repository could therefore pass its own gate and fail its first
/// build, on a check it had no local way to run.
///
/// Compared by **subcommand**, not by command line. CI runs `lint --commits --range
/// origin/main..HEAD`, which needs a remote-tracking ref that a repository seven commits old
/// working locally may not have; requiring the flags to match would force the local gate to
/// carry an argument that cannot work there. What has to travel is the check itself.
///
/// Both sides are read out of the files. A list here would be a third thing to keep in step,
/// which is the failure this is guarding against in the first place.
#[test]
fn the_local_gate_runs_what_ci_gates_on() {
    let root = repo_root();
    let workflow = std::fs::read_to_string(root.join("sadhana/github/workflows/ci.yml")).unwrap();
    let mise_root = std::fs::read_to_string(root.join("sadhana/root/mise.toml")).unwrap();
    let mise_layer = std::fs::read_to_string(root.join("mise.yidam.toml")).unwrap();

    /// The subcommand of a `yidam <cmd> …` invocation on a line, ignoring comments.
    fn yidam_subcommand(line: &str) -> Option<String> {
        let line = line.split('#').next()?.trim();
        let at = line.find("yidam ")?;
        // `.yidam/bin/yidam` and `.yidam.toml` are paths, not invocations.
        if line[..at].ends_with(['/', '.']) {
            return None;
        }
        let word = line[at + "yidam ".len()..]
            .split_whitespace()
            .next()?
            .trim_matches(|c: char| c == '"' || c == '\'');
        // Flags and paths are not subcommands; `--version` asks what answered, not a gate.
        if word.is_empty() || word.starts_with('-') || word.contains('/') || word.contains('.') {
            return None;
        }
        Some(word.to_string())
    }

    let ci_gates: BTreeSet<String> = workflow
        .lines()
        .filter(|l| l.trim_start().starts_with("run: ") || l.trim_start().starts_with("- run: "))
        .filter_map(yidam_subcommand)
        .collect();

    // What `mise run ci` reaches: its own body, plus the `run` of every task it delegates to.
    //
    // Anchored to whole lines. `mise.yidam.toml` explains its own naming in a comment that
    // contains the literal `[graph-check]`, so a substring search finds the prose 19k
    // characters before the task and reads a paragraph as a task body — which is how this
    // first reported the local gate as missing a check it runs.
    fn task_body<'a>(toml: &'a str, name: &str) -> Option<Vec<&'a str>> {
        let head = format!("[{name}]");
        let alt = format!("[tasks.{name}]");
        let mut lines = toml.lines().skip_while(|l| {
            let t = l.trim();
            t != head && t != alt
        });
        lines.next()?;
        Some(
            lines
                .take_while(|l| !l.trim_start().starts_with('['))
                .collect(),
        )
    }

    let ci_task = task_body(&mise_root, "ci").expect("sadhana/root/mise.toml [tasks.ci]");
    let mut local: BTreeSet<String> = ci_task
        .iter()
        .copied()
        .filter_map(yidam_subcommand)
        .collect();
    for line in &ci_task {
        let Some(rest) = line.trim().strip_prefix("mise run ") else {
            continue;
        };
        let name = rest.split_whitespace().next().unwrap_or("");
        for src in [&mise_layer, &mise_root] {
            if let Some(body) = task_body(src, name) {
                local.extend(body.into_iter().filter_map(yidam_subcommand));
            }
        }
    }

    assert!(
        ci_gates.len() >= 3,
        "found only {} yidam gate(s) in the derived ci.yml — the parse is broken, not the \
         workflow: {ci_gates:?}",
        ci_gates.len()
    );
    let missing: Vec<&String> = ci_gates.difference(&local).collect();
    assert!(
        missing.is_empty(),
        "CI's corpus job gates on {missing:?}, and `mise run ci` never runs {}. A repository \
         that passes the gate its README names and fails its first push learns the \
         difference from a red build.\n  ci.yml: {ci_gates:?}\n  mise run ci: {local:?}",
        if missing.len() == 1 { "it" } else { "them" }
    );
}
