//! The layout bootstrap installs, and how to build one.
//!
//! Shared by [`installed_layout_links`], which checks that every relative link resolves in
//! this layout, and [`derived_repo_smoke`], which materializes it and runs its gate. One
//! definition, because a test checking a layout nobody installs is worse than no test — and
//! two copies of the mapping is the shortest path to exactly that.

#![allow(dead_code)] // each test binary uses a different part of this

pub mod git;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The bootstrap skill, repo-relative.
pub const BOOTSTRAP_SKILL: &str = "yidam/prelude/skills/bootstrap.md";

/// Where the capability kinds are defined, repo-relative.
pub const MANIFEST_MODULE: &str = "yidam/cli/src/cmd/run/manifest.rs";

/// The `kind` values a `.yidam/capabilities.toml` may declare, read out of the enum that
/// defines them.
///
/// Parsed from `Kind::as_str`'s match arms because `cmd` is a private module and no test can
/// name `Kind`. Shared by `capability_run` (which declares each one against the real binary)
/// and `run_route_claims` (which holds the documents to the same set), for the reason
/// [`step_one_read_list`] gives: two parsers of one list is how two gates come to disagree
/// about what the list says.
///
/// A literal list here would be the defect #1027 reported, one layer up. `Kind` had two arms
/// and the published taxonomy had three for as long as nothing compared them, and a gate over
/// three strings somebody typed would go quiet again at the fourth.
pub fn kind_spellings() -> Vec<String> {
    let path = repo_root().join(MANIFEST_MODULE);
    let src = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let at = src
        .find("pub fn as_str(self) -> &'static str {")
        .unwrap_or_else(|| {
            panic!("`Kind::as_str` is not in {MANIFEST_MODULE}; the definition this parse                     discovers moved, and every gate over it would otherwise go quiet")
        });
    let body = &src[at..at + src[at..].find("\n    }").expect("the end of `as_str`")];
    let kinds: Vec<String> = body
        .lines()
        .filter_map(|l| l.split_once("=> \"")?.1.split_once('"'))
        .map(|(k, _)| k.to_string())
        .collect();
    assert!(
        kinds.len() >= 3 && kinds.iter().any(|k| k == "calculator"),
        "the kinds were not discovered from {MANIFEST_MODULE}, so every gate over them          asserts nothing: {kinds:?}"
    );
    kinds
}

/// Every `[capability.<name>]` table in `text`, as `(step, kind)`.
///
/// Over documented examples as well as real manifests, because a fenced `toml` block showing a
/// `kind` the binary would refuse is a prediction that fails — and the one a corpus author
/// copies. Scoped to the table rather than to `kind = "..."` anywhere, because fifteen other
/// `kind` vocabularies in this repository spell their values the same way.
pub fn capability_kinds(text: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("[capability.") {
        let after = &rest[at + "[capability.".len()..];
        let Some((step, tail)) = after.split_once(']') else {
            break;
        };
        let table = &tail[..tail.find("\n[").unwrap_or(tail.len())];
        if let Some(kind) = table
            .lines()
            .filter_map(|l| l.split_once('=')?.0.trim().eq("kind").then_some(l))
            .filter_map(|l| l.split_once('"')?.1.split_once('"'))
            .map(|(k, _)| k.to_string())
            .next()
        {
            found.push((step.to_string(), kind));
        }
        rest = tail;
    }
    found
}

/// Step 1's numbered read list, in order, as repo-relative paths.
///
/// Parsed rather than listed, so every assertion over it is about the list the agent actually
/// follows. The section runs from its own heading to the next `###`. Shared by
/// `prelude_glossary` (which holds the list's order and count) and
/// `prelude_rules_and_evidence` (which charges its words to the bootstrap path), because two
/// parsers of one list is how the two gates come to disagree about what the list says.
pub fn step_one_read_list() -> Vec<String> {
    let path = repo_root().join(BOOTSTRAP_SKILL);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{} is unreadable ({e})", path.display()));
    let (_, after) = text
        .split_once("### 1. Internalize the prelude")
        .expect("step 1's heading");
    let body = after.split("\n### ").next().unwrap_or(after);
    body.lines()
        .filter_map(|l| {
            let l = l.trim();
            // `1. \`yidam/prelude/IDENTITY.md\` — …`, and nothing else in the step is numbered.
            let (n, rest) = l.split_once(". `")?;
            n.parse::<usize>().ok()?;
            Some(rest.split('`').next()?.to_string())
        })
        .collect()
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

/// A step 5 calculator named a `yidam/domains/` library. The default is that none does,
/// and step 8 then vendors no `domains/` directory at all.
pub const DOMAIN_SELECTED: &str = "prelude_domains: non-empty";

pub const MAPPING: &[Install] = &[
    // The domain libraries, beside the prelude rather than inside it (#934). Fifteen
    // libraries in three languages each, and step 8 copies only what a step 5 calculator
    // named. A derived repository has no task that builds them, no workspace that includes
    // them, and no CI job that runs them; `domain-parity` is yidam's gate and does not travel.
    Install {
        src: "yidam/domains",
        dst: Some(".yidam/.vendor/domains"),
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
    // Installs new: yidam's own mise.toml includes no overrides file, and a vendor update
    // never writes one, so the repository's overrides of inherited tasks survive it (#1064).
    row(
        "sadhana/root/mise.overrides.toml",
        Some("mise.overrides.toml"),
    ),
    row("sadhana/root/gitattributes", Some(".gitattributes")),
    row("sadhana/root/gitignore", Some(".gitignore")),
    // The one root file yidam keeps no copy of: the template's own history is a CLI's, not
    // a corpus's, so there is nothing for a practice document to describe here (#287).
    row("sadhana/root/PRACTICE.md", Some("PRACTICE.md")),
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
    // Not a directory mirror and not a root file: one named file that installs under
    // `.yidam/`. It is the only member of that shape, and it exists because the file was
    // read, documented and never written — `config.rs` parses it, `sadhana/root/README.md`
    // tells readers the `due` intervals come from it, `index.yml` hard-errors when it holds
    // no `[vault.*]`, and nothing put one in a derived repository (#916).
    row("sadhana/config.toml", Some(".yidam/config.toml")),
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

/// Replace the contents of `` `…` `` spans with spaces, preserving byte offsets.
pub fn blank_code_spans(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut in_code = false;
    for c in line.chars() {
        if c == '`' {
            in_code = !in_code;
            out.push(c);
        } else if in_code {
            for _ in 0..c.len_utf8() {
                out.push(' ');
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Resolve `target` from `from_dir` lexically. `None` if it climbs above the root.
pub fn resolve(from_dir: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = if from_dir.is_empty() {
        vec![]
    } else {
        from_dir.split('/').collect()
    };
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
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
    let out = git::raw(root, &["ls-files", "-z", "--", prefix]);
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
    let out = git::raw(dir, args);
    assert!(
        out.status.success(),
        "git {args:?} failed for {name}: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
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
        let nothing_new = git::succeeded(dir, &["diff", "--cached", "--quiet"]);
        assert!(
            !nothing_new,
            "{name}: history entry {:?} adds nothing not already committed",
            c.message
        );

        // `git_at` rather than `git`, because a reader comparing `replay` to this manifest
        // is comparing dates and git takes the author's and the committer's from different
        // places.
        let args = ["commit", "-q", "-m", &c.message];
        let out = match &c.date {
            Some(date) => git::raw_at(dir, &args, date),
            None => git::raw(dir, &args),
        };
        assert!(
            out.status.success(),
            "{name}: commit {:?} failed: {}",
            c.message,
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }

    git(dir, &["add", "-A"], name);
    let nothing_left = git::succeeded(dir, &["diff", "--cached", "--quiet"]);
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
            git(&here, &args, name);
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

    /// [`Example::materialize`], with every step this build cannot invoke struck from the manifest.
    ///
    /// **For the suites that run a whole plan.** `yidam run` with no step named resolves the
    /// entire manifest and refuses all of it if any one step is unrunnable, deliberately: a plan
    /// holding a step this binary declines is a plan that would half advance the corpus, so it is
    /// stopped before anything runs rather than skipped past. Since #1102 `examples/streamflow`
    /// ships RFC-0042's reference case — a calculator declared `run = { gluon = … }` — so a build
    /// without `calculators-gluon` cannot run that example's plan at all.
    ///
    /// So the plan those suites run is the runnable part of the manifest, and the amendment is a
    /// commit like any other: the corpus they measure is a real corpus and not a materialized tree
    /// with an uncommitted edit in it. In a build carrying the feature nothing is struck and
    /// nothing is committed, and `capability_run.rs`'s
    /// `a_typed_capability_is_shipped_and_is_run_in_one_build_and_declined_in_the_other` is what
    /// holds that to the build rather than to this function's discretion.
    ///
    /// By text surgery on the section rather than by re-serializing the TOML, because these
    /// manifests carry more prose than declaration and a round trip would drop all of it —
    /// including the comments saying why each step reads what it reads, which is the part a failure
    /// in one of these suites is read alongside.
    ///
    /// Typed-ness is read off the *shape* of `run` — a table rather than an array — which is the
    /// same distinction the CLI's own `Run` deserializer draws. Deserializing that type instead
    /// would skip exactly the declarations it got wrong, and it is `pub(crate)` to `yidam` anyway.
    pub fn materialize_runnable(name: &str) -> Self {
        #[derive(serde::Deserialize, Default)]
        struct Manifest {
            #[serde(default)]
            capability: BTreeMap<String, Declaration>,
        }
        #[derive(serde::Deserialize)]
        struct Declaration {
            run: toml::Value,
        }

        let e = Self::materialize(name);
        let manifest = e.path().join(".yidam/capabilities.toml");
        let Ok(text) = std::fs::read_to_string(&manifest) else {
            return e;
        };
        let caps = toml::from_str::<Manifest>(&text)
            .unwrap_or_else(|x| panic!("{name}'s capability manifest does not parse: {x}"))
            .capability;

        let mut kept = text.clone();
        for (step, d) in &caps {
            let typed = matches!(d.run, toml::Value::Table(_));
            if !typed || cfg!(feature = "calculators-gluon") {
                continue;
            }
            let header = format!("[capability.{step}]");
            let at = kept.find(&header).unwrap_or_else(|| {
                panic!("`{step}` is declared in {name} and `{header}` is not in its manifest")
            });
            // To the next section header at column 0, or to the end. Searched from past this one,
            // so a manifest whose last entry is the struck one ends the slice at EOF.
            let end = kept[at + header.len()..]
                .find("\n[")
                .map(|i| at + header.len() + i + 1)
                .unwrap_or(kept.len());
            kept = format!("{}{}", &kept[..at], &kept[end..]);
        }
        if kept == text {
            return e;
        }
        std::fs::write(&manifest, kept).expect("striking the unrunnable steps");
        git::out(&e.path(), &["add", "-A"]);
        git::out(
            &e.path(),
            &["commit", "-m", "scaffold: the plan this build can run"],
        );
        e
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
    let out = git::raw(anchor, &["rev-parse", "--show-toplevel"]);
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
    let out = git::raw(
        top,
        &[
            "ls-files",
            "-z",
            "--others",
            "--ignored",
            "--exclude-standard",
            "--directory",
            "--no-empty-directory",
        ],
    );
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
    // Loud rather than empty. `WalkDir` on a path that does not exist yields one error and
    // then nothing, so a caller that filters errors away sees an empty walk and passes —
    // which is how a guard stops guarding without going red. A walk root that is not there
    // is a defect in the caller or a moved directory, and either is worth a panic.
    let canonical = start.canonicalize().unwrap_or_else(|e| {
        panic!(
            "cannot walk {}: {e}. A walk root that does not exist would otherwise scan \
             nothing and pass.",
            start.display()
        )
    });
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

/// Every subcommand this binary offers, read out of its own `--help-all`.
///
/// The keys of [`writers_from_help`]; see there for how the listing is read and why.
pub fn commands_from_help() -> BTreeSet<String> {
    writers_from_help().into_keys().collect()
}

/// Every subcommand this binary offers, and whether `--help-all` marks it as writing.
///
/// The one reader of that listing for every test binary (#993). Seven test files once parsed
/// it separately, each with its own copy of the listing's shape, so a change to
/// [`yidam::help`]'s template that one copy survived and another did not would take one
/// roster quietly to empty while the others stayed right.
///
/// **Asked of the built binary**, not the source: it is the same question a reader asks,
/// answered the same way, and it stays correct through a refactor of how the clap enum is
/// spelled. A roster listed in a test instead would stop covering a rename without ever going
/// red, which is the rot every check over this surface exists for.
///
/// **`--help-all` and not `--help`**: since #921 `--help` is the thirteen commands a session
/// usually needs and marks five writers, while `--help-all` lists every command and marks
/// every writer. Every question asked of this roster is about the whole surface; reading the
/// short one would narrow each of them and stay green about it.
///
/// **The shape.** The listing is `help::render`'s template, not clap's flat `Commands:` block —
/// a group's own `--help` is clap's, and is a different document that stays with its callers.
/// Group headings sit flush left. A command row is indented two spaces, starts with the name,
/// and is followed by either the `*` write-marker or two spaces before its description. The
/// legend's first line is indented two as well and opens with `*`, and its continuation is
/// indented four. The option block is filtered out by requiring a lowercase-and-hyphens name.
/// `help` is clap's own and is not part of the surface.
///
/// **The marker** is read positionally — the column after the name — rather than searched for,
/// because a description that happened to begin with a star would otherwise read as a writer.
///
/// **Two floors**, because the listing can rot two ways: rows that stop parsing leave every
/// roster built on this empty, and a marker that moves leaves every command a reader.
pub fn writers_from_help() -> BTreeMap<String, bool> {
    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .arg("--help-all")
        .output()
        .expect("running `yidam --help-all`");
    assert!(out.status.success(), "`yidam --help-all` exited nonzero");
    let help = String::from_utf8(out.stdout).expect("--help-all is utf-8");

    let mut found = BTreeMap::new();
    for line in help.lines() {
        let Some(rest) = line.strip_prefix("  ") else {
            continue;
        };
        if rest.starts_with(' ') || rest.starts_with('-') {
            continue; // continuation, or an option like `-h, --help`
        }
        let name = rest.split_whitespace().next().unwrap_or_default();
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_lowercase() || c == '-') {
            continue;
        }
        if name == "help" {
            continue;
        }
        let writes = rest[name.len()..].trim_start().starts_with("* ");
        found.insert(name.to_string(), writes);
    }

    assert!(
        found.len() > 20,
        "parsed only {} command(s) from --help-all — the output shape changed and this is no \
         longer reading it: {found:?}",
        found.len()
    );
    assert!(
        found.values().filter(|w| **w).count() > 10,
        "too few commands parsed as writers, so the marker column is not where this expects \
         it: {found:?}"
    );
    found
}

/// Every path a document actually carries a value at, in the schema's own notation.
pub fn paths_of(
    node: &serde_json::Value,
    path: &str,
    out: &mut std::collections::BTreeSet<String>,
) {
    match node {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let here = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                out.insert(here.clone());
                paths_of(child, &here, out);
            }
        }
        serde_json::Value::Array(items) => {
            for item in items {
                paths_of(item, &format!("{path}[]"), out);
            }
        }
        _ => {}
    }
}

/// Does a report schema declare `path`? Walks `properties` and `items` the way [`paths_of`] builds
/// them, so the two notations are the same notation.
pub fn declares(schema: &serde_json::Value, path: &str) -> bool {
    let mut node = schema;
    for segment in path.split('.') {
        let (key, arrays) = match segment.split_once("[]") {
            Some((key, rest)) => (key, rest.matches("[]").count() + 1),
            None => (segment, 0),
        };
        node = match node.get("properties").and_then(|p| p.get(key)) {
            Some(child) => child,
            None => return false,
        };
        for _ in 0..arrays {
            node = match node.get("items") {
                Some(items) => items,
                None => return false,
            };
        }
    }
    true
}
