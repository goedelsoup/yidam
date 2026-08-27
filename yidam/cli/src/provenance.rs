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

/// Git facts about the yidam tree a derived repo is being created from.
#[derive(Debug, PartialEq, Eq)]
pub struct Provenance {
    pub origin: String,
    pub commit: String,
    pub template: String,
    /// Author date of the pinned commit — not the date this repo ran the vendor step.
    /// It answers "how old is the prelude I am running", which staleness actually turns on.
    pub committed: String,
}

fn git_output(root: &Path, args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("git")
        .current_dir(root)
        .args(args)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    String::from_utf8(out.stdout)
        .ok()
        .map(|s| s.trim().to_string())
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
            // `--exact-match` so a derived repo is pinned to a release or to nothing.
            // `git describe` without it yields "v0.1.0-14-gabc1234", which reads as a
            // version but is not one.
            //
            // `--match` because four layers release from one repository and routinely tag
            // the same commit. Without it this asks "any tag here?" and answers with
            // whichever git prefers: at `a7aaa5f`, which carries `cli/v0.4.0`,
            // `sdk/rust/v0.3.0` and `editor/v0.1.0`, it answers `editor/v0.1.0` — so every
            // repository cloned there recorded a VS Code extension's version as the version
            // of its template layer. The field names one layer and must ask about that one.
            template: git_output(
                root,
                &[
                    "describe",
                    "--tags",
                    "--exact-match",
                    "--match",
                    TEMPLATE_TAG_GLOB,
                    "HEAD",
                ],
            )
            .unwrap_or_else(|| "untagged".to_string()),
            committed: git_output(root, &["log", "-1", "--format=%as", "HEAD"])
                .unwrap_or_else(|| "unknown".to_string()),
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
             # check out. `template` is the release tag at that commit, or \"untagged\".\n\
             # `committed` is that commit's date — how old the vendored prelude is.\n\
             #\n\
             # See .yidam/.vendor/prelude/guidelines/directories.md for the re-vendor procedure.\n\
             \n\
             [yidam]\n\
             origin    = \"{}\"\n\
             commit    = \"{}\"\n\
             template  = \"{}\"\n\
             committed = \"{}\"\n",
            self.origin, self.commit, self.template, self.committed
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

    fn git(dir: &Path, args: &[&str]) {
        let status = std::process::Command::new("git")
            .current_dir(dir)
            .args(args)
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed");
    }

    fn repo_with_commit(root: &Path) {
        git(root, &["init", "-q", "-b", "main"]);
        git(root, &["config", "user.email", "t@t.co"]);
        git(root, &["config", "user.name", "Test"]);
        std::fs::write(root.join("a"), "a").unwrap();
        git(root, &["add", "."]);
        git(root, &["commit", "-q", "-m", "seed"]);
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

        // Without --exact-match this would be "v0.1.0-1-g<sha>", which reads as a
        // version but does not identify a release.
        assert_eq!(Provenance::read(root).template, "untagged");
    }

    #[test]
    fn degrades_to_unknown_outside_a_git_repo() {
        let tmp = tempfile::TempDir::new().unwrap();
        let p = Provenance::read(tmp.path());
        assert_eq!(p.commit, "unknown");
        assert_eq!(p.template, "untagged");
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
    }
}
