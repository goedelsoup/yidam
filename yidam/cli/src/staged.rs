//! `--staged`: a corpus gate run over the tree the next commit would hold (RFC-0070).
//!
//! `graph-check`, `lint` and `regen --check` read the working tree, and a commit is made from
//! the index. A pre-commit hook that runs a gate as written checks the wrong tree: an
//! unstaged fix hides a staged break, and an unstaged break fails a clean commit (#1519).
//!
//! A staged run writes the index as a tree, checks that tree out over a detached scratch
//! worktree of HEAD, and runs the same command again there, rooted at the scratch tree. The
//! scratch worktree shares the repository's objects and refs, so every check that reads
//! history still reads it, and the staged changes are the scratch tree's difference from
//! HEAD. The child's output names the scratch tree; this module rewrites it to name the
//! repository before printing, so a staged report reads as one made in the repository.
//!
//! Running the command again, rather than calling the gate in-process, is what lets one
//! module serve all three gates: each prints its own paths in its own places, and none of them
//! needs to know a staged run exists.

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::git::Git;

/// Git LFS's switch for leaving each LFS file as its pointer. `core.hooksPath` stops hooks, not
/// filters, and a checkout that smudged would fetch LFS objects on every staged run and fail
/// an offline commit. No corpus gate reads LFS content (RFC-0070 §2).
const LFS_SKIP_SMUDGE: &str = "GIT_LFS_SKIP_SMUDGE";

/// The variables git exports into a hook that would move the child off the scratch tree.
/// [`Git`] strips the same set from every git call; the child is a `yidam`, not a git, so it
/// is stripped here as well.
const HOOK_LOCATION: &[&str] = &[
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_INDEX_FILE",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_COMMON_DIR",
    "GIT_PREFIX",
];

/// Run the command this process was invoked as over the staged tree of `root`'s repository.
///
/// `args` is the invocation without the program name. `--staged` and any `--root` are taken
/// out of it, and the child is given the scratch tree's root instead.
pub fn rerun(root: Option<&Path>, args: Vec<OsString>) -> Result<()> {
    let resolved = crate::paths::resolve_root(root)?;
    let top = toplevel(&resolved)?;
    let tree = staged_tree(&top)?;
    let scratch = Scratch::add(&top)?;
    Git::new(&scratch.path)
        .env(LFS_SKIP_SMUDGE, "1")
        .args(["read-tree", "-u", "--reset"])
        .revs([tree.as_str()])
        .run()
        .context("checking the staged tree out into the scratch worktree")?;

    let below = canonical(&resolved)
        .strip_prefix(&top)
        .map(Path::to_path_buf)
        .unwrap_or_default();
    // Joining an empty path appends a separator, and the envelope's `root` would then end in
    // one that a working-tree run's does not.
    let child_root = if below.as_os_str().is_empty() {
        scratch.path.clone()
    } else {
        scratch.path.join(below)
    };
    let exe = std::env::current_exe().context("finding this yidam binary to run again")?;
    let mut cmd = Command::new(exe);
    cmd.args(forwarded(args))
        .arg("--root")
        .arg(&child_root)
        .current_dir(&scratch.path)
        .env_remove(crate::paths::ROOT_VAR)
        .stdin(Stdio::null());
    for k in HOOK_LOCATION {
        cmd.env_remove(k);
    }
    let out = cmd
        .output()
        .context("running the gate over the staged tree")?;

    // The child's root is named as this process resolved it, so the envelope's `root` reads
    // as a working-tree run's would, a relative `--root` included. Everything else under the
    // scratch tree is named under the top level.
    let pairs = spellings(&[(&child_root, &resolved), (&scratch.path, &top)]);
    // Rewritten as bytes, so output that is not UTF-8 passes through as the child wrote it.
    // The two streams are captured apart, so all of stdout prints before any of stderr; each
    // keeps its own order, and a gate's report is on stdout and its failures on stderr.
    {
        use std::io::Write;
        let _ = std::io::stdout().write_all(&renamed(&out.stdout, &pairs));
        let _ = std::io::stdout().flush();
        let _ = std::io::stderr().write_all(&renamed(&out.stderr, &pairs));
    }
    drop(scratch);
    if out.status.success() {
        Ok(())
    } else {
        Err(crate::report::GateFailed.into())
    }
}

/// The invocation, less `--staged` and any `--root` with its value.
fn forwarded(args: Vec<OsString>) -> Vec<OsString> {
    let mut out = Vec::with_capacity(args.len());
    let mut it = args.into_iter();
    while let Some(a) = it.next() {
        if a == OsStr::new("--staged") {
            continue;
        }
        if a == OsStr::new("--root") {
            it.next();
            continue;
        }
        if a.to_string_lossy().starts_with("--root=") {
            continue;
        }
        out.push(a);
    }
    out
}

/// Every spelling of each scratch path, paired with the repository's path in the same form
/// (RFC-0070 §2).
///
/// A path is spelled as it was made and as it canonicalizes, so `/var/…` and `/private/var/…`
/// are both caught, and a symlinked git directory beside its target. Each is spelled with and
/// without the verbatim `\\?\` prefix Windows canonicalization adds, and each of those as
/// JSON escapes it. The longest comes first, so a spelling that is a prefix of another does
/// not split it.
fn spellings(paths: &[(&Path, &Path)]) -> Vec<(String, String)> {
    const VERBATIM: &str = r"\\?\";
    let mut out: Vec<(String, String)> = vec![];
    let mut add = |from: String, to: String| {
        if !from.is_empty() && !out.iter().any(|(f, _)| *f == from) {
            out.push((from, to));
        }
    };
    for (from, to) in paths {
        for (f, t) in [
            (from.to_path_buf(), to.to_path_buf()),
            (canonical(from), canonical(to)),
        ] {
            let (f, t) = (f.display().to_string(), t.display().to_string());
            let bare = |s: &str| s.strip_prefix(VERBATIM).unwrap_or(s).to_string();
            let (bf, bt) = (bare(&f), bare(&t));
            for (f, t) in [
                (f, t),
                (format!("{VERBATIM}{bf}"), format!("{VERBATIM}{bt}")),
                (bf, bt),
            ] {
                add(f.replace('\\', r"\\"), t.replace('\\', r"\\"));
                add(f, t);
            }
        }
    }
    out.sort_by_key(|(f, _)| std::cmp::Reverse(f.len()));
    out
}

/// `bytes` with every spelling of a scratch path replaced by the repository's.
///
/// On bytes, not text: a gate may print a file's name or content that is not UTF-8, and a
/// lossy decode would print U+FFFD where a working-tree run prints what is there.
fn renamed(bytes: &[u8], pairs: &[(String, String)]) -> Vec<u8> {
    let mut out = bytes.to_vec();
    for (from, to) in pairs {
        let (from, to) = (from.as_bytes(), to.as_bytes());
        let mut next = Vec::with_capacity(out.len());
        let mut i = 0;
        while i < out.len() {
            if out[i..].starts_with(from) {
                next.extend_from_slice(to);
                i += from.len();
            } else {
                next.push(out[i]);
                i += 1;
            }
        }
        out = next;
    }
    out
}

fn canonical(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// The repository's top level, canonical, with a commit at HEAD to stage against.
fn toplevel(root: &Path) -> Result<PathBuf> {
    let Some(top) = Git::new(root)
        .args(["rev-parse", "--show-toplevel"])
        .try_run()
    else {
        bail!(
            "--staged reads the git index, and {} is not inside a git working tree",
            root.display()
        );
    };
    let top = canonical(Path::new(top.trim()));
    let born = Git::new(&top)
        .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .succeeded();
    if !born {
        bail!(
            "nothing is committed yet, so there is no HEAD to stage against; run the gate \
             without --staged"
        );
    }
    Ok(top)
}

/// The index written as a tree object.
///
/// A hook git runs for `git commit <paths>` or `git commit -a` exports `GIT_INDEX_FILE`
/// naming the temporary index the commit is made from, not `.git/index`. [`Git`] strips it
/// from every call, so it is read here on purpose: the tree a staged gate checks is the one
/// the commit will hold.
///
/// `write-tree` writes its cache-tree back into the index it reads, and a staged run writes
/// nothing of the person's (RFC-0070 §2). So it reads a copy, made beside the scratch
/// worktrees under the git directory and removed before this returns.
fn staged_tree(top: &Path) -> Result<String> {
    let index = match std::env::var_os("GIT_INDEX_FILE").filter(|v| !v.is_empty()) {
        // Git may hand a hook a path relative to the top level. Made absolute against it, so
        // a `--root` below the top level, or a hook that changed directory, does not move it.
        Some(index) => top.join(index),
        None => top.join(
            Git::new(top)
                .args(["rev-parse", "--git-path", "index"])
                .run()
                .context("finding the index a staged run reads")?
                .trim(),
        ),
    };
    let copy = IndexCopy(git_dir(top)?.join(format!("yidam-staged-{}-index", std::process::id())));
    std::fs::copy(&index, &copy.0)
        .with_context(|| format!("copying the index at {} for a staged run", index.display()))?;
    // The only location variable a staged run sets, and only for these two calls.
    let at_copy = || Git::new(top).env("GIT_INDEX_FILE", &copy.0);
    if let Some(tree) = at_copy().args(["write-tree"]).try_run() {
        return Ok(tree.trim().to_string());
    }
    let unmerged = at_copy()
        .args(["ls-files", "--unmerged"])
        .lines()
        .is_some_and(|l| !l.is_empty());
    if unmerged {
        bail!("the index holds unmerged paths; finish or abort the merge before a staged run");
    }
    bail!("git could not write the index as a tree");
}

/// A copy of the index, removed when dropped.
struct IndexCopy(PathBuf);

impl Drop for IndexCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The repository's git directory, absolute.
fn git_dir(top: &Path) -> Result<PathBuf> {
    let dir = Git::new(top)
        .args(["rev-parse", "--absolute-git-dir"])
        .run()
        .context("finding the git directory a staged run works under")?;
    Ok(PathBuf::from(dir.trim()))
}

/// A detached worktree of HEAD under the git directory, removed when dropped.
struct Scratch {
    top: PathBuf,
    path: PathBuf,
}

impl Scratch {
    fn add(top: &Path) -> Result<Self> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let git_dir = git_dir(top)?;
        sweep(top, &git_dir);
        let path = git_dir.join(format!("yidam-staged-{}-{n}", std::process::id()));
        let scratch = Self {
            top: top.to_path_buf(),
            path,
        };
        // A leftover from an interrupted run would make `worktree add` refuse the path.
        scratch.remove();
        Git::new(top)
            // A checkout runs `post-checkout`, and a hook is the person's, for their checkouts.
            .config("core.hooksPath", "/dev/null")
            .env(LFS_SKIP_SMUDGE, "1")
            .args(["worktree", "add", "--detach", "--no-checkout", "--quiet"])
            .arg(&scratch.path)
            .arg("HEAD")
            .run()
            .context("adding the scratch worktree a staged run checks")?;
        Ok(scratch)
    }

    fn remove(&self) {
        let _ = Git::new(&self.top)
            .args(["worktree", "remove", "--force"])
            .arg(&self.path)
            .run();
        let _ = std::fs::remove_dir_all(&self.path);
        let _ = Git::new(&self.top).args(["worktree", "prune"]).run();
    }
}

/// Remove what a staged run killed before its cleanup left: each `yidam-staged-<pid>-…` under
/// the git directory whose process is gone, worktree or index copy, and then the worktree
/// records that pointed at them. A live run's entries are left alone.
fn sweep(top: &Path, git_dir: &Path) {
    let Ok(entries) = std::fs::read_dir(git_dir) else {
        return;
    };
    let mine = std::process::id();
    let mut removed = false;
    for e in entries.filter_map(Result::ok) {
        let name = e.file_name();
        let Some(pid) = name
            .to_str()
            .and_then(|n| n.strip_prefix("yidam-staged-"))
            .and_then(|rest| rest.split('-').next())
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid == mine || alive(pid, &e.path()) {
            continue;
        }
        let path = e.path();
        if path.is_dir() {
            let _ = Git::new(top)
                .args(["worktree", "remove", "--force"])
                .arg(&path)
                .run();
            let _ = std::fs::remove_dir_all(&path);
        } else {
            let _ = std::fs::remove_file(&path);
        }
        removed = true;
    }
    if removed {
        let _ = Git::new(top).args(["worktree", "prune"]).run();
    }
}

/// Is the run that made `path` still going? On unix, whether `pid` is a live process. Where
/// that cannot be asked, whether the entry is less than a day old.
///
/// A pid outside `1..=i32::MAX` names no process, and is asked nothing: `kill` reads 0 as its
/// own process group, and Linux's reads 4294967295 as -1, every process, so either would
/// answer that it is alive.
pub(crate) fn alive(pid: u32, path: &Path) -> bool {
    if pid == 0 || i32::try_from(pid).is_err() {
        return false;
    }
    #[cfg(unix)]
    {
        let _ = path;
        Command::new("kill")
            .args(["-0", &pid.to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_or(true, |s| s.success())
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        std::fs::metadata(path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.elapsed().ok())
            .is_none_or(|age| age < std::time::Duration::from_secs(24 * 60 * 60))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        self.remove();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(v: &[&str]) -> Vec<OsString> {
        v.iter().map(OsString::from).collect()
    }

    #[test]
    fn the_flag_and_the_root_are_not_forwarded() {
        assert_eq!(
            forwarded(os(&["lint", "--staged", "--root", "x", "--warn"])),
            os(&["lint", "--warn"])
        );
        assert_eq!(
            forwarded(os(&[
                "graph-check",
                "--root=x",
                "--staged",
                "--format",
                "json"
            ])),
            os(&["graph-check", "--format", "json"])
        );
    }

    fn r(text: &str, pairs: &[(String, String)]) -> String {
        String::from_utf8(renamed(text.as_bytes(), pairs)).unwrap()
    }

    #[test]
    fn the_scratch_tree_is_named_as_the_repository() {
        let s = Path::new("/r/.git/yidam-staged-1-0");
        let pairs = spellings(&[(&s.join("sub"), Path::new("sub")), (s, Path::new("/r"))]);
        assert_eq!(
            r(
                "/r/.git/yidam-staged-1-0/.yidam/corpus/a.yml: broken",
                &pairs
            ),
            "/r/.yidam/corpus/a.yml: broken"
        );
        assert_eq!(r("root: /r/.git/yidam-staged-1-0/sub", &pairs), "root: sub");
    }

    /// A Windows path, as printed and as JSON escapes it, with and without the verbatim prefix.
    #[test]
    fn every_spelling_of_a_windows_path_is_renamed() {
        let pairs = spellings(&[(Path::new(r"C:\r\.git\yidam-staged-1-0"), Path::new(r"C:\r"))]);
        for (text, want) in [
            (r"C:\r\.git\yidam-staged-1-0\a.yml", r"C:\r\a.yml"),
            (r"C:\\r\\.git\\yidam-staged-1-0\\a.yml", r"C:\\r\\a.yml"),
            (r"\\?\C:\r\.git\yidam-staged-1-0\a.yml", r"\\?\C:\r\a.yml"),
            (
                r"\\\\?\\C:\\r\\.git\\yidam-staged-1-0\\a.yml",
                r"\\\\?\\C:\\r\\a.yml",
            ),
        ] {
            assert_eq!(r(text, &pairs), want, "{text}");
        }
    }

    /// Output that is not UTF-8 is renamed and otherwise passes through as written.
    #[test]
    fn bytes_that_are_not_utf8_pass_through() {
        let pairs = spellings(&[(Path::new("/r/.git/yidam-staged-1-0"), Path::new("/r"))]);
        let mut text = b"/r/.git/yidam-staged-1-0/caf".to_vec();
        text.push(0xe9);
        let mut want = b"/r/caf".to_vec();
        want.push(0xe9);
        assert_eq!(renamed(&text, &pairs), want);
    }

    /// A run killed before its cleanup leaves its worktree and index copy; the next run's
    /// sweep removes them and leaves a live run's.
    #[cfg(unix)]
    #[test]
    fn a_dead_runs_scratch_is_swept() {
        let tmp = tempfile::tempdir().unwrap();
        crate::git::fixture::init(tmp.path());
        let git_dir = tmp.path().join(".git");
        let dead_tree = git_dir.join(format!("yidam-staged-{}-0", u32::MAX));
        let dead_index = git_dir.join(format!("yidam-staged-{}-index", u32::MAX));
        let live = git_dir.join(format!("yidam-staged-{}-index", std::process::id()));
        std::fs::create_dir_all(dead_tree.join("x")).unwrap();
        std::fs::write(&dead_index, "").unwrap();
        std::fs::write(&live, "").unwrap();
        sweep(tmp.path(), &git_dir);
        assert!(!dead_tree.exists());
        assert!(!dead_index.exists());
        assert!(live.exists());
    }
}
