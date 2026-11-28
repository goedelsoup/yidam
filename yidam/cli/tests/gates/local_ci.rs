//! `mise run ci` is what a pull request's CI runs (#1640).
//!
//! It was three tasks, `ci-harness`, `ci-cli` and `ci-domains`, while a pull request ran
//! fifteen jobs. A contributor who ran it green could push into a red `editor-web`,
//! `parity` or `supply-chain` job that `mise run ci` had never run. The list was written once,
//! when there were three jobs, and nothing compared it with the workflow after that.
//!
//! This gate makes that comparison from the two files. It finds which jobs in
//! `.github/workflows/ci.yml` run on a pull request and which `mise run` tasks each calls.
//! Every one of those tasks must be reached from `[tasks.ci]`, have a named local counterpart,
//! or be excluded below with a reason. The aggregate must not name a task that does not exist.
//! An exclusion that stops being true fails the gate too, so the table cannot keep entries
//! after their reason is gone. Nothing here counts jobs, because the count is the thing that
//! drifted.
//!
//! Reaching a task is not enough when CI sets variables around it. `ci (vscode)` runs
//! `ci-vscode` with `YIDAM_REQUIRE_CONTRACT` set, so a missing binary fails there. Run bare,
//! the same task skips those tests. So the variables a pull-request job sets around a
//! `mise run` must also be set wherever `mise run ci` reaches that task.
//!
//! The second half is the feature list without `vector-read`. It used to be typed out on four
//! mise.toml command lines, in ci.yml and in the install guide, and the copies could disagree.
//! mise.toml states it once, as `[env] CLI_FEATURES_WITHOUT_VECTOR_READ`, which the shell
//! expands on each command line. The two copies mise cannot reach are held equal to it here,
//! and it is held to Cargo.toml.

use std::collections::{BTreeMap, BTreeSet};

use crate::common::repo_root;

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()))
}

fn mise() -> toml::Table {
    read("mise.toml").parse().expect("mise.toml is not TOML")
}

fn workflow() -> serde_yaml::Value {
    serde_yaml::from_str(&read(".github/workflows/ci.yml")).expect("ci.yml is not YAML")
}

/// The mise `[env]` variable holding the CLI's default features less `vector-read`.
const FEATURES_VAR: &str = "CLI_FEATURES_WITHOUT_VECTOR_READ";

/// Pull-request jobs with no counterpart in `mise run ci`, and why each has none.
///
/// Each reason names something the job needs that `mise install` does not provide. If mise
/// later provides it, the job belongs in the aggregate, and this entry should go.
const EXCLUDED_JOBS: &[(&str, &str)] = &[
    (
        "verify",
        "needs `lake` from elan, which lean-action installs in CI and mise does not \
         provision, plus a Dafny install",
    ),
    (
        "semver",
        "compares against the published crate on crates.io with a `+stable` toolchain \
         that mise.toml does not pin",
    ),
    (
        "receipt-compat",
        "downloads the last release's binary to replay its receipts",
    ),
    (
        "cross",
        "needs an apt-installed aarch64 cross-linker and a Linux target",
    ),
    (
        "quality",
        "merges the other jobs' uploaded artifacts through the GitHub API; there is \
         nothing to merge locally",
    ),
];

/// Tasks a pull-request job runs that the aggregate leaves out, inside a job it otherwise
/// covers.
const EXCLUDED_TASKS: &[(&str, &str)] = &[(
    "mark-guard",
    "needs `rsvg-convert`, a system package the `design` job installs with \
     apt-install.sh; mise does not provision it",
)];

/// A task CI runs in place of a local one, and what holds the two equal.
const COUNTERPARTS: &[(&str, &str, &str)] = &[(
    "ci-cli-cov",
    "ci-cli",
    "the same gate with its suite run once under coverage; coverage_reporting.rs \
     `the_instrumented_gate_is_the_local_gate_with_its_suite_measured` holds them equal",
)];

/// Variables a pull-request job sets that `mise run ci` need not set, and why.
///
/// Any other variable a workflow, job or step sets around a `mise run <task>` is part of what
/// that task checks in CI, so the aggregate must set it on the way to the task.
const RUNNER_ENV: &[(&str, &str)] = &[(
    "CARGO_BUILD_JOBS",
    "a runner memory cap that also keys rust-cache; it changes how many rustc processes run \
     at once, not what is built or tested",
)];

// ── the workflow side ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
enum Val {
    Str(String),
    Bool(bool),
}

impl Val {
    fn truthy(&self) -> bool {
        match self {
            Self::Str(s) => !s.is_empty(),
            Self::Bool(b) => *b,
        }
    }
}

/// An expression as a pull request sees it. Only the names a pull request settles are
/// known. Any other name panics, because guessing a value would decide the job set
/// silently.
fn atom(name: &str) -> Val {
    match name {
        "github.event_name" => Val::Str("pull_request".into()),
        "github.ref" => Val::Str("refs/pull/0/merge".into()),
        "always()" => Val::Bool(true),
        "true" => Val::Bool(true),
        "false" => Val::Bool(false),
        // `workflow_dispatch` inputs are absent on a pull request.
        n if n.starts_with("inputs.") => Val::Str(String::new()),
        // Only reached in an `always()` condition, after the job itself is known to run;
        // a pull request's run is assumed green, which is the run a contributor is checking.
        n if n.starts_with("needs.") && n.ends_with(".result") => Val::Str("success".into()),
        other => panic!(
            "a job condition in ci.yml reads `{other}`, and this gate does not know its \
             value on a pull request. Add it to `atom` in local_ci.rs rather than letting \
             the job set be guessed."
        ),
    }
}

fn tokens(expr: &str) -> Vec<String> {
    let mut out = Vec::new();
    let chars: Vec<char> = expr.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c.is_whitespace() {
            i += 1;
        } else if c == '\'' {
            let start = i + 1;
            i = start;
            while i < chars.len() && chars[i] != '\'' {
                i += 1;
            }
            out.push(format!("'{}", chars[start..i].iter().collect::<String>()));
            i += 1;
        } else if "&|=!".contains(c) && i + 1 < chars.len() && "&|=".contains(chars[i + 1]) {
            out.push(chars[i..i + 2].iter().collect());
            i += 2;
        } else if c == '!' || c == '(' || c == ')' {
            out.push(c.to_string());
            i += 1;
        } else {
            // A name: `github.event_name`, `inputs.mutants`, or a call with no arguments such
            // as `always()`, whose `()` belongs to the name rather than opening a group.
            let start = i;
            while let Some(&ch) = chars.get(i) {
                if ch.is_alphanumeric() || "._-".contains(ch) {
                    i += 1;
                } else if ch == '(' && chars.get(i + 1) == Some(&')') {
                    i += 2;
                } else {
                    break;
                }
            }
            assert!(
                i > start,
                "a ci.yml condition has `{c}` where a name was expected"
            );
            out.push(chars[start..i].iter().collect());
        }
    }
    out
}

/// Recursive descent over `||`, `&&`, `!`, `==`, `!=` and parentheses, the operators ci.yml's
/// conditions use. Another operator panics in `atom`, never as a silent `false`.
struct Expr {
    toks: Vec<String>,
    at: usize,
}

impl Expr {
    fn peek(&self) -> Option<&str> {
        self.toks.get(self.at).map(String::as_str)
    }
    fn next(&mut self) -> String {
        let t = self.toks[self.at].clone();
        self.at += 1;
        t
    }
    fn or(&mut self) -> Val {
        let mut v = self.and();
        while self.peek() == Some("||") {
            self.next();
            let r = self.and();
            v = if v.truthy() { v } else { r };
        }
        v
    }
    fn and(&mut self) -> Val {
        let mut v = self.cmp();
        while self.peek() == Some("&&") {
            self.next();
            let r = self.cmp();
            v = if v.truthy() { r } else { v };
        }
        v
    }
    fn cmp(&mut self) -> Val {
        let l = self.unary();
        match self.peek() {
            Some("==") => {
                self.next();
                Val::Bool(l == self.unary())
            }
            Some("!=") => {
                self.next();
                Val::Bool(l != self.unary())
            }
            _ => l,
        }
    }
    fn unary(&mut self) -> Val {
        match self.peek() {
            Some("!") => {
                self.next();
                Val::Bool(!self.unary().truthy())
            }
            Some("(") => {
                self.next();
                let v = self.or();
                assert_eq!(
                    self.next(),
                    ")",
                    "unbalanced parentheses in a ci.yml condition"
                );
                v
            }
            _ => {
                let t = self.next();
                match t.strip_prefix('\'') {
                    Some(s) => Val::Str(s.to_string()),
                    None => atom(&t),
                }
            }
        }
    }
}

fn evaluate(condition: &str) -> bool {
    let inner = condition
        .trim()
        .strip_prefix("${{")
        .and_then(|s| s.strip_suffix("}}"))
        .unwrap_or(condition);
    let mut e = Expr {
        toks: tokens(inner),
        at: 0,
    };
    let v = e.or();
    assert!(
        e.at == e.toks.len(),
        "a ci.yml condition did not parse to its end: `{condition}`"
    );
    v.truthy()
}

/// The jobs of ci.yml, by id.
fn jobs(wf: &serde_yaml::Value) -> BTreeMap<String, serde_yaml::Value> {
    wf.get("jobs")
        .and_then(serde_yaml::Value::as_mapping)
        .expect("ci.yml has no jobs")
        .iter()
        .map(|(k, v)| (k.as_str().expect("job id").to_string(), v.clone()))
        .collect()
}

fn needs(job: &serde_yaml::Value) -> Vec<String> {
    match job.get("needs") {
        Some(serde_yaml::Value::String(s)) => vec![s.clone()],
        Some(serde_yaml::Value::Sequence(v)) => v
            .iter()
            .filter_map(|n| n.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// The ids of the jobs a pull request runs.
///
/// A job without `always()` in its condition is skipped when a job it needs is skipped, so
/// that is followed through `needs`.
fn pull_request_jobs(wf: &serde_yaml::Value) -> BTreeSet<String> {
    let all = jobs(wf);
    fn runs(
        id: &str,
        all: &BTreeMap<String, serde_yaml::Value>,
        memo: &mut BTreeMap<String, bool>,
    ) -> bool {
        if let Some(r) = memo.get(id) {
            return *r;
        }
        let job = all
            .get(id)
            .unwrap_or_else(|| panic!("a ci.yml job needs `{id}`, which is not a job"));
        let condition = match job.get("if") {
            Some(serde_yaml::Value::String(s)) => s.clone(),
            Some(serde_yaml::Value::Bool(b)) => b.to_string(),
            None => "true".to_string(),
            Some(other) => panic!("`{id}` has a non-string `if`: {other:?}"),
        };
        let mut r = evaluate(&condition);
        if r && !condition.contains("always()") {
            r = needs(job).iter().all(|n| runs(n, all, memo));
        }
        memo.insert(id.to_string(), r);
        r
    }
    let mut memo = BTreeMap::new();
    all.keys()
        .filter(|id| runs(id, &all, &mut memo))
        .cloned()
        .collect()
}

/// The values one `${{ matrix.KEY }}` takes in a job.
fn matrix_values(job: &serde_yaml::Value, key: &str) -> Vec<String> {
    let matrix = job.get("strategy").and_then(|s| s.get("matrix"));
    let mut out: Vec<String> = Vec::new();
    if let Some(list) = matrix
        .and_then(|m| m.get(key))
        .and_then(serde_yaml::Value::as_sequence)
    {
        out.extend(list.iter().filter_map(|v| v.as_str().map(str::to_string)));
    }
    if let Some(rows) = matrix
        .and_then(|m| m.get("include"))
        .and_then(serde_yaml::Value::as_sequence)
    {
        out.extend(
            rows.iter()
                .filter_map(|r| r.get(key).and_then(|v| v.as_str()).map(str::to_string)),
        );
    }
    out
}

/// The task names after `mise run` in one shell line.
fn mise_run_tasks(line: &str) -> Vec<String> {
    line.match_indices("mise run ")
        .filter_map(|(i, _)| {
            line[i + "mise run ".len()..]
                .split_whitespace()
                .next()
                .map(str::to_string)
        })
        .collect()
}

/// The mise tasks a job's steps run, with its matrix expanded.
fn job_tasks(id: &str, job: &serde_yaml::Value) -> Vec<String> {
    job.get("steps")
        .and_then(serde_yaml::Value::as_sequence)
        .into_iter()
        .flatten()
        .flat_map(|step| step_tasks(id, job, step))
        .collect()
}

/// The mise tasks one step of a job runs, with the job's matrix expanded.
fn step_tasks(id: &str, job: &serde_yaml::Value, step: &serde_yaml::Value) -> Vec<String> {
    let mut out = Vec::new();
    let Some(run) = step.get("run").and_then(serde_yaml::Value::as_str) else {
        return out;
    };
    for line in run.lines() {
        let line = line.split(" #").next().unwrap_or_default();
        for task in mise_run_tasks(line) {
            if task == "${{" {
                let key = line
                    .split("${{")
                    .nth(1)
                    .and_then(|s| s.split("}}").next())
                    .map(str::trim)
                    .and_then(|s| s.strip_prefix("matrix."))
                    .unwrap_or_else(|| {
                        panic!("`{id}` runs `{line}`, a task name this gate cannot expand")
                    });
                let values = matrix_values(job, key);
                assert!(
                    !values.is_empty(),
                    "`{id}` runs `mise run ${{{{ matrix.{key} }}}}` and its matrix has no `{key}`"
                );
                out.extend(values);
            } else {
                assert!(
                    !task.contains("${{"),
                    "`{id}` runs `mise run {task}`, a task name this gate cannot expand"
                );
                out.push(task);
            }
        }
    }
    out
}

// ── the mise side ────────────────────────────────────────────────────────────

fn tasks(mise: &toml::Table) -> &toml::Table {
    mise.get("tasks")
        .and_then(toml::Value::as_table)
        .expect("mise.toml declares no [tasks]")
}

fn string_list(v: Option<&toml::Value>) -> Vec<String> {
    match v {
        Some(toml::Value::String(s)) => vec![s.clone()],
        Some(toml::Value::Array(a)) => a
            .iter()
            .filter_map(|x| x.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    }
}

/// Every task `root` runs: its `depends`, and every `mise run` its commands make, followed
/// through. A name that is not a task is returned in the second set rather than skipped.
fn reached(mise: &toml::Table, root: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let tasks = tasks(mise);
    let mut seen = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut todo = vec![root.to_string()];
    while let Some(name) = todo.pop() {
        if !seen.insert(name.clone()) {
            continue;
        }
        let Some(task) = tasks.get(&name) else {
            seen.remove(&name);
            missing.insert(name);
            continue;
        };
        todo.extend(string_list(task.get("depends")));
        for command in string_list(task.get("run")) {
            for line in command.lines() {
                todo.extend(mise_run_tasks(line));
            }
        }
    }
    (seen, missing)
}

/// The keys of a workflow `env:` mapping.
fn yaml_env_keys(v: Option<&serde_yaml::Value>) -> BTreeSet<String> {
    v.and_then(serde_yaml::Value::as_mapping)
        .into_iter()
        .flatten()
        .filter_map(|(k, _)| k.as_str().map(str::to_string))
        .collect()
}

/// Each `(job, task, variables)` where a pull-request job runs `mise run <task>` with
/// variables set around it, by the workflow, the job or the step, less `RUNNER_ENV`.
fn pull_request_task_env(wf: &serde_yaml::Value) -> Vec<(String, String, BTreeSet<String>)> {
    let all = jobs(wf);
    let workflow_env = yaml_env_keys(wf.get("env"));
    let mut out = Vec::new();
    for id in pull_request_jobs(wf) {
        let job = &all[&id];
        let job_env = yaml_env_keys(job.get("env"));
        for step in job
            .get("steps")
            .and_then(serde_yaml::Value::as_sequence)
            .into_iter()
            .flatten()
        {
            let vars: BTreeSet<String> = workflow_env
                .iter()
                .chain(&job_env)
                .chain(&yaml_env_keys(step.get("env")))
                .filter(|v| !RUNNER_ENV.iter().any(|(r, _)| r == v))
                .cloned()
                .collect();
            if vars.is_empty() {
                continue;
            }
            for task in step_tasks(&id, job, step) {
                out.push((id.clone(), task, vars.clone()));
            }
        }
    }
    out
}

/// The keys of a mise task's own `env` table.
fn task_env_keys(task: &toml::Value) -> BTreeSet<String> {
    task.get("env")
        .and_then(toml::Value::as_table)
        .map(|t| t.keys().cloned().collect())
        .unwrap_or_default()
}

/// The variable a shell word assigns, if it is `NAME=value`.
fn assignment(word: &str) -> Option<String> {
    let (name, _) = word.split_once('=')?;
    (!name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_'))
    .then(|| name.to_string())
}

/// The variables a shell line assigns in front of its `mise run <task>`.
fn assigned_before(line: &str, task: &str) -> BTreeSet<String> {
    line.find(&format!("mise run {task}"))
        .map(|at| {
            line[..at]
                .split_whitespace()
                .filter_map(assignment)
                .collect()
        })
        .unwrap_or_default()
}

/// Every way `root` arrives at a task, with the variables set at that point. A `depends`
/// edge keeps the caller's environment; a `mise run` line adds what it assigns in front of
/// the command or `export`s on an earlier line of the same script; a task's own `env` table
/// adds its keys. Each `(caller, task, variables)` is returned once.
fn arrivals(mise: &toml::Table, root: &str) -> Vec<(String, String, BTreeSet<String>)> {
    let tasks = tasks(mise);
    let mut seen: BTreeSet<(String, BTreeSet<String>)> = BTreeSet::new();
    let mut out = Vec::new();
    let mut todo = vec![("(root)".to_string(), root.to_string(), BTreeSet::new())];
    while let Some((caller, name, inherited)) = todo.pop() {
        let Some(task) = tasks.get(&name) else {
            continue;
        };
        let mut env: BTreeSet<String> = inherited;
        env.extend(task_env_keys(task));
        out.push((caller, name.clone(), env.clone()));
        if !seen.insert((name.clone(), env.clone())) {
            continue;
        }
        for dep in string_list(task.get("depends")) {
            todo.push((name.clone(), dep, env.clone()));
        }
        for command in string_list(task.get("run")) {
            let mut exported = BTreeSet::new();
            for line in command.lines() {
                for child in mise_run_tasks(line) {
                    let mut child_env = env.clone();
                    child_env.extend(exported.iter().cloned());
                    child_env.extend(assigned_before(line, &child));
                    todo.push((name.clone(), child, child_env));
                }
                if let Some(rest) = line.trim_start().strip_prefix("export ") {
                    exported.extend(rest.split_whitespace().filter_map(assignment));
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// The string values of mise.toml's top-level `[env]` table.
fn mise_env() -> BTreeMap<String, String> {
    mise()
        .get("env")
        .and_then(toml::Value::as_table)
        .map(|t| {
            t.iter()
                .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                .collect()
        })
        .unwrap_or_default()
}

/// A mise.toml command line with each `$NAME` or `${NAME}` that names a mise.toml `[env]`
/// variable replaced by its value, as the shell expands it before cargo sees the line.
/// Gates that compare command lines read what cargo is given, so they go through this.
/// Other `$` words (`$MISE_PROJECT_ROOT`, `$1`) are left as they are.
pub(crate) fn render_mise_env(line: &str) -> String {
    if !line.contains('$') {
        return line.to_string();
    }
    let env = mise_env();
    let mut out = String::new();
    let mut rest = line;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let (name, used) = after.strip_prefix('{').map_or_else(
            || {
                let end = after
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(after.len());
                (&after[..end], end)
            },
            |braced| {
                braced
                    .find('}')
                    .map_or(("", 0), |end| (&braced[..end], end + 2))
            },
        );
        match env.get(name) {
            Some(value) if !name.is_empty() => {
                out.push_str(value);
                rest = &after[used..];
            }
            _ => {
                out.push('$');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn set(list: &str) -> BTreeSet<String> {
    list.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Every value given to `--features` in a text.
fn feature_values(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut words = line.split_whitespace();
        while let Some(w) = words.next() {
            if w == "--features" {
                if let Some(v) = words.next() {
                    out.push(v.trim_matches(|c| c == '"' || c == '\\').to_string());
                }
            } else if let Some(v) = w.strip_prefix("--features=") {
                out.push(v.trim_matches(|c| c == '"' || c == '\\').to_string());
            }
        }
    }
    out
}

// ── the gates ────────────────────────────────────────────────────────────────

/// The evaluator, before it decides anything: a parser that returned `true` for everything
/// would put every job on a pull request and read as a wide, green aggregate.
#[test]
fn the_condition_reader_tells_a_pull_request_from_a_push() {
    assert!(evaluate("github.event_name == 'pull_request'"));
    assert!(!evaluate("github.event_name != 'pull_request'"));
    assert!(!evaluate(
        "github.event_name == 'schedule' || inputs.mutants"
    ));
    assert!(evaluate("always()"));
    assert!(!evaluate(
        "always()\n&& needs.quality.result == 'success'\n&& github.event_name == 'push'\n&& github.ref == 'refs/heads/main'"
    ));
    assert!(evaluate("${{ !(github.event_name == 'push') }}"));
}

/// Every pull-request job is in `mise run ci`, or excluded with a reason.
#[test]
fn mise_run_ci_runs_every_pull_request_job() {
    let wf = workflow();
    assert!(
        wf.get("on").and_then(|on| on.get("pull_request")).is_some(),
        "ci.yml no longer runs on pull_request; this gate's subject is gone"
    );
    let all = jobs(&wf);
    let pr = pull_request_jobs(&wf);
    let mise = mise();
    let (local, missing) = reached(&mise, "ci");

    assert!(
        missing.is_empty(),
        "`mise run ci` names tasks mise.toml does not define: {missing:?}. mise would refuse \
         the whole run, or the name is a typo for a task the aggregate meant to cover."
    );
    assert!(
        local.contains("ci-cli") && local.len() > 3,
        "the aggregate reached only {local:?}; the reader is broken, not the repository"
    );

    let excluded_jobs: BTreeMap<_, _> = EXCLUDED_JOBS.iter().copied().collect();
    let excluded_tasks: BTreeMap<_, _> = EXCLUDED_TASKS.iter().copied().collect();
    let counterpart: BTreeMap<_, _> = COUNTERPARTS.iter().map(|(c, l, _)| (*c, *l)).collect();

    let mut uncovered = Vec::new();
    for id in &pr {
        if excluded_jobs.contains_key(id.as_str()) {
            continue;
        }
        let ran = job_tasks(id, &all[id]);
        if ran.is_empty() {
            uncovered.push(format!(
                "`{id}` runs no `mise run` task, so `mise run ci` cannot reach it"
            ));
        }
        for task in ran {
            let local_name = counterpart.get(task.as_str()).copied().unwrap_or(&task);
            if !local.contains(local_name) && !excluded_tasks.contains_key(task.as_str()) {
                uncovered.push(format!("`{id}` runs `mise run {task}`"));
            }
        }
    }
    assert!(
        uncovered.is_empty(),
        "pull-request jobs that `mise run ci` does not run:\n  {}\n\nAdd the task to \
         `[tasks.ci]`'s `depends` in mise.toml. If mise cannot provide what it needs, add \
         the job to EXCLUDED_JOBS (or the task to EXCLUDED_TASKS) in local_ci.rs and say \
         what it needs.",
        uncovered.join("\n  ")
    );
}

/// Each exclusion and counterpart is still true. Otherwise the table would only grow.
#[test]
fn every_exclusion_still_holds() {
    let wf = workflow();
    let all = jobs(&wf);
    let pr = pull_request_jobs(&wf);
    let mise = mise();
    let (local, _) = reached(&mise, "ci");
    let pr_tasks: BTreeSet<String> = pr.iter().flat_map(|id| job_tasks(id, &all[id])).collect();

    let mut stale = Vec::new();
    for (job, reason) in EXCLUDED_JOBS {
        assert!(
            !reason.trim().is_empty(),
            "`{job}` is excluded with no reason"
        );
        if !pr.contains(*job) {
            stale.push(format!(
                "job `{job}` is excluded and does not run on a pull request"
            ));
            continue;
        }
        let ran = job_tasks(job, &all[*job]);
        if !ran.is_empty() && ran.iter().all(|t| local.contains(t)) {
            stale.push(format!(
                "job `{job}` is excluded and `mise run ci` runs all of it"
            ));
        }
    }
    for (task, reason) in EXCLUDED_TASKS {
        assert!(
            !reason.trim().is_empty(),
            "`{task}` is excluded with no reason"
        );
        if !pr_tasks.contains(*task) {
            stale.push(format!(
                "task `{task}` is excluded and no pull-request job runs it"
            ));
        } else if local.contains(*task) {
            stale.push(format!(
                "task `{task}` is excluded and `mise run ci` runs it"
            ));
        }
    }
    for (ci, here, reason) in COUNTERPARTS {
        assert!(
            !reason.trim().is_empty(),
            "`{ci}` -> `{here}` has no reason"
        );
        if !pr_tasks.contains(*ci) {
            stale.push(format!(
                "`{ci}` has a counterpart and no pull-request job runs it"
            ));
        }
        if !local.contains(*here) {
            stale.push(format!(
                "`{ci}`'s counterpart `{here}` is not in `mise run ci`"
            ));
        }
    }
    assert!(
        stale.is_empty(),
        "exclusions in local_ci.rs that no longer describe the workflow:\n  {}",
        stale.join("\n  ")
    );
}

/// `ci-fast` is a strict part of `ci`, and every task it names exists.
#[test]
fn the_fast_pass_is_part_of_the_full_one() {
    let mise = mise();
    let (full, _) = reached(&mise, "ci");
    let (fast, missing) = reached(&mise, "ci-fast");
    assert!(
        missing.is_empty(),
        "`ci-fast` names tasks mise.toml does not define: {missing:?}"
    );
    let extra: Vec<_> = fast
        .iter()
        .filter(|t| *t != "ci-fast" && !full.contains(*t))
        .collect();
    assert!(
        extra.is_empty(),
        "`ci-fast` runs {extra:?}, which `ci` does not; the fast pass would then check \
         something the full one skips"
    );
    assert!(fast.len() < full.len(), "`ci-fast` is no smaller than `ci`");
}

/// A variable a pull-request job sets around `mise run <task>` is part of what that task
/// checks in CI. `ci (vscode)` sets `YIDAM_REQUIRE_CONTRACT`, and without it a missing or
/// stale `yidam` skips the contract tests and the task goes green. So wherever `mise run ci`
/// reaches such a task, each of those variables is set on the way: by the command line that
/// runs it, an `export` before that line, or a task's `env`. A bare `depends` sets nothing.
#[test]
fn mise_run_ci_sets_what_a_pull_request_step_sets() {
    let wf = workflow();
    let mise = mise();
    let required = pull_request_task_env(&wf);
    assert!(
        !required.is_empty(),
        "no pull-request step sets a variable around `mise run`, so this test reads nothing; \
         if ci.yml really sets none, the test should go"
    );
    // An entry of RUNNER_ENV whose variable no pull-request job sets any more is stale.
    let all = jobs(&wf);
    let mut set_by_pr: BTreeSet<String> = yaml_env_keys(wf.get("env"));
    for id in pull_request_jobs(&wf) {
        set_by_pr.extend(yaml_env_keys(all[&id].get("env")));
        for step in all[&id]
            .get("steps")
            .and_then(serde_yaml::Value::as_sequence)
            .into_iter()
            .flatten()
        {
            set_by_pr.extend(yaml_env_keys(step.get("env")));
        }
    }
    for (var, why) in RUNNER_ENV {
        assert!(
            set_by_pr.contains(*var),
            "RUNNER_ENV excuses `{var}` ({why}), and no pull-request job sets it; remove the entry"
        );
    }

    let arrivals = arrivals(&mise, "ci");
    let mut problems = Vec::new();
    for (job, task, vars) in &required {
        for (caller, _, env) in arrivals.iter().filter(|(_, t, _)| t == task) {
            let unset: Vec<&String> = vars.difference(env).collect();
            if !unset.is_empty() {
                problems.push(format!(
                    "`{caller}` reaches `{task}` without {unset:?}, which `{job}` sets around \
                     `mise run {task}`"
                ));
            }
        }
    }
    assert!(
        problems.is_empty(),
        "`mise run ci` runs a task without the environment CI gives it:\n{}\nSet each one on \
         the command line that runs the task (`NAME=value mise run {{task}}`), or list it in \
         RUNNER_ENV with why it does not change what the task checks.",
        problems.join("\n")
    );
}

/// The feature list without `vector-read` is stated once, and its two out-of-reach copies
/// agree with it.
#[test]
fn the_feature_list_without_vector_read_is_said_once() {
    let env = mise_env();
    let value = env
        .get(FEATURES_VAR)
        .unwrap_or_else(|| panic!("mise.toml's [env] has no `{FEATURES_VAR}`"))
        .clone();

    // Cargo.toml's default, less the one feature the list exists to leave out.
    let cargo: toml::Table = read("yidam/cli/Cargo.toml").parse().expect("Cargo.toml");
    let default: BTreeSet<String> = cargo
        .get("features")
        .and_then(|f| f.get("default"))
        .and_then(toml::Value::as_array)
        .expect("yidam/cli/Cargo.toml has no [features] default")
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    assert!(
        default.contains("vector-read"),
        "the default build no longer has `vector-read`, so a list without it means nothing"
    );
    let expected: BTreeSet<String> = default.into_iter().filter(|f| f != "vector-read").collect();
    assert_eq!(
        set(&value),
        expected,
        "`[env] {FEATURES_VAR}` is not yidam/cli/Cargo.toml's default less `vector-read`"
    );

    // No second spelling in mise.toml's commands: every `--features` naming this set must
    // read the variable. Compared as a set, so a reordered copy is still a copy.
    let raw = read("mise.toml");
    let code: String = raw
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");
    let copies: Vec<String> = feature_values(&code)
        .into_iter()
        .filter(|v| !v.starts_with('$') && set(v) == expected)
        .collect();
    assert!(
        copies.is_empty(),
        "mise.toml spells the list out again: {copies:?}. Write \
         `--features ${FEATURES_VAR}` so the list has one home."
    );
    assert!(
        code.contains(&format!("--features ${FEATURES_VAR}")),
        "no mise.toml command reads `${FEATURES_VAR}`; the variable is held to Cargo.toml \
         and builds nothing"
    );

    // The two copies mise cannot reach: an action input, and a guide's command.
    for (file, why) in [
        (
            ".github/workflows/ci.yml",
            "the `list-features` input of `cli-full`'s summary",
        ),
        (
            "docs/installation.md",
            "the guide's build without ONNX Runtime",
        ),
    ] {
        let found: Vec<String> = feature_values(&read(file))
            .into_iter()
            .filter(|v| set(v).contains("reports") && !set(v).contains("vector-read"))
            .collect();
        assert!(
            !found.is_empty(),
            "{file} has no `--features` list without `vector-read`; {why} was moved or removed"
        );
        for v in found {
            assert_eq!(
                set(&v),
                expected,
                "{file}: {why} is `{v}`, and mise.toml's `{FEATURES_VAR}` is `{value}`"
            );
        }
    }
}
