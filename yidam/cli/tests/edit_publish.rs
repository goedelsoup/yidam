//! What `edit.yml` hands npm has to be a path npm reads as a path.
//!
//! `edit/v0.1.0` packed a correct tarball, graded it, uploaded it, downloaded it into
//! `dist/`, and then published nothing. The publish step said:
//!
//! ```text
//! npm error command git --no-replace-objects ls-remote ssh://git@github.com/dist/yidam-edit.tgz.git
//! ```
//!
//! npm does not try the filesystem first. `npm-package-arg` classifies the argument, and a
//! bare `a/b` is a **GitHub shorthand** before it is anything else — `dist/yidam-edit.tgz`
//! parsed as the repository `dist/yidam-edit.tgz`, which no key opens. A leading `./` makes
//! it a file spec. The two spellings differ by two characters and by whether a release
//! happens.
//!
//! This is asserted on the workflow rather than on a run of it because the failure is in an
//! argument, and the only environment that evaluates that argument is a tag push — which is
//! the one occasion where finding out afterwards is too late.

use std::path::PathBuf;

fn workflow() -> serde_yaml::Value {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/workflows/edit.yml");
    let text =
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} unreadable: {e}", p.display()));
    serde_yaml::from_str(&text).expect("edit.yml parses")
}

/// Every `npm publish` in the workflow, as the shell will see it.
///
/// Read out of the parsed `run:` scripts with comment lines dropped, so the prose above the
/// command — which quotes both spellings, because explaining the bug requires naming it —
/// cannot answer for the command.
fn publish_commands() -> Vec<String> {
    let w = workflow();
    let jobs = w["jobs"].as_mapping().expect("edit.yml has jobs");
    let mut found = Vec::new();
    for (_, job) in jobs {
        let Some(steps) = job["steps"].as_sequence() else {
            continue;
        };
        for step in steps {
            let Some(script) = step["run"].as_str() else {
                continue;
            };
            for line in script.lines() {
                let line = line.trim();
                if line.starts_with('#') {
                    continue;
                }
                if line.starts_with("npm publish") {
                    found.push(line.to_string());
                }
            }
        }
    }
    found
}

/// The tarball is published by a spec npm reads as a file.
#[test]
fn the_tarball_is_published_as_a_path_and_not_as_a_repository() {
    let commands = publish_commands();
    assert!(
        !commands.is_empty(),
        "edit.yml runs no `npm publish`. npm is this layer's only channel — the workflow \
         creates no GitHub release — so a workflow that does not publish delivers nothing"
    );
    for command in &commands {
        let spec = command
            .split_whitespace()
            .nth(2)
            .unwrap_or_else(|| panic!("`{command}` publishes nothing"));
        assert!(
            spec.starts_with("./") || spec.starts_with("../") || spec.starts_with('/'),
            "edit.yml publishes `{spec}`. npm-package-arg reads a bare `a/b` as a GitHub \
             shorthand, not a path, and resolves it with `git ls-remote` — which is how \
             edit/v0.1.0 exited non-zero next to the very tarball it had just downloaded. \
             Spell it `./{spec}`."
        );
    }
}

/// The file the publish names is the file the download job puts there.
///
/// The two halves are written in different jobs and neither mentions the other. A rename on
/// one side leaves a workflow that packs, uploads, downloads and then publishes a path that
/// is not there — and npm's answer to a path that is not there is not "no such file", it is
/// a git error about a repository nobody has heard of, which is what made this cost an
/// afternoon rather than a minute.
#[test]
fn the_published_path_is_where_the_artifact_is_downloaded() {
    let w = workflow();
    let steps = w["jobs"]["publish"]["steps"]
        .as_sequence()
        .expect("edit.yml has a publish job with steps");
    let dir = steps
        .iter()
        .find(|s| {
            s["uses"]
                .as_str()
                .is_some_and(|u| u.starts_with("actions/download-artifact"))
        })
        .and_then(|s| s["with"]["path"].as_str())
        .expect("the publish job downloads the tarball artifact")
        .trim_end_matches('/')
        .to_string();

    for command in publish_commands() {
        let spec = command.split_whitespace().nth(2).expect("a spec");
        let named = spec.trim_start_matches("./");
        assert!(
            named.starts_with(&format!("{dir}/")),
            "edit.yml publishes `{spec}`, and the artifact is downloaded into `{dir}/`. \
             The publish job has no checkout: nothing else puts a file anywhere, so a spec \
             outside that directory names a file that does not exist"
        );
    }
}
