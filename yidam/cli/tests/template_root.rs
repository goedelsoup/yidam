//! Every tracked path in this repository is accounted for by the bootstrap protocol.
//!
//! `derived_repo_smoke` builds its model of a derived repository **additively**:
//! `common::materialize` starts from an empty directory and copies in what [`MAPPING`] rows
//! and [`KEPT_AT_ROOT`] name. Production runs the other way round — `yidam clone` copies the
//! template wholesale and the bootstrap skill then *subtracts*. The two directions agree only
//! about paths somebody thought to name, and a path nobody named is invisible to a
//! constructive model no matter how long it sits there.
//!
//! It sat there twice. #589 found five of yidam's own workflows surviving genesis; #807
//! found ten more root paths plus the rest of `.github/`, reported by a repository a few
//! hours old whose F5 opened an extension directory the vendor step had deleted. Both were
//! fixed the same way — name the path — and the second was possible because the first fix
//! left the model alone.
//!
//! So this suite asks the converse question. Not "is everything the protocol names
//! installed?" but **"is everything in this repository named by the protocol?"** — over the
//! tracked set, discovered rather than restated, so a file added to the template root next
//! month is asked the same question without anyone remembering to ask it.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;

use common::{repo_root, tracked_under, Install, KEPT_AT_ROOT, MAPPING};

/// How a tracked path reaches — or does not reach — a derived repository.
#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Fate {
    /// Never copied: `yidam clone` excludes it at the top level.
    NotInherited,
    /// A [`MAPPING`] row's source — installed somewhere in the derived repo, or consumed at
    /// genesis (`sadhana/README.md`, `samudaya/`).
    Installed,
    /// Yidam's own copy of a file the scaffold overwrites in step 3. It is copied, and then
    /// a `sadhana/root/` file lands on top of it.
    Overwritten,
    /// Kept at the root, unchanged, by name: `LICENSE` and `mise.yidam.toml`.
    Kept,
    /// Deleted by a command the bootstrap skill gives — `rm -rf yidam/`, `rm -f
    /// BOOTSTRAP.md VERSIONING.md`. Parsed out of the skill, not restated here.
    Deleted,
}

/// The rules, gathered once. Everything here is read from the thing that decides it.
struct Protocol {
    not_inherited: &'static [&'static str],
    installed: Vec<&'static str>,
    overwritten: BTreeSet<String>,
    kept: BTreeSet<String>,
    deleted: BTreeSet<String>,
}

impl Protocol {
    fn read(root: &Path) -> Self {
        Self {
            not_inherited: yidam::NOT_INHERITED,
            installed: MAPPING.iter().map(|e: &Install| e.src).collect(),
            overwritten: MAPPING
                .iter()
                .filter_map(|e| e.dst)
                .map(str::to_string)
                .collect(),
            kept: KEPT_AT_ROOT.iter().map(|s| s.to_string()).collect(),
            deleted: skill_deletions(root),
        }
    }

    fn fate(&self, path: &str) -> Option<Fate> {
        if self.not_inherited.iter().any(|e| under(path, e)) {
            return Some(Fate::NotInherited);
        }
        if self.installed.iter().any(|s| under(path, s)) {
            return Some(Fate::Installed);
        }
        if self.overwritten.contains(path) {
            return Some(Fate::Overwritten);
        }
        if self.kept.contains(path) {
            return Some(Fate::Kept);
        }
        if self.deleted.iter().any(|d| under(path, d)) {
            return Some(Fate::Deleted);
        }
        None
    }
}

/// `path` is `prefix` itself or lies beneath it. Segment-wise, so `scripts` never claims
/// `scripts-that-stay/`.
fn under(path: &str, prefix: &str) -> bool {
    path == prefix || path.starts_with(&format!("{prefix}/"))
}

/// Paths the bootstrap skill deletes, read out of the skill's own code fences.
///
/// Parsed rather than listed. The whole defect being guarded against is a list in one place
/// that stopped agreeing with a list in another, and a hardcoded copy of the skill's
/// deletions here would be the third one.
fn skill_deletions(root: &Path) -> BTreeSet<String> {
    let skill = std::fs::read_to_string(root.join("yidam/prelude/skills/bootstrap.md"))
        .expect("the bootstrap skill is readable");
    let mut fenced = false;
    let mut out = BTreeSet::new();
    for line in skill.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if !fenced {
            continue;
        }
        let Some(args) = line.trim().strip_prefix("rm ") else {
            continue;
        };
        for arg in args.split_whitespace() {
            if arg.starts_with('-') || arg.contains('*') {
                continue;
            }
            let path = arg.trim_end_matches('/');
            if !path.is_empty() {
                out.insert(path.to_string());
            }
        }
    }
    assert!(
        out.contains("BOOTSTRAP.md") && out.contains("yidam"),
        "no `rm` line in the bootstrap skill's fences deletes BOOTSTRAP.md and yidam/ — the \
         vendor step moved, and this parse is now excusing nothing: {out:?}"
    );
    out
}

/// Every path git tracks, from the repository root.
fn tracked(root: &Path) -> Vec<String> {
    let all = tracked_under(root, ".");
    assert!(
        all.len() > 1000,
        "only {} tracked paths — the ls-files walk is broken, and an empty universe passes \
         every assertion below",
        all.len()
    );
    all
}

/// **The load-bearing test.** Nothing in this repository reaches a derived repository
/// unasked.
///
/// A path that fails here is not necessarily a defect — it is a path whose fate nobody has
/// decided. Decide it: exclude it from the copy ([`yidam::NOT_INHERITED`]), install it
/// through a `MAPPING` row, keep it by name, or have the bootstrap skill delete it.
#[test]
fn every_tracked_path_is_accounted_for() {
    let root = repo_root();
    let protocol = Protocol::read(&root);

    let mut unaccounted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for path in tracked(&root) {
        if protocol.fate(&path).is_none() {
            let top = path.split('/').next().unwrap_or(&path).to_string();
            unaccounted.entry(top).or_default().push(path);
        }
    }

    assert!(
        unaccounted.is_empty(),
        "{} path(s) under {} top-level entries are copied into every derived repository and \
         named by nothing — not excluded from the copy, not installed by a mapping row, not \
         kept, not deleted by the skill:\n{}\nEach needs a decision: add it to \
         `NOT_INHERITED` in cmd/clone.rs, give it a MAPPING row, list it in KEPT_AT_ROOT, or \
         have the bootstrap skill's vendor step delete it.",
        unaccounted.values().map(Vec::len).sum::<usize>(),
        unaccounted.len(),
        unaccounted
            .iter()
            .map(|(top, paths)| format!("  {top}  ({} file(s))", paths.len()))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// The partition is non-degenerate: every fate claims something real.
///
/// Without this, a rule that quietly stopped matching — a `MAPPING` prefix that no longer
/// exists, an exclusion list emptied by a bad merge — would make the test above *more*
/// likely to pass, because everything it fails to claim falls through to the next rule and
/// the whole set is claimed by nobody only at the very end.
#[test]
fn each_fate_claims_at_least_one_path() {
    let root = repo_root();
    let protocol = Protocol::read(&root);

    let mut seen: BTreeMap<Fate, usize> = BTreeMap::new();
    for path in tracked(&root) {
        if let Some(fate) = protocol.fate(&path) {
            *seen.entry(fate).or_default() += 1;
        }
    }

    for fate in [
        Fate::NotInherited,
        Fate::Installed,
        Fate::Overwritten,
        Fate::Kept,
        Fate::Deleted,
    ] {
        assert!(
            seen.get(&fate).copied().unwrap_or(0) > 0,
            "no tracked path is classified {fate:?} — that rule matches nothing and is \
             excusing nothing: {seen:?}"
        );
    }
}

/// An exclusion naming nothing excludes nothing.
///
/// `NOT_INHERITED` is matched against directory entries by name. Rename `scripts/` and the
/// entry stays, still legal, still sorted, and silently stops doing anything — which is the
/// same failure as never having added it.
#[test]
fn every_exclusion_names_something_that_is_here() {
    let root = repo_root();
    for entry in yidam::NOT_INHERITED {
        assert!(
            root.join(entry).exists(),
            "NOT_INHERITED names `{entry}`, which is not in the template root: it excludes \
             nothing, and whatever replaced it is being copied"
        );
    }
    let mut sorted = yidam::NOT_INHERITED.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    assert_eq!(
        sorted,
        yidam::NOT_INHERITED.to_vec(),
        "keep NOT_INHERITED sorted and unique — it is read by people deciding whether a path \
         is already there"
    );
}

/// End to end through the real command, because everything above tests the list and this
/// tests the copy. The bug they are guarding against is a call site that kept its literal
/// while the constant beside it grew.
#[test]
fn clone_delivers_none_of_it() {
    let root = repo_root();
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("derived");

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(&root)
        .args(["clone", target.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let leaked: Vec<&&str> = yidam::NOT_INHERITED
        .iter()
        .filter(|entry| target.join(entry).exists())
        .collect();
    assert!(
        leaked.is_empty(),
        "`yidam clone` delivered {leaked:?} — paths NOT_INHERITED says a derived repository \
         does not get"
    );
    // The paired assertion. An exclusion list that excluded everything would pass the one
    // above, and the bootstrap skill reads `sadhana/` in step 3.
    assert!(
        target.join("sadhana/github/workflows").is_dir(),
        "the scaffold did not arrive — the copy is now excluding what bootstrap reads"
    );
    assert!(
        target.join("sadhana/docs").is_dir(),
        "`sadhana/docs/` is a different directory at a different depth from the excluded \
         `docs/`, and step 3 reads it"
    );
}

// ── #912: the copy is the tracked set, and nothing else ──────────────────────────────────

/// Every tracked path at `root` that is a regular file, with its git mode.
///
/// Derived from `git ls-files -s` rather than from the filesystem, so the symlinks are
/// identified by what git recorded rather than by the same `symlink_metadata` call production
/// makes. Two derivations of one set can disagree; one computation compared against itself
/// cannot.
fn tracked_regular_files(root: &Path) -> BTreeSet<String> {
    // `common::git`, which is the integration suite's one fixture helper (#929) — a `-z`
    // list survives its trim, because NUL is not whitespace.
    let listing = common::git::out(root, &["ls-files", "-s", "-z"]);
    listing
        .split('\0')
        .filter(|s| !s.is_empty())
        .filter_map(|row| {
            // `<mode> <sha> <stage>\t<path>`
            let (meta, path) = row.split_once('\t')?;
            let mode = meta.split_whitespace().next()?;
            // 120000 is a symlink and 160000 a submodule gitlink; the copy skips both, as
            // the filesystem walk before it did.
            (mode == "100644" || mode == "100755").then(|| path.to_string())
        })
        .collect()
}

/// Every file `yidam clone` wrote into `target`, excluding what it wrote *itself*.
///
/// `.git/` is `git init`'s, and `.yidam.toml` is the provenance pin — neither is copied from
/// the template, and neither is tracked here, so both would read as a leak.
fn delivered(target: &Path) -> BTreeSet<String> {
    fn walk(dir: &Path, base: &Path, out: &mut BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for e in entries.filter_map(Result::ok) {
            let path = e.path();
            if path.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if path.is_dir() {
                walk(&path, base, out);
            } else {
                out.insert(path.strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = BTreeSet::new();
    walk(target, target, &mut out);
    out.remove(yidam::provenance::MANIFEST);
    out
}

/// **`yidam clone` delivers the tracked set minus [`yidam::NOT_INHERITED`], exactly.**
///
/// The strongest form of the question `clone_delivers_none_of_it` asks one list at a time.
/// That test could only ever be as complete as the list it iterates, which is how #912
/// happened: a clone taken from a working checkout was 2,931 files, and 1,436 of them were
/// `.claude/worktrees/` — the operator's own agent worktrees, gitignored at `.gitignore:17`,
/// named by no exclusion because they were never authored and never tracked.
///
/// Set equality in both directions, because each failure is real and different. A delivered
/// path that git does not track is working-tree debris shipped into every repository created
/// after it. A tracked path that was not delivered is template content that silently stopped
/// arriving.
#[test]
fn clone_delivers_exactly_the_tracked_set() {
    let root = repo_root();
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("derived");

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(&root)
        .args(["clone", target.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clone failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let expected: BTreeSet<String> = tracked_regular_files(&root)
        .into_iter()
        .filter(|p| {
            let top = p.split('/').next().unwrap_or(p);
            !yidam::NOT_INHERITED.contains(&top)
        })
        .collect();
    assert!(
        expected.len() > 1000,
        "only {} path(s) expected — the ls-files derivation is broken, and an empty \
         expectation is satisfied by an empty clone",
        expected.len()
    );

    let got = delivered(&target);
    let untracked: Vec<&String> = got.difference(&expected).collect();
    let missing: Vec<&String> = expected.difference(&got).collect();

    assert!(
        untracked.is_empty(),
        "`yidam clone` delivered {} path(s) this repository does not track — working-tree \
         debris now ships into every repository created from it:\n{}",
        untracked.len(),
        untracked
            .iter()
            .take(20)
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
    assert!(
        missing.is_empty(),
        "`yidam clone` did not deliver {} tracked path(s) — template content stopped \
         arriving:\n{}",
        missing.len(),
        missing
            .iter()
            .take(20)
            .map(|p| format!("  {p}"))
            .collect::<Vec<_>>()
            .join("\n"),
    );
}

/// The reported case, manufactured: a checkout carrying a gitignored file at its root.
///
/// [`clone_delivers_exactly_the_tracked_set`] would catch this too, but only when the
/// checkout it runs against happens to have such a file — which a CI runner's does not. This
/// builds one, through the real binary, so the property is asserted rather than waited for.
///
/// The fixture is the smallest thing that is a template: the two directories
/// [`yidam::TEMPLATE_MARKERS`] names, which is also what makes this a test of the refusal's
/// other half. A predicate that rejected everything would fail here.
#[test]
fn a_clone_of_a_checkout_with_gitignored_files_does_not_carry_them() {
    let tmp = tempfile::tempdir().unwrap();
    let source = tmp.path().join("template");
    let target = tmp.path().join("derived");

    let write = |rel: &str, body: &str| {
        let path = source.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, body).unwrap();
    };
    for marker in yidam::TEMPLATE_MARKERS {
        write(&format!("{marker}/README.md"), "the template");
    }
    write(".gitignore", ".claude/worktrees/\n.local/\n");
    common::git::git(&source, &["init", "-q", "-b", "main"]);
    common::git::git(&source, &["config", "user.email", "t@t.co"]);
    common::git::git(&source, &["config", "user.name", "Test"]);
    common::git::git(&source, &["add", "-A"]);
    common::git::git(&source, &["commit", "-q", "-m", "seed"]);
    // Written after the commit: gitignored at the root, and absent from `git ls-files`.
    write(".claude/worktrees/task/corpus.yml", "an embedded worktree");
    write(".local/bin/yidam", "an ELF from August");

    let out = Command::new(env!("CARGO_BIN_EXE_yidam"))
        .current_dir(&source)
        .args(["clone", target.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "clone refused a checkout carrying both markers: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    for leaked in [".claude", ".local"] {
        assert!(
            !target.join(leaked).exists(),
            "`{leaked}` is gitignored at the source root and is not part of the template, \
             but the clone carried it"
        );
    }
    assert_eq!(
        delivered(&target),
        yidam::TEMPLATE_MARKERS
            .iter()
            .map(|m| format!("{m}/README.md"))
            .chain([".gitignore".to_string()])
            .collect::<BTreeSet<String>>(),
        "the tracked set is what should have arrived, and only that"
    );
}
