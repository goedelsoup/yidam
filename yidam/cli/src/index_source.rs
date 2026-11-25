//! Which vector index a checkout reads (RFC-0069).
//!
//! `.yidam/index/` is gitignored, so it belongs to one checkout. A `git worktree add` checkout
//! starts without one, and before this module every such checkout answered `retrieve` by
//! keyword for its whole life (#1517). The rule here is one step longer than "read
//! `root/.yidam/index`":
//!
//! 1. A local index wins, whatever commit it was built at.
//! 2. Otherwise a linked worktree reads its main worktree's index, if that index was built at
//!    HEAD or at an ancestor of it.
//! 3. Otherwise there is no index.
//!
//! Ancestry is the whole test. An index built at an ancestor names nodes this branch
//! inherited, and `stale: true` already says what may have changed since. One built on another
//! branch can name nodes this branch never held, and no flag here can say that.

use std::path::{Path, PathBuf};

use crate::git::Git;
use crate::paths::yidam_index_dir;

/// Where the index a checkout reads came from. Its wire form is frozen in the MCP contract's
/// `index_source` (0.37.0), beside a third value, `remote`, that only a server reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexSource {
    /// `root/.yidam/index`.
    Local,
    /// The main worktree's index, accepted under the ancestry rule.
    Shared,
}

impl IndexSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Shared => "shared",
        }
    }
}

/// What [`resolve`] found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// An index to read, and where it lives.
    Found { dir: PathBuf, source: IndexSource },
    /// A linked worktree with no index of its own, whose main worktree has one built at a
    /// commit this checkout's HEAD does not contain. Kept apart from [`Resolution::None`] so
    /// `doctor` can name the cause instead of saying only that nothing is here.
    Refused {
        dir: PathBuf,
        indexed_commit: String,
    },
    /// Nothing to read.
    None,
}

impl Resolution {
    /// The directory to read, when there is one.
    pub fn found(&self) -> Option<(&Path, IndexSource)> {
        match self {
            Self::Found { dir, source } => Some((dir, *source)),
            _ => None,
        }
    }
}

/// Resolve the index `root` reads. See the module doc for the order.
pub fn resolve(root: &Path) -> Resolution {
    let local = yidam_index_dir(root);
    if local.exists() {
        return Resolution::Found {
            dir: local,
            source: IndexSource::Local,
        };
    }
    let Some(main) = main_worktree(root) else {
        return Resolution::None;
    };
    let dir = yidam_index_dir(&main);
    let Some(indexed_commit) = indexed_commit(&dir) else {
        return Resolution::None;
    };
    if contains(root, &indexed_commit) {
        Resolution::Found {
            dir,
            source: IndexSource::Shared,
        }
    } else {
        Resolution::Refused {
            dir,
            indexed_commit,
        }
    }
}

/// The main worktree of the repository `root` is a *linked* worktree of, or `None`.
///
/// `None` for a main worktree itself, a bare repository's linked worktrees (a bare repository
/// has no main worktree to read), and anything git cannot answer about — a projected mirror
/// carries no repository at all.
fn main_worktree(root: &Path) -> Option<PathBuf> {
    let lines = Git::new(root)
        .args(["worktree", "list", "--porcelain"])
        .lines()?;
    // The first record is always the main worktree. A record is a run of lines ended by a
    // blank one; `bare` among them marks a repository with no working tree.
    let first: Vec<&str> = lines
        .iter()
        .map(String::as_str)
        .take_while(|l| !l.is_empty())
        .collect();
    if first.contains(&"bare") {
        return None;
    }
    let main = PathBuf::from(first.first()?.strip_prefix("worktree ")?);
    let here = Git::new(root)
        .args(["rev-parse", "--show-toplevel"])
        .try_run()?;
    if same_dir(&main, Path::new(&here)) {
        return None;
    }
    Some(main)
}

fn same_dir(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(a), Ok(b)) => a == b,
        _ => a == b,
    }
}

/// `indexed_commit` out of `dir/meta.json`, when there is an index there that says one.
fn indexed_commit(dir: &Path) -> Option<String> {
    if !dir.join("corpus.arrow").exists() {
        return None;
    }
    let raw = std::fs::read(dir.join("meta.json")).ok()?;
    let meta: serde_json::Value = serde_json::from_slice(&raw).ok()?;
    meta["indexed_commit"]
        .as_str()
        .filter(|s| !s.is_empty() && *s != crate::git::UNKNOWN_COMMIT)
        .map(str::to_string)
}

/// Is `commit` HEAD, or an ancestor of it, in `root`'s repository?
fn contains(root: &Path, commit: &str) -> bool {
    Git::new(root)
        .args(["merge-base", "--is-ancestor"])
        .revs([commit, "HEAD"])
        .succeeded()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::fixture::{commit, git, git_out, init, write};

    fn index_at(dir: &Path, commit: &str) {
        let index = yidam_index_dir(dir);
        std::fs::create_dir_all(&index).unwrap();
        std::fs::write(index.join("corpus.arrow"), b"").unwrap();
        std::fs::write(
            index.join("meta.json"),
            format!(r#"{{"indexed_commit":"{commit}"}}"#),
        )
        .unwrap();
    }

    fn head(dir: &Path) -> String {
        git_out(dir, &["rev-parse", "--short", "HEAD"])
    }

    /// A main checkout with one commit, and a linked worktree of it on its own branch.
    fn repo_with_worktree(tmp: &Path) -> (PathBuf, PathBuf) {
        let main = tmp.join("main");
        std::fs::create_dir_all(&main).unwrap();
        init(&main);
        write(&main, "a.txt", "a");
        commit(&main, "genesis: test");
        let wt = tmp.join("wt");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        (main, wt)
    }

    #[test]
    fn a_local_index_wins() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, wt) = repo_with_worktree(tmp.path());
        index_at(&main, &head(&main));
        index_at(&wt, "0000000");
        assert_eq!(
            resolve(&wt).found().map(|(_, s)| s),
            Some(IndexSource::Local)
        );
    }

    #[test]
    fn a_worktree_reads_the_main_index_built_at_an_ancestor() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, wt) = repo_with_worktree(tmp.path());
        index_at(&main, &head(&main));
        write(&wt, "b.txt", "b");
        commit(&wt, "observe: b");
        let r = resolve(&wt);
        let (dir, source) = r.found().expect("the main index is an ancestor's");
        assert_eq!(source, IndexSource::Shared);
        assert!(same_dir(dir, &yidam_index_dir(&main)));
    }

    #[test]
    fn an_index_this_branch_does_not_contain_is_refused() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, wt) = repo_with_worktree(tmp.path());
        write(&main, "c.txt", "c");
        commit(&main, "observe: c");
        let ahead = head(&main);
        index_at(&main, &ahead);
        match resolve(&wt) {
            Resolution::Refused {
                dir,
                indexed_commit,
            } => {
                assert!(same_dir(&dir, &yidam_index_dir(&main)));
                assert_eq!(indexed_commit, ahead);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_main_worktree_reads_only_its_own() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, _wt) = repo_with_worktree(tmp.path());
        assert_eq!(resolve(&main), Resolution::None);
    }

    #[test]
    fn no_repository_is_no_index() {
        let tmp = tempfile::tempdir().unwrap();
        assert_eq!(resolve(tmp.path()), Resolution::None);
    }
}
