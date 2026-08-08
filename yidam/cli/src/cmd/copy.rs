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

pub(crate) fn excluded_dir(name: &str) -> bool {
    EXCLUDE_DIRS.contains(&name) || name.ends_with(".lance") || name.ends_with(".egg-info")
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
            if excluded_dir(&name_str) {
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
