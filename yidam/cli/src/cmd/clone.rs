use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;
use crate::provenance::Provenance;

use super::copy::copy_dir_excluding_top;

/// Top-level paths a derived repository does not inherit.
///
/// The rule is one question asked of each entry: **does this run yidam against yidam?** A
/// derived repository has no `yidam/cli` to release, no Homebrew tap to render, no plugin
/// marketplace to publish itself to, and no extension source for an editor launch config to
/// point at. Everything here is machinery this repository operates on itself.
///
/// It is a list of what does *not* travel rather than a list of what does, because the copy
/// is wholesale: a path added to this repository ships into every repository created after
/// it, and the question is only ever asked of paths somebody thought to ask about.
/// `template_root.rs` asks it of all of them, by discovering the tracked set rather than
/// restating it — this list stayed at two entries for as long as nothing looked (#807).
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
    "packages",
    "release.sh",
    "render-formula.sh",
    "render-manifest.sh",
    "scripts",
];

pub fn clone(target: &Path) -> Result<()> {
    if target.exists() {
        bail!("target already exists: {}", target.display());
    }

    let root = repo_root()?;

    // Read before copying: `copy_dir` excludes `.git`, so once the copy is the only
    // thing left there is nothing to read the origin commit from.
    let provenance = Provenance::read(&root);

    println!("Copying template → {} …", target.display());
    copy_dir_excluding_top(&root, target, NOT_INHERITED)?;

    provenance.write(target)?;
    println!(
        "Pinned to yidam {} ({})",
        &provenance.commit[..provenance.commit.len().min(8)],
        provenance.template
    );

    let init = std::process::Command::new("git")
        .args(["init", "-q"])
        .current_dir(target)
        .status()
        .context("running git init")?;
    if !init.success() {
        bail!("git init failed in {}", target.display());
    }

    // mise trust is best-effort — not all environments have mise
    let _ = std::process::Command::new("mise")
        .args(["trust", "-q"])
        .current_dir(target)
        .status();

    println!("yidam template copied to {}", target.display());
    Ok(())
}
