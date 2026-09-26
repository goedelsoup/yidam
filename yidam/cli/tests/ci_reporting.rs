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

mod common;

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

/// Does this command line run the suite through nextest?
///
/// Two spellings, and for three years this file only knew one. `cargo llvm-cov … nextest` is
/// the same runner with counters, and since #1013 it is the *only* thing that executes the
/// CLI suite in `ci (cli)` — so a scan for `cargo nextest run` alone would have stopped
/// covering the run that writes the JUnit every summary in that job renders.
/// `nextest` as a whole token, because every one of these lines also carries
/// `--config-file .config/nextest.toml`. A `contains("nextest")` is therefore true of
/// `cargo llvm-cov --no-report test --config-file .config/nextest.toml` — a line that runs the
/// suite through *cargo test*, writes no JUnit, and would have read here as a nextest run. Found
/// by mutating the subcommand away and watching this stay green.
fn runs_nextest(line: &str) -> bool {
    line.contains("cargo nextest run")
        || (line.contains("llvm-cov") && line.split_whitespace().any(|token| token == "nextest"))
}

/// Every command line in `mise.toml` that runs the suite through nextest, instrumented or not.
fn nextest_lines() -> Vec<String> {
    let text = mise_toml();
    let found: Vec<String> = text
        .lines()
        .map(str::trim)
        .filter(|l| runs_nextest(l))
        .map(|l| {
            l.trim_matches(|c| c == '"' || c == ',' || c == ' ')
                .to_string()
        })
        .collect();
    assert!(
        !found.is_empty(),
        "no nextest invocation in mise.toml. Either the gates went back to `cargo test`, \
         which writes no JUnit and leaves every summary step with nothing to render, or this \
         parser is reading the wrong thing — and both make every assertion below vacuous."
    );
    // Both spellings are present, and each is a separate hole when it goes missing: the plain
    // one is the local gate and the instrumented one is what CI runs.
    for spelling in ["cargo nextest run", "llvm-cov"] {
        assert!(
            found.iter().any(|l| l.contains(spelling)),
            "no nextest line in mise.toml contains `{spelling}`; whichever assertion below \
             was meant to cover that form now covers nothing. Lines found: {found:?}"
        );
    }
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
    let tracked = common::git::out(&repo_root(), &["ls-files", "--", ".config/nextest.toml"]);
    assert!(
        !tracked.is_empty(),
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
        .filter(|block| block.lines().any(runs_nextest) || block.contains("npm run test"))
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
        !common::git::out(
            &repo_root(),
            &["ls-files", "--", &format!("{ws}/Cargo.lock")],
        )
        .is_empty()
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
    for entry in common::repo_walk(&repo_root().join(".github")) {
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
        // `yidam/tests/harness` is its own cargo package, so on any machine that has built
        // it this walk used to descend 566 MB of `target/` and read the generated `.rs`
        // files a build script left there. None of them happened to carry a skip-print, so
        // it never reddened — the same latent shape as #900, one tree over.
        for entry in common::repo_walk(&repo_root().join(tree)) {
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

/// The jobs that must run on a pipeline with a failure say so.
///
/// A job whose `if` carries no status check function gets an implicit `success()`, and GitHub
/// evaluates that across the whole ancestry rather than the direct `needs`. So a job that
/// depends on an `always()` job still skips when something further up failed — which is what
/// happened on the merge that landed #468: `cli-full` failed, `quality` ran on its own
/// `always()` and succeeded, and `series` was skipped anyway. The first record was never
/// written and the branch was never created.
///
/// Discovered from the workflow rather than listed: any job that `needs` a job whose own `if`
/// says `always()` has inherited that intent, and must state it too or be silently skipped by
/// the thing its dependency was written to survive.
#[test]
fn a_job_needing_an_always_job_says_always_itself() {
    let jobs = ci_jobs();
    let unconditional: Vec<&String> = jobs
        .iter()
        .filter(|(_, body)| body.contains("if: always()"))
        .map(|(name, _)| name)
        .collect();
    assert!(
        !unconditional.is_empty(),
        "no job in ci.yml runs unconditionally; this test is looking at the wrong thing"
    );

    let mut silent = Vec::new();
    for (name, body) in &jobs {
        let Some(needs) = body
            .split("needs:")
            .nth(1)
            .and_then(|r| r.lines().next())
            .map(str::trim)
        else {
            continue;
        };
        let inherits = unconditional.iter().any(|dep| needs.contains(dep.as_str()));
        // `always()` anywhere in the job's own condition, however it is spelled — the block
        // scalar form wraps it onto its own line.
        let condition: String = body
            .lines()
            .skip_while(|l| !l.trim_start().starts_with("if:"))
            .take_while(|l| {
                !l.trim_start().starts_with("needs:") && !l.trim_start().starts_with("runs-on:")
            })
            .collect();
        if inherits && !condition.contains("always()") {
            silent.push(format!("  {name} needs {needs}"));
        }
    }
    assert!(
        silent.is_empty(),
        "these jobs depend on a job that runs unconditionally, and do not run \
         unconditionally themselves. GitHub applies an implicit `success()` across the whole \
         ancestry, so one failure anywhere upstream skips them — without a message, and \
         without the work they were supposed to do:\n{}\n\nSay `always() && \
         needs.<job>.result == 'success' && …` so the condition is the one that was meant.",
        silent.join("\n")
    );
}

/// Every gate's numbers can be attributed to a job that exists (#516).
///
/// The quality report stamps each gate with the conclusion of the job it ran in, matched by
/// the name the run displays. A gate whose name matches no job is not a loud failure — it is
/// a conclusion that stays `None` for ever, which the pages draw as "unknown". One unknown
/// among fourteen greens is exactly the kind of thing nobody notices.
///
/// It had already happened before the field existed. `ci (parity)` runs the Rust, TypeScript
/// and Python parity arms and summarises only the Rust one, so its gate reads
/// `ci (parity · rust sdk)` — a heading truer than the job name, and one no job will ever be
/// called. The composite action's `job:` input is how such a gate says where it ran, and this
/// is what makes it say so.
///
/// Both sides come from `ci.yml`: the gates from the `gate:`/`job:` inputs, the job names
/// from the `name:` of each job, with the one matrix expanded from its own `include:`.
#[test]
fn the_gate_names_name_real_jobs() {
    let yml = read(".github/workflows/ci.yml");

    // Job display names. `name: ci (…)` at job level — two indents under `jobs:` — including
    // the matrix template, which is expanded below from the values it interpolates.
    let mut jobs: Vec<String> = Vec::new();
    for line in yml.lines() {
        let Some(rest) = line.strip_prefix("    name: ") else {
            continue;
        };
        jobs.push(rest.trim().to_string());
    }
    assert!(
        jobs.len() > 5,
        "found {} job names in ci.yml, which is too few to be the whole workflow — the shape \
         this scan keys on has changed and every assertion below is now vacuous",
        jobs.len()
    );

    // The matrix. `name: ci (${{ matrix.<key> }})` becomes one job per value of that key,
    // read from the `include:` block rather than assumed to be `cli` and `harness`.
    let expanded: Vec<String> = jobs
        .iter()
        .flat_map(|name| match name.split_once("${{ matrix.") {
            None => vec![name.clone()],
            Some((before, rest)) => {
                let key = rest.split_once(" }}").map(|(k, _)| k.trim()).unwrap_or("");
                let after = rest.split_once(" }}").map(|(_, a)| a).unwrap_or("");
                let values: Vec<String> = yml
                    .lines()
                    .filter_map(|l| l.trim().strip_prefix(&format!("- {key}: ")))
                    .map(|v| v.trim().to_string())
                    .collect();
                assert!(
                    !values.is_empty(),
                    "{name} interpolates `matrix.{key}` and no `- {key}:` entry defines a \
                     value for it, so this test cannot know what the job is called"
                );
                values
                    .into_iter()
                    .map(|v| format!("{before}{v}{after}"))
                    .collect()
            }
        })
        .collect();

    // What each summary claims, and where it says it ran. The `job:` input overrides `gate:`
    // for matching; a gate that supplies neither is matched by its own name.
    let mut claims: Vec<(String, String)> = Vec::new();
    let mut current_gate: Option<String> = None;
    for line in yml.lines() {
        let trimmed = line.trim();
        if let Some(g) = trimmed.strip_prefix("gate: ") {
            if let Some(prev) = current_gate.take() {
                claims.push((prev.clone(), prev));
            }
            current_gate = Some(g.trim().to_string());
        } else if let Some(j) = trimmed.strip_prefix("job: ") {
            let gate = current_gate
                .take()
                .expect("a `job:` input with no `gate:` above it");
            claims.push((gate, j.trim().to_string()));
        }
    }
    if let Some(prev) = current_gate {
        claims.push((prev.clone(), prev));
    }
    assert!(
        !claims.is_empty(),
        "no `gate:` input was found in ci.yml — the summaries are gone, or this scan is"
    );

    let mut orphans = Vec::new();
    for (gate, job) in &claims {
        // The matrix gate is itself a template and expands the same way its job name does.
        let candidates: Vec<String> = match job.split_once("${{ matrix.") {
            None => vec![job.clone()],
            Some(_) => expanded.clone(),
        };
        if !candidates.iter().any(|c| expanded.contains(c)) {
            orphans.push(format!(
                "  gate `{gate}` reports from job `{job}`, and no job in ci.yml is called that"
            ));
        }
    }
    assert!(
        orphans.is_empty(),
        "a gate's conclusion is matched against the run's job list by name, so each of these \
         would be stamped `unknown` on every run — silently, for ever:\n{}\n\nJobs in this \
         workflow: {:?}\n\nIf the heading is deliberately not the job name, say where it ran \
         with the composite action's `job:` input.",
        orphans.join("\n"),
        expanded
    );
}

/// The run's job conclusions are fetched, and they reach the merge (#516).
///
/// Two halves, each silent on its own. A merge that is never given `--jobs` writes a report
/// in which every gate's outcome is unknown; a fetch whose output nothing reads is a step
/// that costs an API call and changes nothing. Neither fails a build. The report still
/// merges, the pages still render, and the one thing they cannot say is whether the run
/// passed — which is the state this whole issue was filed about.
///
/// It is the shape `YIDAM_QUALITY_SERIES` was in for a whole phase: written by one job, read
/// by nobody, with a comment saying otherwise.
#[test]
fn the_runs_job_conclusions_are_fetched_and_reach_the_merge() {
    let yml = code_only(&read(".github/workflows/ci.yml"), "#");

    // The reporter has to accept them, or passing them is a crash rather than a feature.
    let reporter = read("yidam/tests/harness/ci-report/src/main.rs");
    assert!(
        reporter.contains("\"--jobs\""),
        "`ci-report merge` no longer parses `--jobs`, so the workflow below is passing a flag \
         it will reject"
    );

    // The `quality` job's own lines: from its header to the next job header, which is a line
    // indented by exactly two spaces. Splitting on `"\n  "` would also split on every line
    // inside the job, since those start with more.
    let mut quality = String::new();
    let mut inside = false;
    for line in yml.lines() {
        let header = line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':');
        if header {
            if inside {
                break;
            }
            inside = line.trim() == "quality:";
            continue;
        }
        if inside {
            quality.push_str(line);
            quality.push('\n');
        }
    }
    assert!(
        !quality.trim().is_empty(),
        "ci.yml declares no `quality` job, or its shape is no longer one this scan can find"
    );

    assert!(
        quality.contains("actions/runs/") && quality.contains("/jobs"),
        "the quality job never asks what the run's jobs concluded, so every gate's outcome \
         is `unknown` on every run:\n{quality}"
    );
    assert!(
        quality.contains("--jobs"),
        "the run's job list is fetched and never handed to the merge — an API call whose \
         result is discarded, and a report that still cannot say whether the run passed"
    );
    assert!(
        quality.contains("per_page=100"),
        "the jobs endpoint paginates at 30 and this workflow has more jobs than that is safe \
         for. `parse_jobs` refuses a truncated list, so this would be a red quality job \
         rather than a wrong report — but the fix is to ask for them all"
    );
    assert!(
        quality.contains("actions: read"),
        "the jobs API needs `actions: read`. The repository's default is read-all today, \
         which grants it by accident; a settings change would take it away and this step \
         would start failing for a reason nobody would connect to it"
    );
}

/// The body of one job in `ci.yml`, comments already stripped.
///
/// Textual rather than parsed: this repository takes no YAML dependency, and the shape a
/// guard needs is unambiguous here — every job is a two-space key, every job property a
/// four-space one, so a job ends at the next line whose indent is exactly two.
fn job_block(name: &str) -> String {
    let yml = ci_yml();
    let mut out: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in yml.lines() {
        let opens_a_job = line.len() > 2 && line.starts_with("  ") && !line.starts_with("   ");
        if opens_a_job && line.trim_start().starts_with(&format!("{name}:")) {
            inside = true;
            continue;
        }
        if inside && opens_a_job {
            break;
        }
        if inside {
            out.push(line);
        }
    }
    assert!(
        !out.is_empty(),
        "no `{name}:` job in ci.yml, or this parser stopped finding one. Every assertion \
         below reads this block, so an empty one passes them all."
    );
    out.join("\n")
}

/// Every step that runs work in the full-feature job streams resource samples while it runs.
///
/// A GitHub-hosted runner killed for memory names no resource: the step ends with "The runner
/// has received a shutdown signal" and an exit code, and memory and disk are equally good
/// guesses that take opposite fixes. #539 added a sampler to the step that was failing and
/// measured it. On d596634 the job was killed two steps earlier, where the sampler was not,
/// and left exactly the empty log #539 had started from — a second undiagnosable kill, from a
/// fix that closed the instance and not the class.
///
/// So the rule is the class: **anything in this job that runs a mise task goes through the
/// wrapper.** Discovered by walking the job's own steps, because a list of heavy steps kept
/// here would stop covering whatever was added next without ever going red — which is the
/// same failure one level up.
#[test]
fn every_heavy_step_in_the_full_feature_job_is_sampled() {
    let block = job_block("cli-full");
    let invocations: Vec<&str> = block
        .lines()
        .map(str::trim)
        .filter(|l| l.contains("mise run "))
        .collect();
    assert!(
        !invocations.is_empty(),
        "the full-feature job runs no mise task. Either the job stopped doing the work this \
         gate exists for, or this parser is reading the wrong block — and both make the \
         assertion below vacuous."
    );

    let unsampled: Vec<&str> = invocations
        .iter()
        .filter(|l| !l.contains("with-sampler.sh"))
        .copied()
        .collect();
    assert!(
        unsampled.is_empty(),
        "these steps in `cli-full` run without a resource sampler:\n  {}\nThis job has been \
         killed for memory twice, and each time the log named no resource. Wrap it: \
         `.github/scripts/with-sampler.sh mise run <task>`.",
        unsampled.join("\n  ")
    );
}

/// The sampler the steps call still samples.
///
/// The guard above matches a path. A wrapper that had been emptied to `exec \"$@\"` would
/// satisfy it and restore the silence, so the marker every `[res]` line carries is checked
/// where it is produced.
#[test]
fn the_sampler_the_steps_call_still_reports_both_resources() {
    let script = read(".github/scripts/with-sampler.sh");
    for needle in ["[res]", "MemAvailable", "df -h"] {
        assert!(
            script.contains(needle),
            "with-sampler.sh no longer emits `{needle}`. Memory and disk take opposite fixes, \
             so a sampler reporting one of them leaves half the diagnosis to a guess."
        );
    }
}

/// The compile-job cap belongs to the job, not to a step inside it.
///
/// #539 capped `CARGO_BUILD_JOBS` on `coverage-full`, the step that happened to be failing.
/// d596634 was killed in `ci-cli-full`, which had no cap, for the same reason. A four-space
/// `env:` covers every step; a step-level one covers exactly the instance that was last seen
/// to fail, and moving it back would be invisible in review.
#[test]
fn the_build_job_cap_covers_the_whole_full_feature_job() {
    let block = job_block("cli-full");
    let at_job_level = block
        .lines()
        .any(|l| l.starts_with("      CARGO_BUILD_JOBS:") && !l.starts_with("       "));
    assert!(
        at_job_level,
        "`CARGO_BUILD_JOBS` is not set at the job's own `env:` (six-space key under a \
         four-space `env:`). Every heavy step in this job links the same 59 test binaries \
         against the same dependency tree; a cap on one of them is a cap on the step that \
         failed last time.\n{block}"
    );
}

/// The command lines one `mise.toml` task runs, comments already stripped.
///
/// Both spellings, because both are in this file: `run = "…"` for a task with one command
/// and `run = [ … ]` for a list. A parser that read only the list form would return an empty
/// vector for the other, and an empty vector passes every comparison below.
fn task_commands(name: &str) -> Vec<String> {
    let text = mise_toml();
    let block = text
        .split(&format!("[tasks.{name}]\n"))
        .nth(1)
        .unwrap_or_else(|| panic!("no `[tasks.{name}]` in mise.toml, or it was renamed"))
        .split("\n[")
        .next()
        .unwrap_or_default()
        .to_string();

    let mut out = Vec::new();
    for line in block.lines().map(str::trim) {
        let command = if let Some(rest) = line.strip_prefix("run = ") {
            rest
        } else if line.starts_with('"') {
            line
        } else {
            continue;
        };
        let command = command.trim_matches(|c| c == '"' || c == ',' || c == ' ');
        if !command.is_empty() && command != "[" && command != "]" {
            out.push(command.to_string());
        }
    }
    assert!(
        !out.is_empty(),
        "`[tasks.{name}]` parsed to no commands; every comparison built on this would be \
         vacuous"
    );
    out
}

/// The feature-gated code is compiled on every event, by one job or the other (#922).
///
/// `cli-full` has been `if: github.event_name != 'pull_request'` since it was written, which
/// left the only gate that compiles `--all-features` running *after* the merge. Three outages
/// went through that hole — `unsafe_code = "forbid"` against `--features export-sqlite`, the
/// fastembed 4→6 bump, and #1003's unused import in `index_push.rs` — and each was green
/// through review and red on a job nobody was watching.
///
/// `cli-features` closes it by taking the complement of that condition, so the assertion is
/// the complement itself rather than either half: an edit that narrows one condition without
/// widening the other reopens the hole at whatever event falls between them, and the sign
/// would again be a red `main`.
#[test]
fn the_feature_gated_build_is_compiled_on_every_event() {
    let condition = |block: &str| -> String {
        block
            .lines()
            .map(str::trim)
            .find(|l| l.starts_with("if:"))
            .unwrap_or_default()
            .to_string()
    };

    let full = condition(&job_block("cli-full"));
    let check = condition(&job_block("cli-features"));
    assert_eq!(
        full, "if: github.event_name != 'pull_request'",
        "`cli-full` no longer skips pull requests in the spelling `cli-features` was written \
         to complement. The two conditions have to partition the events between them; this \
         one is now `{full}`."
    );
    assert_eq!(
        check, "if: github.event_name == 'pull_request'",
        "`cli-features` is the pull-request half of `cli-full`, and it now reads `{check}`. \
         Anything other than the exact complement leaves an event on which nothing compiles \
         the gated code — which is the state #922 was filed about."
    );
}

/// The pull-request check compiles exactly what the merge compiles.
///
/// It is worth what it resembles. A check that fell behind `ci-cli-full` — a third feature
/// set added there and not here — would be a green pull request in front of a red main, for
/// the feature that was added *because* it needed compiling.
///
/// So the clippy legs of the two tasks are compared as a whole and in order, rather than the
/// check being asserted to contain something. Character for character, also because that is
/// what makes them cache hits: cargo fingerprints a command's features, and a leg spelled
/// differently is a dependency tree rebuilt from nothing on a job budgeted for 20 seconds.
///
/// Clippy and not `cargo check`, which is what #922 proposed: two of the three outages were
/// lint failures rather than compile errors, and `-D warnings` is what makes them red.
#[test]
fn the_pull_request_check_compiles_what_the_merge_compiles() {
    let legs = |cmds: &[String]| -> Vec<String> {
        cmds.iter()
            .filter(|c| c.contains("cargo clippy"))
            .cloned()
            .collect()
    };

    let merge = legs(&task_commands("ci-cli-full"));
    let check = legs(&task_commands("check-features"));
    assert!(
        merge.len() >= 2,
        "`ci-cli-full` runs {} clippy leg(s); it has had two since the vector-read split, so \
         this is reading the wrong thing: {merge:?}",
        merge.len()
    );
    assert_eq!(
        merge, check,
        "`check-features` and `ci-cli-full` no longer compile the same feature sets. The \
         pull-request gate is worth exactly its resemblance to the merge gate, and the legs \
         must also match character for character or they miss the cache they borrow."
    );

    let job = job_block("cli-features");
    assert!(
        job.contains("mise run check-features"),
        "the `cli-features` job does not run `check-features`, so the task above is compared \
         against a gate that runs something else:\n{job}"
    );
}

/// The check borrows the cache it could never afford to build.
///
/// `cargo clippy --all-features` finishes in 8.5s against a restored tree and takes twenty
/// minutes against an empty one, so this gate is cheap only while it hits `cli-full`'s cache.
/// Two things decide that, and neither announces itself when it breaks — a miss is a slow
/// green, not a red.
///
/// **The key.** rust-cache's `key` input is *added to* an automatic job-id component, so two
/// jobs cannot share a cache through it; `shared-key` replaces that component. Both jobs must
/// name the same one.
///
/// **The environment.** rust-cache hashes every variable prefixed `CARGO`, `CC`, `CFLAGS`,
/// `CXX`, `CMAKE` or `RUST` into the same key. `cli-full` carries `CARGO_BUILD_JOBS: '2'` for
/// a memory cap that has nothing to do with this job, and a job that does not carry it writes
/// a different key — which is why the two environments are compared rather than the flag
/// being explained where only a reader would find it.
#[test]
fn the_feature_check_borrows_the_full_feature_cache() {
    let full = job_block("cli-full");
    let check = job_block("cli-features");

    for (name, block) in [("cli-full", &full), ("cli-features", &check)] {
        assert!(
            block.contains("shared-key: cli-full"),
            "`{name}` does not restore under `shared-key: cli-full`. With rust-cache's `key` \
             input instead, the job id goes into the key and the two jobs get one cache each \
             — the pull-request one built from nothing, every time.\n{block}"
        );
    }
    assert!(
        check.contains("save-if: ${{ false }}"),
        "`cli-features` would save a cache. A pull request's writes land in the PR's own \
         scope, where nothing else can read them and where they count against the \
         repository's limit until they are evicted — so it would pay to store a tree it \
         borrowed.\n{check}"
    );

    // The prefixes rust-cache hashes, from its `config.ts`. An `env:` key is `NAME: value`
    // at some indent; nothing else in a job body starts with one of these in upper case.
    let keyed_env = |block: &str| -> BTreeSet<String> {
        block
            .lines()
            .map(str::trim)
            .filter(|l| {
                ["CARGO", "CC", "CFLAGS", "CXX", "CMAKE", "RUST"]
                    .iter()
                    .any(|p| l.starts_with(p))
                    && l.contains(':')
            })
            .map(str::to_string)
            .collect()
    };
    assert_eq!(
        keyed_env(&full),
        keyed_env(&check),
        "`cli-full` and `cli-features` set different cache-keyed environment variables, so \
         they compute different cache keys and the pull-request job restores nothing. \
         rust-cache hashes CARGO*, CC*, CFLAGS*, CXX*, CMAKE* and RUST* into the key; \
         whatever one job sets, the other has to set too."
    );
}
