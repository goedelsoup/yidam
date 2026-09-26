use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;
use crate::provenance::Provenance;

use super::clone::require_template_root;
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

/// What `overlay` tells a reader to run instead.
///
/// Two lines rather than `clone`'s one, and the second names a path rather than `.`, because
/// the target of an overlay is a repository that already exists: the reader who reached this
/// refusal was standing *in* it, and the remedy has to move them out and then point back.
/// `docs/first-corpus-by-hand.md` gives the same route — clone the template, `cd` into it,
/// overlay the named project — and is the one place it was already written down correctly.
/// It differs in one detail: the clone lands in `/tmp` rather than beside the shell, because
/// the shell that reached *this* message is standing in the project being overlaid, and a
/// checkout cloned into it would become part of it.
const OVERLAY_REMEDY: &str = concat!(
    "      git clone https://github.com/goedelsoup/yidam /tmp/yidam\n",
    "      cd /tmp/yidam && yidam overlay <path-to-your-project>",
);

/// Every subtree that was not deliberately skipped is in the target, read off the target.
///
/// The marker gate above asks the *source* a question about its filesystem; the copy asks it
/// a different question — what does git track — and those two can disagree. A checkout whose
/// `yidam/prelude/` and `sadhana/` exist but hold nothing git tracks passes the first and
/// copies nothing, and the loop's `continue` on an empty path list is silent about it. So the
/// success banner is earned here, against the tree the banner is about, rather than inferred
/// from having reached the end of a loop.
///
/// `seedless` is the one legitimate absence: `samudaya/` travels only when the source holds
/// seeds of its own, and this repository's own must not (see [`count_seeds`]). It is passed
/// in rather than re-derived so that this function cannot disagree with the line that was
/// printed about it.
fn require_delivered(root: &Path, target: &Path, seedless: bool) -> Result<()> {
    let missing: Vec<&str> = SUBTREES
        .iter()
        .copied()
        .filter(|s| !(*s == "samudaya" && seedless))
        .filter(|s| !target.join(s).is_dir())
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    // What this says is what it looked at. It read the target and those directories are not
    // there; the sentence after names the mechanism that produces that, rather than claiming
    // to have measured it. Nor does it say the target is untouched — the subtrees that did
    // arrive are still in it, and a partial overlay is what the reader has to clear.
    bail!(
        "overlay delivered no {}: {} not in the target after the copy\n  \
         `overlay` copies what git tracks under each subtree of {}, so a source that tracks \
         nothing there delivers nothing — and the success banner would name infrastructure \
         that never arrived. Anything that did arrive is still in {}.",
        missing
            .iter()
            .map(|s| format!("`{s}/`"))
            .collect::<Vec<_>>()
            .join(", "),
        if missing.len() == 1 {
            "it is"
        } else {
            "they are"
        },
        root.display(),
        target.display(),
    )
}

pub fn overlay(target: &Path, backfill: bool, backfill_ref: Option<&str>) -> Result<()> {
    overlay_from(&repo_root()?, target, backfill, backfill_ref)
}

/// The overlay itself, with the source named rather than discovered.
///
/// Split out of [`overlay`] so the refusals below can be asked of a fixture. The discovery is
/// the whole of what [`overlay`] adds, and it is what put a target-shaped answer — the
/// caller's own project — where the template belonged.
fn overlay_from(
    root: &Path,
    target: &Path,
    backfill: bool,
    backfill_ref: Option<&str>,
) -> Result<()> {
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

    require_template_root(root, "overlay", OVERLAY_REMEDY)?;

    // What travels is what git tracks, asked once and sliced per subtree below (#984). The
    // copy used to walk the working tree and subtract a list of names, which is a list of
    // what somebody thought to ask about: measured against this checkout it carried
    // `yidam/editors/vscode/out/`, two `.astro/` build caches, two `junit.xml` reports and a
    // pytest cache into every repository `overlay` was run against — all gitignored, all
    // absent from `git ls-files` the whole time. This is the half of bootstrap where the
    // target already holds someone else's work, so it is the worse place to deposit debris.
    let tracked = tracked::paths(root)?;

    // Recorded before anything is copied — the copy never carries `.git`, and the target's
    // own `.git` belongs to the existing repo, not to yidam.
    let provenance = Provenance::read(root);

    // The one absence [`require_delivered`] is allowed to forgive, recorded where it is
    // decided rather than asked again afterwards.
    let mut seedless = false;

    for subtree in SUBTREES {
        let paths = tracked::under(&tracked, subtree);

        // `samudaya/` is the one subtree with a condition beyond being tracked: it travels
        // only when this checkout holds seeds of its own, which this one must not. Asked
        // before the emptiness check below, so a `samudaya/` that git tracks nothing under
        // is reported as the skip it is rather than passed on to the refusal.
        if *subtree == "samudaya" {
            match count_seeds(&paths) {
                0 => {
                    println!("samudaya/ has no seeds — skipping");
                    seedless = true;
                    continue;
                }
                n => println!("samudaya/ has {n} seed file(s)"),
            }
        }

        if paths.is_empty() {
            continue;
        }

        // No exclusions: `tracked::copy_into` matches them against a path's first segment,
        // and every path here already has this subtree's name there. `clone`'s
        // `NOT_INHERITED` is about what a *root* copy leaves behind, and none of its entries
        // is one of these three.
        let copied = tracked::copy_into(root, target, &paths, &[])?;
        println!(
            "{subtree}/ → {}/{subtree}/ ({copied} tracked file(s))",
            target.display()
        );
    }

    require_delivered(root, target, seedless)?;

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

    /// An ordinary project — the reported case. `overlay` derives its source from the
    /// repository the shell is standing in, and in a project that is not a yidam checkout
    /// that source *is the target*: every subtree slice comes back empty, every copy is
    /// skipped, and what the reader was shown was a success banner over a tree holding their
    /// own work and a pin to a repository they have no copy of.
    ///
    /// The exit code alone was never the whole of it, so three things are asserted: that it
    /// refuses, that it says what to run instead, and — in two assertions, the pin by name
    /// and then the tree — that it leaves the target as it found it. A refusal added after
    /// the copy rather than before it would pass the first two and fail the last.
    #[test]
    fn an_ordinary_project_is_refused_and_left_alone() {
        let tmp = tempfile::TempDir::new().unwrap();
        let project = tmp.path().join("notes");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(project.join("README.md"), "hi").unwrap();
        crate::git::fixture::repo(&project, "init");

        let err = overlay_from(&project, &project, false, None)
            .unwrap_err()
            .to_string();

        assert!(
            err.contains("not a yidam template checkout"),
            "the refusal must name what the source is not: {err}"
        );
        assert!(
            err.contains("git clone https://github.com/goedelsoup/yidam"),
            "a refusal with no remedy leaves the reader where the success banner did: {err}"
        );
        assert!(
            !project.join(".yidam.toml").exists(),
            "a pin to a repository the tree has no copy of was written anyway"
        );
        assert_eq!(
            delivered(&project, &project)
                .into_iter()
                .filter(|p| !p.starts_with(".git/"))
                .collect::<BTreeSet<_>>(),
            ["README.md".to_string()]
                .into_iter()
                .collect::<BTreeSet<_>>(),
            "the refused overlay wrote into the target"
        );
    }

    /// The paired positive: from a real template checkout, the subtrees arrive and the run
    /// succeeds. Without this the refusal above is satisfied by an `overlay` that refuses
    /// everything.
    #[test]
    fn a_template_checkout_delivers_the_subtrees() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("existing-repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        template(&root, &[]);
        std::fs::write(dst.join("README.md"), "someone else's work").unwrap();
        crate::git::fixture::repo(&dst, "init");

        overlay_from(&root, &dst, false, None).expect("a template checkout overlays");

        assert!(dst.join("yidam/prelude/GRAPH.md").is_file());
        assert!(dst.join("sadhana/root/README.md").is_file());
        assert!(dst.join(".yidam.toml").is_file());
        assert!(
            !dst.join("samudaya").exists(),
            "this checkout holds no seeds of its own, so `samudaya/` must not travel"
        );
        assert!(
            dst.join("README.md").is_file(),
            "the target's own work survives the overlay"
        );
    }

    /// The gap the marker gate does not close, and the reason [`require_delivered`] reads the
    /// tree rather than trusting the loop.
    ///
    /// `require_template_root` asks the filesystem whether two directories exist; the copy
    /// asks git what it tracks. A checkout where both markers are present and gitignored
    /// answers *yes* to the first and delivers nothing, which is the same empty target the
    /// reported case produced — reached by a different route, so a fix that only added the
    /// marker gate would still announce it as a success.
    #[test]
    fn a_source_that_tracks_nothing_under_a_subtree_is_refused() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = tmp.path().join("yidam");
        let dst = tmp.path().join("existing-repo");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&dst).unwrap();
        crate::git::fixture::write(&root, ".gitignore", "yidam/\nsadhana/\n");
        crate::git::fixture::write(&root, "LICENSE", "MIT");
        crate::git::fixture::repo(&root, "seed");
        // After the commit, so both markers are directories git tracks nothing under.
        crate::git::fixture::write(&root, "yidam/prelude/GRAPH.md", "the graph");
        crate::git::fixture::write(&root, "sadhana/root/README.md", "the scaffold");
        std::fs::write(dst.join("README.md"), "someone else's work").unwrap();
        crate::git::fixture::repo(&dst, "init");

        // The marker gate is satisfied — this is the state it cannot see.
        require_template_root(&root, "overlay", OVERLAY_REMEDY)
            .expect("both markers are directories here");

        let err = overlay_from(&root, &dst, false, None)
            .unwrap_err()
            .to_string();
        assert!(
            err.starts_with(
                "overlay delivered no `yidam/`, `sadhana/`: they are not in the target"
            ),
            "the refusal must name the subtrees that did not arrive, and agree with itself \
             about how many: {err}"
        );
        assert!(
            !dst.join(".yidam.toml").exists(),
            "the pin was written over a target the infrastructure never reached"
        );
    }

    /// `samudaya/` is the one subtree allowed to be absent, and only for the one declared
    /// reason. Both directions, because a postcondition that forgave it unconditionally
    /// would pass a target that received neither seeds nor the skip that explains them.
    #[test]
    fn only_a_seedless_samudaya_is_forgiven() {
        let tmp = tempfile::TempDir::new().unwrap();
        let dst = tmp.path();
        std::fs::create_dir_all(dst.join("yidam")).unwrap();
        std::fs::create_dir_all(dst.join("sadhana")).unwrap();

        require_delivered(dst, dst, true).expect("a checkout with no seeds of its own");

        let err = require_delivered(dst, dst, false).unwrap_err().to_string();
        assert!(
            err.starts_with("overlay delivered no `samudaya/`: it is not in the target"),
            "a samudaya/ that was counted as having seeds and did not arrive, named in the \
             singular because it is the only one: {err}"
        );
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
