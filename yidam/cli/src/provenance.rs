//! Recording which yidam a derived repository came from.
//!
//! [VERSIONING.md](../../../VERSIONING.md) specifies that "every derived repo carries a
//! `.yidam.toml` at its root" pinning the template and bootstrap-protocol versions. Nothing
//! wrote it, so no derived repo had one, and a repo with no recorded origin cannot be
//! upgraded — there is no baseline to compute a forward change against.
//!
//! This module writes that file at the only moment the information is available: while the
//! yidam source tree is still a git repository. `clone` and `overlay` both copy the template
//! *without* its `.git`, so by the time the bootstrap agent runs there is nothing left to
//! read the origin from.
//!
//! `commit` is the field that makes the pin resolvable. `template` and `bootstrap` are
//! semantic versions from tags, and yidam is not yet tagged — a pin that records only an
//! untagged version number points at nothing.

use anyhow::{Context, Result};
use std::path::Path;

/// Where a derived repository's provenance is recorded, relative to its root.
pub const MANIFEST: &str = ".yidam.toml";

/// The tag pattern that names the **template** layer, per VERSIONING.md: bare `v<semver>`.
///
/// Every other layer prefixes its tag — `cli/v*`, `sdk/rust/v*`, `bootstrap/v*`, `editor/v*` —
/// so this glob is what distinguishes the one layer `.yidam.toml`'s `template` field is about
/// from the four it is not.
pub const TEMPLATE_TAG_GLOB: &str = "v[0-9]*";

/// The tag pattern that names a **release**: `cli/v<semver>`, the tag the release workflow
/// publishes binaries for and the re-vendor task pins (#1308).
pub const RELEASE_TAG_GLOB: &str = "cli/v[0-9]*";

/// Release tags `release` never names: a pre-release such as `cli/v0.27.0-rc.1`. The
/// `template` clock could not place one in the version order (RFC-0065 §1).
pub const RELEASE_TAG_EXCLUDE: &str = "cli/v*-*";

/// What `release` reads when no release tag is in the pin's history.
pub const UNRELEASED: &str = "unreleased";

/// The version a release tag names, when the tag is `cli/vX.Y.Z` and nothing more.
///
/// The same shape `newest_release` in the task layer counts, so the field and the clock
/// that reads it agree on what a release is.
pub fn release_version(tag: &str) -> Option<(u64, u64, u64)> {
    let rest = tag.strip_prefix("cli/v")?;
    let mut parts = rest.split('.');
    let mut next = || -> Option<u64> {
        let p = parts.next()?;
        if p.is_empty() || !p.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        p.parse().ok()
    };
    let v = (next()?, next()?, next()?);
    parts.next().is_none().then_some(v)
}

/// The release a `release` field counts from: `cli/v0.23.0+5` counts from `cli/v0.23.0`.
pub fn base_release(field: &str) -> &str {
    field.split_once('+').map_or(field, |(base, _)| base)
}

/// The nearest tag matching `glob` at or before HEAD, and how many commits HEAD is past it.
///
/// Asked with `--long`, whose answer is always `<tag>-<n>-g<sha>`. The tag itself may hold
/// a `-`, so the two trailing fields are split off from the right.
fn describe_nearest(root: &Path, glob: &str, exclude: Option<&str>) -> Option<(String, u64)> {
    let mut args = vec!["describe", "--tags", "--long", "--match", glob];
    if let Some(ex) = exclude {
        args.extend(["--exclude", ex]);
    }
    args.push("HEAD");
    let out = git_output(root, &args)?;
    let mut fields = out.rsplitn(3, '-');
    let _sha = fields.next()?;
    let n = fields.next()?.parse().ok()?;
    let tag = fields.next()?;
    Some((tag.to_string(), n))
}

/// `<tag>` when exact, `<tag>+<n>` when `n` commits past (RFC-0065 §2). Git's own
/// `<tag>-<n>-g<sha>` repeats the commit already in `commit`, and `-<n>` reads as a semver
/// pre-release.
fn nearest(tag: &str, n: u64) -> String {
    if n == 0 {
        tag.to_string()
    } else {
        format!("{tag}+{n}")
    }
}

/// Git facts about the yidam tree a derived repo is being created from.
#[derive(Debug, PartialEq, Eq)]
pub struct Provenance {
    pub origin: String,
    pub commit: String,
    pub template: String,
    /// Author date of the pinned commit — not the date this repo ran the vendor step.
    /// It answers "how old is the prelude I am running", which staleness actually turns on.
    pub committed: String,
    /// The nearest `cli/vX.Y.Z` release at or before `commit`: the version this pin
    /// installs. `cli/v0.23.0+5` is five commits past it; `unreleased` is no release at all.
    ///
    /// `cli` cannot answer this. It names the binary that wrote the file, which is often
    /// not the one the pin installs.
    pub release: String,
    /// The `yidam` binary that wrote this manifest, as `<version> (<build commit>)`.
    ///
    /// The other five fields describe the template tree, and the tree is not what copies
    /// itself. A derived repository once carried a `.claude/worktrees/` copy of the template
    /// that the pinned commit had already learned to leave behind; an older binary earlier on
    /// PATH had done the copy, and nothing recorded that it had (#1076).
    pub cli: String,
}

/// This binary, in the form `cli` records.
///
/// Version and commit only. The feature list `--version` also prints says what the binary
/// can do next, not what it did here, and would make the field differ between two builds
/// of one commit that copied identically.
pub fn cli() -> String {
    let b = crate::report::YidamBlock::current();
    format!("{} ({})", b.version, b.commit)
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    crate::git::Git::new(root)
        .args(args)
        .try_run()
        .filter(|s| !s.is_empty())
}

impl Provenance {
    /// Read provenance from the yidam source tree at `root`.
    ///
    /// Every field degrades to `"unknown"` rather than failing: a template copied by hand,
    /// or extracted from a tarball, still produces a well-formed manifest. An honest
    /// `"unknown"` is upgradable-by-hand; a missing file is not upgradable at all.
    pub fn read(root: &Path) -> Self {
        Self {
            origin: git_output(root, &["remote", "get-url", "origin"])
                .unwrap_or_else(|| "unknown".to_string()),
            commit: git_output(root, &["rev-parse", "HEAD"])
                .unwrap_or_else(|| "unknown".to_string()),
            // The nearest template tag at or before the pin, rendered `<tag>+<n>` when
            // `n` commits past it (RFC-0065 §2). An exact match turned every pin between
            // template tags into "untagged", and since #1308 most pins are.
            //
            // `--match` because four layers release from one repository and routinely tag
            // the same commit. Without it this asks "any tag here?" and answers with
            // whichever git prefers: at `a7aaa5f`, which carries `cli/v0.4.0`,
            // `sdk/rust/v0.3.0` and `editor/v0.1.0`, it answers `editor/v0.1.0` — so every
            // repository cloned there recorded a VS Code extension's version as the version
            // of its template layer. The field names one layer and must ask about that one.
            template: describe_nearest(root, TEMPLATE_TAG_GLOB, None)
                .map_or_else(|| "untagged".to_string(), |(tag, n)| nearest(&tag, n)),
            committed: git_output(root, &["log", "-1", "--format=%as", "HEAD"])
                .unwrap_or_else(|| "unknown".to_string()),
            release: describe_nearest(root, RELEASE_TAG_GLOB, Some(RELEASE_TAG_EXCLUDE))
                .filter(|(tag, _)| release_version(tag).is_some())
                .map_or_else(|| UNRELEASED.to_string(), |(tag, n)| nearest(&tag, n)),
            cli: cli(),
        }
    }

    pub fn render(&self) -> String {
        format!(
            "# Provenance of the yidam template this repository was derived from.\n\
             #\n\
             # Written at clone/overlay time, when the yidam source was still a git\n\
             # repository. Updated by `mise run yidam-vendor-update`.\n\
             #\n\
             # `commit` is the resolvable pin: it is what the re-vendor procedure and CI\n\
             # check out. `template` is the nearest template tag at or before it, `+n` when\n\
             # n commits past, or \"untagged\". `committed` is that commit's date.\n\
             # `release` is the cli/v* release the pin installs, `+n` when past it.\n\
             # `cli` is the binary that wrote this file, not the one the pin installs:\n\
             # version (build commit).\n\
             #\n\
             # See .yidam/.vendor/prelude/guidelines/directories.md for the re-vendor procedure.\n\
             \n\
             [yidam]\n\
             origin    = \"{}\"\n\
             commit    = \"{}\"\n\
             template  = \"{}\"\n\
             committed = \"{}\"\n\
             release   = \"{}\"\n\
             cli       = \"{}\"\n",
            self.origin, self.commit, self.template, self.committed, self.release, self.cli
        )
    }

    /// Write `.yidam.toml` into a derived repository root.
    pub fn write(&self, target: &Path) -> Result<()> {
        let path = target.join(MANIFEST);
        std::fs::write(&path, self.render())
            .with_context(|| format!("writing {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::git::fixture::git;

    fn repo_with_commit(root: &Path) {
        crate::git::fixture::write(root, "a", "a");
        crate::git::fixture::repo(root, "seed");
    }

    #[test]
    fn reads_commit_and_marks_untagged_release() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        repo_with_commit(root);

        let p = Provenance::read(root);
        assert_eq!(p.commit.len(), 40, "commit should be a full sha: {p:?}");
        assert_eq!(p.template, "untagged");
        assert_ne!(p.committed, "unknown");
    }

    #[test]
    fn reads_exact_tag_as_template_version() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        repo_with_commit(root);
        git(root, &["tag", "v0.1.0"]);

        assert_eq!(Provenance::read(root).template, "v0.1.0");
    }

    #[test]
    fn a_tag_on_an_ancestor_does_not_count_as_this_release() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        repo_with_commit(root);
        git(root, &["tag", "v0.1.0"]);
        std::fs::write(root.join("b"), "b").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "later"]);

        // The nearest tag and the distance from it. Git's own "v0.1.0-1-g<sha>" would
        // repeat `commit` and read as a pre-release; "untagged" would hide both.
        assert_eq!(Provenance::read(root).template, "v0.1.0+1");
    }

    #[test]
    fn release_names_the_cli_tag_and_counts_commits_past_it() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path();
        repo_with_commit(root);
        assert_eq!(Provenance::read(root).release, UNRELEASED);

        // Every layer tags this commit; only the cli layer's tag names the release.
        for t in ["v0.1.0", "editor/v0.9.0", "sdk/rust/v0.5.0", "cli/v0.23.0"] {
            git(root, &["tag", t]);
        }
        assert_eq!(Provenance::read(root).release, "cli/v0.23.0");

        std::fs::write(root.join("b"), "b").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "later"]);
        // A pre-release is never named, and the release behind it still is.
        git(root, &["tag", "cli/v0.24.0-rc.1"]);
        let p = Provenance::read(root);
        assert_eq!(p.release, "cli/v0.23.0+1");
        assert_eq!(p.template, "v0.1.0+1");
    }

    #[test]
    fn a_release_tag_is_cli_v_and_three_numbers() {
        assert_eq!(release_version("cli/v0.23.0"), Some((0, 23, 0)));
        assert_eq!(release_version("cli/v10.2.33"), Some((10, 2, 33)));
        for bad in [
            "v0.23.0",
            "cli/v0.23",
            "cli/v0.23.0.1",
            "cli/v0.23.0-rc.1",
            "cli/v0.x.0",
            "cli/v.1.2",
            "cli/v0.23.0+5",
        ] {
            assert_eq!(release_version(bad), None, "{bad}");
        }
        assert_eq!(base_release("cli/v0.23.0+5"), "cli/v0.23.0");
        assert_eq!(base_release("cli/v0.23.0"), "cli/v0.23.0");
    }

    #[test]
    fn degrades_to_unknown_outside_a_git_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = Provenance::read(tmp.path());
        assert_eq!(p.commit, "unknown");
        assert_eq!(p.template, "untagged");
        assert_eq!(p.release, UNRELEASED);
    }

    #[test]
    fn writes_a_parseable_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(&src).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        repo_with_commit(&src);

        let p = Provenance::read(&src);
        p.write(&dst).unwrap();

        let text = std::fs::read_to_string(dst.join(MANIFEST)).unwrap();
        let parsed: toml::Value = toml::from_str(&text).expect("manifest must be valid TOML");
        assert_eq!(
            parsed["yidam"]["commit"].as_str().unwrap(),
            p.commit,
            "commit must round-trip — it is the field the re-vendor resolves"
        );
        assert_eq!(parsed["yidam"]["release"].as_str().unwrap(), p.release);
        assert_eq!(
            parsed["yidam"]["cli"].as_str().unwrap(),
            format!(
                "{} ({})",
                env!("CARGO_PKG_VERSION"),
                env!("YIDAM_BUILD_COMMIT")
            ),
            "cli must name this binary — it is what attributes a bad copy to the tool that made it"
        );
    }
}
