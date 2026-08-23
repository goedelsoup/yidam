//! The binary says so when it is not the one this repository pins.
//!
//! `.yidam/bin/yidam` is per repository; an installed `yidam` is per machine. The
//! conventions recorded that as an installation hazard — `cargo install` with no `--root`
//! making the last repository to build the winner — and the same hazard arrives through
//! invocation, needing no mis-installation at all. A `yidam` left in `~/.cargo/bin` shadows
//! the pin for any process whose `PATH` puts cargo's directory first, which is the default
//! for a shell that sources a Rust environment.
//!
//! What makes it worth a test rather than a paragraph is how it fails. An older binary
//! lacking a subcommand exits with `unrecognized subcommand`, and inside a script with
//! output redirected that is indistinguishable from success. `yidam regen` silently no-opped
//! twice in one derived repository this way, and neither time was it caught by the thing
//! that failed.
//!
//! These exercise the real binary, because the whole point is the invocation route.

use std::path::Path;
use std::process::Command;

/// A git repository that pins a `yidam` which is not the one under test.
fn repo_pinning_something_else(root: &Path) {
    for args in [
        vec!["init", "-q", "-b", "main"],
        vec!["config", "user.email", "fixture@yidam.test"],
        vec!["config", "user.name", "Fixture"],
    ] {
        assert!(Command::new("git")
            .current_dir(root)
            .args(&args)
            .status()
            .unwrap()
            .success());
    }
    let bin = root.join(".yidam/bin");
    std::fs::create_dir_all(&bin).unwrap();
    // Contents are irrelevant: nothing runs it, it only has to exist and not be us.
    std::fs::write(
        bin.join(format!("yidam{}", std::env::consts::EXE_SUFFIX)),
        b"x",
    )
    .unwrap();
}

fn run(root: &Path, args: &[&str]) -> (String, String, Option<i32>) {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(root)
        .args(args)
        .output()
        .expect("running yidam");
    (
        String::from_utf8_lossy(&out.stdout).to_string(),
        String::from_utf8_lossy(&out.stderr).to_string(),
        out.status.code(),
    )
}

#[test]
fn a_shadowed_pin_is_reported_on_stderr() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());

    let (stdout, stderr, _) = run(tmp.path(), &["status"]);
    assert!(
        stderr.contains("a different one is answering"),
        "expected the shadow warning, got stderr:\n{stderr}"
    );
    assert!(
        stderr.contains(".yidam/bin/yidam"),
        "the warning must name the pin it is not:\n{stderr}"
    );
    // stdout is the report contract. A warning that lands there breaks every `--format
    // json` consumer, which is a worse bug than the one being warned about.
    assert!(
        !stdout.contains("a different one is answering"),
        "the warning must not be on stdout:\n{stdout}"
    );
}

#[test]
fn a_shadowed_pin_does_not_break_the_json_contract() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());

    let (stdout, _, _) = run(tmp.path(), &["status", "--format", "json"]);
    serde_json::from_str::<serde_json::Value>(&stdout)
        .expect("stdout is still parseable JSON while the warning is on stderr");
}

/// A repository that pins nothing is silent — which is every repository that is not a
/// yidam derivation, and yidam itself.
#[test]
fn an_unpinned_repository_is_silent() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());
    std::fs::remove_dir_all(tmp.path().join(".yidam/bin")).unwrap();

    let (_, stderr, _) = run(tmp.path(), &["status"]);
    assert!(
        !stderr.contains("a different one is answering"),
        "nothing is pinned, so nothing is shadowed:\n{stderr}"
    );
}

/// The mutation that makes the warning worth having: the pin *is* the running binary.
///
/// Without canonicalization this passes anyway, because the two paths differ as strings.
#[test]
fn the_pinned_binary_running_itself_is_silent() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());
    let pinned = tmp
        .path()
        .join(".yidam/bin")
        .join(format!("yidam{}", std::env::consts::EXE_SUFFIX));
    std::fs::remove_file(&pinned).unwrap();
    #[cfg(unix)]
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_yidam"), &pinned).unwrap();
    #[cfg(not(unix))]
    std::fs::copy(env!("CARGO_BIN_EXE_yidam"), &pinned).unwrap();

    #[cfg(unix)]
    {
        let (_, stderr, _) = run(tmp.path(), &["status"]);
        assert!(
            !stderr.contains("a different one is answering"),
            "the pin is the binary running; warning here would be noise on every \
             invocation:\n{stderr}"
        );
    }
}

/// The quiet failure, made loud. An unrecognized subcommand names the binary that refused.
#[test]
fn an_unrecognized_subcommand_names_the_binary_that_refused_it() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());

    let (_, stderr, code) = run(tmp.path(), &["definitely-not-a-subcommand"]);
    assert_eq!(code, Some(2), "clap's usage-error exit code is preserved");
    assert!(
        stderr.contains("unrecognized subcommand"),
        "clap's own message is still shown:\n{stderr}"
    );
    assert!(
        stderr.contains("the binary that answered is"),
        "the note must name the executable:\n{stderr}"
    );
    assert!(
        stderr.contains(".yidam/bin/yidam"),
        "and the pin it is not, which is the actual diagnosis:\n{stderr}"
    );
}

/// `--help` and `--version` are not errors and must keep clap's exit code of 0.
#[test]
fn help_is_not_treated_as_a_refusal() {
    let tmp = tempfile::tempdir().unwrap();
    repo_pinning_something_else(tmp.path());

    let (stdout, _, code) = run(tmp.path(), &["--help"]);
    assert_eq!(code, Some(0), "help exits 0");
    // The subcommand listing is `help::render`'s, not clap's flat `Commands:` block, so
    // this asserts on a group heading and a command under it.
    assert!(
        stdout.contains("Checks and gates") && stdout.contains("graph-check"),
        "help still prints the grouped command listing:\n{stdout}"
    );
}
