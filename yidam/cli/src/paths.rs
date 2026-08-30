use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn repo_root() -> Result<PathBuf> {
    let out = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .context("running git rev-parse")?;
    if out.status.success() {
        let s = String::from_utf8(out.stdout).context("git output utf8")?;
        Ok(PathBuf::from(s.trim()))
    } else {
        std::env::current_dir().context("current dir fallback")
    }
}

/// Fail unless the current directory really is a yidam-derived repository.
///
/// [`repo_root`] falls back to the working directory when `git rev-parse` fails, which is
/// what lets every report run somewhere no repository exists. For a *report* that is
/// tolerable — it prints that it found nothing. For a **gate** it is not: run
/// `yidam graph-check` from an empty directory and it prints "No corpus content found"
/// and **exits 0**. A misconfigured repository and a clean one were the same observation,
/// which is the one thing a gate must never allow. A derived repository that has never
/// been able to see itself passes every check it runs, forever, and nothing says so.
///
/// The test is `.yidam/`, not the corpus: a repository bootstrapped an hour ago has the
/// directory and no nodes in it yet, and that is a legitimate empty corpus rather than an
/// absent one. `.yidam/` is written at genesis and is what `repo_root` should have found.
pub fn require_yidam_repo(root: &Path) -> Result<()> {
    if root.join(".yidam").is_dir() {
        return Ok(());
    }

    let in_git = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);

    if in_git {
        anyhow::bail!(
            "not a yidam repository: {} has no .yidam/ directory\n  \
             This is a git repository, but not one yidam bootstrapped. Derive one with \
             `yidam clone <target>`, or overlay this one with `yidam overlay .`.",
            root.display()
        )
    }
    anyhow::bail!(
        "not a yidam repository: {} is not inside a git repository\n  \
         yidam locates a repository with `git rev-parse --show-toplevel` and found none, so \
         it fell back to the working directory. Run this from inside a derived repository.",
        root.display()
    )
}

pub fn yidam_corpus_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("corpus")
}

pub fn yidam_catalog_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("catalog")
}

pub fn yidam_skills_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("skills")
}

pub fn yidam_decisions_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("decisions")
}

/// The sangha's governance records: `PROTOCOL.md`, `electors.md`, `positions/`,
/// `resolutions/`. Absent in single-elector repositories, where collective mode is
/// opt-in — every caller must tolerate it not existing.
pub fn yidam_sangha_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("sangha")
}

pub fn yidam_embeddings_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("embeddings")
}

pub fn yidam_index_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("index")
}

/// The benchmark's fixed inputs — `goals.yml`, and later the scaling generator's config.
///
/// Absent in most repositories, and `bench` says so rather than inventing a goal set: a
/// benchmark that supplies its own goals is measuring whoever wrote the fallback.
pub fn yidam_bench_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("bench")
}

pub fn samudaya_dir(root: &Path) -> PathBuf {
    root.join("samudaya")
}

// Not gated on the `tonpa` feature, and that is the point: the feature buys the
// network — resolving a source, fetching an archive, writing a lock. READING a
// dependency that is already on disk needs none of it, and a derived repository
// installs the light build, so gating these made an installed dependency
// invisible to the only binary that repository has.
pub fn tonpa_dir(root: &Path) -> PathBuf {
    root.join(".yidam").join("tonpa")
}

// Ungated for the same reason as `tonpa_dir`. This was gated one commit ago on the argument
// that only the fetching commands read the declaration — which stopped being true the moment
// a *path* dependency existed, because a path dependency has nothing on disk under
// `.yidam/tonpa/` and the declaration is the only record that it exists at all.
pub fn tonpa_config_path(root: &Path) -> PathBuf {
    root.join(".yidam").join("tonpa.toml")
}

// Third in the same family, ungated for the same reason, and named here rather than spelled
// `dir.join("tonpa.lock")` at each call site — `doctor` is now a second reader of the lock
// and a second spelling of where it lives is how the two would drift apart.
pub fn tonpa_lock_path(root: &Path) -> PathBuf {
    tonpa_dir(root).join("tonpa.lock")
}

// ── the binary that is answering ──────────────────────────────────────────────
//
// The pin is per repository; an installed binary is per machine. `directories.md` records
// that hazard as an *installation* problem — `cargo install` with no `--root` making the
// last repository to build the winner. The same hazard arrives through *invocation* and
// needs no mis-installation at all: a `yidam` left in `~/.cargo/bin` by any earlier install
// shadows `.yidam/bin/yidam` for any process whose `PATH` puts cargo's directory first,
// which is what a shell sourcing a Rust environment does by default.
//
// The derived repository's `mise.toml` puts `.yidam/bin` first for anything mise runs —
// its own config, not the inherited task file, which cannot carry an `[env]` section and
// spent a while proving it. That guards a human shell. It does not guard a script, a CI
// step, or an agent assembling `PATH` itself, and those are increasingly what runs these
// commands.
//
// The failure is quiet in the way that matters. An older binary lacking a subcommand exits
// with `unrecognized subcommand`, and inside a script with output redirected — which is how
// a regen step is usually written — that is indistinguishable from success. It happened
// twice in one derived repository; neither time was it caught by the thing that failed.

/// The binary this repository pins, whether or not it is the one running.
pub fn yidam_bin_path(root: &Path) -> PathBuf {
    root.join(".yidam")
        .join("bin")
        .join(format!("yidam{}", std::env::consts::EXE_SUFFIX))
}

/// Which `yidam` is answering for a repository.
#[derive(Debug, PartialEq, Eq)]
pub enum Pinned {
    /// No `.yidam/bin/yidam`. The repository does not pin a binary, so nothing is shadowed.
    Unpinned,
    /// The running binary is the pinned one.
    Running,
    /// A different binary is answering for this repository.
    Shadowed { pinned: PathBuf, running: PathBuf },
}

/// Compare the running binary against the repository's pin.
///
/// `running` is passed rather than read so the comparison is testable; production callers
/// hand it [`std::env::current_exe`]. Both sides are canonicalized, because the interesting
/// case is two paths that name one file through a symlink and must not be reported as a
/// conflict.
pub fn pinned_binary(root: &Path, running: Option<&Path>) -> Pinned {
    let pinned = yidam_bin_path(root);
    if !pinned.exists() {
        return Pinned::Unpinned;
    }
    let Some(running) = running else {
        return Pinned::Unpinned;
    };
    let real = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    if real(&pinned) == real(running) {
        Pinned::Running
    } else {
        Pinned::Shadowed {
            pinned,
            running: running.to_path_buf(),
        }
    }
}

/// The stderr warning for a shadowed pin, or `None` when nothing is wrong.
///
/// Text, and on stderr, so a `--format json` consumer reading stdout is unaffected.
pub fn shadowed_warning(status: &Pinned) -> Option<String> {
    let Pinned::Shadowed { pinned, running } = status else {
        return None;
    };
    Some(format!(
        "warning: this repository pins a yidam binary, and a different one is answering\n\
         \x20        running: {}\n\
         \x20         pinned: {}\n\
         \x20        Put `.yidam/bin` first on PATH. An older binary missing a subcommand \
         exits with\n\
         \x20        `unrecognized subcommand`, which a script with output redirected \
         cannot tell from success.",
        running.display(),
        pinned.display()
    ))
}

/// Say so on stderr when a binary other than the repository's pin is answering.
///
/// Called once per invocation rather than per command, because the routes that hit this are
/// the ones nobody anticipated. Failures to locate the repository or the running binary are
/// swallowed: this is a warning about a hazard, not a check anything depends on.
pub fn warn_if_shadowed() {
    let Ok(root) = repo_root() else { return };
    let running = std::env::current_exe().ok();
    if let Some(msg) = shadowed_warning(&pinned_binary(&root, running.as_deref())) {
        eprintln!("{msg}");
    }
}

/// Where the binary that just refused a command actually lives.
///
/// clap reports an unrecognized subcommand without saying who did not recognize it, and the
/// answer is often "a binary older than the command you typed, from somewhere you did not
/// mean". Naming the executable turns that into one line.
pub fn running_binary_note() -> String {
    let running = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    let mut note = format!("note: the binary that answered is {running}");
    if let Ok(root) = repo_root() {
        let pinned = yidam_bin_path(&root);
        if pinned.exists() {
            note.push_str(&format!(
                "\nnote: this repository pins {}. If the command exists there and not here, \
                 `.yidam/bin` is not first on PATH.",
                pinned.display()
            ));
        }
    }
    note
}

#[cfg(test)]
mod pinned_tests {
    use super::*;
    use tempfile::TempDir;

    fn repo_with_pin(tmp: &TempDir) -> PathBuf {
        let bin = yidam_bin_path(tmp.path());
        std::fs::create_dir_all(bin.parent().unwrap()).unwrap();
        std::fs::write(&bin, b"#!/bin/sh\n").unwrap();
        bin
    }

    /// A repository that pins nothing cannot be shadowed, and must not be warned at.
    /// This is every repository that is not a yidam derivation, including yidam itself.
    #[test]
    fn a_repository_without_a_pin_is_never_shadowed() {
        let tmp = TempDir::new().unwrap();
        let elsewhere = tmp.path().join("cargo/bin/yidam");
        assert_eq!(
            pinned_binary(tmp.path(), Some(&elsewhere)),
            Pinned::Unpinned
        );
        assert_eq!(shadowed_warning(&Pinned::Unpinned), None);
    }

    #[test]
    fn the_pinned_binary_running_itself_is_silent() {
        let tmp = TempDir::new().unwrap();
        let bin = repo_with_pin(&tmp);
        assert_eq!(pinned_binary(tmp.path(), Some(&bin)), Pinned::Running);
        assert_eq!(shadowed_warning(&Pinned::Running), None);
    }

    /// The reported case: a `yidam` from `~/.cargo/bin` answering for a repository that
    /// pins its own. Both paths are named, because the useful part of the warning is which
    /// two files disagree.
    #[test]
    fn a_binary_from_elsewhere_is_reported_with_both_paths() {
        let tmp = TempDir::new().unwrap();
        let pinned = repo_with_pin(&tmp);
        let stale = tmp.path().join("home/.cargo/bin/yidam");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, b"#!/bin/sh\n").unwrap();

        let status = pinned_binary(tmp.path(), Some(&stale));
        assert_eq!(
            status,
            Pinned::Shadowed {
                pinned: pinned.clone(),
                running: stale.clone(),
            }
        );
        let msg = shadowed_warning(&status).expect("a shadowed pin warns");
        assert!(msg.contains(&stale.display().to_string()), "{msg}");
        assert!(msg.contains(&pinned.display().to_string()), "{msg}");
        // The quiet half is the point: say why a silent no-op is the shape to expect.
        assert!(msg.contains("unrecognized subcommand"), "{msg}");
    }

    /// One file reached by two paths is not two binaries. Without this, a repository whose
    /// `.yidam/bin/yidam` is a symlink to the build output warns on every invocation, and a
    /// warning that is always wrong is one nobody reads when it is right.
    #[test]
    fn a_symlink_to_the_pinned_binary_is_the_pinned_binary() {
        let tmp = TempDir::new().unwrap();
        let bin = repo_with_pin(&tmp);
        let link = tmp.path().join("link-to-yidam");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&bin, &link).unwrap();
        #[cfg(not(unix))]
        std::fs::copy(&bin, &link).unwrap();
        #[cfg(unix)]
        assert_eq!(pinned_binary(tmp.path(), Some(&link)), Pinned::Running);
    }

    /// Not knowing what is running is not evidence that it is wrong.
    #[test]
    fn an_unknown_running_binary_does_not_warn() {
        let tmp = TempDir::new().unwrap();
        repo_with_pin(&tmp);
        assert_eq!(pinned_binary(tmp.path(), None), Pinned::Unpinned);
    }
}
