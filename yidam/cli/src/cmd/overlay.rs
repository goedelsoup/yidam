use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;
use crate::provenance::Provenance;

use super::tracked;

/// The three subtrees `overlay` copies, in the order it reports them.
///
/// Named here rather than three times below because each one is now asked the same question —
/// what does git track under it — and a list of three is how that stays one question. What
/// makes them the three is that they are yidam's contribution to a repository that already
/// has its own content: the prelude and harness, the scaffold, and the seeds.
const SUBTREES: &[&str] = &["yidam", "sadhana", "samudaya"];

/// Seed files the bootstrap will read, counted over the tracked set.
///
/// `samudaya/examples/` is excluded and nothing else is, which is the whole of why the domain
/// seed sets live there — `samudaya/examples/README.md` argues it at length, and
/// `yidam samudaya-audit` skips exactly the same one subdirectory. A set placed anywhere else
/// under `samudaya/` would be read as a **live seed of this repository** and copied into every
/// repository `overlay` is run against.
///
/// The count is over `paths`, so an uncommitted seed file does not raise it. That is the same
/// answer the copy gives: a file git does not track is not going to be copied either, and a
/// count that disagreed with the copy would announce a seed over a directory that arrived
/// empty.
fn count_seeds(paths: &[String]) -> usize {
    paths
        .iter()
        .filter(|p| !p.starts_with("samudaya/examples/"))
        .filter(|p| !p.ends_with("/README.md"))
        .filter(|p| {
            let name = p.rsplit('/').next().unwrap_or(p);
            matches!(
                name.rsplit_once('.').map(|(_, ext)| ext).unwrap_or(""),
                "md" | "toml" | "yml"
            )
        })
        .count()
}

pub fn overlay(target: &Path, backfill: bool, backfill_ref: Option<&str>) -> Result<()> {
    if !target.exists() {
        bail!(
            "target does not exist: {}\n       use `yidam clone` to create a new repo from the template",
            target.display()
        );
    }
    if !target.join(".git").exists() {
        bail!("target is not a git repository: {}", target.display());
    }
    if target.join(".yidam").exists() {
        bail!(
            ".yidam/ already exists in {} — yidam is already bootstrapped",
            target.display()
        );
    }

    let root = repo_root()?;

    // What travels is what git tracks, asked once and sliced per subtree below (#984). The
    // copy used to walk the working tree and subtract a list of names, which is a list of
    // what somebody thought to ask about: measured against this checkout it carried
    // `yidam/editors/vscode/out/`, two `.astro/` build caches, two `junit.xml` reports and a
    // pytest cache into every repository `overlay` was run against — all gitignored, all
    // absent from `git ls-files` the whole time. This is the half of bootstrap where the
    // target already holds someone else's work, so it is the worse place to deposit debris.
    let tracked = tracked::paths(&root)?;

    // Recorded before anything is copied — the copy never carries `.git`, and the target's
    // own `.git` belongs to the existing repo, not to yidam.
    let provenance = Provenance::read(&root);

    for subtree in SUBTREES {
        let paths = tracked::under(&tracked, subtree);
        if paths.is_empty() {
            continue;
        }

        // `samudaya/` is the one subtree with a condition beyond being tracked: it travels
        // only when this checkout holds seeds of its own, which this one must not.
        if *subtree == "samudaya" {
            match count_seeds(&paths) {
                0 => {
                    println!("samudaya/ has no seeds — skipping");
                    continue;
                }
                n => println!("samudaya/ has {n} seed file(s)"),
            }
        }

        // No exclusions: `tracked::copy_into` matches them against a path's first segment,
        // and every path here already has this subtree's name there. `clone`'s
        // `NOT_INHERITED` is about what a *root* copy leaves behind, and none of its entries
        // is one of these three.
        let copied = tracked::copy_into(&root, target, &paths, &[])?;
        println!(
            "{subtree}/ → {}/{subtree}/ ({copied} tracked file(s))",
            target.display()
        );
    }

    // BOOTSTRAP.md — skip if target already has one
    let bootstrap_dst = target.join("BOOTSTRAP.md");
    let bootstrap_src = root.join("BOOTSTRAP.md");
    if bootstrap_src.exists() {
        if bootstrap_dst.exists() {
            println!("BOOTSTRAP.md already exists in target — skipping (existing file kept)");
        } else {
            std::fs::copy(&bootstrap_src, &bootstrap_dst).context("copying BOOTSTRAP.md")?;
            println!("BOOTSTRAP.md → {}/BOOTSTRAP.md", target.display());
        }
    }

    // mise.yidam.toml — yidam CLI task definitions
    let mise_src = root.join("mise.yidam.toml");
    if mise_src.exists() {
        std::fs::copy(&mise_src, target.join("mise.yidam.toml"))
            .context("copying mise.yidam.toml")?;
        println!("mise.yidam.toml → {}/mise.yidam.toml", target.display());
    }

    // .yidam.toml — provenance pin (see VERSIONING.md, Layer 1)
    provenance.write(target)?;
    println!(
        ".yidam.toml → {}/.yidam.toml (pinned to {} · {})",
        target.display(),
        &provenance.commit[..provenance.commit.len().min(8)],
        provenance.template
    );

    println!();
    println!("yidam infrastructure overlaid on {}", target.display());
    println!();
    println!("Next steps:");
    println!("  1. Add to your mise.toml:  [task_config]");
    println!("                             includes = [\"mise.yidam.toml\"]");
    println!("  2. Open {} in your IDE", target.display());
    println!("  3. The agent will read BOOTSTRAP.md and enter existing-repo mode");

    if backfill {
        println!();
        super::backfill::backfill_history(target, backfill_ref)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    /// A template checkout holding the three subtrees, plus the gitignored debris a real
    /// working tree accumulates inside them.
    ///
    /// The debris is not invented: these are the paths `git status --ignored` reported under
    /// `yidam/` in this repository when #984 was filed, and not one of them was reached by
    /// any entry in the exclusion list the copy used to subtract.
    fn template(root: &Path, extras: &[&str]) {
        let write = |rel: &str, body: &str| {
            let path = root.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, body).unwrap();
        };
        write("yidam/prelude/GRAPH.md", "the graph");
        write("yidam/prelude/skills/bootstrap.md", "the protocol");
        write("yidam/editors/vscode/src/extension.ts", "authored");
        write("sadhana/root/README.md", "the scaffold");
        write("sadhana/docs/README.md", "read by step 3");
        write("samudaya/README.md", "how seeds work");
        write("samudaya/examples/axioms.md", "a format stub");
        write("docs/rfcs/0001.md", "yidam's own");
        write("LICENSE", "MIT");
        write(
            ".gitignore",
            "junit.xml\nout/\n.astro/\n.pytest_cache/\ntarget/\n",
        );
        for rel in extras {
            write(rel, "a seed");
        }
        crate::git::fixture::init(root);
        crate::git::fixture::commit(root, "seed");

        // Written after the commit, so git tracks none of it.
        for rel in [
            "yidam/editors/vscode/junit.xml",
            "yidam/editors/vscode/out/extension.js",
            "yidam/editors/web/.astro/types.d.ts",
            "yidam/editors/web/junit.xml",
            "yidam/prelude/sdks/python/.pytest_cache/v/cache/lastfailed",
            "yidam/web/docs/.astro/settings.json",
            "sadhana/target/debris",
        ] {
            write(rel, "not the template");
        }
    }

    /// Every file under `dir`, as paths relative to `root`.
    fn delivered(root: &Path, dir: &Path) -> BTreeSet<String> {
        fn walk(at: &PathBuf, out: &mut Vec<PathBuf>) {
            let Ok(entries) = std::fs::read_dir(at) else {
                return;
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_dir() {
                    walk(&path, out);
                } else {
                    out.push(path);
                }
            }
        }
        let mut found = Vec::new();
        walk(&dir.to_path_buf(), &mut found);
        found
            .iter()
            .map(|p| {
                p.strip_prefix(root)
                    .unwrap_or(p)
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect()
    }

    /// The gate #912 got and this one did not: what each subtree delivers is *exactly* what
    /// git tracks under it. Equality in both directions — a copy that dropped an authored
    /// file would satisfy a one-sided "nothing untracked travelled" assertion.
    #[test]
    fn each_subtree_delivers_exactly_the_tracked_set() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("existing-repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        template(&root, &["samudaya/axioms.md"]);

        let all = tracked::paths(&root).unwrap();
        for subtree in SUBTREES {
            let paths = tracked::under(&all, subtree);
            tracked::copy_into(&root, &dst, &paths, &[]).unwrap();
            assert_eq!(
                delivered(&dst, &dst.join(subtree)),
                paths.iter().cloned().collect::<BTreeSet<_>>(),
                "`{subtree}/` does not deliver the tracked set under it"
            );
        }
    }

    /// The reported case, named file by file. Each of these is gitignored, each was reported
    /// by `git status --ignored` under `yidam/`, and each travelled into every repository
    /// `overlay` was run against because the list of names it subtracted did not spell it.
    #[test]
    fn the_gitignored_debris_under_a_subtree_does_not_travel() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("existing-repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        template(&root, &[]);

        let all = tracked::paths(&root).unwrap();
        for subtree in SUBTREES {
            tracked::copy_into(&root, &dst, &tracked::under(&all, subtree), &[]).unwrap();
        }

        // The debris is in the source, so the loop below is asking a question with an answer.
        assert!(root.join("yidam/editors/vscode/out/extension.js").is_file());

        for rel in [
            "yidam/editors/vscode/junit.xml",
            "yidam/editors/vscode/out",
            "yidam/editors/web/.astro",
            "yidam/editors/web/junit.xml",
            "yidam/prelude/sdks/python/.pytest_cache",
            "yidam/web/docs/.astro",
            "sadhana/target",
        ] {
            assert!(
                !dst.join(rel).exists(),
                "`{rel}` is gitignored and is not part of the template, but it was overlaid \
                 onto a repository that already holds someone's work"
            );
        }
        // The paired assertion: a copy that delivered nothing would satisfy the loop above.
        assert!(dst.join("yidam/prelude/GRAPH.md").exists());
        assert!(dst.join("yidam/editors/vscode/src/extension.ts").exists());
    }

    /// What `overlay` does *not* copy is the rest of the repository. It names three subtrees,
    /// so a tracked file outside them — `LICENSE`, yidam's own `docs/` — has no route in, and
    /// needs no exclusion list to keep it out.
    #[test]
    fn nothing_outside_the_three_subtrees_travels() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("existing-repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        template(&root, &[]);

        let all = tracked::paths(&root).unwrap();
        for subtree in SUBTREES {
            tracked::copy_into(&root, &dst, &tracked::under(&all, subtree), &[]).unwrap();
        }

        assert!(
            !dst.join("docs").exists(),
            "yidam's own docs/ are not yidam"
        );
        assert!(!dst.join("LICENSE").exists(), "the target has its own");
        assert!(
            dst.join("sadhana/docs/README.md").exists(),
            "`sadhana/docs/` is a different directory at a different depth from `docs/`, and \
             step 3 reads it"
        );
    }

    /// This repository's own `samudaya/` holds documentation and the four format stubs and
    /// nothing else, and `overlay` must keep printing *"samudaya/ has no seeds — skipping"*
    /// over it. Otherwise every repository overlaid from this template inherits three other
    /// domains' axioms as seeds for its own bootstrap to consume.
    #[test]
    fn the_example_seed_sets_are_not_seeds() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        std::fs::create_dir_all(&root).unwrap();
        template(
            &root,
            &[
                "samudaya/examples/genealogy/axioms.md",
                "samudaya/examples/genealogy/constraints.yml",
            ],
        );

        let paths = tracked::under(&tracked::paths(&root).unwrap(), "samudaya");
        assert_eq!(
            count_seeds(&paths),
            0,
            "a set under `samudaya/examples/` was counted as a live seed of this repository"
        );
    }

    /// And the converse, so the count above is not vacuous: a seed placed where a derived
    /// repository's own would go is counted, and `samudaya/` then travels.
    #[test]
    fn a_seed_outside_the_examples_is_counted() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        std::fs::create_dir_all(&root).unwrap();
        template(&root, &["samudaya/axioms.md", "samudaya/constraints.yml"]);

        let paths = tracked::under(&tracked::paths(&root).unwrap(), "samudaya");
        assert_eq!(count_seeds(&paths), 2);
    }
}
