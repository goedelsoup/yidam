//! `.github/scripts/apt-install.sh` installs what it was asked for, or says why it could not.
//!
//! Three CI and release steps install their packages through it, and nothing else in the
//! repository ran it, so each way it has failed was found by a red job. Two are pinned here.
//!
//! - 2026-11-11: the runner's Ubuntu mirror did not resolve. `apt-get update` exits 0 on that,
//!   `apt-get install` fails, and the script retries against a mirror in another DNS zone.
//! - 2026-11-27 (#1653): `packages.microsoft.com`, a source the runner image adds and that none
//!   of these packages come from, answered 403 on its `InRelease`. apt treats that as a hard
//!   error, so `apt-get update` exited 100, the script never reached `install`, and the retry
//!   failed the same way because it rewrites only the Ubuntu mirror.
//!
//! The script runs here with `sudo` and `apt-get` replaced by stubs first on `PATH`, and its
//! mirror list and sources directory pointed into a scratch directory through the two
//! variables it already reads. The stubs cannot show what real apt does with a 403. They pin
//! what the script does with each exit status apt can give it.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

const FALLBACK: &str = "http://mirrors.edge.kernel.org/ubuntu/";

fn script() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../.github/scripts/apt-install.sh")
}

/// How the stub `apt-get` answers each subcommand.
#[derive(Clone, Copy)]
enum Install {
    Succeeds,
    /// Fails until the mirror list names the fallback, as an unresolvable mirror does.
    SucceedsAfterRewrite,
    Fails,
}

struct Run {
    ok: bool,
    output: String,
    /// The stub `apt-get`'s invocations, one per line: `update` or `install <args>`.
    calls: Vec<String>,
    mirrors: String,
    sources: String,
}

fn write_exe(path: &Path, body: &str) {
    std::fs::write(path, body).expect("a stub writes");
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
        .expect("a stub is made executable");
}

fn run(update_exit: u8, install: Install) -> Run {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let root = dir.path();
    let bin = root.join("bin");
    let sources = root.join("apt");
    std::fs::create_dir_all(&bin).expect("a stub directory");
    std::fs::create_dir_all(sources.join("sources.list.d")).expect("a sources directory");

    let mirrors = sources.join("apt-mirrors.txt");
    std::fs::write(&mirrors, "http://azure.archive.ubuntu.com/ubuntu/\n").expect("mirrors");
    let list = sources.join("sources.list.d/ubuntu.list");
    std::fs::write(
        &list,
        "deb http://azure.archive.ubuntu.com/ubuntu/ noble main\n\
         deb https://packages.microsoft.com/ubuntu/24.04/prod noble main\n",
    )
    .expect("a sources list");
    let calls = root.join("calls.log");

    write_exe(&bin.join("sudo"), "#!/bin/sh\nexec \"$@\"\n");
    let install_rule = match install {
        Install::Succeeds => "exit 0",
        Install::SucceedsAfterRewrite => {
            "grep -qF mirrors.edge.kernel.org \"$APT_MIRRORS_FILE\" && exit 0; \
             echo 'E: Unable to locate package' >&2; exit 100"
        }
        Install::Fails => "echo 'E: Unable to locate package' >&2; exit 100",
    };
    write_exe(
        &bin.join("apt-get"),
        &format!(
            "#!/bin/sh\n\
             echo \"$*\" >> \"$STUB_CALLS\"\n\
             case \"$1\" in\n\
               update)\n\
                 if [ {update_exit} -ne 0 ]; then\n\
                   echo 'E: Failed to fetch https://packages.microsoft.com/ubuntu/24.04/prod/dists/noble/InRelease  403  Forbidden' >&2\n\
                 fi\n\
                 exit {update_exit} ;;\n\
               install) {install_rule} ;;\n\
             esac\n\
             exit 1\n"
        ),
    );

    let path = format!(
        "{}:{}",
        bin.display(),
        std::env::var("PATH").unwrap_or_default()
    );
    let out = Command::new("bash")
        .arg(script())
        .args(["--no-install-recommends", "librsvg2-bin"])
        .env("PATH", path)
        .env("APT_MIRRORS_FILE", &mirrors)
        .env("APT_SOURCES_DIR", &sources)
        .env("STUB_CALLS", &calls)
        .output()
        .expect("bash runs apt-install.sh");
    let mut output = String::from_utf8_lossy(&out.stdout).into_owned();
    output.push_str(&String::from_utf8_lossy(&out.stderr));
    Run {
        ok: out.status.success(),
        output,
        calls: std::fs::read_to_string(&calls)
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect(),
        mirrors: std::fs::read_to_string(&mirrors).expect("the mirror list reads"),
        sources: std::fs::read_to_string(&list).expect("the sources list reads"),
    }
}

/// #1653: a source the install does not need refuses `update`, and the install still runs.
///
/// The packages are judged by `apt-get install`, which fails on its own if the indexes it
/// needs are missing. `update`'s exit status is logged as a warning and decides nothing.
#[test]
fn a_failing_update_does_not_stop_the_install() {
    let r = run(100, Install::Succeeds);
    assert!(
        r.ok,
        "apt-get update exited 100 and apt-get install would have succeeded, but the \
         script failed without trying it. calls: {:?}\n{}",
        r.calls, r.output
    );
    assert_eq!(
        r.calls,
        ["update", "install -y --no-install-recommends librsvg2-bin"],
        "one update and one install, with the caller's flags passed through"
    );
    assert!(
        r.output.contains("::warning::"),
        "a failed update must still be visible in the job log: {}",
        r.output
    );
    assert!(
        !r.mirrors.contains(FALLBACK),
        "the install succeeded first time, so the mirror list is left alone: {}",
        r.mirrors
    );
}

/// 2026-11-11: the runner's mirror does not resolve, `update` exits 0, `install` fails, and the
/// second attempt against the fallback mirror succeeds.
#[test]
fn a_failing_install_retries_against_the_fallback_mirror() {
    let r = run(0, Install::SucceedsAfterRewrite);
    assert!(
        r.ok,
        "the retry against the fallback did not succeed: {:?}\n{}",
        r.calls, r.output
    );
    assert_eq!(
        r.calls.iter().filter(|c| c.starts_with("install")).count(),
        2,
        "one install against the runner's mirror and one against the fallback: {:?}",
        r.calls
    );
    assert!(
        r.mirrors.starts_with(FALLBACK),
        "the mirror list must name the fallback first: {}",
        r.mirrors
    );
    assert!(
        r.sources.contains(&format!("deb {FALLBACK} noble main"))
            && !r.sources.contains("azure.archive.ubuntu.com"),
        "a sources line naming the Azure host directly must be rewritten: {}",
        r.sources
    );
    assert!(
        r.sources.contains("packages.microsoft.com"),
        "only the Ubuntu mirror is rewritten: {}",
        r.sources
    );
}

/// Both failure shapes at once: `update` refuses on every attempt, and only the fallback has
/// the packages. The retry still has to reach `install`.
#[test]
fn a_failing_update_does_not_stop_the_retry() {
    let r = run(100, Install::SucceedsAfterRewrite);
    assert!(
        r.ok,
        "the retry against the fallback must run install even when update fails again: \
         {:?}\n{}",
        r.calls, r.output
    );
}

/// An install that fails against both mirrors fails the step, with apt's exit status.
#[test]
fn an_install_that_fails_twice_fails_the_step() {
    for update_exit in [0, 100] {
        let r = run(update_exit, Install::Fails);
        assert!(
            !r.ok,
            "the install failed on both attempts (update exit {update_exit}) and the script \
             reported success: {:?}\n{}",
            r.calls, r.output
        );
        assert_eq!(
            r.calls.iter().filter(|c| c.starts_with("install")).count(),
            2,
            "exactly one retry (update exit {update_exit}): {:?}",
            r.calls
        );
    }
}
