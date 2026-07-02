use anyhow::{Context, Result};
use std::path::Path;

const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    "docs",
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
