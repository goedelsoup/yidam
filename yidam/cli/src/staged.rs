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

    let rename = |bytes: &[u8]| renamed(&String::from_utf8_lossy(bytes), &scratch.path, &top);
    print!("{}", rename(&out.stdout));
    eprint!("{}", rename(&out.stderr));
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

/// `text` with every mention of the scratch tree replaced by the repository's top level.
fn renamed(text: &str, scratch: &Path, top: &Path) -> String {
    let to = top.display().to_string();
    let mut out = text.to_string();
    let mut forms = vec![scratch.display().to_string()];
    let canon = canonical(scratch).display().to_string();
    if !forms.contains(&canon) {
        forms.push(canon);
    }
    // Longest first, so a form that is a prefix of another does not split it.
    forms.sort_by_key(|f| std::cmp::Reverse(f.len()));
    for f in forms {
        out = out.replace(&f, &to);
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
/// from every call, so it is handed back here on purpose: the tree a staged gate checks is
/// the one the commit will hold.
fn staged_tree(top: &Path) -> Result<String> {
    let mut git = Git::new(top).args(["write-tree"]);
    if let Some(index) = std::env::var_os("GIT_INDEX_FILE").filter(|v| !v.is_empty()) {
        let index = PathBuf::from(index);
        // Relative to the directory git ran the hook in, which is the top level.
        let index = if index.is_relative() {
            std::env::current_dir().map_or(index.clone(), |d| d.join(&index))
        } else {
            index
        };
        git = git.env("GIT_INDEX_FILE", index);
    }
    if let Some(tree) = git.try_run() {
        return Ok(tree.trim().to_string());
    }
    let unmerged = Git::new(top)
        .args(["ls-files", "--unmerged"])
        .lines()
        .is_some_and(|l| !l.is_empty());
    if unmerged {
        bail!("the index holds unmerged paths; finish or abort the merge before a staged run");
    }
    bail!("git could not write the index as a tree");
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
        let git_dir = Git::new(top)
            .args(["rev-parse", "--absolute-git-dir"])
            .run()
            .context("finding the git directory for the staged run's scratch worktree")?;
        let path =
            Path::new(git_dir.trim()).join(format!("yidam-staged-{}-{n}", std::process::id()));
        let scratch = Self {
            top: top.to_path_buf(),
            path,
        };
        // A leftover from an interrupted run would make `worktree add` refuse the path.
        scratch.remove();
        Git::new(top)
            // A checkout runs `post-checkout`, and a hook is the person's, for their checkouts.
            .config("core.hooksPath", "/dev/null")
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

    #[test]
    fn the_scratch_tree_is_named_as_the_repository() {
        let s = Path::new("/r/.git/yidam-staged-1-0");
        let t = Path::new("/r");
        assert_eq!(
            renamed("/r/.git/yidam-staged-1-0/.yidam/corpus/a.yml: broken", s, t),
            "/r/.yidam/corpus/a.yml: broken"
        );
    }
}
