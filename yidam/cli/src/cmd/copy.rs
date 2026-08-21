use anyhow::{Context, Result};
use std::path::Path;

/// Directories never copied, at any depth: version control and build output.
///
/// `docs` is deliberately absent. It used to be here to keep yidam's own `docs/` (the RFC
/// set, the docs-site source) out of derived repos — but the match is by name at every
/// depth, so it also dropped `sadhana/docs/`, the scaffold the bootstrap skill is told to
/// read in step 3. Root-level exclusions are the caller's business now; see
/// [`copy_dir_excluding_top`].
const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "dist",
    ".venv",
    "venv",
    "__pycache__",
    ".pnpm-store",
];

const EXCLUDE_FILES: &[&str] = &[".mise.local.toml", ".DS_Store"];

/// The Cache Directory Tagging Specification's marker file, and the signature its first
/// line must carry. <https://bford.info/cachedir/>
///
/// Cargo writes one into every target directory it creates. The tag exists so that copy and
/// backup tools skip the directory, which makes honouring it exactly the right instrument
/// here — and a general one: it costs one read per directory and has no name to guess wrong.
const CACHEDIR_TAG: &str = "CACHEDIR.TAG";
const CACHEDIR_SIGNATURE: &str = "Signature: 8a477f597d28d172789f06886806bc55";

pub(crate) fn excluded_dir(name: &str) -> bool {
    EXCLUDE_DIRS.contains(&name) || name.ends_with(".lance") || name.ends_with(".egg-info")
}

/// True when the directory declares itself a cache, per the tagging specification.
///
/// The name list above is the fast path and the fallback — a directory whose tag is missing
/// is still excluded if it is named `target`. This is the rule for everything else, and it
/// exists because name-matching was demonstrably the wrong instrument: `EXCLUDE_DIRS` held
/// the literal `"target"`, so `yidam clone` skipped `target/` and copied **282M** of
/// `target-pin/` — a `cargo --target-dir` output — into every derived repository.
///
/// `6aaf18e` had already fixed this exact miss one file over, in `.gitignore`: "`target/`
/// and not `target-*/`, so a stray `--target-dir` run left 567 build artifacts that
/// `git add -A` was happy to stage." Adding `target-*` here would fix the case that was
/// reported and wait for `build-out/`. The tag is what a build directory says about itself.
///
/// The signature is checked, not just the filename: a corpus is free to have a node called
/// `CACHEDIR.TAG`, and a directory is only a cache if it says so in the spec's words.
fn is_cache_dir(path: &Path) -> bool {
    let Ok(tag) = std::fs::read_to_string(path.join(CACHEDIR_TAG)) else {
        return false;
    };
    tag.lines().next().is_some_and(|l| l == CACHEDIR_SIGNATURE)
}

pub(crate) fn excluded_file(name: &str) -> bool {
    EXCLUDE_FILES.contains(&name)
        || name.ends_with(".pyc")
        || name.ends_with(".pyo")
        || name.ends_with(".tsbuildinfo")
}

pub(crate) fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
    copy_dir_excluding_top(src, dst, &[])
}

/// Copy `src` to `dst`, additionally skipping `top_level` entries directly under `src`.
///
/// The extra exclusions apply at the first level only. `yidam clone` uses it to leave
/// yidam's own `docs/` behind without also dropping `sadhana/docs/` — the two differ in
/// depth, not in name.
pub(crate) fn copy_dir_excluding_top(src: &Path, dst: &Path, top_level: &[&str]) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src).with_context(|| format!("reading {}", src.display()))? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_symlink() {
            continue;
        }
        if top_level.contains(&name_str.as_ref()) {
            continue;
        }
        if src_path.is_dir() {
            if excluded_dir(&name_str) || is_cache_dir(&src_path) {
                continue;
            }
            copy_dir(&src_path, &dst_path)?;
        } else {
            if excluded_file(&name_str) {
                continue;
            }
            std::fs::copy(&src_path, &dst_path)
                .with_context(|| format!("copying {}", src_path.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn top_level_exclusion_spares_the_same_name_deeper_down() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("docs")).unwrap();
        std::fs::create_dir_all(src.join("sadhana/docs")).unwrap();
        std::fs::write(src.join("docs/rfc.md"), "yidam's own").unwrap();
        std::fs::write(src.join("sadhana/docs/README.md"), "the scaffold").unwrap();

        copy_dir_excluding_top(&src, &dst, &["docs"]).unwrap();

        assert!(!dst.join("docs").exists(), "root docs/ should be skipped");
        assert!(
            dst.join("sadhana/docs/README.md").exists(),
            "sadhana/docs/ is what step 3 of the bootstrap skill reads — it must ship"
        );
    }

    /// The reported case: a `cargo --target-dir` output directory, which the name list
    /// misses and the tag catches. Deleting the `is_cache_dir` call fails this and passes
    /// [`build_output_is_dropped_at_every_depth`] — the name-only rule looked correct
    /// against a fixture that only ever used the blessed name.
    #[test]
    fn a_tagged_cache_is_dropped_whatever_it_is_called() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("cli/target-pin/debug")).unwrap();
        std::fs::write(
            src.join("cli/target-pin/CACHEDIR.TAG"),
            "Signature: 8a477f597d28d172789f06886806bc55\n             # This file is a cache directory tag created by cargo.\n",
        )
        .unwrap();
        std::fs::write(src.join("cli/target-pin/debug/blob"), "282 megabytes of it").unwrap();
        std::fs::write(src.join("cli/lib.rs"), "code").unwrap();

        copy_dir(&src, &dst).unwrap();

        assert!(
            !dst.join("cli/target-pin").exists(),
            "a directory tagged as a cache was copied anyway"
        );
        assert!(
            dst.join("cli/lib.rs").exists(),
            "and the source came with it"
        );
    }

    /// The tag is a signature, not a filename. A corpus may hold a node called
    /// `CACHEDIR.TAG` — deciding on the name alone would silently drop the directory
    /// holding it, which is a worse failure than the one being fixed: it loses a node
    /// rather than copying junk.
    #[test]
    fn a_file_borrowing_the_name_is_not_a_cache() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("corpus/artifact")).unwrap();
        std::fs::write(
            src.join("corpus/artifact/CACHEDIR.TAG"),
            "class: artifact\nlabel: The CACHEDIR.TAG convention\n",
        )
        .unwrap();

        copy_dir(&src, &dst).unwrap();

        assert!(
            dst.join("corpus/artifact/CACHEDIR.TAG").exists(),
            "a node named after the tag is not a cache directory"
        );
    }

    #[test]
    fn build_output_is_dropped_at_every_depth() {
        let tmp = tempfile::TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");
        std::fs::create_dir_all(src.join("crates/a/target")).unwrap();
        std::fs::write(src.join("crates/a/target/blob"), "junk").unwrap();
        std::fs::write(src.join("crates/a/lib.rs"), "code").unwrap();

        copy_dir(&src, &dst).unwrap();

        assert!(!dst.join("crates/a/target").exists());
        assert!(dst.join("crates/a/lib.rs").exists());
    }
}
