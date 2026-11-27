//! `--staged` on the corpus gates (RFC-0070, #1519).
//!
//! A commit is made from the index, and without this flag every gate reads the working tree.
//! A pre-commit hook running a gate as written then answers about the wrong tree. Both arms are
//! asserted for every gate, because a flag that ignored the index would pass one of them:
//! - a staged break with an unstaged fix fails `--staged` and passes without it;
//! - an unstaged break over a clean index does the reverse.
//!
//! A staged run checks the index inside a scratch worktree, so the rest of this file asserts
//! that the scratch tree never shows: not in what is printed, and not in `git worktree list`
//! afterwards.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::common;

/// The three gates `doctor` counts, as each is invoked.
const GATES: &[&[&str]] = &[&["graph-check"], &["lint"], &["regen", "--check"]];

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../sdks/parity/fixtures/reports/basic/repo")
}

fn copy_fixture(to: &Path) {
    let src = fixture_dir();
    for entry in walkdir::WalkDir::new(&src)
        .into_iter()
        .filter_map(Result::ok)
    {
        let dest = to.join(entry.path().strip_prefix(&src).unwrap());
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest).unwrap();
        } else {
            std::fs::create_dir_all(dest.parent().unwrap()).unwrap();
            std::fs::copy(entry.path(), &dest).unwrap();
        }
    }
}

fn git(root: &Path, args: &[&str]) {
    common::git::git_at(root, args, common::git::FIXTURE_DATE);
}

/// The reports fixture with every gate passing: its broken link fixed, a lint baseline, and
/// its REGEN blocks refreshed, all committed.
fn clean() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    copy_fixture(root);
    let low_flow = root.join(".yidam/corpus/concept/low-flow.yml");
    let text = std::fs::read_to_string(&low_flow).unwrap();
    std::fs::write(
        &low_flow,
        text.replace("assimilative-capacit.yml", "tailwater.yml"),
    )
    .unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "fixture@yidam.test"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "genesis: staged fixture"]);
    assert_eq!(run(root, &["lint", "--init-baseline"]).code, 0);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "observe: lint baseline"]);
    let fresh = run(root, &["regen", "--commit"]);
    assert_eq!(fresh.code, 0, "{}{}", fresh.stdout, fresh.stderr);
    for gate in GATES {
        let r = run(root, gate);
        assert_eq!(r.code, 0, "{gate:?} must pass on the fixture: {}", r.stdout);
    }
    tmp
}

/// An instance every gate fails on: a broken link for `graph-check`, an introduced finding
/// for `lint`, and a count the `status` block has not caught up with for `regen --check`.
const BREAK: &str = ".yidam/corpus/concept/extra.yml";
const BROKEN: &str = "class: concept\nlabel: Extra\nlinks:\n  - target: ../concept/nowhere.yml\n    relationship: relates-to\n";

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

fn run(root: &Path, args: &[&str]) -> Run {
    run_env(root, args, &[])
}

/// [`run`] with environment variables set, as a hook's environment sets them.
fn run_env(root: &Path, args: &[&str], env: &[(&str, &Path)]) -> Run {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_yidam"));
    cmd.current_dir(root).args(args).env_remove("YIDAM_ROOT");
    for (k, v) in env {
        cmd.env(k, v);
    }
    let out = cmd.output().unwrap();
    Run {
        code: out.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
    }
}

fn staged<'a>(gate: &[&'a str]) -> Vec<&'a str> {
    let mut v = gate.to_vec();
    v.push("--staged");
    v
}

fn worktrees(root: &Path) -> usize {
    common::git::out(root, &["worktree", "list", "--porcelain"])
        .lines()
        .filter(|l| l.starts_with("worktree "))
        .count()
}

#[test]
fn a_staged_break_fails_staged_though_the_working_tree_is_fixed() {
    let tmp = clean();
    let root = tmp.path();
    std::fs::write(root.join(BREAK), BROKEN).unwrap();
    git(root, &["add", BREAK]);
    std::fs::remove_file(root.join(BREAK)).unwrap();
    for gate in GATES {
        let plain = run(root, gate);
        assert_eq!(
            plain.code, 0,
            "{gate:?} reads the working tree: {}",
            plain.stdout
        );
        let s = run(root, &staged(gate));
        assert_eq!(
            s.code, 1,
            "{gate:?} --staged must read the index: {}{}",
            s.stdout, s.stderr
        );
    }
}

#[test]
fn an_unstaged_break_passes_staged() {
    let tmp = clean();
    let root = tmp.path();
    std::fs::write(root.join(BREAK), BROKEN).unwrap();
    for gate in GATES {
        assert_eq!(
            run(root, gate).code,
            1,
            "{gate:?} reads the untracked break"
        );
        let s = run(root, &staged(gate));
        assert_eq!(
            s.code, 0,
            "{gate:?} --staged must leave out what is not staged: {}{}",
            s.stdout, s.stderr
        );
    }
}

/// A failing staged run names the repository, in text and JSON, and leaves only the
/// repository's own worktree behind.
#[test]
fn a_staged_run_names_the_repository_and_leaves_no_worktree() {
    let tmp = clean();
    let root = tmp.path();
    std::fs::write(root.join(BREAK), BROKEN).unwrap();
    git(root, &["add", BREAK]);
    let canonical = root.canonicalize().unwrap();
    for gate in GATES {
        for format in ["text", "json"] {
            let mut args = staged(gate);
            args.extend(["--format", format]);
            let r = run(root, &args);
            assert_eq!(r.code, 1, "{args:?}: {}{}", r.stdout, r.stderr);
            let both = format!("{}{}", r.stdout, r.stderr);
            assert!(
                !both.contains("yidam-staged"),
                "{args:?} named the scratch tree:\n{both}"
            );
            if format == "json" {
                let v: serde_json::Value = serde_json::from_str(&r.stdout).unwrap();
                assert_eq!(
                    v["root"].as_str().map(PathBuf::from),
                    Some(canonical.clone()),
                    "{args:?}"
                );
            }
        }
    }
    let r = run(root, &["graph-check", "--staged"]);
    assert!(
        r.stdout.contains(BREAK),
        "the break is named by its path: {}",
        r.stdout
    );
    assert_eq!(worktrees(root), 1, "a staged run left a worktree behind");
}

/// The hook #1519 asked for. `git commit -a` makes the commit from a temporary index named by
/// `GIT_INDEX_FILE`, so this is also the case where reading `.git/index` would be wrong.
#[cfg(unix)]
#[test]
fn a_pre_commit_hook_gates_the_commit_it_is_run_for() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = clean();
    let root = tmp.path();
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\nset -e\n'{}' graph-check --staged\n",
            env!("CARGO_BIN_EXE_yidam")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();

    // A tracked instance broken in the working tree, committed with `-a`: refused.
    let low_flow = ".yidam/corpus/concept/low-flow.yml";
    let text = std::fs::read_to_string(root.join(low_flow)).unwrap();
    std::fs::write(
        root.join(low_flow),
        text.replace("tailwater.yml", "nowhere.yml"),
    )
    .unwrap();
    let refused = common::git::raw_at(
        root,
        &["commit", "-a", "-q", "-m", "observe: break"],
        common::git::FIXTURE_DATE,
    );
    assert!(
        !refused.status.success(),
        "the hook let a broken commit through"
    );

    // The same break left unstaged while another file is committed: let through.
    std::fs::write(root.join("NOTES.md"), "notes\n").unwrap();
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "-q", "-m", "observe: notes"]);
    assert_eq!(worktrees(root), 1);
}

/// The reports fixture with its corpus at `sub/`, below the repository's top level, its broken
/// link fixed and committed. Only `graph-check` is asked of it.
fn nested() -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let sub = root.join("sub");
    copy_fixture(&sub);
    let low_flow = sub.join(".yidam/corpus/concept/low-flow.yml");
    let text = std::fs::read_to_string(&low_flow).unwrap();
    std::fs::write(
        &low_flow,
        text.replace("assimilative-capacit.yml", "tailwater.yml"),
    )
    .unwrap();
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["config", "user.email", "fixture@yidam.test"]);
    git(root, &["config", "user.name", "Fixture"]);
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "genesis: nested fixture"]);
    let r = run(root, &["graph-check", "--root", "sub"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    tmp
}

/// `git commit -a` from a hook with `--root` below the top level. The break reaches the commit
/// only through the temporary index git hands the hook, and a relative `GIT_INDEX_FILE` read
/// against the corpus root would miss it.
#[cfg(unix)]
#[test]
fn a_pre_commit_hook_gates_a_commit_with_a_root_below_the_top_level() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = nested();
    let root = tmp.path();
    let hook = root.join(".git/hooks/pre-commit");
    std::fs::create_dir_all(hook.parent().unwrap()).unwrap();
    std::fs::write(
        &hook,
        format!(
            "#!/bin/sh\nset -e\n'{}' graph-check --staged --root sub\n",
            env!("CARGO_BIN_EXE_yidam")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
    let low_flow = root.join("sub/.yidam/corpus/concept/low-flow.yml");
    let text = std::fs::read_to_string(&low_flow).unwrap();
    std::fs::write(&low_flow, text.replace("tailwater.yml", "nowhere.yml")).unwrap();
    let refused = common::git::raw_at(
        root,
        &["commit", "-a", "-q", "-m", "observe: break"],
        common::git::FIXTURE_DATE,
    );
    assert!(
        !refused.status.success(),
        "the hook let a broken commit through"
    );
    std::fs::write(root.join("NOTES.md"), "notes\n").unwrap();
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "-q", "-m", "observe: notes"]);
    assert_eq!(worktrees(root), 1);
}

/// Every file under `root`, `.git/index` included and the rest of `.git` left out, by content.
fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut files: Vec<(PathBuf, Vec<u8>)> = walkdir::WalkDir::new(root)
        .into_iter()
        .filter_entry(|e| e.path() != root.join(".git"))
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_file())
        .map(|e| (e.path().to_path_buf(), std::fs::read(e.path()).unwrap()))
        .collect();
    files.push((
        PathBuf::from(".git/index"),
        std::fs::read(root.join(".git/index")).unwrap(),
    ));
    files.sort();
    files
}

/// A staged run started with a hook's location variables set writes nothing of the person's:
/// not the commit's index, and not the working tree (RFC-0070 §2).
#[test]
fn a_staged_run_inside_a_hook_leaves_the_index_and_working_tree_alone() {
    let tmp = clean();
    let root = tmp.path();
    std::fs::write(root.join(BREAK), BROKEN).unwrap();
    git(root, &["add", BREAK]);
    std::fs::remove_file(root.join(BREAK)).unwrap();
    let before = snapshot(root);
    let git_dir = root.join(".git");
    for gate in GATES {
        let r = run_env(
            root,
            &staged(gate),
            &[
                ("GIT_DIR", &git_dir),
                ("GIT_INDEX_FILE", Path::new(".git/index")),
            ],
        );
        assert_eq!(r.code, 1, "{gate:?}: {}{}", r.stdout, r.stderr);
        let after = snapshot(root);
        let changed: Vec<_> = after
            .iter()
            .filter(|f| !before.contains(f))
            .chain(before.iter().filter(|f| !after.contains(f)))
            .map(|(p, _)| p.display().to_string())
            .collect();
        assert!(changed.is_empty(), "{gate:?} wrote {changed:?}");
    }
    assert_eq!(worktrees(root), 1);
}

/// The envelope's `root` is the one a working-tree run names, from a symlinked directory and
/// with `--root` below the top level.
#[test]
fn a_staged_run_names_the_root_a_working_tree_run_names() {
    fn json_root(r: &Run) -> String {
        let v: serde_json::Value = serde_json::from_str(&r.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}{}", r.stdout, r.stderr));
        v["root"].as_str().unwrap().to_string()
    }
    let tmp = nested();
    let root = tmp.path();
    std::fs::write(root.join("sub").join(BREAK), BROKEN).unwrap();
    git(root, &["add", "-A"]);
    let args = ["graph-check", "--root", "sub", "--format", "json"];
    let plain = run(root, &args);
    let mut with = args.to_vec();
    with.push("--staged");
    let s = run(root, &with);
    assert_eq!(s.code, 1, "{}{}", s.stdout, s.stderr);
    assert!(!s.stdout.contains("yidam-staged"), "{}", s.stdout);
    assert_eq!(json_root(&s), json_root(&plain));

    #[cfg(unix)]
    {
        let links = tempfile::tempdir().unwrap();
        let link = links.path().join("link");
        std::os::unix::fs::symlink(root, &link).unwrap();
        let plain = run(&link, &args);
        let s = run(&link, &with);
        assert_eq!(s.code, 1, "{}{}", s.stdout, s.stderr);
        assert!(!s.stdout.contains("yidam-staged"), "{}", s.stdout);
        assert_eq!(json_root(&s), json_root(&plain));
    }
    assert_eq!(worktrees(root), 1);
}

/// A repository with an LFS filter and no LFS server. The smudge filter here fails unless git
/// is told to leave pointers alone, as `git-lfs` is by `GIT_LFS_SKIP_SMUDGE`, so the test
/// needs no `git-lfs` and no network.
#[test]
fn a_staged_run_does_not_smudge_lfs_files() {
    let tmp = clean();
    let root = tmp.path();
    git(root, &["config", "filter.lfs.clean", "cat"]);
    git(
        root,
        &[
            "config",
            "filter.lfs.smudge",
            "test \"$GIT_LFS_SKIP_SMUDGE\" = 1 && cat",
        ],
    );
    git(root, &["config", "filter.lfs.required", "true"]);
    std::fs::write(root.join(".gitattributes"), "*.bin filter=lfs\n").unwrap();
    std::fs::write(root.join("data.bin"), "pointer\n").unwrap();
    git(root, &["add", "-A"]);
    git(root, &["commit", "-q", "-m", "observe: lfs file"]);
    let r = run(root, &["graph-check", "--staged"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    std::fs::write(root.join(BREAK), BROKEN).unwrap();
    git(root, &["add", BREAK]);
    let r = run(root, &["graph-check", "--staged"]);
    assert_eq!(r.code, 1, "{}{}", r.stdout, r.stderr);
    assert_eq!(worktrees(root), 1);
}

#[test]
fn a_staged_run_refuses_an_unborn_head() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    copy_fixture(root);
    git(root, &["init", "-q", "-b", "main"]);
    git(root, &["add", "-A"]);
    let r = run(root, &["graph-check", "--staged"]);
    assert_ne!(r.code, 0);
    assert!(
        r.stderr.contains("nothing is committed yet"),
        "{}",
        r.stderr
    );
}

#[test]
fn a_staged_run_refuses_an_unmerged_index() {
    let tmp = clean();
    let root = tmp.path();
    git(root, &["checkout", "-q", "-b", "side"]);
    std::fs::write(root.join("NOTES.md"), "side\n").unwrap();
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "-q", "-m", "observe: side"]);
    git(root, &["checkout", "-q", "main"]);
    std::fs::write(root.join("NOTES.md"), "main\n").unwrap();
    git(root, &["add", "NOTES.md"]);
    git(root, &["commit", "-q", "-m", "observe: main"]);
    let merge = common::git::raw_at(root, &["merge", "-q", "side"], common::git::FIXTURE_DATE);
    assert!(!merge.status.success(), "the fixture needs a conflict");
    let r = run(root, &["lint", "--staged"]);
    assert_ne!(r.code, 0);
    assert!(r.stderr.contains("unmerged paths"), "{}", r.stderr);
    assert_eq!(worktrees(root), 1);
}

/// `CI_GATES` in `doctor.rs` and the commands that take `--staged` are one list (RFC-0070
/// §3). Read from the source, so the list is written once, and asked of the built binary in
/// both directions: a gate without the flag, and a flag on a command that is not a gate.
#[test]
fn the_corpus_gates_are_the_commands_that_take_staged() {
    let src = std::fs::read_to_string(
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/cmd/doctor.rs"),
    )
    .unwrap();
    let start = src
        .find("const CI_GATES: &[&str] = &[")
        .expect("doctor.rs declares CI_GATES");
    let list = &src[start..start + src[start..].find("];").unwrap()];
    let gates: std::collections::BTreeSet<String> = list
        .split('"')
        .skip(1)
        .step_by(2)
        .map(|g| {
            g.split_whitespace()
                .nth(1)
                .expect("each gate is `yidam <command> …`")
                .to_string()
        })
        .collect();
    assert_eq!(gates.len(), 3, "read {gates:?} out of CI_GATES");

    let takes_staged: std::collections::BTreeSet<String> = common::commands_from_help()
        .into_iter()
        .filter(|cmd| {
            let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
                .args([cmd.as_str(), "--help"])
                .output()
                .unwrap();
            String::from_utf8_lossy(&out.stdout).contains("--staged")
        })
        .collect();
    assert_eq!(takes_staged, gates);
}

/// The sweep before a staged run leaves another user's run alone (#1703). pid 1 always runs
/// and, unless the tests run as root, may not be signalled; an exited child's pid is swept.
#[cfg(unix)]
#[test]
fn a_staged_run_sweeps_an_exited_runs_files_and_not_a_running_ones() {
    let tmp = clean();
    let root = tmp.path();
    let mut child = Command::new("true").spawn().unwrap();
    let dead = child.id();
    child.wait().unwrap();
    let git_dir = root.join(".git");
    let others = git_dir.join("yidam-staged-1-index");
    let exited = git_dir.join(format!("yidam-staged-{dead}-index"));
    std::fs::write(&others, "").unwrap();
    std::fs::write(&exited, "").unwrap();
    let r = run(root, &["regen", "--check", "--staged"]);
    assert_eq!(r.code, 0, "{}{}", r.stdout, r.stderr);
    assert!(others.exists(), "pid 1's file was swept");
    assert!(!exited.exists(), "an exited run's file was left");
}
