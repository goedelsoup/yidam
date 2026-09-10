//! A gate must not pass on a repository it cannot see.
//!
//! `repo_root()` falls back to the working directory when `git rev-parse` fails, so every
//! report would run anywhere. For a report that is tolerable — it prints that it found
//! nothing. For `graph-check` and `lint` it was not: from an empty directory that was not
//! even a git repository, `graph-check` printed "No corpus content found" and **exited 0**,
//! and `lint` reported "0 finding(s), no errors".
//!
//! That made a misconfigured repository and a clean one the same observation. A derived
//! repository whose CI ran the gate from the wrong directory — or which never had the
//! infrastructure at all — would go green forever, and nothing anywhere would say so.
//!
//! Each test below asserts on the **exit code**, never on the prose. A gate that says the
//! right words and returns 0 is still broken.

use std::path::Path;
use std::process::Command;

struct Run {
    stdout: String,
    stderr: String,
    code: i32,
}

fn run(dir: &Path, args: &[&str]) -> Run {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap();
    Run {
        stdout: String::from_utf8_lossy(&out.stdout).to_string(),
        stderr: String::from_utf8_lossy(&out.stderr).to_string(),
        code: out.status.code().unwrap_or(-1),
    }
}

/// A directory that is not a git repository at all.
fn bare() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// A git repository that yidam never bootstrapped: no `.yidam/`.
fn plain_git() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let ok = Command::new("git")
        .args(["init", "-q"])
        .current_dir(tmp.path())
        .status()
        .unwrap()
        .success();
    assert!(ok, "git init failed");
    tmp
}

/// A derived repository an hour after genesis: `.yidam/` exists, the corpus is empty.
///
/// This is the case the check must NOT reject, and the reason it tests for `.yidam/`
/// rather than for corpus content. An empty corpus is a legitimate state; an absent
/// `.yidam/` is a repository yidam is not looking at.
fn bootstrapped_but_empty() -> tempfile::TempDir {
    let tmp = plain_git();
    std::fs::create_dir_all(tmp.path().join(".yidam").join("corpus")).unwrap();
    tmp
}

#[test]
fn graph_check_fails_outside_a_git_repository() {
    let d = bare();
    let r = run(d.path(), &["graph-check"]);
    assert_ne!(
        r.code,
        0,
        "graph-check passed outside a repository: {r:?}",
        r = r.stdout
    );
    assert!(
        r.stderr.contains("not a yidam repository"),
        "unhelpful stderr: {}",
        r.stderr
    );
}

#[test]
fn graph_check_fails_in_a_git_repository_yidam_never_bootstrapped() {
    let d = plain_git();
    let r = run(d.path(), &["graph-check"]);
    assert_ne!(
        r.code, 0,
        "graph-check passed on a non-yidam repo: {}",
        r.stdout
    );
    assert!(
        r.stderr.contains(".yidam/"),
        "the message must name what is missing: {}",
        r.stderr
    );
}

#[test]
fn lint_fails_outside_a_yidam_repository() {
    let d = plain_git();
    let r = run(d.path(), &["lint"]);
    assert_ne!(
        r.code, 0,
        "lint reported clean on a non-yidam repo: {}",
        r.stdout
    );
}

/// The exemption is measured, not assumed: assert that the case being excused is real.
///
/// Without this, the check above could be satisfied by rejecting every repository, and a
/// freshly bootstrapped one — which has no nodes yet — would be unable to run its own gate.
#[test]
fn a_bootstrapped_repository_with_an_empty_corpus_still_passes() {
    let d = bootstrapped_but_empty();
    let r = run(d.path(), &["graph-check"]);
    assert_eq!(
        r.code, 0,
        "an empty corpus is a legitimate state and must pass\nstdout: {}\nstderr: {}",
        r.stdout, r.stderr
    );
    let l = run(d.path(), &["lint"]);
    assert_eq!(
        l.code, 0,
        "lint must run in a bootstrapped repo: {}",
        l.stderr
    );
}

/// `--version` must answer, and must name the build — not just the crate version.
///
/// Every correctness story here rests on the binary matching the pin in `.yidam.toml`, and
/// `yidam --version` used to be refused as an unexpected argument. Asserting on the shape
/// keeps it from decaying back into a bare `0.1.0`, which cannot distinguish two builds.
#[test]
fn version_names_the_build_and_its_features() {
    let d = bare();
    let r = run(d.path(), &["--version"]);
    assert_eq!(r.code, 0, "--version failed: {}", r.stderr);
    assert!(
        r.stdout.contains('(') && r.stdout.contains('['),
        "--version must carry the build commit and feature set, got: {}",
        r.stdout.trim()
    );
    assert!(
        r.stdout.contains("reports"),
        "the light default feature must be named: {}",
        r.stdout.trim()
    );
}

// ── #793: a command that writes must not write into a directory that is not a corpus ────

/// Every file under `dir`, excluding `.git/`, so a run's effect on the tree is decidable.
fn files(dir: &Path) -> Vec<String> {
    fn walk(dir: &Path, base: &Path, out: &mut Vec<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let p = e.path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(&p, base, out);
            } else {
                out.push(p.strip_prefix(base).unwrap_or(&p).display().to_string());
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

/// The commands `yidam --help` marks with `*`, which its own legend defines as those that
/// "rewrite files in the repository it is run against".
///
/// **Discovered, not listed.** A hardcoded roster stops covering whatever is added next
/// without ever going red, and the help text is where this repository already declares which
/// commands write — so a new writer joins this population by being documented as one.
fn writing_commands() -> Vec<String> {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .arg("--help")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&out.stdout).to_string();
    let mut names: Vec<String> = help
        .lines()
        .filter_map(|l| {
            let rest = l.strip_prefix("  ")?;
            let (name, after) = rest.split_once(char::is_whitespace)?;
            after
                .trim_start()
                .strip_prefix("* ")
                .map(|_| name.to_string())
        })
        .filter(|n| !n.is_empty() && !n.starts_with('-'))
        .collect();
    names.sort();
    names.dedup();
    names
}

/// The floor under the two tests below. A parser that stops recognising the `*` legend would
/// leave both of them iterating an empty set and passing over nothing.
#[test]
fn the_writing_commands_are_discovered_from_the_help_legend() {
    let names = writing_commands();
    assert!(
        names.len() > 10,
        "only {} writing command(s) found — the `*` legend parser is looking at nothing: {names:?}",
        names.len()
    );
    for expected in ["export", "schema", "bundle", "regen"] {
        assert!(
            names.iter().any(|n| n == expected),
            "`{expected}` writes and must be in the discovered set: {names:?}"
        );
    }
}

/// The defect: `export` and `schema` skipped `require_yidam_repo`, so in a directory that was
/// not a corpus every export format wrote an artefact and exited 0.
///
/// Worse than an empty artefact, `schema` and `export --format bundle`/`--format web` created
/// `.yidam/` on the way — **manufacturing the one marker every gate tests for**, so a single
/// wrong-directory run left a tree that every later check would accept.
///
/// Asserted as "wrote nothing", not as an exit code, because that is the property that was
/// violated and it holds for a command refused on its arguments as much as one refused on its
/// directory.
#[test]
fn no_writing_command_creates_a_file_outside_a_corpus() {
    // Bare, with no extra arguments. An earlier version of this test passed `--format bundle`
    // to every command so that `export` would run, and that made the whole loop vacuous: every
    // other command refused the unknown argument, wrote nothing for that reason, and satisfied
    // the assertion without its gate being exercised at all. Mutating away `schema`'s gate left
    // this test green, which is how the flaw showed. `export` gets its own loop below.
    let mut invocations: Vec<Vec<String>> =
        writing_commands().into_iter().map(|n| vec![n]).collect();
    for format in ["bundle", "rdf", "graphml", "llms", "web"] {
        invocations.push(vec!["export".into(), "--format".into(), format.into()]);
    }

    for (label, dir) in [
        ("no git repository", bare()),
        ("git, no .yidam/", plain_git()),
    ] {
        for args in &invocations {
            let argv: Vec<&str> = args.iter().map(String::as_str).collect();
            let before = files(dir.path());
            let r = run(dir.path(), &argv);
            let after = files(dir.path());
            assert_eq!(
                before,
                after,
                "`yidam {}` wrote into a directory that is not a corpus ({label})\n\
                 exit: {}\nstdout: {}\nstderr: {}",
                args.join(" "),
                r.code,
                r.stdout,
                r.stderr
            );
        }
    }
}

/// The three that were violating it, held to the exit code and to naming the reason. A gate
/// that writes nothing because it crashed is not the same as one that refuses.
///
/// `bundle` is here because it and `export --format bundle` are one operation reached by two
/// names, and they disagreed: `export` wrote an empty bundle and exited 0, while `bundle`
/// exited 1 with `No such file or directory (os error 2)` — the write failing for want of a
/// parent directory, naming neither the path nor the cause.
#[test]
fn export_bundle_and_schema_refuse_outside_a_corpus_and_say_why() {
    let d = plain_git();
    for args in [
        &["export", "--format", "bundle"][..],
        &["export", "--format", "rdf"][..],
        &["export", "--format", "llms"][..],
        &["export", "--format", "web"][..],
        &["export", "--format", "graphml"][..],
        &["bundle"][..],
        &["schema"][..],
    ] {
        let r = run(d.path(), args);
        assert_ne!(
            r.code,
            0,
            "`yidam {}` succeeded outside a corpus\nstdout: {}",
            args.join(" "),
            r.stdout
        );
        assert!(
            r.stderr.contains("not a yidam repository"),
            "`yidam {}` refused without saying why: {}",
            args.join(" "),
            r.stderr
        );
    }
}

/// The other half, without which the gate could be satisfied by refusing everything: a
/// bootstrapped repository with no nodes yet must still be able to export and to compile its
/// schemas. An empty corpus is a legitimate corpus.
#[test]
fn a_bootstrapped_repository_with_an_empty_corpus_can_still_export() {
    let d = bootstrapped_but_empty();
    for args in [&["export", "--format", "llms"][..], &["schema"][..]] {
        let r = run(d.path(), args);
        assert_eq!(
            r.code,
            0,
            "`yidam {}` must run in a bootstrapped repo\nstdout: {}\nstderr: {}",
            args.join(" "),
            r.stdout,
            r.stderr
        );
    }
}

/// `schema --settings` prints a compiled-in editor configuration and reads nothing from disk,
/// so it has no corpus to refuse over. Gating the command rather than its writing branch would
/// have taken that away, which is why the gate sits after the early return.
#[test]
fn schema_settings_still_answers_outside_a_repository() {
    let r = run(bare().path(), &["schema", "--settings"]);
    assert_eq!(r.code, 0, "schema --settings failed: {}", r.stderr);
    assert!(
        r.stdout.trim_start().starts_with('{'),
        "expected a JSON object: {}",
        r.stdout
    );
}
