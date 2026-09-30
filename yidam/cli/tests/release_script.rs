//! The release ordering is enforced rather than described.
//!
//! `sdk/rust/v*` must publish before `cli/v*` or the CLI's publish fails on a missing
//! `yidam-core`. That was true, documented, and written at the top of a workflow file — and
//! then not followed by the person who had written it a few hours earlier, because a comment
//! in a workflow is not in front of anyone at the moment they type `git tag`.
//!
//! `release.sh` is where that ordering lives now. These tests check the two ways it could
//! quietly stop being true: a layer VERSIONING.md names that the script cannot release, and
//! a document that goes back to telling people to type `git tag` directly.

use std::path::PathBuf;
use std::process::Command;

mod common;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A tracked file as HEAD carries it, which is what the tag will carry.
fn at_head(rel: &str) -> String {
    // `raw`, not `out`: a tracked file's trailing newline is part of what the tag carries,
    // and the section parsers below split on it.
    let out = common::git::raw(&repo_root(), &["show", &format!("HEAD:{rel}")]);
    assert!(
        out.status.success(),
        "git show HEAD:{rel} failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn read(rel: &str) -> String {
    let p = repo_root().join(rel);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{} is unreadable ({e})", p.display()))
}

/// Run `./release.sh` from the repository root and return (stdout + stderr, success).
///
/// Always `--dry-run`: these tests must never be able to create a tag, whatever else they
/// get wrong.
///
/// **Memoised, because a dry run is expensive and nextest runs tests in parallel.** For the
/// `cli` and `sdk/rust` layers release.sh verifies the packaged build with
/// `cargo publish --dry-run`, so each distinct invocation is a full package build. Two of them
/// at once contend on one target directory: the second test to call this took **63s against
/// the first one's 10s**, making it the slowest test in the whole suite by six times and
/// adding about a minute to `ci (cli)` — twice, since coverage runs the suite again.
///
/// A dry run has no side effects on the tree it reads, which is what makes one invocation
/// answerable to several assertions. Every refusal is reported by one run rather than the
/// first one stopping it, so a test asking about a different refusal asks the same output.
fn release(args: &[&str]) -> (String, bool) {
    use std::sync::{Mutex, OnceLock};
    static RUNS: OnceLock<Mutex<std::collections::HashMap<String, (String, bool)>>> =
        OnceLock::new();

    let key = args.join(" ");
    // The lock is held across the run on purpose: two threads reaching an unmemoised
    // invocation are exactly the contention this exists to remove, and a dry run is fast
    // enough that serialising them beats letting both build.
    let mut runs = RUNS
        .get_or_init(|| Mutex::new(std::collections::HashMap::new()))
        .lock()
        .expect("the release.sh memo is not poisoned");
    if let Some(cached) = runs.get(&key) {
        return cached.clone();
    }

    let out = Command::new("./release.sh")
        .args(args)
        .arg("--dry-run")
        .current_dir(repo_root())
        .output()
        .expect("release.sh is executable");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    let result = (text, out.status.success());
    runs.insert(key, result.clone());
    result
}

/// A version the manifest does not declare is refused, not tagged.
///
/// The tag and the manifest disagreeing is the mistake that cannot be undone once a publish
/// has run: crates.io keeps a version forever, and a yanked one is still downloadable by
/// anything that already resolved it.
#[test]
fn a_version_the_manifest_does_not_declare_is_refused() {
    let (out, ok) = release(&["sdk/rust", "9.9.9"]);
    assert!(
        !ok,
        "release.sh accepted a version nothing declares:\n{out}"
    );
    assert!(
        out.contains("version-mismatch"),
        "expected a version-mismatch refusal, got:\n{out}"
    );
    assert!(
        out.contains("Nothing was tagged"),
        "release.sh must say plainly that it did not tag:\n{out}"
    );
}

/// A layer that is not a layer is refused before anything else is inspected.
#[test]
fn a_layer_that_is_not_a_layer_is_refused() {
    let (out, ok) = release(&["sdk/haskell", "1.0.0"]);
    assert!(
        !ok,
        "release.sh accepted a layer that does not exist:\n{out}"
    );
    assert!(
        out.contains("unknown-layer"),
        "expected an unknown-layer refusal, got:\n{out}"
    );
}

/// Every layer VERSIONING.md gives a tag pattern must be one the script can cut.
///
/// A layer the script does not know is a layer released by hand, and a layer released by
/// hand is the state this whole file is a response to.
#[test]
fn every_layer_versioning_md_names_is_one_the_script_can_release() {
    let script = read("release.sh");
    // (the tag pattern as VERSIONING.md writes it, the layer argument release.sh takes)
    const LAYERS: &[(&str, &str)] = &[
        ("`sdk/rust/v{major}.{minor}.{patch}`", "sdk/rust)"),
        ("`cli/v{major}.{minor}.{patch}`", "cli)"),
        ("`bootstrap/v{major}.{minor}.{patch}`", "bootstrap)"),
        ("`editor/v{major}.{minor}.{patch}`", "editor)"),
        // Four characters from the row above it, and a separate layer argument rather than a
        // flag on `editor`: the two editor clients carry separate versions, and a shared
        // entrance is a shared cadence by accident.
        ("`edit/v{major}.{minor}.{patch}`", "edit)"),
        ("`v{major}.{minor}.{patch}`", "template)"),
    ];
    let versioning = read("VERSIONING.md");
    for (pattern, branch) in LAYERS {
        assert!(
            versioning.contains(pattern),
            "VERSIONING.md no longer documents the tag pattern {pattern}; this test is \
             checking a layer that may not exist any more"
        );
        assert!(
            script.contains(branch),
            "VERSIONING.md documents {pattern} and release.sh has no `{branch}` branch — \
             that layer would be tagged by hand"
        );
    }
}

/// The CLI's release must ask crates.io whether `yidam-core` is there yet.
///
/// This is the precondition that actually failed. The manifest says which version is
/// *required*; only the registry says which exists, so the check has to be a request. The
/// assertion is on the script rather than on a run of it, because a run would need the
/// network and this suite must not.
#[test]
fn the_cli_release_checks_that_yidam_core_is_published() {
    let script = read("release.sh");
    assert!(
        script.contains("crates.io/api/v1/crates/yidam-core/"),
        "release.sh must ask crates.io whether the required yidam-core exists; the CLI's \
         publish fails on a missing one, and the manifest cannot answer it"
    );
    assert!(
        script.contains("dependency-unpublished"),
        "release.sh must name the unpublished-dependency refusal, so the message says `tag \
         sdk/rust first` rather than reporting a cargo error from inside CI"
    );
    // 404 and "crates.io is down" want opposite responses — tag the SDK first, or try again
    // later. Collapsing them is how a precondition check becomes one people learn to skip.
    assert!(
        script.contains("registry-unreachable"),
        "release.sh must distinguish an unreachable registry from an unpublished version"
    );
}

/// The CLI's release must ask whether the Homebrew tap's credential exists.
///
/// `cli/v0.2.1` published four tarballs, both crates, and every checksum, and went red at
/// one job: the tap push, on a `HOMEBREW_TAP_TOKEN` that had never been created. The
/// failure was loud and the log said exactly what to do — and "loud" and "fixed" are
/// different states. In between them the tap served 0.2.0, which is the release whose Linux
/// binary does not run on Debian 12, which is why 0.2.1 exists (#246).
///
/// A missing credential is knowable before the tag. After it, the assets are out and the
/// release notes already claim `brew install` works.
#[test]
fn the_cli_release_checks_that_the_tap_token_exists() {
    let script = read("release.sh");
    assert!(
        script.contains("HOMEBREW_TAP_TOKEN"),
        "release.sh must ask whether the tap's token exists; without it a cli/v* tag \
         publishes every channel but the tap, and finds out afterwards"
    );
    assert!(
        script.contains("tap-token-missing"),
        "release.sh must name the missing-token refusal, so the message says which PAT to \
         create rather than reporting a red job from inside a release that already shipped"
    );
    // Secrets are listable only with admin. "Not there" and "you cannot see" want opposite
    // responses — create the PAT, or ask someone who can look — and collapsing them is how
    // a precondition check becomes one people learn to skip.
    assert!(
        script.contains("tap-token-unknown"),
        "release.sh must distinguish an absent token from one it is not allowed to see"
    );
}

/// The `edit` release must ask whether npm's credential exists.
///
/// The same argument as the tap check above, with the stakes one step higher: npm is the
/// `edit` layer's only channel — `edit.yml` creates no GitHub release, because the package
/// *is* the artifact. A missing token there is not "every channel but one", it is the tag
/// delivering nothing while the packed tarball, the green package job and the tag itself all
/// say a release happened.
///
/// `edit/v0.1.0` is why this exists. The precondition was written, worked, and ran only for
/// `cli` — a layer-shaped hole in a check whose argument never had anything to do with which
/// layer was being tagged.
///
/// **Comments are stripped before this looks.** The block below is prose-heavy on purpose,
/// and a test that greps the file is answered by its own explanation (the same failure as
/// a job name satisfying a guard). What is asserted is the code.
#[test]
fn the_edit_release_checks_that_the_npm_token_exists() {
    let script = read("release.sh");
    let code: String = script
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    // The guard has to be reached on the layer it is about. A check under the wrong `if` is
    // the bug this test exists for, spelled a second way — so this reads the bodies of the
    // edit-layer conditionals rather than the file, and there is more than one of them.
    let blocks: Vec<&str> = code
        .split(r#"if [ "$LAYER" = "edit" ]; then"#)
        .skip(1)
        .map(|rest| rest.split_once("\nfi").map(|(b, _)| b).unwrap_or(rest))
        .collect();
    assert!(
        !blocks.is_empty(),
        "release.sh no longer guards anything on the edit layer"
    );
    let guard = blocks.join("\n");
    assert!(
        guard.contains("NPM_TOKEN"),
        "release.sh asks about no npm credential when tagging `edit`. Without it the tag          packs a tarball, uploads it, and fails at the publish — with npm serving nothing          and the version spent"
    );
    assert!(
        guard.contains("npm-token-missing"),
        "release.sh must name the missing-token refusal, so the message says which token to          create rather than reporting a red job from inside a release that cannot be undone"
    );
    // Secrets are listable only with admin. "Not there" and "you cannot see" want opposite
    // responses, and collapsing them is how a precondition becomes one people skip.
    assert!(
        guard.contains("npm-token-unknown"),
        "release.sh must distinguish an absent npm token from one it is not allowed to see"
    );
}

/// A `cli/v*` tag must require tap.yml at HEAD, not only release.yml.
///
/// release.yml calls the tap by path, and a local `uses:` resolves at the caller's ref. A
/// tag carrying one file and not the other fires a release whose tap job cannot start —
/// the same silence as tagging before a workflow exists, which is the failure the
/// no-workflow refusal was written for.
#[test]
fn the_cli_release_requires_every_workflow_the_tag_fires() {
    let script = read("release.sh");
    let cli = script
        .lines()
        .find(|l| l.contains("TAG=\"cli/v$VERSION\""))
        .expect("release.sh no longer has a cli layer");
    for wf in [
        ".github/workflows/release.yml",
        ".github/workflows/tap.yml",
        ".github/workflows/publish-crates.yml",
    ] {
        assert!(
            cli.contains(wf),
            "a cli tag fires {wf} and release.sh does not require it at HEAD: {cli}"
        );
        assert!(
            repo_root().join(wf).exists(),
            "release.sh requires {wf} and it does not exist"
        );
    }
}

/// The release process must point at the script, not at `git tag`.
///
/// Every fact this script enforces was already written down somewhere. Being written down is
/// what failed; a document that goes back to spelling out `git tag -s` restores exactly the
/// state that produced the problem.
#[test]
fn the_documented_release_process_uses_the_script() {
    let versioning = read("VERSIONING.md");
    let process = versioning
        .split("## Release process")
        .nth(1)
        .expect("VERSIONING.md has a Release process section");
    assert!(
        process.contains("release.sh"),
        "the release process does not mention release.sh; the ordering is back to being \
         something the releaser has to remember"
    );
    assert!(
        !process.contains("git tag -s v"),
        "the release process spells out `git tag` again — that is the instruction that was \
         followed in the wrong order"
    );
    let mise = read("mise.toml");
    assert!(
        mise.contains("[tasks.release]"),
        "mise.toml has no `release` task, so `mise tasks` does not list the one command a \
         releaser needs"
    );
}

/// The tag-exists guard asks about one layer, not about any layer whose tag ends the same way.
///
/// `git ls-remote --tags <origin> <pattern>` matches a path **suffix**, not a ref. Four
/// layers share one tag namespace here, so asking for `v0.1.0` returns `editor/v0.1.0` and
/// `sdk/rust/v0.1.0` — and the first template release was refused as already existing,
/// against two tags belonging to other layers. The refusal is the safe direction and it is
/// still wrong: it blocks a release that should proceed, and no amount of looking at the tag
/// list explains it.
///
/// Behavioural, against a real remote, because the defect is in what git does with the
/// pattern and not in what the script says. Asserting both halves: the exact form must not
/// match a sibling layer, and must still find the tag it is actually about — a guard that
/// only checked the first would pass against a query matching nothing at all.
#[test]
fn the_tag_exists_check_does_not_match_another_layers_tag() {
    let dir = tempfile::tempdir().expect("tempdir");
    let origin = dir.path().join("origin.git");

    let work = dir.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let git = common::git::git;
    git(
        dir.path(),
        &["init", "-q", "--bare", origin.to_str().unwrap()],
    );
    git(&work, &["init", "-q", "-b", "main"]);
    git(&work, &["config", "user.email", "t@yidam.test"]);
    git(&work, &["config", "user.name", "T"]);
    git(&work, &["config", "tag.gpgsign", "false"]);
    git(&work, &["commit", "-q", "--allow-empty", "-m", "x"]);
    // Two other layers at 0.1.0. The template layer has no tag at all.
    git(&work, &["tag", "editor/v0.1.0"]);
    git(&work, &["tag", "sdk/rust/v0.1.0"]);
    git(
        &work,
        &["remote", "add", "origin", origin.to_str().unwrap()],
    );
    git(&work, &["push", "-q", "origin", "main", "--tags"]);

    let matches = |pattern: &str| -> Vec<String> {
        common::git::out(&work, &["ls-remote", "--tags", "origin", pattern])
            .lines()
            .filter_map(|l| l.split("refs/tags/").nth(1).map(str::to_string))
            .collect()
    };

    // The spelling that shipped, and why it refused.
    assert!(
        !matches("v0.1.0").is_empty(),
        "the bare pattern no longer over-matches, so this fixture no longer reproduces the \
         defect and must be rebuilt rather than deleted"
    );
    // The spelling the script uses now.
    assert!(
        matches("refs/tags/v0.1.0").is_empty(),
        "the tag-exists check still matches another layer's tag: {:?}",
        matches("refs/tags/v0.1.0")
    );
    // …and it must still find the tag it is actually about.
    assert_eq!(
        matches("refs/tags/editor/v0.1.0"),
        vec!["editor/v0.1.0".to_string()],
        "the exact form must still detect a tag that really exists, or the guard is a \
         query that matches nothing"
    );

    // And the script asks it that way.
    let script = read("release.sh");
    let asked = script
        .lines()
        .find(|l| l.contains("ls-remote") && l.contains("--tags") && l.contains("$TAG"))
        .expect("release.sh's remote tag-exists check");
    assert!(
        asked.contains("refs/tags/$TAG"),
        "release.sh asks the remote about `$TAG` rather than `refs/tags/$TAG`, which matches \
         any layer whose tag ends the same way:\n  {}",
        asked.trim()
    );
}

/// Where a note waits for its release, one file each.
const STAGED_NOTES: &str = ".changes/upgrading";

/// The line `scripts/file-upgrade-notes.sh` inserts a release's section under.
const FILING_ANCHOR: &str = "<!-- file-upgrade-notes: new release sections go below this line -->";

/// The notes staged in `.changes/upgrading/`, **as HEAD carries them** — file names, sorted.
///
/// From HEAD and not the working tree, because that is where `release.sh` reads them and its
/// reason is load-bearing: a tag names a commit, so a note only in the working tree is a note
/// the tagged ref does not carry. Reading the working tree here made the old version of this
/// test fail for any *uncommitted* note — which is exactly when someone writing one runs the
/// suite — and blamed the script for not refusing a note it could not see.
///
/// The directory's `README.md` explains the directory and is not a note.
fn staged_notes() -> Vec<String> {
    let out = common::git::raw(
        &repo_root(),
        &["ls-tree", "--name-only", &format!("HEAD:{STAGED_NOTES}")],
    );
    // A missing directory is "nothing staged", which is what release.sh reads it as too.
    let mut notes: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|n| n.ends_with(".md") && *n != "README.md")
        .map(str::to_string)
        .collect();
    notes.sort();
    notes
}

/// No note is written into `docs/upgrading.md` by hand, and the filing task has its anchor.
///
/// Notes used to be staged under `## Unreleased`, each pull request inserting at the top of it.
/// Two insertions at one line are a merge conflict, so any two open pull requests with a note
/// conflicted. A branch cut before the move and rebased after it would bring the heading back,
/// and `release.sh` no longer reads it — the note under it would reach no release. So the
/// heading is refused here, at pull-request time, rather than discovered at the tag.
#[test]
fn upgrade_notes_are_staged_as_files_and_not_under_an_unreleased_heading() {
    let doc = read("docs/upgrading.md");
    assert!(
        !doc.lines().any(|l| l.trim() == "## Unreleased"),
        "docs/upgrading.md has a `## Unreleased` heading again. Notes are staged one per file in \
         {STAGED_NOTES}/ and filed by `mise run file-upgrade-notes <tag>`; release.sh does not \
         read this heading, so a note under it reaches no release. Move each `### ` note into \
         its own {STAGED_NOTES}/<issue>-<slug>.md."
    );
    assert!(
        doc.lines().any(|l| l == FILING_ANCHOR),
        "docs/upgrading.md has lost the line `{FILING_ANCHOR}`. The filing task inserts each \
         release's section under it and refuses to file without it."
    );
}

/// A staged note refuses the tag — and only then.
///
/// Asserted against the repository's actual state rather than as a fixed expectation, so it
/// stays true through the state it is describing: between releases notes are staged and the
/// refusal must fire; right after one is filed nothing is staged and the refusal must *not*
/// fire. A test that only checked one of those would go green the moment the mechanism started
/// mattering, or the moment it stopped.
#[test]
fn a_staged_upgrade_note_refuses_the_tag_and_an_empty_directory_does_not() {
    // The same arguments `a_version_the_manifest_does_not_declare_is_refused` uses, so the
    // whole suite makes one release.sh invocation and one packaged-build check. The layer and
    // version are irrelevant here: this refusal is raised before either is consulted, and a
    // dry run reports every refusal rather than stopping at the first.
    let (out, _) = release(&["sdk/rust", "9.9.9"]);

    let staged = staged_notes();
    let refused = out.contains("staged-upgrade-note");

    if staged.is_empty() {
        assert!(
            !refused,
            "nothing is staged in {STAGED_NOTES}/ and release.sh refused anyway:\n{out}"
        );
    } else {
        assert!(
            refused,
            "{} note(s) are staged in {STAGED_NOTES}/ and release.sh did not refuse. They would \
             be dropped from this release and carried into the next.\nrelease.sh said:\n{out}",
            staged.len()
        );
    }
}

// ── the filing task ──────────────────────────────────────────────────────────────

/// A scratch repository holding this repository's `docs/upgrading.md` and filing script, plus
/// the given notes. The script needs a git work tree (it stages its result) and nothing else.
fn filing_fixture(notes: &[(&str, &str)]) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a scratch directory");
    let root = dir.path();
    for sub in ["docs", "scripts", STAGED_NOTES] {
        std::fs::create_dir_all(root.join(sub)).expect("a fixture directory");
    }
    std::fs::write(root.join("docs/upgrading.md"), read("docs/upgrading.md"))
        .expect("the document copies");
    let script = root.join("scripts/file-upgrade-notes.sh");
    std::fs::copy(repo_root().join("scripts/file-upgrade-notes.sh"), &script)
        .expect("the script copies");
    for (name, body) in notes {
        std::fs::write(root.join(STAGED_NOTES).join(name), body).expect("a note writes");
    }
    common::git::git(root, &["init", "--quiet"]);
    common::git::git(root, &["add", "--all"]);
    dir
}

/// Run the filing script in `root` for `tag`: (stdout + stderr, success).
fn file_notes(root: &std::path::Path, tag: &str) -> (String, bool) {
    let out = Command::new("bash")
        .arg("scripts/file-upgrade-notes.sh")
        .arg(tag)
        .current_dir(root)
        .output()
        .expect("bash runs the filing script");
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    (text, out.status.success())
}

/// The `### ` headings under `## <heading>` in a document, in order.
fn notes_under(doc: &str, heading: &str) -> Vec<String> {
    let mut inside = false;
    let mut out = Vec::new();
    for line in doc.lines() {
        if let Some(h) = line.strip_prefix("## ") {
            inside = h.trim() == heading;
        } else if let Some(note) = line.strip_prefix("### ").filter(|_| inside) {
            out.push(note.trim().to_string());
        }
    }
    out
}

/// Every note staged today is one the filing task accepts, and filing it loses no line of it.
///
/// Run over the real notes rather than a description of them, so a malformed note — a name
/// without its issue number, a `##` heading that would cut the release section short — fails
/// the pull request that adds it rather than the person cutting the release.
#[test]
fn every_staged_upgrade_note_files_whole() {
    let root = repo_root().join(STAGED_NOTES);
    let staged: Vec<(String, String)> = std::fs::read_dir(&root)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".md") && n != "README.md")
                .map(|n| {
                    let body = std::fs::read_to_string(root.join(&n)).expect("a note reads");
                    (n, body)
                })
                .collect()
        })
        .unwrap_or_default();
    if staged.is_empty() {
        // Right after a release. The fixture tests below still exercise the script.
        return;
    }
    let notes: Vec<(&str, &str)> = staged
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let dir = filing_fixture(&notes);
    let (out, ok) = file_notes(dir.path(), "cli/v99.0.0");
    assert!(ok, "the filing task refuses a staged note:\n{out}");

    let doc = std::fs::read_to_string(dir.path().join("docs/upgrading.md")).expect("reads");
    let section: String = {
        let mut inside = false;
        doc.lines()
            .filter(|l| {
                if let Some(h) = l.strip_prefix("## ") {
                    inside = h.trim() == "cli/v99.0.0";
                    return false;
                }
                inside
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    for (name, body) in &staged {
        for line in body.lines().filter(|l| !l.trim().is_empty()) {
            assert!(
                section.lines().any(|l| l == line),
                "{STAGED_NOTES}/{name}: the line {line:?} did not reach the filed section — \
                 the release would publish the note without it"
            );
        }
    }
}

/// Filing orders notes newest issue first, removes the files, and stages both sides.
#[test]
fn filing_orders_notes_by_issue_and_empties_the_directory() {
    let dir = filing_fixture(&[
        ("919-older.md", "### Older\n\nFirst written.\n"),
        ("1172-newer.md", "### Newer\n\nWritten later.\n\n\n"),
        ("1054-middle.md", "### Middle\n\nIn between.\n"),
    ]);
    let (out, ok) = file_notes(dir.path(), "cli/v99.0.0");
    assert!(ok, "filing three well-formed notes failed:\n{out}");

    let doc = std::fs::read_to_string(dir.path().join("docs/upgrading.md")).expect("reads");
    // Numeric, not lexical: "919" sorts after "1172" as text and would file the oldest first.
    assert_eq!(
        notes_under(&doc, "cli/v99.0.0"),
        ["Newer", "Middle", "Older"],
        "notes must file newest issue first"
    );
    let anchor = doc.find(FILING_ANCHOR).expect("the anchor survives");
    let section = doc.find("## cli/v99.0.0").expect("the section is written");
    assert!(
        anchor < section && doc[anchor..section].trim() == FILING_ANCHOR,
        "the new section must sit directly under the anchor"
    );
    // Up to the section after it — the older sections are not this task's to reformat.
    let filed = &doc[section..];
    let filed = &filed[..filed[3..].find("\n## ").map_or(filed.len(), |i| i + 3)];
    assert!(
        !filed.contains("\n\n\n"),
        "filing left a run of blank lines — a note's trailing blanks were copied:\n{filed}"
    );

    let left: Vec<_> = std::fs::read_dir(dir.path().join(STAGED_NOTES))
        .expect("the directory remains")
        .filter_map(Result::ok)
        .map(|e| e.file_name())
        .collect();
    assert!(left.is_empty(), "filed notes were left behind: {left:?}");
    let unstaged = common::git::out(dir.path(), &["status", "--porcelain"]);
    assert!(
        unstaged
            .lines()
            .all(|l| !l.starts_with(' ') && !l.starts_with('?')),
        "filing must stage what it changed, so the commit is one command:\n{unstaged}"
    );
}

/// A note merged after the first filing but before the tag goes into the same section, on top.
#[test]
fn filing_again_adds_to_the_existing_section() {
    let dir = filing_fixture(&[("1000-first.md", "### First\n\nFiled first.\n")]);
    assert!(file_notes(dir.path(), "cli/v99.0.0").1);
    std::fs::write(
        dir.path().join(STAGED_NOTES).join("1001-late.md"),
        "### Late\n\nMerged after.\n",
    )
    .expect("a note writes");
    let (out, ok) = file_notes(dir.path(), "cli/v99.0.0");
    assert!(ok, "filing into an existing section failed:\n{out}");

    let doc = std::fs::read_to_string(dir.path().join("docs/upgrading.md")).expect("reads");
    assert_eq!(
        doc.lines().filter(|l| *l == "## cli/v99.0.0").count(),
        1,
        "a second filing must not write a second section for the same tag"
    );
    assert_eq!(notes_under(&doc, "cli/v99.0.0"), ["Late", "First"]);
}

/// A malformed note refuses the whole filing and touches nothing.
///
/// Each shape is one that would go wrong silently if filed: no issue number leaves the order
/// undefined, no `### ` heading leaves the note without a title, and a `##` heading ends the
/// release section early — `release.yml` reads up to the next `## `, so what follows it would
/// reach no release.
#[test]
fn a_malformed_note_refuses_the_filing_and_changes_nothing() {
    for (name, body, says) in [
        ("no-issue.md", "### Fine\n\nText.\n", "the name must be"),
        ("12-no-heading.md", "Just prose.\n", "'### ' heading"),
        (
            "13-outer.md",
            "### Fine\n\n## A section of its own\n",
            "split the release section",
        ),
    ] {
        let dir = filing_fixture(&[("1-good.md", "### Good\n\nText.\n"), (name, body)]);
        let before = std::fs::read_to_string(dir.path().join("docs/upgrading.md")).unwrap();
        let (out, ok) = file_notes(dir.path(), "cli/v99.0.0");
        assert!(!ok, "{name} was filed:\n{out}");
        assert!(out.contains(says), "{name}: expected {says:?} in:\n{out}");
        let after = std::fs::read_to_string(dir.path().join("docs/upgrading.md")).unwrap();
        assert_eq!(
            before, after,
            "{name}: a refused filing changed the document"
        );
        assert!(
            dir.path().join(STAGED_NOTES).join("1-good.md").exists(),
            "{name}: a refused filing removed a good note"
        );
    }
    // A `##` inside a fenced block is an example, not a heading.
    let dir = filing_fixture(&[("14-fenced.md", "### Fine\n\n```md\n## not a heading\n```\n")]);
    let (out, ok) = file_notes(dir.path(), "cli/v99.0.0");
    assert!(ok, "a fenced `##` was read as a heading:\n{out}");
}

/// The release must publish the note for the tag it is cutting, and must not lose the list.
///
/// `--generate-notes` produces a flat list of PR titles with nowhere in it for prose, which is
/// how #549 — a change that stops a working client configuration from starting — would have
/// shipped as one `fix(serve): …` line among twenty. The repair is a composed body, and it has
/// two halves that can each be lost silently: the upgrade note, and the list it goes above.
#[test]
fn the_release_publishes_the_upgrade_note_for_the_tag_it_cuts() {
    let workflow = read(".github/workflows/release.yml");
    let commands = workflow
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        commands.contains("docs/upgrading.md"),
        "release.yml never reads docs/upgrading.md, so a filed upgrade note reaches the docs \
         site and not the release anyone is reading before they upgrade"
    );
    assert!(
        commands.contains("releases/generate-notes"),
        "release.yml must still generate the list of merged PRs; prepending a note is not a \
         reason to stop saying what changed"
    );
    assert!(
        commands.contains("--notes-file"),
        "release.yml must publish the composed body with --notes-file"
    );
    // Both flags on one `gh release create` is an ambiguity nothing here has tested, and it
    // would be discovered by pushing a tag. One mechanism.
    let create = commands
        .split("gh release create")
        .nth(1)
        .expect("release.yml creates a release");
    assert!(
        !create.contains("--generate-notes"),
        "`gh release create` is passed both --notes-file and --generate-notes; which one wins \
         is untested here and a tag push is a poor place to find out"
    );
}

// ── #772: a note filed under a release that already shipped ──────────────────────

/// Every `git` invocation this section makes, from the repository root.
fn git_out(args: &[&str]) -> std::process::Output {
    common::git::raw(&repo_root(), args)
}

/// The `## ` sections of `docs/upgrading.md` at HEAD, each with its `### ` note headings.
///
/// **Discovered, not listed.** A hardcoded set of tags stops covering new releases without
/// ever going red — the shape #680 is about one layer up.
fn upgrade_sections() -> Vec<(String, Vec<String>)> {
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for line in at_head("docs/upgrading.md").lines() {
        if let Some(heading) = line.strip_prefix("## ") {
            out.push((heading.trim().to_string(), Vec::new()));
        } else if let Some(note) = line.strip_prefix("### ") {
            if let Some(last) = out.last_mut() {
                last.1.push(note.trim().to_string());
            }
        }
    }
    out
}

/// A note filed under a release that had already shipped reaches no release at all.
///
/// `release.yml` reads the section matching the tag **at tag time**:
///
/// ```sh
/// awk '/^## '"$GITHUB_REF_NAME"'$/{inside=1;next} /^## /{inside=0} inside' docs/upgrading.md
/// ```
///
/// So a note written after `cli/v0.10.0` was tagged and filed under `## cli/v0.10.0` was
/// generated-and-published before it existed, and the next tag reads a different heading. It is
/// written, reviewed, merged, and unreachable.
///
/// **Three of them were.** `08cbbbe`, `1af765d` and `238bc32` each landed after `fc96175` (the
/// `cli/v0.10.0` tag) and each filed under that heading; found while cutting `cli/v0.11.0`.
///
/// This is a third way the mechanism fails silently, and it is not the one `release.sh` covers.
/// A staged note sits in `.changes/upgrading/`, where it looks unfinished. A misfiled note sits
/// under a heading that looks **filed**, so every existing check reads it as done. Staging notes
/// as files and filing them by task (`scripts/file-upgrade-notes.sh`) takes the heading out of
/// the author's hands, which removes the usual way in; this still catches a hand edit.
///
/// The rule: for every `## <tag>` section whose tag exists, the commit that introduced each
/// `### ` heading must be an ancestor of that tag.
#[test]
fn no_upgrade_note_is_filed_under_a_release_that_already_shipped() {
    // A shallow clone with no tags cannot answer this. Refused rather than skipped, for the
    // reason the tap-token check refuses when it cannot see: a check that guesses when blind is
    // one people learn to ignore.
    let tags = git_out(&["tag", "--list", "cli/v*"]);
    let tag_list = String::from_utf8_lossy(&tags.stdout);
    assert!(
        tags.status.success() && tag_list.split_whitespace().count() > 0,
        "no `cli/v*` tags are visible, so no note's placement can be checked. Fetch tags \
         (`git fetch --tags`) rather than reading this as a pass."
    );

    let sections = upgrade_sections();
    let mut checked = 0usize;
    let mut stranded: Vec<String> = Vec::new();

    for (heading, notes) in &sections {
        // A section whose tag does not exist is the release being prepared — there is nothing
        // to compare against yet: a section filed for a tag not cut yet, or one of the
        // document's own `## ` sections, which are not releases.
        if !git_out(&[
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("refs/tags/{heading}"),
        ])
        .status
        .success()
        {
            continue;
        }
        for note in notes {
            // The *introducing* commit — the oldest one whose diff changed the count of this
            // string — not the latest. A later typo fix or reflow of an already-correct note
            // must not fail, and moving a stranded note into the right section must pass, which
            // is what makes the repair verifiable.
            let log = git_out(&[
                "log",
                "--format=%H",
                "-S",
                &format!("### {note}"),
                "--",
                "docs/upgrading.md",
            ]);
            let commits = String::from_utf8_lossy(&log.stdout);
            let Some(introduced) = commits.split_whitespace().last() else {
                // A heading `-S` cannot find is one this test cannot place. Reported, because
                // silently skipping it is how a scan comes to look at nothing.
                stranded.push(format!(
                    "  {heading} — no commit introduces `### {note}`; -S found nothing"
                ));
                continue;
            };
            checked += 1;
            if !git_out(&["merge-base", "--is-ancestor", introduced, heading])
                .status
                .success()
            {
                stranded.push(format!(
                    "  under `## {heading}`: `### {note}`\n      introduced by {}, which is not \
                     an ancestor of {heading} — the note was written after that release shipped, \
                     so it reaches no release at all. Move it under the tag that carries it.",
                    &introduced[..introduced.len().min(8)]
                ));
            }
        }
    }

    // The floor. A parser that stopped recognising `## `/`### ` would leave the loop above
    // iterating nothing and passing, and the population here is discovered rather than fixed.
    assert!(
        checked >= 10,
        "only {checked} note(s) were placed against a tag — the section scan is looking at \
         nothing ({} sections found)",
        sections.len()
    );
    assert!(
        stranded.is_empty(),
        "upgrade notes filed under a release that had already shipped:\n{}",
        stranded.join("\n")
    );
}

/// A job that runs the CLI test suite must check out tags and history.
///
/// Part of that suite reads *this repository's* git rather than a fixture: the misfiled-note gate
/// above resolves `cli/v*` tags and walks `docs/upgrading.md` with `git log -S`. `actions/checkout`
/// is shallow and tagless by default, so a job with a bare checkout cannot answer, and the gate
/// refuses — by design, rather than passing over nothing.
///
/// That is how `ci (cli · full features)` went red on `main` while `ci (cli)` was green on the
/// same commit. It is a main-only job, so no pull request could have shown it. `ci (mutants)` had
/// the same checkout and has never run: `cargo mutants` runs the suite for its unmutated baseline,
/// and a failing baseline aborts before a single mutation is applied.
///
/// Asserted over the jobs **discovered** to run the suite, so a fourth one joins this by running
/// it rather than by someone remembering to add it here.
#[test]
fn every_job_that_runs_the_cli_suite_checks_out_tags_and_history() {
    // Comments stripped first. Every one of these jobs explains itself at length, and three of
    // them — `parity`, `quality`, `series` — mention a coverage run or a test task in prose while
    // running neither: `quality`'s note that a run "died afterwards on a coverage flag" is enough
    // to match a task called `coverage`. Prose about the code must not answer for the code.
    let workflow: String = read(".github/workflows/ci.yml")
        .lines()
        .map(|line| match line.split_once('#') {
            Some((before, _)) if before.trim().is_empty() || before.ends_with(' ') => before,
            _ => line,
        })
        .collect::<Vec<_>>()
        .join("\n");

    // Split into jobs on the two-space-indented `<name>:` that opens one.
    let mut jobs: Vec<(String, String)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in workflow.lines() {
        let opens_job = line.len() > 3
            && line.starts_with("  ")
            && !line.starts_with("   ")
            && line.trim_end().ends_with(':')
            && !line.trim_start().starts_with('#');
        if opens_job {
            if let Some((name, body)) = current.take() {
                jobs.push((name, body.join("\n")));
            }
            current = Some((line.trim().trim_end_matches(':').to_string(), Vec::new()));
        } else if let Some((_, body)) = current.as_mut() {
            body.push(line);
        }
    }
    if let Some((name, body)) = current {
        jobs.push((name, body.join("\n")));
    }
    assert!(
        jobs.len() > 5,
        "only {} job(s) parsed out of ci.yml — the job splitter is looking at nothing",
        jobs.len()
    );

    // What counts as running the suite: the mise tasks that invoke nextest over this crate, read
    // out of `mise.toml`, plus `cargo mutants`, which runs it as its baseline from a command line
    // this workflow spells itself.
    //
    // This was a list of four needles, and the doc comment above already claimed the population
    // was discovered. #1013 is what the difference costs: the light gate's step changed from
    // `mise run ci-cli` to a task named through the matrix, `mise run ci-cli-full` and
    // `coverage-full` still matched in the job next door, the floor of two was still cleared —
    // and `ci (cli)`, which runs the suite and needs the history, had silently dropped out of
    // the set being checked.
    let mise: toml::Table = read("mise.toml").parse().expect("mise.toml parses");
    let suite_tasks: Vec<String> = mise
        .get("tasks")
        .and_then(toml::Value::as_table)
        .expect("mise.toml declares tasks")
        .iter()
        .filter(|(_, task)| {
            let steps = match task.get("run") {
                Some(toml::Value::String(one)) => vec![one.as_str().to_string()],
                Some(toml::Value::Array(many)) => many
                    .iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect(),
                _ => Vec::new(),
            };
            steps.iter().any(|step| {
                // `nextest` as a token: every such line also passes
                // `--config-file .config/nextest.toml`, so a substring test is true of a line
                // that runs `cargo test` under llvm-cov instead.
                step.contains("yidam/cli/Cargo.toml")
                    && (step.contains("cargo nextest run")
                        || (step.contains("llvm-cov")
                            && step.split_whitespace().any(|token| token == "nextest")))
            })
        })
        .map(|(name, _)| name.clone())
        .collect();
    assert!(
        suite_tasks.len() >= 3,
        "only {suite_tasks:?} look like tasks that run the CLI suite; the light gate, the \
         full-feature gate and the coverage runs are each one, so this is reading the wrong thing \
         and every job below would pass by not being looked at"
    );

    // Whole tokens, never substrings: `mise run coverage-full` must not answer for a job that
    // runs `coverage`, and `ci-cli` must not answer for `ci-cli-cov`. A job "runs" a task when it
    // names it — as a step, or as the matrix value a step expands to.
    let names = |body: &str, task: &str| {
        body.split(|c: char| c.is_whitespace() || c == '\'' || c == '"' || c == '`')
            .any(|token| token == task)
    };
    let runs_suite =
        |body: &str| body.contains("cargo mutants") || suite_tasks.iter().any(|t| names(body, t));

    let suite_jobs: Vec<&(String, String)> = jobs.iter().filter(|(_, b)| runs_suite(b)).collect();
    // Three, and three is what this repository has: `ci (cli)`, `ci (cli · full features)` and
    // `ci (mutants)`. No slack — a floor with room in it is a budget for a scanner going blind.
    assert!(
        suite_jobs.len() >= 3,
        "expected three jobs running the CLI suite, found {}: {:?}. Tasks searched for: \
         {suite_tasks:?}",
        suite_jobs.len(),
        suite_jobs.iter().map(|(n, _)| n).collect::<Vec<_>>()
    );

    let shallow: Vec<&String> = suite_jobs
        .iter()
        .filter(|(_, body)| !body.contains("fetch-depth: 0"))
        .map(|(name, _)| name)
        .collect();
    assert!(
        shallow.is_empty(),
        "these jobs run the CLI test suite with a shallow, tagless checkout: {shallow:?}\n  \
         Part of the suite reads this repository's tags and the history of docs/upgrading.md, so \
         it cannot answer there and refuses rather than passing. Add `with: fetch-depth: 0` to \
         their `actions/checkout` step."
    );
}
