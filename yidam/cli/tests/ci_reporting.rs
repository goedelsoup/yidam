//! The gates report, and they report the same way.
//!
//! #462's finding was that 1,399 tests reported as a colored dot: `$GITHUB_STEP_SUMMARY`
//! appeared zero times in 1,527 lines of workflow, `cargo test` emitted nothing any consumer
//! could read, and four suites that skip on an absent environment printed an honest reason
//! into scrollback nobody opens.
//!
//! The fix has three moving parts and each fails silently on its own:
//!
//! - **A profile that is not applied.** `.config/nextest.toml`'s `ci` profile is what turns
//!   on JUnit output. Run under any other profile and the gate passes, writes nothing, and
//!   the summary step has nothing to render.
//! - **A runner that stops running a class of test.** nextest does not run doctests. A swap
//!   that dropped them would look exactly like a swap that did not.
//! - **A skip that announces itself in its own words.** The census matches one marker. A
//!   second spelling is a skip nothing counts — the original defect, one level down.
//!
//! Both sides are discovered throughout: the tasks come from `mise.toml`, the gates from
//! `.github/workflows/ci.yml`, and the skip sites from walking the test trees. A roster in
//! this file would stop covering whatever was added next without ever going red.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Text with comments removed, so prose about the code cannot answer for the code.
///
/// Learned in #461, where a guard scanning `ci.yml` for `mise run verify` stayed green with
/// the step deleted: the job's own comment said the words while explaining why the step was
/// there. `//` for Rust, `#` for TOML and YAML — the three languages this file reads.
fn code_only(text: &str, line_comment: &str) -> String {
    text.lines()
        .map(|line| match line.split_once(line_comment) {
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => before,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn mise_toml() -> String {
    code_only(&read("mise.toml"), "#")
}

fn ci_yml() -> String {
    code_only(&read(".github/workflows/ci.yml"), "#")
}

/// Every `cargo nextest run …` command line in `mise.toml`.
fn nextest_lines() -> Vec<String> {
    let text = mise_toml();
    let found: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo nextest run"))
        .map(|l| {
            l.trim_matches(|c| c == '"' || c == ',' || c == ' ')
                .to_string()
        })
        .collect();
    assert!(
        !found.is_empty(),
        "no `cargo nextest run` in mise.toml. Either the gates went back to `cargo test`, \
         which writes no JUnit and leaves every summary step with nothing to render, or this \
         parser is reading the wrong thing — and both make every assertion below vacuous."
    );
    found
}

/// The profile is applied, on every invocation.
///
/// nextest resolves `.config/nextest.toml` relative to a *workspace* root, and these are
/// three separate workspaces under one repository. Without `--config-file` each gate silently
/// falls back to the default profile: no JUnit, no captured output, and a green check whose
/// summary step then fails for a reason that has nothing to do with the tests.
#[test]
fn every_gate_runs_under_the_config_that_turns_on_junit() {
    for line in nextest_lines() {
        assert!(
            line.contains("--config-file .config/nextest.toml"),
            "this gate does not name the shared nextest config, so it runs under the default \
             profile and writes no JUnit:\n  {line}"
        );
    }
}

/// The config that profile lives in is committed, not merely present.
///
/// It very nearly was not. `.config/` is a common entry in a *global* gitignore — the machine
/// this was written on had `/.config` in one — which hides the file from `git status`, so it
/// is never added, and CI then runs under the default profile: green, silent, and no JUnit.
/// The repository's own `.gitignore` carries a negation for it, and this asserts the outcome
/// rather than the negation, because it is the outcome the local machine cannot see.
#[test]
fn the_nextest_profile_is_tracked_in_git() {
    let out = Command::new("git")
        .current_dir(repo_root())
        .args(["ls-files", "--", ".config/nextest.toml"])
        .output()
        .expect("git should be runnable");
    assert!(
        !String::from_utf8_lossy(&out.stdout).trim().is_empty(),
        ".config/nextest.toml is not tracked by git. It exists on this machine and would not \
         exist in CI, where every gate would silently run under nextest's default profile and \
         write no JUnit at all."
    );
}

/// nextest does not run doctests, so something else must.
///
/// Discovered from the tasks rather than listed: every manifest a gate runs nextest against
/// is a manifest that must also see `cargo test --doc`. Dropping a whole class of test while
/// the gate stays green is this issue's own subject, one level up.
#[test]
fn every_workspace_that_runs_nextest_also_runs_the_doctests() {
    // The token is trimmed of TOML punctuation because a `run` entry ends `Cargo.toml",` and
    // the same path spelled two ways compares unequal — which would make this test pass or
    // fail on where in the list a line happened to sit rather than on what it runs.
    let manifest_of = |line: &str| -> Option<String> {
        line.split_whitespace()
            .skip_while(|w| *w != "--manifest-path")
            .nth(1)
            .map(|m| m.trim_matches(|c| c == '"' || c == ',').to_string())
    };

    let under_nextest: BTreeSet<String> = nextest_lines()
        .iter()
        .filter_map(|l| manifest_of(l))
        .collect();
    let under_doctests: BTreeSet<String> = mise_toml()
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("cargo test --doc"))
        .filter_map(manifest_of)
        .collect();

    let uncovered: Vec<&String> = under_nextest.difference(&under_doctests).collect();
    assert!(
        uncovered.is_empty(),
        "these workspaces run under nextest, which does not run doctests, and no task runs \
         `cargo test --doc` against them: {uncovered:?}"
    );
}

/// Every gate that runs tests renders a summary.
///
/// The bar is *uniformity*, not any one job: a gate that reports differently from its
/// neighbours is how a reader learns to stop trusting the summary, and a gate whose summary
/// step was never added looks identical to one with nothing to say.
/// `ci.yml`'s jobs and their bodies. Jobs are the two-space keys under `jobs:`; a job's body
/// runs to the next one.
fn ci_jobs() -> Vec<(String, String)> {
    let yml = ci_yml();
    let mut jobs: Vec<(String, String)> = Vec::new();
    let mut name = String::new();
    let mut body = String::new();
    for line in yml.lines().skip_while(|l| !l.starts_with("jobs:")) {
        let is_header = line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim().starts_with('-');
        if is_header {
            if !name.is_empty() {
                jobs.push((name.clone(), std::mem::take(&mut body)));
            }
            name = line.trim().trim_end_matches(':').to_string();
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !name.is_empty() {
        jobs.push((name, body));
    }
    assert!(
        jobs.len() >= 5,
        "only {} jobs parsed out of ci.yml; every assertion built on this would be vacuous",
        jobs.len()
    );
    jobs
}

#[test]
fn every_job_that_runs_tests_also_renders_a_summary() {
    let jobs = ci_jobs();

    // A job runs tests when it invokes a task that does. Read from the task file rather than
    // guessed, so a task that stops running tests stops being required to report them.
    let test_tasks: Vec<String> = mise_toml()
        .split("[tasks.")
        .skip(1)
        .filter(|block| block.contains("cargo nextest run") || block.contains("npm run test"))
        .filter_map(|block| block.lines().next())
        .map(|l| l.trim_end_matches(']').to_string())
        .collect();
    assert!(
        test_tasks.len() >= 3,
        "only {test_tasks:?} look like test tasks; the parser is reading the wrong thing"
    );

    let mut silent = Vec::new();
    for (job, body) in &jobs {
        if !body.contains("test-summary") && runs_a_test_task(body, &test_tasks) {
            silent.push(job.clone());
        }
    }
    assert!(
        silent.is_empty(),
        "these jobs run a test task and render no summary, so their results reach a reader \
         only as a colored dot: {silent:?}"
    );
}

/// `--locked` is passed exactly where there is a lock to honour.
///
/// The flag means "fail rather than re-resolve", and `ci-cli`'s note explains why every gate
/// wants it: without it cargo silently rewrites the lock and the gate goes green on a tree
/// the release build refuses. Against a workspace with **no committed lock** it means
/// something else entirely — the step fails before it starts, because there is nothing to be
/// locked to.
///
/// This repository has both kinds. `yidam/cli/Cargo.lock` is tracked; the harness workspace's
/// and the Rust SDK's are not, hidden by the same shape of global-gitignore entry that nearly
/// swallowed `.config/`. So the rule is a pairing rather than a blanket, and it is checked in
/// both directions: a `--locked` build of an unlocked workspace is a step that cannot run,
/// and a lock that becomes tracked should get its flag back.
#[test]
fn locked_is_passed_against_the_workspaces_that_have_a_lock() {
    let tracked = |ws: &str| {
        Command::new("git")
            .current_dir(repo_root())
            .args(["ls-files", "--", &format!("{ws}/Cargo.lock")])
            .output()
            .ok()
            .is_some_and(|o| !String::from_utf8_lossy(&o.stdout).trim().is_empty())
    };

    let mut wrong = Vec::new();
    for rel in workflow_and_action_files() {
        let text = code_only(&read(&rel), "#");
        for (i, line) in text.lines().enumerate() {
            if !line.contains("--manifest-path") {
                continue;
            }
            let Some(manifest) = line
                .split_whitespace()
                .skip_while(|w| *w != "--manifest-path")
                .nth(1)
            else {
                continue;
            };
            let manifest = manifest.trim_matches('"');
            let Some(ws) = manifest.strip_suffix("/Cargo.toml") else {
                continue;
            };
            // The flag may sit on a previous continuation line of the same command.
            let window: String = text
                .lines()
                .skip(i.saturating_sub(3))
                .take(5)
                .collect::<Vec<_>>()
                .join(" ");
            let locked = window.contains("--locked");
            if locked != tracked(ws) {
                wrong.push(format!(
                    "  {rel}:{}: --locked={locked} but {ws}/Cargo.lock tracked={}",
                    i + 1,
                    tracked(ws)
                ));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "`--locked` and a committed lockfile must agree. A locked build of an unlocked          workspace fails outright; an unlocked build of a locked one re-resolves in          silence:\n{}",
        wrong.join("\n")
    );
}

/// Every workflow and composite action in `.github/`, discovered.
fn workflow_and_action_files() -> Vec<String> {
    let mut out = Vec::new();
    for entry in WalkDir::new(repo_root().join(".github"))
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if entry.file_type().is_file() && path.extension().is_some_and(|e| e == "yml") {
            out.push(
                path.strip_prefix(repo_root())
                    .unwrap_or(path)
                    .display()
                    .to_string(),
            );
        }
    }
    assert!(
        out.len() >= 5,
        "only {} workflow files found; this test is looking at the wrong tree",
        out.len()
    );
    out
}

/// Does this job body invoke one of the test tasks?
///
/// Written out rather than a `contains`, because the first version of this was a `contains`
/// and it never matched the main gate at all: the `ci` job runs
/// `mise run ci-${{ matrix.workspace }}`, which contains the name of no task. Deleting that
/// job's summary step left this file green — the failure it exists to catch, in the job it
/// most needed to catch it in.
///
/// So a `${{ matrix.<key> }}` is expanded against the values the job's own matrix declares,
/// and an invocation that still cannot be resolved counts as running tests. Being wrong in
/// the direction of "report it anyway" costs a summary step nobody reads; being wrong the
/// other way is how the finding above happened.
fn runs_a_test_task(body: &str, test_tasks: &[String]) -> bool {
    let invocations: Vec<String> = body
        .lines()
        .filter_map(|l| l.split("mise run ").nth(1))
        .map(|rest| rest.trim().to_string())
        .collect();

    for invocation in invocations {
        if test_tasks
            .iter()
            .any(|t| invocation.starts_with(t.as_str()))
        {
            return true;
        }
        let Some(key) = invocation
            .split_once("${{ matrix.")
            .and_then(|(_, rest)| rest.split_once(' ').map(|(k, _)| k.to_string()))
        else {
            continue;
        };
        // The values that key takes, from the `include:` rows in this job's own matrix.
        let mut resolved_any = false;
        for line in body.lines() {
            let line = line.trim().trim_start_matches("- ");
            let Some(value) = line.strip_prefix(&format!("{key}: ")) else {
                continue;
            };
            resolved_any = true;
            let candidate = invocation.replace(&format!("${{{{ matrix.{key} }}}}"), value.trim());
            if test_tasks.iter().any(|t| candidate.starts_with(t.as_str())) {
                return true;
            }
        }
        if !resolved_any {
            return true;
        }
    }
    false
}

/// Every runtime skip announces itself the one way the census reads.
///
/// This is the census's coverage, and it is the assertion that keeps the count honest. A
/// suite added tomorrow that gates on an environment variable and writes its own
/// `eprintln!("skipping …")` would skip in silence and be counted by nobody — which is
/// precisely the state #462 found, with four such sites.
///
/// Narrow on purpose: it looks for a print whose text *opens* with a skip word. It cannot
/// tell a gated early-return from any other branch, and does not try; what it catches is the
/// shape all four original sites had.
#[test]
fn a_skip_is_announced_through_the_helper_and_not_in_its_own_words() {
    let mut offenders = Vec::new();
    let mut scanned = 0;
    let mut using_helper = 0;

    for tree in ["yidam/cli/tests", "yidam/tests/harness"] {
        for entry in WalkDir::new(repo_root().join(tree))
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if !entry.file_type().is_file() || path.extension() != Some("rs".as_ref()) {
                continue;
            }
            // The crate that defines the marker necessarily writes it.
            if path.components().any(|c| c.as_os_str() == "ci-report") {
                continue;
            }
            scanned += 1;
            let text = std::fs::read_to_string(path).expect("source is readable");
            let code = code_only(&text, "//");
            if code.contains("ci_report::skipped") {
                using_helper += 1;
            }
            for (i, line) in code.lines().enumerate() {
                let Some(open) = line.find("println!(\"").or_else(|| line.find("print!(\"")) else {
                    continue;
                };
                let literal = line[open..].split_once('"').map(|x| x.1).unwrap_or("");
                let lowered = literal.to_ascii_lowercase();
                if lowered.starts_with("skip") {
                    offenders.push(format!(
                        "  {}:{}",
                        path.strip_prefix(repo_root()).unwrap_or(path).display(),
                        i + 1
                    ));
                }
            }
        }
    }

    assert!(
        scanned > 30,
        "only {scanned} test sources scanned; this test is looking at the wrong tree"
    );
    assert!(
        using_helper > 0,
        "nothing calls `ci_report::skipped`. Either every gated suite was removed, or the \
         skips went back to announcing themselves in their own words and the census now \
         counts nothing while reporting a confident zero."
    );
    assert!(
        offenders.is_empty(),
        "these announce a skip in their own words rather than through `ci_report::skipped`, \
         so the census does not see them:\n{}",
        offenders.join("\n")
    );
}

/// Every gate that renders a summary also contributes a fragment, and the merge collects it.
///
/// Three joins, none of them visible in one file, and each fails quietly on its own:
///
/// 1. **A gate that summarises but writes no fragment.** Its results are in a job summary and
///    absent from the report. The page renders the gates it was given and says nothing about
///    the one it was not — which reads as a complete run.
/// 2. **A renamed artifact.** The action uploads `quality-<artifact>`; the merge downloads a
///    pattern. `ci-report merge` refuses an *empty* set, so losing every fragment is red —
///    and losing one is a report describing half a run, green.
/// 3. **A merge that does not wait.** A job absent from `needs:` may not have uploaded yet.
///
/// All three are discovered from the two files rather than listed here.
#[test]
fn every_summary_also_writes_a_report_fragment() {
    let jobs = ci_jobs();
    let action = code_only(&read(".github/actions/test-summary/action.yml"), "#");

    // The literal prefix of the artifact the action uploads, up to the first expansion.
    let upload = action
        .split("name: quality-")
        .nth(1)
        .expect("the composite action no longer uploads a `quality-` artifact");
    let templated = upload.lines().next().unwrap_or("").trim();
    assert!(
        templated.contains("${{ inputs.artifact }}"),
        "the fragment artifact is no longer named after the gate ({templated:?}); two gates \
         writing one name would leave a report describing half a run"
    );

    let mut summarising = Vec::new();
    let mut without_fragment = Vec::new();
    for (job, body) in &jobs {
        if !body.contains("./.github/actions/test-summary") {
            continue;
        }
        summarising.push(job.clone());
        if !body.contains("json:") {
            without_fragment.push(job.clone());
        }
    }
    assert!(
        summarising.len() >= 4,
        "only {summarising:?} render a summary; the parse is reading the wrong thing"
    );
    assert!(
        without_fragment.is_empty(),
        "these gates render a summary and write no report fragment, so their results reach \
         the quality pages not at all — and a page that lists the gates it was given reads \
         as a complete run: {without_fragment:?}"
    );

    let (_, merge) = jobs
        .iter()
        .find(|(_, body)| body.contains("ci-report merge"))
        .expect("no job merges the fragments into a quality report");

    let pattern = merge
        .split("pattern: ")
        .nth(1)
        .and_then(|r| r.lines().next())
        .map(str::trim)
        .expect("the merge job downloads no artifact pattern");
    assert!(
        pattern.starts_with("quality-") && pattern.ends_with('*'),
        "the merge downloads {pattern:?}, which does not match what the action uploads \
         (`quality-<artifact>`). A pattern that matches nothing is caught — `merge` refuses \
         an empty set — but one that matches some of them is a partial report, green."
    );

    let needs = merge
        .split("needs: ")
        .nth(1)
        .and_then(|r| r.lines().next())
        .unwrap_or_default()
        .to_string();
    let unawaited: Vec<&String> = summarising.iter().filter(|j| !needs.contains(*j)).collect();
    assert!(
        unawaited.is_empty(),
        "the merge job does not wait for {unawaited:?}, which write fragments it is supposed \
         to collect. It would merge whatever had finished."
    );
}
