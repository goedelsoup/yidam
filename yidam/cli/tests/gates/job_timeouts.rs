//! Every CI job carries its own cap (#1740).
//!
//! A job with no `timeout-minutes` runs on GitHub's default of six hours. A hung step then
//! holds a runner for that long, and every gate stays green while it does. #1641 and #1715
//! capped each job in `ci.yml`, and no test required the caps, so a new job, or a deleted
//! line, went back to the default without a sound.
//!
//! The rule is per job, at job level. A step-level `timeout-minutes` caps one step, and the
//! job's other steps still run on the default. A job that calls a reusable workflow with
//! `uses:` is the one exception: GitHub refuses `timeout-minutes` on it, and the jobs of the
//! workflow it calls are checked where they are written.
//!
//! The files are found by walking `.github/workflows/`, so a new workflow is covered with no
//! edit here. This reads this repository's own workflows only. The workflows a derived
//! repository inherits, under `sadhana/github/workflows/`, are not checked: a derived
//! repository's run times are its own, and so is its choice of cap.

use crate::common;

/// Every workflow file in `.github/workflows/`, repository-relative, with its parsed text.
fn workflows() -> Vec<(String, serde_yaml::Value)> {
    let root = common::repo_root();
    let mut out = Vec::new();
    for entry in common::repo_walk(&root.join(".github/workflows")) {
        let path = entry.path();
        let is_yaml = path.extension().is_some_and(|e| e == "yml" || e == "yaml");
        if !entry.file_type().is_file() || !is_yaml {
            continue;
        }
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .display()
            .to_string();
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{rel} is unreadable ({e})"));
        let doc: serde_yaml::Value =
            serde_yaml::from_str(&text).unwrap_or_else(|e| panic!("{rel} does not parse: {e}"));
        out.push((rel, doc));
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(
        out.iter().any(|(rel, _)| rel == ".github/workflows/ci.yml"),
        "the walk of .github/workflows/ did not find ci.yml, so it is reading the wrong tree. \
         Found: {:?}",
        out.iter().map(|(rel, _)| rel).collect::<Vec<_>>()
    );
    out
}

/// What is wrong with one job's cap, or `None` when it has one.
fn missing_cap(job: &serde_yaml::Value) -> Option<String> {
    if job
        .get("uses")
        .and_then(serde_yaml::Value::as_str)
        .is_some()
    {
        return None;
    }
    match job.get("timeout-minutes") {
        None => Some("no job-level `timeout-minutes`".to_string()),
        Some(v) if v.as_u64().is_some_and(|m| m > 0) => None,
        Some(v) => Some(format!(
            "a job-level `timeout-minutes` of {v:?}, which is not a positive whole number"
        )),
    }
}

#[test]
fn every_job_in_every_workflow_has_a_job_level_timeout() {
    let mut jobs = 0;
    let mut uncapped = Vec::new();
    for (rel, doc) in workflows() {
        let Some(map) = doc.get("jobs").and_then(serde_yaml::Value::as_mapping) else {
            panic!("{rel} has no `jobs:` mapping, so this gate read nothing in it");
        };
        assert!(!map.is_empty(), "{rel} declares no jobs");
        for (id, job) in map {
            jobs += 1;
            if let Some(why) = missing_cap(job) {
                uncapped.push(format!(
                    "  {rel}: `{}` has {why}",
                    id.as_str().unwrap_or("?")
                ));
            }
        }
    }
    assert!(jobs > 0, "no jobs were read out of .github/workflows/");
    assert!(
        uncapped.is_empty(),
        "these jobs run on GitHub's six-hour default. Give each a job-level `timeout-minutes` \
         (four spaces in, beside `runs-on:`) of about twice its usual run time. A cap on a step \
         covers that step only.\n{}",
        uncapped.join("\n")
    );
}

/// The rule reads the job's own key, and not a step's.
#[test]
fn a_step_level_cap_does_not_count_and_a_reusable_call_needs_none() {
    let parse = |text: &str| -> serde_yaml::Value {
        serde_yaml::from_str(text).unwrap_or_else(|e| panic!("fixture does not parse: {e}"))
    };
    let capped = parse("runs-on: ubuntu-latest\ntimeout-minutes: 10\nsteps: []\n");
    let on_a_step =
        parse("runs-on: ubuntu-latest\nsteps:\n  - run: 'true'\n    timeout-minutes: 10\n");
    let reusable = parse("uses: ./.github/workflows/tap.yml\n");
    let zero = parse("runs-on: ubuntu-latest\ntimeout-minutes: 0\nsteps: []\n");
    assert_eq!(missing_cap(&capped), None);
    assert!(missing_cap(&on_a_step).is_some());
    assert_eq!(missing_cap(&reusable), None);
    assert!(missing_cap(&zero).is_some());
}
