//! The domain-computer indexes — what `crates/` and `packages/` hold, and what runs.
//!
//! # A directory is not a capability
//!
//! These commands reported on **directories**, so a crate implementing a connector and a crate
//! implementing nothing produced the same row (#472). The distinction is not cosmetic in a
//! repository whose whole subject is the domain computer: `sadhana/crates/README.md` describes
//! connectors, calculators and feature engineering, and an index that cannot say which of them
//! exist is describing the scaffold rather than the thing it scaffolds.
//!
//! So a row now names the capability that runs it, read from `.yidam/capabilities.toml` — the
//! file the executor reads, rather than a second register of what exists. A crate nothing
//! declares gets an em dash, which is the finding and not an omission.
//!
//! **The correspondence is declared, never inferred.** A capability claims a crate by naming
//! it in `run`: the package's name as a token, which is `cargo run -p <name>`'s own spelling,
//! or a path at or under the crate's directory. Both are literal facts about the declaration.
//! Nothing here guesses from a crate's name, its dependencies or its shape — #460's failure
//! table has that as *"a gather aligns schemas by name"*, and the answer there is the answer
//! here: correspondence is declared.

use anyhow::Result;
use std::path::{Path, PathBuf};

use crate::cmd::run::manifest::{Manifest, MANIFEST};
use crate::parse::{
    parse_cargo_manifest, parse_npm_manifest, parse_pyproject_manifest, parse_workspace_package,
    ManifestEntry, WorkspacePackage,
};
use crate::regen::update_file_regen;

pub fn crates_index(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let crates_dir = root.join("crates");
    let workspace = workspace_package(&root, &crates_dir);
    let declared = declared_capabilities(&root)?;

    let entries: Vec<Row> = manifests(&crates_dir, &["Cargo.toml"])
        .into_iter()
        .filter_map(|path| {
            let text = std::fs::read_to_string(&path).ok()?;
            let entry = parse_cargo_manifest(&text, workspace.as_ref())?;
            let capability = claimed_by(&declared, &root, &path, &entry.name);
            Some(Row {
                target: link(&crates_dir, &path),
                entry,
                capability,
            })
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

pub fn packages_index(root: Option<&std::path::Path>) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    let packages_dir = root.join("packages");
    let workspace = workspace_package(&root, &packages_dir);
    let declared = declared_capabilities(&root)?;

    let entries: Vec<Row> = manifests(
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
        let capability = claimed_by(&declared, &root, &path, &entry.name);
        Some(Row {
            target: link(&packages_dir, &path),
            entry,
            capability,
        })
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

/// One row: where it links, what its manifest says, and what declares it.
struct Row {
    target: String,
    entry: ManifestEntry,
    /// Every capability whose `run` names this package, as `kind \`name\``. Empty where none
    /// does, which is the row the index existed to be unable to distinguish.
    capability: Vec<String>,
}

/// The capabilities this repository declares, or none where it declares no manifest.
///
/// A manifest that is present and does not parse is an error rather than an empty set. The
/// column's claim is *nothing declares this crate*, and rendering that from a file this
/// command could not read would be asserting a finding out of an unread file — the flattering
/// direction to be wrong in, and the one a reader has no way to notice.
fn declared_capabilities(root: &Path) -> Result<Manifest> {
    match root.join(MANIFEST).exists() {
        true => Manifest::load(root),
        false => Ok(Manifest::default()),
    }
}

/// The capabilities that name this package, by the two literal forms a declaration takes.
///
/// `dir` is the package's own directory, repository-relative, so a path token is matched
/// against where the package actually is rather than against its name a second time.
fn claimed_by(m: &Manifest, root: &Path, manifest_path: &Path, name: &str) -> Vec<String> {
    let dir = manifest_path
        .parent()
        .and_then(|p| p.strip_prefix(root).ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    m.capability
        .iter()
        .filter(|(_, cap)| {
            cap.run.iter().any(|arg| {
                arg == name
                    || (!dir.is_empty() && (arg == &dir || arg.starts_with(&format!("{dir}/"))))
            })
        })
        .map(|(step, cap)| format!("{} `{step}`", cap.kind.as_str()))
        .collect()
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
fn render(entries: &[Row], header: &str, empty: &str) -> String {
    if entries.is_empty() {
        return empty.to_string();
    }
    let mut rows = vec![
        format!("| {header} | Capability | Description |"),
        "|---|---|---|".to_string(),
    ];
    for row in entries {
        let description = row.entry.description.as_deref().unwrap_or("—");
        // Two em dashes in one row mean two different absences, which the headers disambiguate
        // and nothing else has to: a manifest may honestly have no description, and a crate
        // that nothing declares is the finding this column was added for.
        let capability = match row.capability.is_empty() {
            true => "—".to_string(),
            false => row.capability.join(", "),
        };
        rows.push(format!(
            "| [{}]({}) | {capability} | {description} |",
            row.entry.name, row.target
        ));
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kuten::Registers;

    fn entry(name: &str, description: Option<&str>) -> ManifestEntry {
        ManifestEntry {
            name: name.to_string(),
            description: description.map(str::to_string),
        }
    }

    fn row(name: &str, description: Option<&str>, capability: &[&str]) -> Row {
        Row {
            target: format!("{name}/"),
            entry: entry(name, description),
            capability: capability.iter().map(|c| c.to_string()).collect(),
        }
    }

    #[test]
    fn empty_index_says_so() {
        assert_eq!(render(&[], "Crate", "_No crates yet._"), "_No crates yet._");
    }

    #[test]
    fn a_row_links_the_directory_and_carries_the_description() {
        let entries = vec![row(
            "retrieval",
            Some("A connector crate"),
            &["connector `gather-gages`"],
        )];
        assert_eq!(
            render(&entries, "Crate", "_No crates yet._"),
            "| Crate | Capability | Description |\n|---|---|---|\n\
             | [retrieval](retrieval/) | connector `gather-gages` | A connector crate |"
        );
    }

    /// A manifest may honestly have no description; that absence is the em dash's only job in
    /// the description column.
    #[test]
    fn a_missing_description_is_an_em_dash() {
        let entries = vec![row("retrieval", None, &["calculator `x`"])];
        assert!(render(&entries, "Crate", "—")
            .ends_with("| [retrieval](retrieval/) | calculator `x` | — |"));
    }

    /// #472's finding, at the render: the two rows the index could not tell apart.
    ///
    /// The one that implements something names it and its kind; the one that implements
    /// nothing says so. Asserted as a *difference* rather than as two literals, because the
    /// claim is that the rows are distinguishable and a pair of expected strings would pass
    /// just as well if both columns rendered the same constant.
    #[test]
    fn a_crate_that_implements_a_capability_does_not_render_as_one_that_implements_nothing() {
        let implements = render(
            &[row("retrieval", Some("d"), &["connector `gather-gages`"])],
            "Crate",
            "—",
        );
        let scaffold = render(&[row("retrieval", Some("d"), &[])], "Crate", "—");
        assert_ne!(
            implements, scaffold,
            "a crate implementing a connector and a crate implementing nothing render the \
             same row, which is the defect #472 is about"
        );
        assert!(implements.contains("gather-gages"), "{implements}");
        assert!(
            implements.contains("connector"),
            "the row does not say which kind of capability it is: {implements}"
        );
    }

    /// A crate two capabilities name is named by both.
    #[test]
    fn a_crate_several_capabilities_name_lists_them_all() {
        let out = render(
            &[row(
                "compute",
                None,
                &["calculator `low-flow`", "calculator `travel-tier`"],
            )],
            "Crate",
            "—",
        );
        assert!(
            out.contains("calculator `low-flow`, calculator `travel-tier`"),
            "{out}"
        );
    }

    // ── which declarations claim a package ────────────────────────────────────

    fn manifest(run: &str) -> Manifest {
        let text = format!(
            "[capability.gather]\nkind   = \"connector\"\nrun    = {run}\n\
             reads  = [\".yidam/corpus/**\"]\nwrites = [\".yidam/computed/**\"]\n\
             verb   = \"refresh\"\n"
        );
        Manifest::parse(&text, &Registers::corpus_only()).expect(&text)
    }

    fn claims(run: &str, dir: &str, name: &str) -> Vec<String> {
        claimed_by(
            &manifest(run),
            Path::new("/repo"),
            &Path::new("/repo").join(dir).join("Cargo.toml"),
            name,
        )
    }

    /// `cargo run -p <name>` is RFC-0026 §4's own example, and the token is the package name.
    #[test]
    fn a_declaration_naming_the_package_claims_it() {
        assert_eq!(
            claims(
                "[\"cargo\", \"run\", \"-p\", \"lowflow\", \"--\"]",
                "crates/lowflow",
                "lowflow"
            ),
            ["connector `gather`"]
        );
    }

    /// A path at or under the crate's directory claims it, which is the shell form.
    #[test]
    fn a_declaration_naming_a_path_in_the_crate_claims_it() {
        assert_eq!(
            claims(
                "[\"sh\", \"crates/lowflow/run.sh\"]",
                "crates/lowflow",
                "lowflow"
            ),
            ["connector `gather`"]
        );
    }

    /// Nothing is claimed by resemblance. The near-miss is the one that matters: a sibling
    /// directory whose name is a prefix of this one's.
    #[test]
    fn a_declaration_naming_something_else_claims_nothing() {
        assert!(claims(
            "[\"sh\", \"crates/lowflow-legacy/run.sh\"]",
            "crates/lowflow",
            "lowflow"
        )
        .is_empty());
        assert!(claims(
            "[\"cargo\", \"run\", \"-p\", \"other\"]",
            "crates/lowflow",
            "lowflow"
        )
        .is_empty());
        assert!(claims("[\"true\"]", "crates/lowflow", "lowflow").is_empty());
    }

    /// A repository declaring no manifest claims nothing, rather than failing.
    #[test]
    fn a_repository_with_no_manifest_claims_nothing() {
        let m = Manifest::default();
        assert!(claimed_by(
            &m,
            Path::new("/repo"),
            Path::new("/repo/crates/lowflow/Cargo.toml"),
            "lowflow"
        )
        .is_empty());
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
