//! The one place that spawns `git`.
//!
//! #929: fifty production invocations stood across twenty-six files, and behind them eight
//! hand-written runners giving four different answers to *what does failure mean* — `None`,
//! `Err` carrying stderr, `Option<Vec<String>>`, and twice the empty string. Which directory
//! to ask about, what to do with the environment, and whether a revision could be read as an
//! option were decided again at every site.
//!
//! The cost of deciding a thing in fifty places is that a decision made once is made in one
//! of them. Each of the three below was a defence that existed somewhere and was missing
//! everywhere else; each is now a property of the invocation rather than of the call site,
//! and [`tests/git_hygiene.rs`] holds all three from outside the crate.
//!
//! [`tests/git_hygiene.rs`]: https://github.com/goedelsoup/yidam/blob/main/yidam/cli/tests/git_hygiene.rs

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};

use anyhow::{bail, Context, Result};

/// Variables that move the repository out from under `-C`.
///
/// **`GIT_DIR` in the environment wins over both `-C <path>` and `current_dir`.** Measured
/// on git 2.50.1, in one directory, with only the environment changing:
///
/// ```text
/// $ git -C inner log -1 --format=%s                       inner
/// $ GIT_DIR=outer/.git git -C inner log -1 --format=%s    outer
/// ```
///
/// Anything that exports these puts yidam in that position — a hook, `git rebase -x`,
/// `git bisect run`, a wrapper script. What made it worse than a wrong answer is that
/// `rev-parse --show-toplevel` keeps answering about the *working directory*, so the
/// repository yidam says it is describing and the repository it reads are different ones,
/// with nothing in the output to say so.
///
/// Removed on every invocation. A caller that means to set one says so through
/// [`Git::env`], which is applied afterwards — `propose` writes through its own
/// `GIT_INDEX_FILE` and still does.
const INHERITED_LOCATION: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
];

/// Configuration this crate needs to be true of every invocation, whatever the user's own
/// config says.
///
/// **`core.quotepath`** defaults to `true`, which renders a path holding any byte over 0x7f
/// as a quoted, backslash-escaped string: `".yidam/corpus/concept/caf\303\251.yml"`. Three
/// readers parse paths out of git — `diff --name-status`, `log --name-only`, `ls-tree`
/// without `-z` — and the quoted form does not match the prefix any of them tests for. The
/// effect on `yidam diff` was not a mangled name but a **dropped change**: git reported the
/// node as modified, the report said there was nothing to report.
///
/// **`log.showSignature`**, when a user has set it, prepends `Good "git" signature with …`
/// to the output of anything that prints a commit — ahead of the `--format` the caller
/// asked for, so the first field of the first record is somebody else's prose. `query/at.rs`
/// found this and defended its own three invocations; eight other `git log` parsers had no
/// defence. It does **not** suppress `%G?` or `%GS`, which are format placeholders and
/// which `lint/attest.rs` reads — that is a separate mechanism and it is unaffected.
const PINNED_CONFIG: &[(&str, &str)] =
    &[("core.quotepath", "false"), ("log.showSignature", "false")];

/// The separator after which git reads no more options.
///
/// A revision is a string from outside the program — a `--range` argument, a ref name out of
/// a file — and git's own argument parser cannot tell one from a flag. `yidam query --at`
/// was given `--output=<path>`, git's `rev-list` read it as an option, and a read-only query
/// truncated the file it named. That was fixed where it was found and nowhere else: `log`
/// and `diff` take a revision from the command line the same way and had no separator, so
///
/// ```text
/// $ yidam log -- --output=$T/pwned
/// No commits in --output=/…/pwned matching the filter.   # exit 0, and the file is written
/// ```
///
/// Passing a revision through [`Git::rev`] is the only way this module accepts one, so the
/// separator is not something a call site can forget.
///
/// Supported by every subcommand this crate gives a revision to, verified against git
/// 2.50.1: `log`, `show`, `rev-list`, `rev-parse`, `diff`, `merge-base`, `ls-tree`,
/// `cat-file`, `for-each-ref`, `check-ignore`. (`git var` rejects it, and takes no
/// revision.)
const END_OF_OPTIONS: &str = "--end-of-options";

/// One `git` invocation, described before it is run.
///
/// The three argument kinds are kept apart rather than appended to one list, so the
/// separators land correctly no matter what order the call site builds in:
///
/// ```text
/// git -C <root> -c <pinned…> <args…> [--end-of-options <revs…>] [-- <paths…>]
/// ```
#[derive(Debug, Clone)]
pub struct Git {
    cwd: Option<PathBuf>,
    config: Vec<(String, String)>,
    args: Vec<OsString>,
    revs: Vec<OsString>,
    paths: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    stdin: Option<Vec<u8>>,
}

impl Git {
    /// Ask about the repository at `root`.
    pub fn new(root: impl AsRef<Path>) -> Self {
        Self {
            cwd: Some(root.as_ref().to_path_buf()),
            config: Vec::new(),
            args: Vec::new(),
            revs: Vec::new(),
            paths: Vec::new(),
            env: Vec::new(),
            stdin: None,
        }
    }

    /// Ask about whatever repository this process is standing in.
    ///
    /// For the two callers whose question *is* "where am I" — [`crate::paths::repo_root`]
    /// and `doctor`'s "this is a git repository with no `.yidam/`" — and for nothing else.
    /// Every other caller has a root and should name it.
    pub fn here() -> Self {
        let mut g = Self::new(".");
        g.cwd = None;
        g
    }

    /// An extra `-c <key>=<value>`, beyond [`PINNED_CONFIG`].
    pub fn config(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.config.push((key.into(), value.into()));
        self
    }

    /// An environment variable to set for this invocation, applied *after* the removal of
    /// [`INHERITED_LOCATION`] — so a deliberate `GIT_INDEX_FILE` survives and an inherited
    /// one does not.
    pub fn env(mut self, key: impl AsRef<OsStr>, value: impl AsRef<OsStr>) -> Self {
        self.env
            .push((key.as_ref().to_owned(), value.as_ref().to_owned()));
        self
    }

    pub fn arg(mut self, a: impl AsRef<OsStr>) -> Self {
        self.args.push(a.as_ref().to_owned());
        self
    }

    pub fn args<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, a: I) -> Self {
        self.args
            .extend(a.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// A revision: a ref, a hash, a range, a `<rev>:<path>` object spec. Placed after
    /// [`END_OF_OPTIONS`], which is why it is a separate method from [`Git::arg`].
    pub fn rev(mut self, r: impl AsRef<OsStr>) -> Self {
        self.revs.push(r.as_ref().to_owned());
        self
    }

    pub fn revs<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, r: I) -> Self {
        self.revs
            .extend(r.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// Pathspecs, placed after `--`.
    pub fn paths<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(mut self, p: I) -> Self {
        self.paths
            .extend(p.into_iter().map(|s| s.as_ref().to_owned()));
        self
    }

    /// Bytes to feed the child on stdin. Written from a separate thread by every terminal
    /// that uses it: a payload large enough to fill the pipe buffer would otherwise deadlock
    /// against a child blocked writing its own answer.
    pub fn stdin(mut self, bytes: impl Into<Vec<u8>>) -> Self {
        self.stdin = Some(bytes.into());
        self
    }

    /// The full argument list, without the program name. Exposed so the separators can be
    /// asserted directly rather than inferred from what git did with them.
    pub(crate) fn argv(&self) -> Vec<OsString> {
        let mut v: Vec<OsString> = Vec::new();
        if let Some(cwd) = &self.cwd {
            v.push("-C".into());
            v.push(cwd.clone().into_os_string());
        }
        for (k, val) in PINNED_CONFIG
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .chain(self.config.iter().cloned())
        {
            v.push("-c".into());
            v.push(format!("{k}={val}").into());
        }
        v.extend(self.args.iter().cloned());
        if !self.revs.is_empty() {
            v.push(END_OF_OPTIONS.into());
            v.extend(self.revs.iter().cloned());
        }
        if !self.paths.is_empty() {
            v.push("--".into());
            v.extend(self.paths.iter().cloned());
        }
        v
    }

    /// How the invocation reads in an error message.
    fn display(&self) -> String {
        self.argv()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join(" ")
    }

    fn command(&self) -> Command {
        let mut cmd = Command::new("git");
        cmd.args(self.argv());
        for k in INHERITED_LOCATION {
            cmd.env_remove(k);
        }
        // Belt-and-braces rather than a fix: no reader in this crate parses localized
        // output — `--format` placeholders, `--porcelain` and `--name-status` are all
        // locale-independent by construction. It is here so that the stderr we bubble up
        // to a user, and quote in a bug report, is the same text on every machine.
        cmd.env("LC_ALL", "C");
        cmd.env("LANG", "C");
        for (k, v) in &self.env {
            cmd.env(k, v);
        }
        cmd
    }

    // ── terminals ─────────────────────────────────────────────────────────────

    /// Run it and hand back everything, with the exit status unexamined. For the callers
    /// that read `stderr` or branch on the code themselves.
    ///
    /// `Err` means git could not be run at all, which is not the same as git saying no.
    pub fn output(self) -> Result<Output> {
        let mut cmd = self.command();
        let Some(bytes) = self.stdin.clone() else {
            cmd.stdin(Stdio::null());
            return cmd
                .output()
                .with_context(|| format!("running git {}", self.display()));
        };

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        let mut child = cmd
            .spawn()
            .with_context(|| format!("running git {}", self.display()))?;
        let mut sink = child
            .stdin
            .take()
            .context("git was spawned without the stdin pipe it was asked for")?;
        let writer = std::thread::spawn(move || sink.write_all(&bytes));
        let out = child
            .wait_with_output()
            .with_context(|| format!("running git {}", self.display()))?;
        // A child that exited without reading its input is not an error here: git said what
        // it had to say, and the status below is what decides whether that counts.
        let _ = writer.join();
        Ok(out)
    }

    /// Trimmed stdout, or `Err` naming the invocation and carrying git's stderr.
    pub fn run(self) -> Result<String> {
        let shown = self.display();
        let out = self.output()?;
        if !out.status.success() {
            bail!(
                "git {shown} failed: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Trimmed stdout, or `None` for any reason at all — git missing, git refusing, output
    /// that is not UTF-8.
    pub fn try_run(self) -> Option<String> {
        let out = self.output().ok()?;
        out.status
            .success()
            .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    }

    /// Stdout split into lines, or `None` when git refused. Lines are **not** trimmed
    /// individually and an empty trailing line is dropped, which is what `lines()` does.
    pub fn lines(self) -> Option<Vec<String>> {
        let out = self.output().ok()?;
        out.status.success().then(|| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
    }

    /// Did git exit zero? Output is captured and discarded, so a `--quiet` probe stays
    /// quiet and a chatty one does not reach the user's terminal.
    pub fn succeeded(self) -> bool {
        self.output().map(|o| o.status.success()).unwrap_or(false)
    }

    /// A spawned child with stdin and stdout piped, for the one caller that must interleave
    /// writing and reading: `cat-file --batch` answers each request as it arrives, and
    /// reading its framed responses is the caller's protocol, not this module's.
    ///
    /// stderr is discarded — the batch protocol reports a missing object in its own stdout
    /// framing, so git's stderr here is noise on a path that handles absence itself.
    pub fn spawn_piped(self) -> Result<Child> {
        let shown = self.display();
        self.command()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .with_context(|| format!("running git {shown}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(g: Git) -> Vec<String> {
        g.argv()
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    /// The separator is not optional and not the call site's to remember: anything reaching
    /// argv through [`Git::rev`] is behind it.
    #[test]
    fn a_revision_lands_behind_the_end_of_options_separator() {
        let v = shown(
            Git::new("/r")
                .args(["log", "--format=%H"])
                .rev("--output=x"),
        );
        let i = v
            .iter()
            .position(|a| a == END_OF_OPTIONS)
            .expect("separator");
        let j = v.iter().position(|a| a == "--output=x").expect("revision");
        assert!(i < j, "the revision is not behind the separator: {v:?}");
    }

    /// Built out of order, and the separators still land where git needs them. This is the
    /// whole reason the three argument kinds are separate vectors rather than one list.
    #[test]
    fn the_separators_do_not_depend_on_the_order_the_call_site_builds_in() {
        let forwards = shown(Git::new("/r").args(["diff"]).rev("HEAD").paths(["a/b"]));
        let backwards = shown(Git::new("/r").paths(["a/b"]).rev("HEAD").args(["diff"]));
        assert_eq!(forwards, backwards);
        assert_eq!(
            forwards,
            [
                "-C",
                "/r",
                "-c",
                "core.quotepath=false",
                "-c",
                "log.showSignature=false",
                "diff",
                "--end-of-options",
                "HEAD",
                "--",
                "a/b",
            ]
        );
    }

    /// A separator with nothing behind it is a separator git has to interpret. Neither is
    /// emitted unless something was put behind it.
    #[test]
    fn an_empty_slot_emits_no_separator() {
        let v = shown(Git::new("/r").args(["status", "--porcelain"]));
        assert!(!v.contains(&END_OF_OPTIONS.to_string()), "{v:?}");
        assert!(!v.contains(&"--".to_string()), "{v:?}");
    }

    /// Every invocation carries them, and a caller's own `-c` is added rather than
    /// substituted.
    #[test]
    fn the_pinned_config_is_on_every_invocation_and_survives_an_extra_one() {
        let v = shown(
            Git::new("/r")
                .config("gpg.ssh.allowedSignersFile", "/tmp/a")
                .args(["log"]),
        );
        for (k, val) in PINNED_CONFIG {
            assert!(
                v.windows(2)
                    .any(|w| w[0] == "-c" && w[1] == format!("{k}={val}")),
                "{k} missing from {v:?}"
            );
        }
        assert!(
            v.windows(2)
                .any(|w| w[0] == "-c" && w[1] == "gpg.ssh.allowedSignersFile=/tmp/a"),
            "the caller's own config was dropped: {v:?}"
        );
    }

    /// [`Git::here`] asks about the working directory, so it must not pass `-C`.
    #[test]
    fn here_names_no_directory() {
        let v = shown(Git::here().args(["rev-parse", "--show-toplevel"]));
        assert!(!v.contains(&"-C".to_string()), "{v:?}");
    }

    /// `GIT_INDEX_FILE` is on [`INHERITED_LOCATION`] *and* is the one variable `propose`
    /// sets on purpose, so the order in which `env_remove` and `env` are applied to the
    /// `Command` is load-bearing and nothing else holds it.
    ///
    /// The other direction — that an *inherited* one is ignored — is not asserted here.
    /// Proving it requires the variable to be in this process's own environment, and
    /// `set_var` against a parallel test binary is a race. `tests/git_hygiene.rs` sets it
    /// on a child it spawns, which is both race-free and the situation a user is actually
    /// in.
    #[test]
    fn a_deliberate_variable_survives_the_removal_of_an_inherited_one() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        crate::git::fixture::repo(root, "genesis: a corpus");

        let index = tmp.path().join("scratch-index");
        let ok = Git::new(root)
            .env("GIT_INDEX_FILE", &index)
            .args(["read-tree"])
            .rev("HEAD")
            .succeeded();
        assert!(ok, "git read-tree failed");
        assert!(
            index.exists(),
            "GIT_INDEX_FILE was stripped along with the inherited ones"
        );
    }

    /// The runner is only worth having if it actually runs git.
    #[test]
    fn a_query_through_the_runner_answers_about_the_root_it_was_given() {
        let tmp = tempfile::tempdir().unwrap();
        crate::git::fixture::repo(tmp.path(), "genesis: a corpus");
        assert_eq!(
            Git::new(tmp.path())
                .args(["log", "-1", "--format=%s"])
                .rev("HEAD")
                .try_run()
                .as_deref(),
            Some("genesis: a corpus")
        );
    }
}
