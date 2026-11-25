//! Which vector index a checkout reads (RFC-0069).
//!
//! `.yidam/index/` is gitignored, so it belongs to one checkout. A `git worktree add` checkout
//! starts without one, and before this module every such checkout answered `retrieve` by
//! keyword for its whole life (#1517). The rule here is one step longer than "read
//! `root/.yidam/index`":
//!
//! 1. A local index wins, whatever commit it was built at. An empty `.yidam/index/` is none.
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
    if holds_anything(&local) {
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

/// Is there anything in `dir`? An empty `.yidam/index/` is what an interrupted build or a
/// bare `mkdir` leaves, and it is no index: counting it as a local one would hide the main
/// worktree's behind a directory that answers nothing.
fn holds_anything(dir: &Path) -> bool {
    std::fs::read_dir(dir).is_ok_and(|mut entries| entries.next().is_some())
}

/// The main worktree of the repository `root` is a *linked* worktree of, or `None`.
///
/// `None` for a main worktree itself, a bare repository's linked worktrees (a bare repository
/// has no main worktree to read), and anything git cannot answer about — a projected mirror
/// carries no repository at all.
///
/// Only a `root` that is a linked worktree's top level is asked about, and that is read off
/// the filesystem before any git runs: a linked worktree's `.git` is a file naming its git
/// directory, where a main worktree's is a directory. So a main checkout, which is most
/// checkouts, spawns nothing here. And a `root` below a worktree's top level, or a directory
/// with no repository of its own inside one, never reads `<main>/.yidam/index`, which would be
/// another corpus's index.
fn main_worktree(root: &Path) -> Option<PathBuf> {
    if !root.join(".git").is_file() {
        return None;
    }
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
    if same_dir(&main, root) {
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

/// How many corpus files in `root` differ from `commit`: tracked files changed since it,
/// committed or not, and files git has not been told about. `None` when git cannot say.
///
/// The freshness count for a shared index. The local count compares file mtimes against the
/// build time, which a main checkout's history makes meaningful and a worktree's does not:
/// `git worktree add` writes every file after the main index was built, so by mtime a worktree
/// checked out a minute ago has changed everything.
pub fn corpus_changed_since(root: &Path, commit: &str) -> Option<usize> {
    let corpus = Path::new(".yidam").join("corpus");
    let changed = Git::new(root)
        .args(["diff", "--name-only"])
        .rev(commit)
        .paths([&corpus])
        .lines()?;
    let untracked = Git::new(root)
        .args(["ls-files", "--others", "--exclude-standard"])
        .paths([&corpus])
        .lines()?;
    Some(changed.len() + untracked.len())
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
    fn an_empty_local_index_dir_does_not_hide_the_main_index() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, wt) = repo_with_worktree(tmp.path());
        index_at(&main, &head(&main));
        std::fs::create_dir_all(yidam_index_dir(&wt)).unwrap();
        assert_eq!(
            resolve(&wt).found().map(|(_, s)| s),
            Some(IndexSource::Shared)
        );
    }

    /// A root below the worktree's top level is not the main worktree's corpus, and reading
    /// `<main>/.yidam/index` for it would answer from another corpus's index.
    #[test]
    fn a_root_below_the_worktree_top_level_reads_no_shared_index() {
        let tmp = tempfile::tempdir().unwrap();
        let (main, wt) = repo_with_worktree(tmp.path());
        index_at(&main, &head(&main));
        let sub = wt.join("sub");
        std::fs::create_dir_all(&sub).unwrap();
        assert_eq!(resolve(&sub), Resolution::None);
    }

    /// A worktree checked out after the main index was built has changed nothing, though
    /// every file in it is newer than the build.
    #[test]
    fn a_fresh_worktree_has_changed_no_corpus_file() {
        let tmp = tempfile::tempdir().unwrap();
        let main = tmp.path().join("main");
        std::fs::create_dir_all(&main).unwrap();
        init(&main);
        write(&main, ".yidam/corpus/a/x.yml", "id: x");
        commit(&main, "genesis: test");
        let built = head(&main);
        let wt = tmp.path().join("wt");
        git(
            &main,
            &["worktree", "add", "-q", "-b", "side", wt.to_str().unwrap()],
        );
        assert_eq!(corpus_changed_since(&wt, &built), Some(0));

        write(&wt, ".yidam/corpus/a/x.yml", "id: x\nedited: true");
        write(&wt, ".yidam/corpus/a/y.yml", "id: y");
        assert_eq!(corpus_changed_since(&wt, &built), Some(2));
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
