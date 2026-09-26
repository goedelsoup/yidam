use anyhow::{bail, Result};
use std::path::Path;

use crate::paths::repo_root;
use crate::provenance::Provenance;

use super::tracked;

/// Top-level paths a derived repository does not inherit.
///
/// The rule is one question asked of each entry: **does this run yidam against yidam?** A
/// derived repository has no `yidam/cli` to release, no Homebrew tap to render, no plugin
/// marketplace to publish itself to, and no extension source for an editor launch config to
/// point at. Everything here is machinery this repository operates on itself.
///
/// Every entry is **tracked**, and that is now the whole content of this list. What the copy
/// delivers is `git ls-files` minus these names (see [`super::tracked`]), so a build
/// directory, an install prefix or an agent worktree is not excluded here and never was —
/// it is simply not part of the template. The list only ever has to answer for things
/// somebody authored and committed on purpose.
///
/// It is a list of what does *not* travel rather than a list of what does, because the copy
/// is wholesale within the tracked set: a file added to this repository ships into every
/// repository created after it. `template_root.rs` asks the question of all of them, by
/// discovering the tracked set rather than restating it — this list stayed at two entries
/// for as long as nothing looked (#807).
///
/// Two of them carry their own argument, and it is not "yidam's machinery":
///
/// - `docs` documents yidam, not the repository being created. `sadhana/docs/` is a
///   different directory at a different depth and must ship — step 3 of the bootstrap skill
///   reads it — which is why these exclusions are top-level only.
/// - `examples` holds a whole worked corpus, its own `.yidam/corpus`, catalog, decisions and
///   skills. Copying it deposits a foreign domain's nodes into a repository at the moment of
///   its creation, before it has an ontology of its own. A newcomer reads the example in
///   yidam; they do not inherit it.
///
/// `.github` is one entry and twelve files. #589 found five of yidam's own workflows
/// surviving genesis and fixed it by having step 3 replace the *workflows directory*
/// wholesale — which left `.github/actions/`, `.github/scripts/`, `.github/dependabot.yml`
/// and an ISSUE_TEMPLATE that files findings against **this** repository in every derived
/// repository. Not one of them is a workflow, so the fix that named workflows did not reach
/// them. Step 3 still replaces the directory: in existing-repo mode there was never a clone
/// to exclude from, and the workflows being replaced there are the target's own.
///
/// `.oxlintrc.json` is the linter contract for three npm packages, all of which live under
/// `yidam/` — which the bootstrap skill deletes. Excluding it leaves no dangling `extends`
/// behind, because the configs that extend it do not survive genesis either. A derived
/// repository has no `package.json` and no TypeScript at all; the file would configure a
/// linter for nothing.
///
/// What is deliberately absent: `BOOTSTRAP.md` and `VERSIONING.md`. Both are yidam's, both
/// are read during bootstrap — the first is the entry prompt that starts it — and the vendor
/// step in step 8 deletes them once they have been used. Shipping a file the protocol
/// consumes is not the same as leaking one it never names.
pub const NOT_INHERITED: &[&str] = &[
    ".claude-plugin",
    ".config",
    ".github",
    ".oxlintrc.json",
    ".vscode",
    "deny.toml",
    "docs",
    "examples",
    "install.sh",
    "release.sh",
    "render-formula.sh",
    "render-manifest.sh",
    "scripts",
];

/// What makes a directory the template, rather than something the template produced.
///
/// `clone` copies the directory it is run from, and nothing asked whether that directory was
/// the template (#913). Both answers it got wrong were reachable from the quickstart, which
/// ends in `/tmp/streamflow` and then says `yidam clone ~/my-corpus`: from an empty directory
/// it wrote a target holding one file, `.yidam.toml`; from a copy of `examples/streamflow` it
/// copied 21 files of someone else's domain and announced streamflow's own genesis sha as a
/// yidam pin. Both exited 0.
///
/// These two directories are the discovery predicate because they are what a derived
/// repository does **not** have. `yidam/prelude/` is vendored into `.yidam/.vendor/` and the
/// directory itself is deleted at genesis; `sadhana/` is the scaffold, consumed in step 3 and
/// deleted with it. A repository that came out of bootstrap therefore fails this test, which
/// is the point: it is not a template and cannot be cloned again.
pub const TEMPLATE_MARKERS: &[&str] = &["yidam/prelude", "sadhana"];

/// Refuse a source that is not the yidam template.
fn require_template_root(root: &Path) -> Result<()> {
    let missing: Vec<String> = TEMPLATE_MARKERS
        .iter()
        .filter(|m| !root.join(m).is_dir())
        .map(|m| format!("`{m}/`"))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    let expected = TEMPLATE_MARKERS
        .iter()
        .map(|m| format!("`{m}/`"))
        .collect::<Vec<_>>()
        .join(" and ");
    bail!(
        "not a yidam template checkout: {} has no {}\n  \
         `clone` copies the directory it is run from, and the template is the yidam \
         repository itself — the one carrying {}. Run it from a checkout of that:\n      \
         git clone https://github.com/goedelsoup/yidam && cd yidam\n  \
         A repository derived from the template is not one: bootstrap vendors the prelude \
         and deletes both directories at genesis, so a derived repository cannot be cloned \
         again. To move a corpus, copy the corpus.",
        root.display(),
        missing.join(" or "),
        expected,
    )
}

pub fn clone(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("target already exists: {}", target.display());
    }

    let root = repo_root()?;
    require_template_root(&root)?;
    let tracked = tracked::paths(&root)?;

    // Read before copying: the copy never carries `.git`, so once the copy is the only
    // thing left there is nothing to read the origin commit from.
    let provenance = Provenance::read(&root);

    println!("Copying template → {} …", target.display());
    let copied = tracked::copy_into(&root, target, &tracked, NOT_INHERITED)?;

    provenance.write(target)?;
    println!(
        "Pinned to yidam {} ({})",
        &provenance.commit[..provenance.commit.len().min(8)],
        provenance.template
    );

    if !crate::git::Git::new(target)
        .args(["init", "-q"])
        .succeeded()
    {
        bail!("git init failed in {}", target.display());
    }

    // mise trust is best-effort — not all environments have mise
    let _ = std::process::Command::new("mise")
        .args(["trust", "-q"])
        .current_dir(target)
        .status();

    println!(
        "yidam template copied to {} ({copied} tracked file(s))",
        target.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal template checkout: the two marker directories, a commit, and the kind of
    /// gitignored debris a real working tree accumulates beside them.
    fn template(root: &Path, extras: &[(&str, &str)]) {
        let write = |rel: &str, body: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write("yidam/prelude/GRAPH.md", "the graph");
        write("sadhana/root/README.md", "the scaffold");
        write("sadhana/docs/README.md", "read by step 3");
        write("docs/rfcs/0001.md", "yidam's own");
        write("LICENSE", "MIT");
        for (rel, body) in extras {
            write(rel, body);
        }
        // `git::fixture`, not a local spawn: a fixture that ran through the runner could
        // build a repository that hides a bug in it, which is why the fixtures are their
        // own module rather than a caller of `Git` (#929).
        crate::git::fixture::init(root);
        crate::git::fixture::commit(root, "seed");
    }

    #[test]
    fn the_copy_is_the_tracked_set_minus_the_exclusions() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("derived");
        std::fs::create_dir_all(&root).unwrap();
        template(&root, &[]);

        let tracked = tracked::paths(&root).unwrap();
        tracked::copy_into(&root, &dst, &tracked, NOT_INHERITED).unwrap();

        assert!(dst.join("yidam/prelude/GRAPH.md").exists());
        assert!(dst.join("LICENSE").exists());
        assert!(
            dst.join("sadhana/docs/README.md").exists(),
            "`sadhana/docs/` is a different directory at a different depth from the excluded \
             `docs/`, and step 3 reads it"
        );
        assert!(!dst.join("docs").exists(), "root docs/ should be skipped");
    }

    /// The reported case. `.claude/worktrees/` was 1,436 files and 14 MB of the 22.7 MB a
    /// clone delivered — gitignored the whole time, and invisible to a copy that walked the
    /// filesystem and subtracted names.
    #[test]
    fn nothing_git_does_not_track_travels() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("derived");
        std::fs::create_dir_all(&root).unwrap();
        template(
            &root,
            &[(".gitignore", ".claude/worktrees/\n.local/\nbuild-out/\n")],
        );

        // Written after the commit, so git tracks none of it.
        for rel in [
            ".claude/worktrees/task/corpus.yml",
            ".local/bin/yidam",
            "build-out/blob",
            "untracked-note.md",
        ] {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, "not the template").unwrap();
        }

        let tracked = tracked::paths(&root).unwrap();
        tracked::copy_into(&root, &dst, &tracked, NOT_INHERITED).unwrap();

        for rel in [
            ".claude/worktrees",
            ".local",
            "build-out",
            "untracked-note.md",
        ] {
            assert!(
                !dst.join(rel).exists(),
                "`{rel}` is not tracked and is not part of the template, but it was copied"
            );
        }
        // The paired assertion: a copy that delivered nothing would satisfy the loop above.
        assert!(dst.join("yidam/prelude/GRAPH.md").exists());
    }

    /// `build-out/` in the fixture above is the assertion a name list cannot make. Nothing
    /// names it, no `CACHEDIR.TAG` marks it, and it is excluded anyway — being untracked is
    /// now the whole of the rule, so there is no fourth convention waiting to be found.
    #[test]
    fn an_unnamed_build_directory_needs_no_convention_to_be_excluded() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("derived");
        std::fs::create_dir_all(&root).unwrap();
        template(&root, &[]);
        std::fs::create_dir_all(root.join("build-out")).unwrap();
        std::fs::write(root.join("build-out/blob"), "output").unwrap();

        let tracked = tracked::paths(&root).unwrap();
        tracked::copy_into(&root, &dst, &tracked, NOT_INHERITED).unwrap();

        assert!(!dst.join("build-out").exists());
    }

    #[test]
    fn a_checkout_carrying_both_markers_is_a_template() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("yidam/prelude")).unwrap();
        std::fs::create_dir_all(tmp.path().join("sadhana")).unwrap();
        require_template_root(tmp.path()).expect("both markers present");
    }

    /// The quickstart's own trap: §3 leaves the reader in a copy of `examples/streamflow`
    /// and §4 says `yidam clone ~/my-corpus`. That directory has a `.yidam/` and neither
    /// marker, and the refusal has to name what it was looking for.
    #[test]
    fn a_corpus_is_not_a_template() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam/corpus")).unwrap();

        let err = require_template_root(tmp.path()).unwrap_err().to_string();
        assert!(
            err.contains("yidam/prelude"),
            "the refusal must name the checkout it expected: {err}"
        );
        assert!(
            err.contains("sadhana"),
            "both markers belong in the message: {err}"
        );
    }

    /// A derived repository: bootstrap deleted `yidam/` and consumed `sadhana/`, so the
    /// marker that survives is neither of them. Cloning it again would copy a corpus.
    #[test]
    fn a_derived_repository_is_not_a_template() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join(".yidam/corpus")).unwrap();
        std::fs::write(tmp.path().join(".yidam.toml"), "[yidam]\n").unwrap();

        assert!(require_template_root(tmp.path()).is_err());
    }

    #[test]
    fn an_empty_directory_is_not_a_template() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(require_template_root(tmp.path()).is_err());
    }
}
