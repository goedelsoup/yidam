//! Writing proposals as commits, without touching the working tree.
//!
//! Plumbing against a temporary index rather than a checkout. The reason is not tidiness: a
//! command that stashed, branched, committed and switched back would fail halfway on a dirty
//! tree and leave somebody's work somewhere they did not put it. Nothing here reads or writes
//! `.git/index`, so `yidam propose` is safe to run mid-edit and changes nothing a person can
//! see — it writes objects and one ref.
//!
//! That is also what keeps `--help` honest. The `*` marker means a command *rewrites files in
//! the repository it is run against*; this one carries it, because it does write to the
//! repository, and its long help says which part.
//!
//! # Author and committer are different, on purpose
//!
//! git separates them and the distinction is exactly the one needed: the tool drafted the
//! commit, a person ran it, and a person will merge it or delete it. Recording both is a true
//! account of what happened without inventing an attestation mechanism — RFC-0012's elector
//! identity work is open (#274), and a proposal is not an elector's position, so it must not
//! borrow one's name.

use std::path::Path;
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use super::draft::{Change, Proposal};

/// The author every proposed commit carries.
const AUTHOR_NAME: &str = "yidam propose";
const AUTHOR_EMAIL: &str = "propose@yidam";

/// What a run wrote.
#[derive(Debug, Clone, serde::Serialize)]
pub struct Written {
    pub branch: String,
    /// Short shas, oldest first — the order `git log --reverse` prints them in.
    pub commits: Vec<String>,
}

/// Run git at `root`, with an optional temporary index and optional stdin.
fn git(root: &Path, index: Option<&Path>, args: &[&str], stdin: Option<&str>) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root).args(args);
    if let Some(i) = index {
        cmd.env("GIT_INDEX_FILE", i);
    }
    cmd.stdin(if stdin.is_some() {
        Stdio::piped()
    } else {
        Stdio::null()
    });
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().context("running git")?;
    if let Some(text) = stdin {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .expect("stdin piped")
            .write_all(text.as_bytes())
            .context("writing to git")?;
    }
    let out = child.wait_with_output().context("running git")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// `HEAD` in full and short form.
///
/// Both, because they answer different questions. The full sha is the parent commit; the
/// short one names the branch, and its *length is git's choice* from this repository's object
/// count — so it is read rather than derived by truncating, exactly as `model.rs` reads the
/// bundle manifest's commit.
pub fn head(root: &Path) -> Result<(String, String)> {
    let full = git(root, None, &["rev-parse", "HEAD"], None)?;
    let short = git(root, None, &["rev-parse", "--short", "HEAD"], None)?;
    Ok((full, short))
}

/// Refuse to propose against a corpus that differs from the commit the proposals are built
/// on.
///
/// The findings come from the working tree — `lint` walks `.yidam/` on disk — and the commits
/// are built on `HEAD`'s tree, because that is what a temporary index can be seeded from
/// without touching the checkout. When those two disagree, every proposal is drafted from one
/// corpus and applied to another: a question is appended to a node whose committed text is
/// not the text it was read from, and a withdrawal removes a version nobody looked at.
///
/// This is the failure `fix(cli): the corpus at a commit was not the one the walk reads` was
/// about, in the other direction, and the answer is the same one: refuse rather than guess.
/// Only `.yidam/` is checked, because that is the whole of what these three acts read and
/// write — a repository with uncommitted application code is not the case at issue, and
/// gating on it would make this command unusable in exactly the repositories that also hold
/// code.
pub fn require_committed_corpus(root: &Path) -> Result<()> {
    let dirty = git(root, None, &["status", "--porcelain", "--", ".yidam"], None)?;
    if dirty.trim().is_empty() {
        return Ok(());
    }
    bail!(
        "`.yidam/` has uncommitted changes, and a proposal cannot be drafted from one \
         corpus and applied to another:\n{}\n\n  Findings are read from the working tree; \
         commits are built on HEAD. Commit or stash the corpus first.",
        dirty
            .lines()
            .map(|l| format!("  {}", l.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    )
}

/// The branch a run at this HEAD writes to.
///
/// Named after the commit rather than the clock, for the reason the residence clock is
/// counted in commits: a date is a function of when you ran the command and a commit is a
/// function of the repository. Two runs at the same HEAD address the same branch, and a
/// proposal branch left behind by an older HEAD is visibly stale.
pub fn branch_for(short_head: &str) -> String {
    format!("propose/{short_head}")
}

fn branch_exists(root: &Path, branch: &str) -> bool {
    Command::new("git")
        .current_dir(root)
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// A scratch index, inside the git directory and removed when the run ends.
///
/// Not a system temp file: the index must be on the same filesystem as the object store for
/// git's rename-into-place to work, and `$GIT_DIR` is where git puts its own. Named so that a
/// leftover after a crash is attributable rather than mysterious.
struct TempIndex(std::path::PathBuf);

impl TempIndex {
    fn new(root: &Path) -> Result<Self> {
        let dir = git(root, None, &["rev-parse", "--absolute-git-dir"], None)?;
        let path = Path::new(&dir).join("yidam-propose-index");
        // A leftover from an interrupted run would be read as the starting tree.
        let _ = std::fs::remove_file(&path);
        Ok(Self(path))
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempIndex {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Write every proposal as one commit on `propose/<head>`.
///
/// Refuses an existing branch unless `force`. A re-run at the same HEAD produces the same
/// proposals in the same order but not byte-identical commits, because the committer date is
/// real and this will not falsify it to buy an idempotent sha. So the honest options are
/// refuse or replace, and refusing is the default.
pub fn write(root: &Path, proposals: &[Proposal], force: bool) -> Result<Written> {
    let (full, short) = head(root)?;
    let branch = branch_for(&short);
    if branch_exists(root, &branch) && !force {
        bail!(
            "{branch} already exists — this HEAD has been proposed on already.\n  \
             Review it with `git log --reverse {}..{branch}`, delete it to reject it, or \
             re-run with --force to replace it.",
            current_branch(root).unwrap_or_else(|| "HEAD".into())
        );
    }

    // Held for the whole function: dropping it deletes the index the commits are built from.
    let scratch = TempIndex::new(root)?;
    let index = scratch.path().to_path_buf();
    git(root, Some(&index), &["read-tree", "HEAD"], None)?;

    let mut parent = full.clone();
    let mut commits = Vec::new();
    for p in proposals {
        for change in &p.changes {
            match change {
                Change::Write { path, content } => {
                    let blob = git(
                        root,
                        Some(&index),
                        &["hash-object", "-w", "--stdin"],
                        Some(content),
                    )?;
                    git(
                        root,
                        Some(&index),
                        &[
                            "update-index",
                            "--add",
                            "--cacheinfo",
                            &format!("100644,{blob},{path}"),
                        ],
                        None,
                    )?;
                }
                Change::Remove { path } => {
                    git(
                        root,
                        Some(&index),
                        &["update-index", "--force-remove", path],
                        None,
                    )?;
                }
            }
        }
        let tree = git(root, Some(&index), &["write-tree"], None)?;
        let sha = commit_tree(root, &tree, &parent, &p.message(&short))?;
        commits.push(short_of(root, &sha));
        parent = sha;
    }

    if commits.is_empty() {
        return Ok(Written {
            branch,
            commits: vec![],
        });
    }
    git(
        root,
        None,
        &["update-ref", &format!("refs/heads/{branch}"), &parent],
        None,
    )?;
    Ok(Written { branch, commits })
}

/// `git commit-tree`, with the tool as author and whoever ran it as committer.
fn commit_tree(root: &Path, tree: &str, parent: &str, message: &str) -> Result<String> {
    let mut cmd = Command::new("git");
    cmd.current_dir(root)
        .args(["commit-tree", tree, "-p", parent, "-F", "-"])
        .env("GIT_AUTHOR_NAME", AUTHOR_NAME)
        .env("GIT_AUTHOR_EMAIL", AUTHOR_EMAIL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().context("running git commit-tree")?;
    {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .expect("stdin piped")
            .write_all(message.as_bytes())
            .context("writing the commit message")?;
    }
    let out = child
        .wait_with_output()
        .context("running git commit-tree")?;
    if !out.status.success() {
        bail!(
            "git commit-tree failed: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn short_of(root: &Path, sha: &str) -> String {
    git(root, None, &["rev-parse", "--short", sha], None).unwrap_or_else(|_| sha.to_string())
}

/// The branch a reader is on, for the `git log` line the refusal prints.
fn current_branch(root: &Path) -> Option<String> {
    let name = git(root, None, &["rev-parse", "--abbrev-ref", "HEAD"], None).ok()?;
    (name != "HEAD").then_some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cmd::propose::draft::Verb;

    fn repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "t@t"],
            vec!["config", "user.name", "Tester"],
        ] {
            Command::new("git")
                .current_dir(root)
                .args(&args)
                .status()
                .unwrap();
        }
        std::fs::create_dir_all(root.join(".yidam/corpus/concept")).unwrap();
        std::fs::write(root.join(".yidam/corpus/concept/a.yml"), "class: concept\n").unwrap();
        std::fs::write(root.join("README.md"), "seed\n").unwrap();
        for args in [
            vec!["add", "-A"],
            vec!["commit", "-q", "-m", "genesis: seed"],
        ] {
            Command::new("git")
                .current_dir(root)
                .args(&args)
                .status()
                .unwrap();
        }
        dir
    }

    fn proposal(verb: Verb, changes: Vec<Change>) -> Proposal {
        Proposal {
            verb,
            subject: "a — something held".into(),
            body: "nothing links to this node\n\nbecause.".into(),
            check: "orphan-in".into(),
            node: ".yidam/corpus/concept/a.yml".into(),
            detail: "nothing links to this node".into(),
            changes,
        }
    }

    fn show(root: &Path, args: &[&str]) -> String {
        git(root, None, args, None).unwrap()
    }

    #[test]
    fn a_run_writes_a_branch_and_leaves_the_working_tree_exactly_as_it_was() {
        let dir = repo();
        let root = dir.path();
        let before = show(root, &["status", "--porcelain"]);
        let head_before = show(root, &["rev-parse", "HEAD"]);

        let w = write(
            root,
            &[proposal(
                Verb::Open,
                vec![Change::Write {
                    path: ".yidam/corpus/concept/a.yml".into(),
                    content: "class: concept\n# proposed\n".into(),
                }],
            )],
            false,
        )
        .unwrap();

        assert_eq!(w.commits.len(), 1);
        assert!(w.branch.starts_with("propose/"));
        assert_eq!(
            show(root, &["status", "--porcelain"]),
            before,
            "the tree moved"
        );
        assert_eq!(
            show(root, &["rev-parse", "HEAD"]),
            head_before,
            "HEAD moved"
        );
        assert_eq!(
            std::fs::read_to_string(root.join(".yidam/corpus/concept/a.yml")).unwrap(),
            "class: concept\n",
            "the file on disk was rewritten"
        );
        // But the branch has it.
        let blob = show(
            root,
            &["show", &format!("{}:.yidam/corpus/concept/a.yml", w.branch)],
        );
        assert_eq!(blob, "class: concept\n# proposed");
    }

    /// An uncommitted edit outside `.yidam/` is none of this command's business, and a
    /// repository that also holds application code is the common case rather than the
    /// exception.
    #[test]
    fn an_uncommitted_edit_outside_the_corpus_is_allowed_and_not_committed() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(root.join("README.md"), "edited, and not staged\n").unwrap();

        require_committed_corpus(root).expect("only .yidam/ is checked");
        let w = write(
            root,
            &[proposal(
                Verb::Open,
                vec![Change::Write {
                    path: ".yidam/corpus/concept/a.yml".into(),
                    content: "class: concept\n# proposed\n".into(),
                }],
            )],
            false,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(root.join("README.md")).unwrap(),
            "edited, and not staged\n",
            "the edit is still there"
        );
        assert_eq!(
            show(root, &["show", &format!("{}:README.md", w.branch)]),
            "seed",
            "and the proposal is built on HEAD, so it did not sweep the edit in"
        );
    }

    /// A corpus that differs from HEAD means the findings and the tree disagree, and every
    /// proposal would be drafted from one and applied to the other.
    #[test]
    fn a_dirty_corpus_is_refused_and_says_which_file() {
        let dir = repo();
        let root = dir.path();
        std::fs::write(
            root.join(".yidam/corpus/concept/a.yml"),
            "class: concept\nlabel: edited\n",
        )
        .unwrap();
        let err = require_committed_corpus(root).unwrap_err().to_string();
        assert!(err.contains("uncommitted changes"), "{err}");
        assert!(err.contains("concept/a.yml"), "it names the file: {err}");
        assert!(err.contains("Commit or stash"), "and the way out: {err}");
    }

    #[test]
    fn the_tool_authors_and_the_person_who_ran_it_commits() {
        let dir = repo();
        let root = dir.path();
        let w = write(
            root,
            &[proposal(
                Verb::Open,
                vec![Change::Write {
                    path: ".yidam/corpus/concept/a.yml".into(),
                    content: "class: concept\n# p\n".into(),
                }],
            )],
            false,
        )
        .unwrap();
        let who = show(root, &["log", "-1", "--format=%an|%ae|%cn|%ce", &w.branch]);
        assert_eq!(who, format!("{AUTHOR_NAME}|{AUTHOR_EMAIL}|Tester|t@t"));
    }

    #[test]
    fn a_withdrawal_removes_the_file_on_the_branch_only() {
        let dir = repo();
        let root = dir.path();
        let w = write(
            root,
            &[proposal(
                Verb::Withdraw,
                vec![Change::Remove {
                    path: ".yidam/corpus/concept/a.yml".into(),
                }],
            )],
            false,
        )
        .unwrap();
        assert!(
            root.join(".yidam/corpus/concept/a.yml").exists(),
            "still on disk"
        );
        let listed = show(root, &["ls-tree", "-r", "--name-only", &w.branch]);
        assert!(!listed.contains("concept/a.yml"), "{listed}");
        assert!(listed.contains("README.md"), "and everything else survived");
    }

    #[test]
    fn each_proposal_is_its_own_commit_in_order() {
        let dir = repo();
        let root = dir.path();
        let w = write(
            root,
            &[
                proposal(
                    Verb::Open,
                    vec![Change::Write {
                        path: "one.md".into(),
                        content: "1\n".into(),
                    }],
                ),
                proposal(
                    Verb::Close,
                    vec![Change::Write {
                        path: "two.md".into(),
                        content: "2\n".into(),
                    }],
                ),
            ],
            false,
        )
        .unwrap();
        assert_eq!(w.commits.len(), 2);
        let subjects = show(
            root,
            &[
                "log",
                "--reverse",
                "--format=%s",
                &format!("main..{}", w.branch),
            ],
        );
        let lines: Vec<&str> = subjects.lines().collect();
        assert!(lines[0].starts_with("open: "), "{subjects}");
        assert!(lines[1].starts_with("close: "), "{subjects}");
    }

    #[test]
    fn a_second_run_at_the_same_head_refuses_unless_forced() {
        let dir = repo();
        let root = dir.path();
        let one = vec![proposal(
            Verb::Open,
            vec![Change::Write {
                path: "one.md".into(),
                content: "1\n".into(),
            }],
        )];
        write(root, &one, false).unwrap();

        let err = write(root, &one, false).unwrap_err().to_string();
        assert!(err.contains("already exists"), "{err}");
        assert!(err.contains("--force"), "the way out is named: {err}");

        write(root, &one, true).expect("--force replaces it");
    }

    /// Nothing to propose writes no ref. An empty branch would read as a run that found
    /// something and could not say what.
    #[test]
    fn no_proposals_writes_no_branch() {
        let dir = repo();
        let root = dir.path();
        let w = write(root, &[], false).unwrap();
        assert!(w.commits.is_empty());
        assert!(!branch_exists(root, &w.branch));
    }
}
