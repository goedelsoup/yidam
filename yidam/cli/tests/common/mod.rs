//! The layout bootstrap installs, and how to build one.
//!
//! Shared by [`installed_layout_links`], which checks that every relative link resolves in
//! this layout, and [`derived_repo_smoke`], which materializes it and runs its gate. One
//! definition, because a test checking a layout nobody installs is worse than no test — and
//! two copies of the mapping is the shortest path to exactly that.

#![allow(dead_code)] // each test binary uses a different part of this

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

// ── the example corpora ───────────────────────────────────────────────────────
//
// Here rather than in one suite because two of them gate examples — `example_corpus` runs
// the corpus checks, `class_schemas` validates every instance against its compiled schema —
// and both were pinned to `examples/streamflow/` by name (#448). A second copy of the
// discovery rule is the same hole one file over.

/// Every example corpus, by directory name, from `git ls-files`.
///
/// An example is a directory directly under `examples/` that contains a `.yidam/`. Read from
/// git rather than from a directory walk for the reason [`Example::materialize`] gives about
/// its own copy: a walk picks up `.DS_Store` and local scratch, and this suite would then be
/// measuring the maintainer's working directory rather than the repository.
pub fn examples() -> Vec<String> {
    let mut names = BTreeSet::new();
    for path in tracked_under(&repo_root(), "examples/") {
        let mut parts = path.split('/');
        if parts.next() != Some("examples") {
            continue;
        }
        let Some(name) = parts.next() else { continue };
        if parts.next() != Some(".yidam") {
            continue;
        }
        names.insert(name.to_string());
    }
    names.into_iter().collect()
}

/// The same set, read from the filesystem instead of from git.
///
/// Only used by [`every_example_on_disk_is_discovered`], and deliberately computed a
/// different way: two derivations of one set can disagree, and one computation compared
/// against itself cannot.
pub fn examples_on_disk() -> Vec<String> {
    let dir = repo_root().join("examples");
    let mut names = BTreeSet::new();
    for entry in std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", dir.display()))
        .flatten()
    {
        if entry.path().join(".yidam").is_dir() {
            names.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.into_iter().collect()
}

/// One template path and where bootstrap puts it.
pub struct Install {
    /// Path in this repository, a file or a directory prefix.
    pub src: &'static str,
    /// Installed path in a derived repository. `None` means consumed at genesis — present
    /// here, absent in every derived repository.
    pub dst: Option<&'static str>,
    /// The bootstrap answer this row is contingent on, if any. `None` is unconditional:
    /// every derived repository has it.
    pub when: Option<&'static str>,
}

/// Governance mode `sadhana/sangha/` requires. The default is `single-elector`, which
/// installs no sangha at all.
pub const COLLECTIVE: &str = "governance: collective";

/// A step 5 calculator named a `prelude/domains/` library. The default is that none does,
/// and step 8 then vendors no `domains/` directory at all.
pub const DOMAIN_SELECTED: &str = "prelude_domains: non-empty";

pub const MAPPING: &[Install] = &[
    // The domain libraries, before the prelude row so the prefix match reaches them first.
    // Fifteen libraries in three languages each — about 320 of the ~540 files the vendor
    // step moves — and step 8 keeps only what a step 5 calculator named. A derived
    // repository has no task that builds them, no workspace that includes them, and no CI
    // job that runs them; `domain-parity` is yidam's gate and does not travel.
    Install {
        src: "yidam/prelude/domains",
        dst: Some(".yidam/.vendor/prelude/domains"),
        when: Some(DOMAIN_SELECTED),
    },
    // The vendored prelude. One directory, deliberately: everything else under `yidam/`
    // is yidam's own machinery and does not survive the vendor step.
    row("yidam/prelude", Some(".yidam/.vendor/prelude")),
    // Root files. `sadhana/root/` is not a directory mirror — each file installs to a
    // specific path, overwriting yidam's own copy.
    row("sadhana/root/README.md", Some("README.md")),
    row("sadhana/root/AGENTS.md", Some("AGENTS.md")),
    row("sadhana/root/CLAUDE.md", Some(".claude/CLAUDE.md")),
    row("sadhana/root/mise.toml", Some("mise.toml")),
    row("sadhana/root/gitattributes", Some(".gitattributes")),
    row("sadhana/root/gitignore", Some(".gitignore")),
    // A directory mirror, not two named files (#589). Step 3 used to name `ci.yml` and
    // `release.yml` and overwrite only those two, leaving the rest of yidam's own
    // `.github/workflows/` behind — `docs.yml`, `editor.yml`, `install-channels.yml`,
    // `publish-crates.yml`, `tap.yml` — each naming a layout that does not survive genesis,
    // and `index.yml` shipped in the scaffold with no row to install it at all. Whatever
    // `sadhana/github/workflows/` holds is what a derived repository's `.github/workflows/`
    // becomes, in full — enumerated by [`materialize`], never named here.
    row("sadhana/github/workflows", Some(".github/workflows")),
    // Directory mirrors.
    Install {
        src: "sadhana/sangha",
        dst: Some(".yidam/sangha"),
        when: Some(COLLECTIVE),
    },
    row("sadhana/catalog", Some(".yidam/catalog")),
    row("sadhana/corpus", Some(".yidam/corpus")),
    row("sadhana/skills", Some(".yidam/skills")),
    row("sadhana/crates", Some("crates")),
    row("sadhana/web", Some("web")),
    // Created on first use rather than at genesis, but they install here when they are.
    row("sadhana/agents", Some("agents")),
    row("sadhana/packages", Some("packages")),
    row("sadhana/docs", Some("docs")),
    // Consumed.
    row("sadhana/README.md", None),
    row("samudaya", None),
];

/// An unconditional row — everything but the sangha.
const fn row(src: &'static str, dst: Option<&'static str>) -> Install {
    Install {
        src,
        dst,
        when: None,
    }
}

/// Files a derived repository keeps from the template root, unchanged.
///
/// The bootstrap skill names these explicitly: "Keep `LICENSE` and `mise.yidam.toml`".
/// `.gitattributes` and `.gitignore` are *not* among them — both arrive through [`MAPPING`],
/// overwritten in step 3 from `sadhana/root/`. `.gitignore` was on this list for as long as
/// bootstrap called it generic, which shipped yidam's own — including an ignore for a path
/// under `yidam/tests/` that the vendor step deletes.
pub const KEPT_AT_ROOT: &[&str] = &["LICENSE", "mise.yidam.toml"];

/// Directories bootstrap creates empty.
///
/// **One, and step 3's tree listing is the authority**: `.yidam/decisions/` — "new, empty;
/// written to in steps 2 and 5". `.yidam/embeddings/` and `.yidam/index/` were also here, and
/// were a fiction twice over. Bootstrap creates neither, and git carries no empty directory,
/// so no clone could have received them however genesis behaved. The cost was not cosmetic:
/// `yidam status` decided a committed cell by stat-ing `.yidam/index/`, and against a fixture
/// where that directory exists from genesis the cell reads `present` before anything is built
/// and `present` after — so the test that should have caught #895 watched a value that could
/// not vary.
pub const CREATED_EMPTY: &[&str] = &[".yidam/decisions"];

/// Paths a derived repository holds regardless of what any [`MAPPING`] row produces.
///
/// Mostly things no template file becomes — the provenance pin, the directories bootstrap
/// creates empty. `.gitattributes` and `.gitignore` are the exceptions and are redundant
/// here: both also arrive through [`MAPPING`], and the tree is a set, so naming them twice
/// costs nothing and asserts they are present however they got there.
pub const ALWAYS_PRESENT: &[&str] = &[
    "LICENSE",
    ".gitignore",
    ".gitattributes",
    "mise.yidam.toml",
    ".yidam.toml",
    ".yidam",
    ".yidam/decisions",
    ".yidam/private-paths",
];

/// The row covering `rel`, and where `rel` itself lands.
pub fn install_of(rel: &str) -> Option<(&'static Install, Option<String>)> {
    for e in MAPPING {
        if rel == e.src {
            return Some((e, e.dst.map(str::to_string)));
        }
        let prefix = format!("{}/", e.src);
        if let Some(tail) = rel.strip_prefix(&prefix) {
            return Some((e, e.dst.map(|d| format!("{d}/{tail}"))));
        }
    }
    None
}

/// Files git tracks under `prefix`, repo-relative.
///
/// Tracked rather than walked, because the production vendor step copies out of a
/// `git clone` and therefore never sees `target/`, `__pycache__`, `.pytest_cache` or
/// `.DS_Store`. A directory walk of a working tree picks up ~38,000 files against the 495 a
/// derived repository actually receives, and would have this test measuring the maintainer's
/// build output.
///
/// `ls-files` and not `archive HEAD`, so an uncommitted edit is covered too — the point is
/// to test the tree about to be committed, not the one already was.
pub fn tracked_under(root: &Path, prefix: &str) -> Vec<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(["ls-files", "-z", "--", prefix])
        .output()
        .expect("git ls-files");
    assert!(out.status.success(), "git ls-files failed for {prefix}");
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Build the repository bootstrap produces, in `target`.
///
/// `conditions` is what the bootstrap dialogue answered yes to; an empty set is
/// single-elector, the documented default and the case the skill takes when the user is
/// unsure. Returns the number of files written.
pub fn materialize(root: &Path, target: &Path, conditions: &BTreeSet<&str>) -> usize {
    let mut written = 0;
    for e in MAPPING {
        if e.dst.is_none() {
            continue; // consumed at genesis
        }
        if e.when.is_some_and(|w| !conditions.contains(w)) {
            continue; // the answer this row needed was not given
        }
        for tracked in tracked_under(root, e.src) {
            let dest = match install_of(&tracked) {
                // The condition of the row that claims this path, not of the row being
                // walked — see `installed_tree` for what a nested conditional row does when
                // only the outer one is consulted.
                Some((owner, Some(d))) if owner.when.is_none_or(|w| conditions.contains(w)) => d,
                _ => continue,
            };
            let to = target.join(&dest);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(&tracked), &to)
                .unwrap_or_else(|e| panic!("copy {tracked} -> {dest}: {e}"));
            written += 1;
        }
    }
    for keep in KEPT_AT_ROOT {
        std::fs::copy(root.join(keep), target.join(keep)).unwrap_or_else(|e| panic!("{keep}: {e}"));
        written += 1;
    }
    for dir in CREATED_EMPTY {
        std::fs::create_dir_all(target.join(dir)).unwrap();
    }
    written
}

// ── the example corpora, materialized ─────────────────────────────────────────
//
// Moved here from `example_corpus` when a second suite needed a corpus it could run a
// command inside: `walkthrough_transcripts` re-runs what the walkthrough pages record, and
// a page's transcript is only evidence if it came from the same tree the example gate
// checks. Two materialisations would be two trees, and the pages would be pinned to the one
// nothing else looks at.

/// The examples an example declares a path dependency on.
///
/// `.yidam/tonpa.toml` is where a path dependency is declared, and its `path` is relative to
/// the repository root — so `../property` from `examples/journalism` names `examples/property`,
/// and that is the only form this reads. A fetched dependency needs a published bundle and a
/// network, which an example gate must not.
pub fn path_dependencies(corpus: &Path) -> Vec<String> {
    #[derive(serde::Deserialize, Default)]
    struct Dep {
        path: Option<String>,
    }
    #[derive(serde::Deserialize, Default)]
    struct Config {
        #[serde(default)]
        dependencies: std::collections::BTreeMap<String, Dep>,
    }
    let text = match std::fs::read_to_string(corpus.join(".yidam/tonpa.toml")) {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let cfg: Config = toml::from_str(&text)
        .unwrap_or_else(|e| panic!("{}/.yidam/tonpa.toml is unusable: {e}", corpus.display()));
    cfg.dependencies
        .into_values()
        .filter_map(|d| d.path)
        .filter_map(|p| p.strip_prefix("../").map(str::to_string))
        .collect()
}

/// The manifest naming the order an example was written in, if it ships one.
const HISTORY: &str = "history.toml";

/// One commit in that order: what it says, when, and which path prefixes it introduces.
#[derive(serde::Deserialize)]
struct HistoryCommit {
    message: String,
    date: Option<String>,
    paths: Vec<String>,
}

#[derive(serde::Deserialize)]
struct History {
    commit: Vec<HistoryCommit>,
}

/// The order `examples/<name>` was written in, or `None` where it does not say.
///
/// `yidam replay` reconstructs corpus health at every commit that touched the corpus, so a
/// corpus materialised with one genesis commit gives it nothing to reconstruct, and a
/// walkthrough's replay section would be a description of the feature rather than a run of it
/// (#452). An example that ships a manifest gets that history; every other example gets the
/// single genesis commit it always had, which is why this returns an option rather than a
/// default.
fn history(root: &Path, name: &str) -> Option<Vec<HistoryCommit>> {
    let p = root.join(format!("examples/{name}/{HISTORY}"));
    let text = std::fs::read_to_string(&p).ok()?;
    let parsed: History = toml::from_str(&text)
        .unwrap_or_else(|e| panic!("{} is not a usable history manifest: {e}", p.display()));
    assert!(
        !parsed.commit.is_empty(),
        "{} declares no commits; delete it rather than shipping an empty history",
        p.display()
    );
    Some(parsed.commit)
}

fn git(dir: &Path, args: &[&str], name: &str) {
    let ok = Command::new("git")
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    assert!(ok, "git {args:?} failed for {name}");
}

/// Build the repository one commit at a time, in the order the manifest gives.
///
/// Paths are **prefixes**, so a commit names a directory or a file and picks up whatever is
/// under it. A commit that stages nothing is a stale manifest entry and fails here rather than
/// producing an empty commit nobody would notice — that is the failure mode this whole file
/// exists to prevent, one layer down.
///
/// Whatever the manifest does not name is swept into a final commit rather than dropped: a
/// file added to an example without a thought about its history must still reach the corpus,
/// or the gate would be checking a subset of what ships and saying nothing.
fn replay_history(dir: &Path, name: &str, commits: &[HistoryCommit], copied: &[String]) {
    for c in commits {
        let mut staged = false;
        for rel in copied {
            if c.paths
                .iter()
                .any(|p| rel == p || rel.starts_with(&format!("{p}/")))
            {
                git(dir, &["add", "--", rel], name);
                staged = true;
            }
        }
        assert!(
            staged,
            "{name}: history entry {:?} names no file that exists — the manifest has drifted \
             from the corpus",
            c.message
        );
        // Already-committed files stage as no-ops, so an entry can still be empty here.
        let nothing_new = Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(dir)
            .status()
            .unwrap()
            .success();
        assert!(
            !nothing_new,
            "{name}: history entry {:?} adds nothing not already committed",
            c.message
        );

        let mut cmd = Command::new("git");
        cmd.args(["commit", "-q", "-m", &c.message])
            .current_dir(dir);
        if let Some(date) = &c.date {
            // Both, because a reader comparing `replay` to this manifest is comparing dates,
            // and git takes the two from different places.
            cmd.arg(format!("--date={date}"));
            cmd.env("GIT_COMMITTER_DATE", date);
        }
        assert!(
            cmd.status().unwrap().success(),
            "{name}: commit {:?} failed",
            c.message
        );
    }

    git(dir, &["add", "-A"], name);
    let nothing_left = Command::new("git")
        .args(["diff", "--cached", "--quiet"])
        .current_dir(dir)
        .status()
        .unwrap()
        .success();
    if !nothing_left {
        git(
            dir,
            &[
                "commit",
                "-q",
                "-m",
                "establish: the remainder of the corpus",
            ],
            name,
        );
    }
}

pub struct Example {
    /// A **workspace**, not the corpus. The corpus is at `dir/<name>`, and a path dependency
    /// is materialised beside it at `dir/<dep>` so that `path = "../dep"` resolves the way it
    /// does in this repository (#456).
    dir: tempfile::TempDir,
    name: String,
}

impl Example {
    /// Materialize `examples/<name>` as a standalone repository.
    ///
    /// From `git ls-files`, matching how every other suite here builds a tree: a directory
    /// walk would pick up `.DS_Store` and any local scratch, and this test would then be
    /// measuring the maintainer's working directory.
    pub fn materialize(name: &str) -> Self {
        let root = repo_root();
        let dir = tempfile::tempdir().unwrap();
        let prefix = format!("examples/{name}/");

        let files = tracked_under(&root, &prefix);
        assert!(
            !files.is_empty(),
            "no tracked files under {prefix} — `{name}` is missing or unstaged. An example \
             directory that git does not know about is invisible to every check here"
        );
        let here = dir.path().join(name);
        let mut copied = Vec::new();
        for tracked in &files {
            let rel = tracked.strip_prefix(&prefix).unwrap();
            // The manifest describes the repository being built; it is not part of it.
            if rel == HISTORY {
                continue;
            }
            let to = here.join(rel);
            std::fs::create_dir_all(to.parent().unwrap()).unwrap();
            std::fs::copy(root.join(tracked), &to)
                .unwrap_or_else(|e| panic!("copy {tracked}: {e}"));
            copied.push(rel.to_string());
        }

        // Anything this example declares a path dependency on is materialised beside it. No
        // git and no history: nothing runs a command inside a dependency, and `deps::resolved`
        // reads its `.yidam/corpus` off the filesystem.
        for dep in path_dependencies(&here) {
            let dep_prefix = format!("examples/{dep}/");
            let dep_files = tracked_under(&root, &dep_prefix);
            assert!(
                !dep_files.is_empty(),
                "{name} declares a path dependency on `{dep}`, which is not an example in \
                 this repository — the walkthrough it appears in cannot be reproduced"
            );
            for tracked in &dep_files {
                let rel = tracked.strip_prefix(&dep_prefix).unwrap();
                if rel == HISTORY {
                    continue;
                }
                let to = dir.path().join(&dep).join(rel);
                std::fs::create_dir_all(to.parent().unwrap()).unwrap();
                std::fs::copy(root.join(tracked), &to).unwrap();
            }
        }

        // A real repository: `lint` reads history for `orphan-in` dating, and every path in
        // the corpus resolves from the toplevel.
        for args in [
            vec!["init", "-q"],
            vec!["config", "user.email", "example@yidam.test"],
            vec!["config", "user.name", "Example"],
        ] {
            let ok = Command::new("git")
                .args(&args)
                .current_dir(&here)
                .status()
                .unwrap()
                .success();
            assert!(ok, "git {args:?} failed for {name}");
        }

        match history(&root, name) {
            Some(commits) => replay_history(&here, name, &commits, &copied),
            None => {
                let genesis = format!("genesis: the {name} example");
                git(&here, &["add", "-A"], name);
                git(&here, &["commit", "-q", "-m", &genesis], name);
            }
        }
        Self {
            dir,
            name: name.to_string(),
        }
    }

    pub fn path(&self) -> PathBuf {
        self.dir.path().join(&self.name)
    }

    pub fn run(&self, args: &[&str]) -> (String, String, i32) {
        self.run_with_env(args, &[])
    }

    /// The same, with variables the caller adds to the environment.
    ///
    /// `walkthrough_transcripts` needs it: a page that documents an `export` before a
    /// transcript is telling the reader what the command was run with, and re-running it
    /// with anything else would be checking a different command.
    pub fn run_with_env(&self, args: &[&str], env: &[(String, String)]) -> (String, String, i32) {
        let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
            .current_dir(self.path())
            .args(args)
            .envs(env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
            .output()
            .unwrap();
        (
            String::from_utf8_lossy(&out.stdout).to_string(),
            String::from_utf8_lossy(&out.stderr).to_string(),
            out.status.code().unwrap_or(-1),
        )
    }
}

// ── walking the repository ────────────────────────────────────────────────────
//
// Every guard below that discovers its inputs by walking the working tree has to answer the
// same question first: which directories here are not authored? Seven of them answered it
// separately, with seven hand-written name lists, and the lists disagreed — `dist` in three,
// `.claude` in three, `results` in one, `.astro` in one, and `dist-*` in none of them.
//
// #900 is what that costs. `yidam/web/docs/test/quality-render.mjs` builds into
// `dist-quality-test` and removes it when the block ends, so an interrupted `npm test` leaves
// a Starlight build behind. `.gitignore` has declared `dist-*/` since #467, so `git status`
// shows nothing; the cargo scanners' lists said `dist` and not `dist-*`, so three
// `design_tokens` assertions failed on hundreds of Starlight's own compiled custom
// properties, during a gate that has nothing to do with the docs, naming files the developer
// did not write in a directory they cannot see.
//
// So the rule is git's, read from git, rather than a name list that has to be remembered
// twice. A build directory that `.gitignore` already covers is covered here the day it is
// added, and the next `dist-`-prefixed name needs no edit in this file.

/// The repository root, from `start`, as git computes it.
///
/// `start` may be a file — `prescribing_targets` walks `mise.yidam.toml` as one of its
/// targets, and `WalkDir` is happy to yield a single file — so git is run in the nearest
/// directory rather than in `start` itself.
fn git_toplevel(start: &Path) -> PathBuf {
    let anchor = if start.is_dir() {
        start
    } else {
        start.parent().unwrap_or(start)
    };
    let out = Command::new("git")
        .current_dir(anchor)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap_or_else(|e| panic!("git rev-parse in {}: {e}", start.display()));
    assert!(
        out.status.success(),
        "{} is not inside a git repository, so nothing here can tell authored files from \
         build output: {}",
        start.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let top = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim().to_string());
    top.canonicalize()
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", top.display()))
}

/// Every path under `top` that git ignores, relative to `top` and without a trailing slash.
///
/// `--directory` collapses a wholly-ignored directory to its own name, so the set stays at
/// 147 entries here rather than the 258,307 individual files inside them — and pruning at the
/// directory is what keeps the walk from descending into `target/` at all. An ignored *file*
/// inside a tracked directory (`yidam/tests/results/**/transcript.jsonl`) is listed
/// individually by the same call, which is why [`repo_walk_keeping`] checks files against
/// this set too and not only directories.
fn git_ignored(top: &Path) -> BTreeSet<String> {
    let out = Command::new("git")
        .current_dir(top)
        .args([
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "--no-empty-directory",
        ])
        .output()
        .unwrap_or_else(|e| panic!("git ls-files in {}: {e}", top.display()));
    // A failure here must not degrade to "ignore nothing": that is the state #900 reports,
    // and it is silent.
    assert!(
        out.status.success(),
        "git could not list ignored paths in {}, so this walk would scan build output: {}",
        top.display(),
        String::from_utf8_lossy(&out.stderr).trim()
    );
    String::from_utf8_lossy(&out.stdout)
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(|s| s.trim_end_matches('/').to_string())
        .collect()
}

/// A walk of the tree under `start` with everything git ignores pruned away.
///
/// `start` may be any directory inside the repository; the ignore rules are always read at
/// the repository root, because that is where they are written.
pub fn repo_walk(start: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    repo_walk_keeping(start, |_| true)
}

/// The same walk, with an additional predicate for directories a caller excludes for reasons
/// of its own.
///
/// The split is deliberate. `keep` is for *semantic* exclusions — `parity_implementations`
/// does not consider `yidam/tests/results/` an implementation, and `install_channels` does
/// not consider a dot-directory documentation — which are judgements about what the guard is
/// asking, and belong with the question. Build output is not a judgement, and is not passed
/// here by anyone.
pub fn repo_walk_keeping<F>(start: &Path, keep: F) -> impl Iterator<Item = walkdir::DirEntry>
where
    F: Fn(&walkdir::DirEntry) -> bool,
{
    let canonical = start
        .canonicalize()
        .unwrap_or_else(|e| panic!("{} is unreadable: {e}", start.display()));
    let top = git_toplevel(&canonical);
    let ignored = git_ignored(&top);

    // Where the walk begins, as a repo-relative prefix, so an entry's path can be named the
    // way `.gitignore` names it.
    //
    // Computed once from the canonical form and then applied to paths the walk builds from
    // `start` **as the caller gave it**, rather than canonicalizing the walk root. Every
    // caller here roots at `repo_root()`, which is `CARGO_MANIFEST_DIR/../..` — not a
    // canonical path — and then strips that same prefix off each entry to get the name it
    // reports. Handing walkdir the canonical root instead silently broke that strip: the
    // paths came back absolute, `rel.starts_with(DESIGN)` matched nothing, and
    // `system_surfaces` returned zero. The `only 0 …; the walk is looking at the wrong tree`
    // assertion is the only reason that was a failure and not a pass over an empty set.
    let prefix = canonical
        .strip_prefix(&top)
        .unwrap_or_else(|_| panic!("{} is not inside {}", canonical.display(), top.display()))
        .to_path_buf();
    let start = start.to_path_buf();

    walkdir::WalkDir::new(start.clone())
        .into_iter()
        .filter_entry(move |e| {
            if e.depth() == 0 {
                return true;
            }
            // By name and not by path: a `.git` at any depth is a repository, never
            // authored content, and the ignore list does not mention the one at the root.
            if e.file_name() == ".git" {
                return false;
            }
            // Cannot fail — walkdir builds every path by joining onto the root above.
            let under = e
                .path()
                .strip_prefix(&start)
                .expect("walkdir yields paths under its own root");
            let rel = prefix.join(under).to_string_lossy().replace('\\', "/");
            if ignored.contains(&rel) {
                return false;
            }
            keep(e)
        })
        .filter_map(Result::ok)
}
