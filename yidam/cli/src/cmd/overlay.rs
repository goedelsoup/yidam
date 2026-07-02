use anyhow::{bail, Context, Result};
use std::path::Path;

use crate::paths::repo_root;

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

fn excluded_dir(name: &str) -> bool {
    EXCLUDE_DIRS.contains(&name) || name.ends_with(".lance") || name.ends_with(".egg-info")
}

fn excluded_file(name: &str) -> bool {
    EXCLUDE_FILES.contains(&name)
        || name.ends_with(".pyc")
        || name.ends_with(".pyo")
        || name.ends_with(".tsbuildinfo")
}

fn copy_dir(src: &Path, dst: &Path) -> Result<()> {
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

fn count_seeds(samudaya: &Path) -> usize {
    let examples = samudaya.join("examples");
    walkdir::WalkDir::new(samudaya)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            let path = e.path();
            if path.starts_with(&examples) {
                return false;
            }
            if path.file_name().map(|n| n == "README.md").unwrap_or(false) {
                return false;
            }
            matches!(
                path.extension().and_then(|e| e.to_str()).unwrap_or(""),
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

    // yidam/ — prelude, skills, harness, CLI tools
    let yidam_src = root.join("yidam");
    if yidam_src.is_dir() {
        copy_dir(&yidam_src, &target.join("yidam"))?;
        println!("yidam/ → {}/yidam/", target.display());
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

    // sadhana/ — scaffold templates
    let sadhana_src = root.join("sadhana");
    if sadhana_src.is_dir() {
        copy_dir(&sadhana_src, &target.join("sadhana"))?;
        println!("sadhana/ → {}/sadhana/", target.display());
    }

    // samudaya/ — only if seeds beyond README.md and examples/ are present
    let samudaya_src = root.join("samudaya");
    if samudaya_src.is_dir() {
        let seed_count = count_seeds(&samudaya_src);
        if seed_count > 0 {
            copy_dir(&samudaya_src, &target.join("samudaya"))?;
            println!(
                "samudaya/ ({seed_count} seed file(s)) → {}/samudaya/",
                target.display()
            );
        } else {
            println!("samudaya/ has no seeds — skipping");
        }
    }

    // mise.yidam.toml — yidam CLI task definitions
    let mise_src = root.join("mise.yidam.toml");
    if mise_src.exists() {
        std::fs::copy(&mise_src, &target.join("mise.yidam.toml"))
            .context("copying mise.yidam.toml")?;
        println!("mise.yidam.toml → {}/mise.yidam.toml", target.display());
    }

    println!();
    println!("yidam infrastructure overlaid on {}", target.display());
    println!();
    println!("Next steps:");
    println!("  1. Add to your mise.toml:  extends = [\"mise.yidam.toml\"]");
    println!("  2. Open {} in your IDE", target.display());
    println!("  3. The agent will read BOOTSTRAP.md and enter existing-repo mode");

    if backfill {
        println!();
        super::backfill::backfill_history(target, backfill_ref)?;
    }

    Ok(())
}
