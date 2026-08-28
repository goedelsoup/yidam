use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::parse::{
    parse_cargo_manifest, parse_npm_manifest, parse_pyproject_manifest, parse_workspace_package,
    ManifestEntry, WorkspacePackage,
};
use crate::paths::repo_root;
use crate::regen::update_file_regen;

pub fn crates_index() -> Result<()> {
    let root = repo_root()?;
    let crates_dir = root.join("crates");
    let workspace = workspace_package(&root, &crates_dir);

    let entries: Vec<(String, ManifestEntry)> = manifests(&crates_dir, &["Cargo.toml"])
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let entry = parse_cargo_manifest(&text, workspace.as_ref())?;
            Some((link(&crates_dir, &path), entry))
        })
        .collect();

    let content = render(&entries, "Crate", "_No crates yet._");
    crate::regen::emit(&content);
    update_file_regen(
        &crates_dir.join("README.md"),
        "yidam crates-index",
        &content,
    )
}

pub fn packages_index() -> Result<()> {
    let root = repo_root()?;
    let packages_dir = root.join("packages");
    let workspace = workspace_package(&root, &packages_dir);

    let entries: Vec<(String, ManifestEntry)> = manifests(
        &packages_dir,
        &["Cargo.toml", "package.json", "pyproject.toml"],
    )
    .into_iter()
    .filter_map(|path| {
        let text = std::fs::read_to_string(&path).ok()?;
        let entry = match path.file_name()?.to_string_lossy().as_ref() {
            "package.json" => parse_npm_manifest(&text)?,
            "pyproject.toml" => parse_pyproject_manifest(&text)?,
            _ => parse_cargo_manifest(&text, workspace.as_ref())?,
        };
        Some((link(&packages_dir, &path), entry))
    })
    .collect();

    let content = render(&entries, "Package", "_No packages yet._");
    crate::regen::emit(&content);
    update_file_regen(
        &packages_dir.join("README.md"),
        "yidam packages-index",
        &content,
    )
}

/// Every manifest the index directory holds, one level down at most.
fn manifests(dir: &Path, names: &[&str]) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = if dir.exists() {
        walkdir::WalkDir::new(dir)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.depth() > 0 && names.contains(&e.file_name().to_string_lossy().as_ref()))
            .map(|e| e.path().to_owned())
            .collect()
    } else {
        vec![]
    };
    found.sort();
    found
}

/// The `[workspace.package]` defaults a manifest under `dir` may inherit from.
///
/// A workspace root is either the virtual manifest in the index directory itself or the
/// repository's own root manifest — both are ordinary layouts, and a member that inherits
/// its description resolves against whichever one declares a workspace.
fn workspace_package(root: &Path, dir: &Path) -> Option<WorkspacePackage> {
    [dir.join("Cargo.toml"), root.join("Cargo.toml")]
        .iter()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .find_map(|text| parse_workspace_package(&text))
}

/// A manifest's directory, relative to the README that will hold the link.
///
/// Relative to the index directory rather than named after the manifest's own parent: a
/// manifest sitting directly in `crates/` took the link `crates/`, which from
/// `crates/README.md` points at a directory that does not exist.
fn link(dir: &Path, manifest: &Path) -> String {
    let Some(parent) = manifest.parent() else {
        return "./".to_string();
    };
    match parent.strip_prefix(dir) {
        Ok(rel) if rel.as_os_str().is_empty() => "./".to_string(),
        Ok(rel) => format!("{}/", rel.to_string_lossy().replace('\\', "/")),
        Err(_) => "./".to_string(),
    }
}

/// Renders the table.
///
/// Separate from the walk, and taking parsed entries rather than paths, so the row logic is
/// reachable without a repository on disk. Both defects this replaced — an aligned
/// `description` read as absent, and a virtual workspace manifest rendered as a crate —
/// lived in a `fn() -> Result<()>` that reads `repo_root()`, so nothing in the tree could
/// call it and no test did.
fn render(entries: &[(String, ManifestEntry)], header: &str, empty: &str) -> String {
    if entries.is_empty() {
        return empty.to_string();
    }
    let mut rows = vec![
        format!("| {header} | Description |"),
        "|---|---|".to_string(),
    ];
    for (target, entry) in entries {
        let description = entry.description.as_deref().unwrap_or("—");
        rows.push(format!("| [{}]({target}) | {description} |", entry.name));
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(name: &str, description: Option<&str>) -> ManifestEntry {
        ManifestEntry {
            name: name.to_string(),
            description: description.map(str::to_string),
        }
    }

    #[test]
    fn empty_index_says_so() {
        assert_eq!(render(&[], "Crate", "_No crates yet._"), "_No crates yet._");
    }

    #[test]
    fn a_row_links_the_directory_and_carries_the_description() {
        let entries = vec![(
            "retrieval/".to_string(),
            entry("retrieval", Some("A connector crate")),
        )];
        assert_eq!(
            render(&entries, "Crate", "_No crates yet._"),
            "| Crate | Description |\n|---|---|\n| [retrieval](retrieval/) | A connector crate |"
        );
    }

    /// A manifest may honestly have no description; that absence is the em dash's only job.
    #[test]
    fn a_missing_description_is_the_only_em_dash() {
        let entries = vec![("retrieval/".to_string(), entry("retrieval", None))];
        assert!(render(&entries, "Crate", "—").ends_with("| [retrieval](retrieval/) | — |"));
    }

    #[test]
    fn a_member_links_relative_to_the_index_directory() {
        assert_eq!(
            link(
                Path::new("/repo/crates"),
                Path::new("/repo/crates/retrieval/Cargo.toml")
            ),
            "retrieval/"
        );
    }

    /// The old link took the parent directory's *name*, so a manifest directly in `crates/`
    /// linked to `crates/` — a path that does not exist relative to `crates/README.md`.
    #[test]
    fn a_manifest_in_the_index_directory_links_to_itself() {
        assert_eq!(
            link(
                Path::new("/repo/crates"),
                Path::new("/repo/crates/Cargo.toml")
            ),
            "./"
        );
    }
}
