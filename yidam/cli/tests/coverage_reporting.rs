//! A coverage number says which build it was taken under, and says it truthfully.
//!
//! The measurement is honest by construction — a file the build did not compile has no LCOV
//! record, and `ci-report` classifies the absence rather than reading it as zero. What is
//! *not* honest by construction is the **label**. The feature list printed beside the number
//! is a string in a workflow, and a string can be right on the day it is written and wrong
//! six months later, with nothing going red: the reader would then be told the index path was
//! measured when it was never compiled, which is precisely the claim #464 exists to prevent.
//!
//! So the label is checked against `[features] default` in `Cargo.toml`, discovered from the
//! manifest rather than restated here.

use std::collections::BTreeSet;
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

/// Text with `#` comments removed — prose about the code must not answer for the code.
fn code_only(text: &str) -> String {
    text.lines()
        .map(|line| match line.split_once('#') {
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => before,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The `default` feature list from yidam/cli's manifest.
fn default_features() -> BTreeSet<String> {
    let manifest = read("yidam/cli/Cargo.toml");
    let key = "\ndefault = [";
    let start = manifest
        .find(key)
        .expect("yidam/cli/Cargo.toml declares no `default` feature");
    let rest = &manifest[start + key.len()..];
    let body = &rest[..rest.find(']').expect("unterminated default list")];
    body.split('"')
        .skip(1)
        .step_by(2)
        .map(str::to_string)
        .collect()
}

/// The features named beside a coverage number are the features that build was compiled with.
///
/// The light gate measures the default set. If the two drift, the summary says a file was
/// measured that was never compiled — or, just as bad, calls a compiled file unmeasured and
/// hides a real coverage gap behind a feature gate that does not exist.
#[test]
fn the_reported_feature_set_is_the_one_the_gate_builds() {
    let yml = code_only(&read(".github/workflows/ci.yml"));
    let declared = default_features();

    let labels: Vec<BTreeSet<String>> = yml
        .lines()
        .filter_map(|l| l.split("features:").nth(1))
        .filter(|v| v.contains(','))
        .map(|v| {
            v.split('\'')
                .find(|s| s.contains(','))
                .unwrap_or("")
                .split(',')
                .map(|f| f.trim().to_string())
                .filter(|f| !f.is_empty())
                .collect()
        })
        .filter(|s: &BTreeSet<String>| !s.is_empty())
        .collect();

    assert!(
        !labels.is_empty(),
        "no coverage `features:` label found in ci.yml. Either the coverage summary stopped \
         naming its build — which is the thing that makes the number readable — or this \
         parser is looking at the wrong shape."
    );

    for label in &labels {
        assert_eq!(
            label, &declared,
            "a coverage summary is labelled {label:?} and yidam/cli's `default` feature set \
             is {declared:?}. A number under the wrong label is worse than no number: it \
             tells a reader the gated paths were measured when the gate never compiled them."
        );
    }
}

/// Does this workflow name that task — as a whole token, not as a substring?
///
/// `contains` is the wrong test and quietly the wrong answer: `coverage` is a substring of
/// `coverage-full`, so a workflow that runs only the full-feature task would read as running
/// the light one. That is how the check this serves went vacuous in the first place.
fn names(yml: &str, task: &str) -> bool {
    yml.split(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '`')
        .any(|token| token == task)
}

/// The gate that produces an LCOV is the gate that renders one.
///
/// Two halves that fail in opposite directions and both in silence: a job that measures
/// coverage and does not pass it to the summary throws the measurement away, and a job that
/// asks for a coverage section without producing an LCOV fails on a missing file for a
/// reason that has nothing to do with the tests.
///
/// The producing side is *discovered* rather than named, because it moved. This asked
/// `ci.yml.contains("mise run coverage")` until #1013, when the light gate stopped running
/// that task and ran `ci-cli-cov` instead — and the assertion did not notice, because
/// `mise run coverage-full` two jobs away contains the string it was looking for. A check
/// whose subject can be deleted while a *different* job keeps it green is a check that has
/// stopped having a subject. So: every `.lcov` path a summary step asks for must be written
/// by some task in `mise.toml`, and that task must be one this workflow runs.
#[test]
fn a_job_that_measures_coverage_reports_it_and_the_reverse() {
    let yml = code_only(&read(".github/workflows/ci.yml"));
    let mise: toml::Table = read("mise.toml").parse().expect("mise.toml parses");

    // Which task writes which LCOV, read off the `--output-path` the task itself passes.
    let mut writers: Vec<(String, String)> = Vec::new();
    let tasks = mise
        .get("tasks")
        .and_then(toml::Value::as_table)
        .expect("mise.toml declares tasks");
    for (name, task) in tasks {
        let steps = match task.get("run") {
            Some(toml::Value::String(one)) => vec![one.clone()],
            Some(toml::Value::Array(many)) => many
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect(),
            _ => continue,
        };
        for step in steps {
            let mut args = step.split_whitespace();
            while let Some(arg) = args.next() {
                if arg == "--output-path" {
                    if let Some(path) = args.next().filter(|p| p.ends_with(".lcov")) {
                        writers.push((path.to_string(), name.clone()));
                    }
                }
            }
        }
    }
    assert!(
        !writers.is_empty(),
        "no task in mise.toml writes an LCOV; nothing can produce the file a summary reads"
    );

    // Which LCOVs the workflow's summary steps ask for. The value is a `${{ … }}` expression,
    // so the paths are taken as the quoted tokens rather than as the whole line.
    let asked: BTreeSet<String> = yml
        .lines()
        .filter(|l| l.trim_start().starts_with("lcov:"))
        .flat_map(|l| {
            l.split(|c: char| c.is_whitespace() || c == '\'' || c == '"')
                .filter(|t| t.ends_with(".lcov"))
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        !asked.is_empty(),
        "no summary step in ci.yml is handed an LCOV — coverage is measured and discarded, \
         which is the half of this that fails in silence"
    );

    for path in &asked {
        let producers: Vec<&(String, String)> =
            writers.iter().filter(|(out, _)| out == path).collect();
        assert!(
            !producers.is_empty(),
            "a summary step asks for `{path}` and no mise task writes it. The step then fails \
             on a missing file for a reason that has nothing to do with the tests. Tasks that \
             write an LCOV: {writers:?}"
        );
        assert!(
            producers.iter().any(|(_, task)| names(&yml, task)),
            "`{path}` is written by {:?} and ci.yml runs none of them — the number the \
             summary renders would be whatever an earlier run happened to leave on disk",
            producers.iter().map(|(_, t)| t).collect::<Vec<_>>()
        );
    }

    // And the reporter must be given everything it needs to tell unmeasured from untested.
    for required in ["src:", "features:", "--diff", "--src", "--features"] {
        assert!(
            yml.contains(required)
                || read(".github/actions/test-summary/action.yml").contains(required),
            "the coverage path never supplies `{required}`, without which the report cannot \
             distinguish a file the build skipped from one no test touched"
        );
    }
}

/// The full-feature run exists, and its output is kept.
///
/// The light number can never cover the gated paths — that is not a defect, it is the trade
/// `ci.yml` makes for pull-request latency. What would be a defect is if the number that
/// *does* cover them were never taken, leaving "unmeasured" as a permanent answer rather
/// than a per-build one.
#[test]
fn the_gated_paths_are_measured_somewhere() {
    let mise = code_only(&read("mise.toml"));
    assert!(
        mise.contains("--all-features") && mise.contains("coverage-full"),
        "no task measures coverage with the gated features compiled in; `unmeasured` would \
         then be the permanent answer for the index, sqlite and rdf paths"
    );
    let yml = code_only(&read(".github/workflows/ci.yml"));
    assert!(
        yml.contains("mise run coverage-full"),
        "`coverage-full` exists as a task and no job runs it — a surface with no consumer, \
         which is the pattern #194's audit named and #461 found in `verify`"
    );
    assert!(
        yml.contains("coverage-full-lcov"),
        "the full-feature LCOV is produced and thrown away; P7's series is a sequence of \
         these and cannot start before its first point is kept"
    );
}

/// The commands of a mise task, whether `run` is one string or a list.
fn task_run(mise: &toml::Table, name: &str) -> Vec<String> {
    let task = mise
        .get("tasks")
        .and_then(|t| t.get(name))
        .unwrap_or_else(|| panic!("mise.toml has no task `{name}`"));
    match task.get("run") {
        Some(toml::Value::String(s)) => vec![s.clone()],
        Some(toml::Value::Array(a)) => a
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect(),
        _ => panic!("task `{name}` has no `run`"),
    }
}

/// The full-feature run renders its report exactly the way the light one does.
///
/// `ci-cli-cov` runs on every pull request, so its rendering step is proven by every green
/// gate. `coverage-full` runs on main and the weekly schedule only — nothing executes it
/// before a merge, and an error in it is discovered afterwards, by main going red.
///
/// It was `coverage` that ran on every pull request until #1013 folded the light gate's two
/// runs of the suite into one. The oracle has to be the task a green PR actually proves, so
/// this names that one; `coverage` is still held to the same lines by
/// `the_instrumented_gate_is_the_local_gate_with_its_suite_measured` below.
///
/// This makes the proven step the oracle for the unproven one. The two measure different
/// feature sets, which is the whole point of the pair, but that difference belongs to the
/// **run**: `--no-report nextest` is where features select what gets compiled. By the time
/// `report` renders, the profile data already exists and there is nothing left to select.
///
/// Not hypothetical, and not catchable by reading the tool's documentation. `coverage-full`
/// carried `--all-features` on its `report` step; `cargo llvm-cov report` rejects it with
/// `invalid option '--all-features' for subcommand 'report'` — while `cargo llvm-cov report
/// --help` *lists* `--all-features` among its options. The help is generic across
/// subcommands and the parser is not, so a guard derived from `--help` would have passed
/// this. A guard derived from the step that actually runs does not.
///
/// If the two ever need to render differently, this test is the place to say why.
#[test]
fn the_full_feature_run_renders_its_report_the_way_the_proven_one_does() {
    let mise: toml::Table = read("mise.toml").parse().expect("mise.toml parses");

    let render = |task: &str| -> String {
        let steps = task_run(&mise, task);
        let found: Vec<&String> = steps
            .iter()
            .filter(|s| s.contains("llvm-cov report"))
            .collect();
        assert_eq!(
            found.len(),
            1,
            "`{task}` has {} `llvm-cov report` steps and this test compares one",
            found.len()
        );
        found[0].clone()
    };

    // Everything except where it writes — the one thing the two are meant to disagree on.
    let shape = |cmd: &str| -> String {
        let mut out = Vec::new();
        let mut args = cmd.split_whitespace();
        while let Some(arg) = args.next() {
            if arg == "--output-path" {
                args.next();
                continue;
            }
            out.push(arg);
        }
        out.join(" ")
    };

    let light = render("ci-cli-cov");
    let full = render("coverage-full");
    assert_eq!(
        shape(&light),
        shape(&full),
        "the two coverage tasks render differently.\n  ci-cli-cov:    {light}\n  \
         coverage-full: {full}\n\nOnly `--output-path` may differ. Feature selection belongs \
         to the `--no-report nextest` step; `llvm-cov report` rejects it, and because nothing \
         runs `coverage-full` before a merge, the rejection is found on main."
    );
}

/// The gate CI runs is the gate a contributor runs, with its suite measured.
///
/// #1013: `ci (cli)` ran `ci-cli` and then `coverage`, which compiled and executed the same
/// 2,757 tests twice — 139s of a 338s job, on the pull-request critical path, for a second
/// uninstrumented run of a suite the instrumented one had just finished faster. It now runs
/// `ci-cli-cov`: the same gate with the plain run replaced by the measured one.
///
/// Which leaves two command lines that must not drift apart, in the direction that is
/// invisible. `ci-cli` is what `mise run ci` runs before a push, and it is what every guard
/// that reads "the CLI gate" out of `mise.toml` reads — so a flag added there and not here
/// would leave the pushed-to gate checking something CI does not, and nothing would say so
/// until a merge. `lint_registry.rs::ci_cli_denies_warnings_over_the_library` is exactly that
/// shape: it greps `mise.toml` for the `-D warnings` clippy line, and a copy of that line in
/// either task satisfies it.
///
/// So the relationship is asserted rather than described: `ci-cli-cov` is `ci-cli` with its
/// one `cargo nextest run` line replaced by `coverage`'s two. Nothing else added, nothing
/// dropped, and no third spelling of the instrumented run to keep in step with the first.
#[test]
fn the_instrumented_gate_is_the_local_gate_with_its_suite_measured() {
    let mise: toml::Table = read("mise.toml").parse().expect("mise.toml parses");

    let local = task_run(&mise, "ci-cli");
    let measured = task_run(&mise, "coverage");
    let ci = task_run(&mise, "ci-cli-cov");

    let plain: Vec<&String> = local
        .iter()
        .filter(|c| c.contains("cargo nextest run"))
        .collect();
    assert_eq!(
        plain.len(),
        1,
        "`ci-cli` runs {} plain nextest commands and this test substitutes one: {plain:?}",
        plain.len()
    );
    assert!(
        measured.iter().any(|c| c.contains("llvm-cov")) && measured.len() >= 2,
        "`coverage` no longer looks like an instrumented run and a substitution built from \
         it would put something else in the gate: {measured:?}"
    );

    let expected: Vec<String> = local
        .iter()
        .flat_map(|cmd| {
            if cmd.contains("cargo nextest run") {
                measured.clone()
            } else {
                vec![cmd.clone()]
            }
        })
        .collect();

    assert_eq!(
        ci,
        expected,
        "`ci-cli-cov` is not `ci-cli` with its test run measured.\n\n  ci-cli-cov runs:\n    \
         {}\n\n  ci-cli + coverage is:\n    {}\n\nThe two are one gate spelled twice: `ci-cli` \
         is what a contributor runs before pushing and `ci-cli-cov` is what CI runs, so a \
         difference here is a check one of them is not doing — found, if at all, after a merge.",
        ci.join("\n    "),
        expected.join("\n    ")
    );

    // And the tool that runs it has to be provisioned where it runs.
    let tools = |task: &str| {
        mise.get("tasks")
            .and_then(|t| t.get(task))
            .and_then(|t| t.get("tools"))
            .cloned()
    };
    assert!(
        tools("coverage").is_some(),
        "`coverage` pins no tools, so the equality below would be satisfied by `ci-cli-cov` \
         pinning none either — and the gate would run whatever `cargo llvm-cov` the runner has"
    );
    assert_eq!(
        tools("ci-cli-cov"),
        tools("coverage"),
        "`ci-cli-cov` runs `cargo llvm-cov` and does not pin it the way `coverage` does. \
         mise provisions a task's tools per task, so the gate would fail on `no such command` \
         — or, worse, on whatever version happened to be installed."
    );
}
