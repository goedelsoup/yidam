/// Markdown frontmatter (---/--- block) for agents, skills, and catalog entries.
#[derive(serde::Deserialize, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Catalog entries only. What kind of source this is — paper, dataset, api, database.
    #[serde(default)]
    pub r#type: Option<String>,
    /// Catalog entries only. Whether the source has actually been retrieved.
    ///
    /// Absent means yes. `obtained: false` declares an entry registered ahead of the
    /// extraction that will use it, which is the honest reason for a source nothing cites
    /// yet — and it is checkable, because citing a source nobody has fetched is a defect
    /// either way (see `catalog-unobtained-but-cited`).
    #[serde(default)]
    pub obtained: Option<bool>,
    /// Catalog entries only. When the source was last actually fetched.
    ///
    /// `YYYY-MM-DD`. Optional, and its absence is not a defect: an entry that never said
    /// falls back to the date its file was last committed, which is a real answer and a
    /// weaker one. What the fallback cannot tell is a re-fetch from a typo fix, and it errs
    /// in the flattering direction — the record looks fresher than the source is — so the
    /// report always says which of the two it used.
    #[serde(default)]
    pub retrieved: Option<String>,
    /// Catalog entries only. How long this record may stand before it is worth looking at
    /// again, in days.
    ///
    /// **Days, and not commits.** Every other clock in this repository counts commits, for
    /// the argued reason that a corpus-state finding must be a function of `HEAD` rather
    /// than of when you ran the report. This one is different in kind: a statute or a gauge
    /// record does not become stale because you committed, it becomes stale because the
    /// world moved. The cost is real and accepted — this is the one report that answers
    /// differently tomorrow, which is exactly what a TTL is for.
    ///
    /// Per entry, because a gauge record and a statute do not age at the same rate. Absent,
    /// the corpus-wide default in `.yidam/config.toml` applies; absent both, the entry never
    /// expires.
    #[serde(default)]
    pub ttl_days: Option<u32>,
    /// Catalog entries only. Where the source can be reached.
    #[serde(default)]
    pub location: Option<Vec<CatalogLocation>>,
    /// Catalog entries only. Corpus nodes known to draw on this source.
    ///
    /// Hand-maintained, and therefore able to drift from the edges — which are
    /// authoritative. Both are kept so the disagreement is visible rather than averaged
    /// away; see `catalog-used-by-drift`.
    #[serde(default, rename = "used-by")]
    pub used_by: Option<Vec<String>>,
}

/// One typed place a catalog source can be reached.
#[derive(serde::Deserialize, Default, Clone)]
pub struct CatalogLocation {
    /// `url`, `url_template`, `address`, or `file`. The type decides how a reader (or the
    /// web export) should treat the value, so a value contradicting its type renders wrong.
    pub kind: Option<String>,
    pub value: Option<String>,
    /// Distinguishes several locations on one entry. Optional when there is only one.
    #[serde(default)]
    pub description: Option<String>,
}

/// The location kinds a catalog entry may declare.
pub const CATALOG_LOCATION_KINDS: &[&str] = &["url", "url_template", "address", "file"];

pub fn parse_frontmatter(text: &str) -> Frontmatter {
    let body = text.trim_start();
    let Some(rest) = body.strip_prefix("---\n") else {
        return Frontmatter::default();
    };
    let Some(end) = rest.find("\n---") else {
        return Frontmatter::default();
    };
    serde_yaml::from_str(&rest[..end]).unwrap_or_default()
}

/// The prose beneath a file's YAML frontmatter, or the whole text when there is none.
///
/// [`parse_frontmatter`] reads the header and discards this; for a catalog entry the body
/// is the substance — what the source holds, what was retrieved, what it does not answer.
pub fn frontmatter_body(text: &str) -> &str {
    let trimmed = text.trim_start();
    let Some(rest) = trimmed.strip_prefix("---\n") else {
        return trimmed;
    };
    match rest.find("\n---") {
        // 4 = the newline plus `---`; skip to the end of that line.
        Some(end) => {
            let after = &rest[end + 4..];
            after.strip_prefix('\n').unwrap_or(after)
        }
        None => trimmed,
    }
}

/// A corpus instance object (.yml file inside a class subdirectory).
#[derive(serde::Deserialize, Default)]
pub struct CorpusInstance {
    pub class: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    /// The typed fields the class declares, as the instance actually wrote them.
    ///
    /// Held untyped because the *ontology* is the type: `properties` is a bag whose keys
    /// and value shapes are declared per class in `<class>.ont.yml`, so a struct here
    /// would be a second, weaker declaration of the same thing. The checks in
    /// `lint::checks` read it against the class; nothing else may assume a shape.
    #[serde(default)]
    pub properties: Option<serde_yaml::Mapping>,
    pub links: Option<Vec<CorpusLink>>,
    /// What this node leaned on in a corpus this repository does not own (RFC-0019).
    ///
    /// **Beside `links:` and never inside it.** A foreign node may be read and may not be an
    /// edge target — `prelude/guidelines/agent-conduct.md` states the rule and the reason —
    /// so a citation is a different object from a relationship. Putting it in `links:` would
    /// have put it in the list `instance_links` reads, and every traversal in the system
    /// would then have to learn to skip it; one that forgot would cross a corpus boundary
    /// silently.
    #[serde(default)]
    pub cites: Option<Vec<ExternalCitation>>,
}

/// One claim resting on a node in an installed dependency.
///
/// `span` is the field the design turns on. A node reference alone rots invisibly — the node
/// keeps its name while its content is rewritten, and the citation still resolves — and a
/// span cannot: it either still appears or it does not. It is also the only check available
/// that does not need the producer's apparatus, which is exactly the apparatus a bundle does
/// not carry: no sangha, no elector register, no resolution history.
#[derive(serde::Deserialize, Default, Debug, Clone)]
pub struct ExternalCitation {
    /// The dependency, as `.yidam/tonpa.toml` names it.
    pub package: Option<String>,
    /// `<class>/<name>` inside that corpus. Unqualified — `package` already says whose.
    pub node: Option<String>,
    /// The `manifest.yml` commit this was read at. Absent for a path dependency, which
    /// cannot be pinned.
    pub commit: Option<String>,
    /// The producer's standing, **as observed at that pin**. Recorded, never transferred:
    /// a foreign tag is the producer's tag, and across this boundary the rule that a derived
    /// assertion travels only as far as the weakest claim beneath it cannot be computed.
    pub tag: Option<String>,
    /// Verbatim text from the cited node.
    pub span: Option<String>,
}

#[derive(serde::Deserialize, Default)]
pub struct CorpusLink {
    pub target: Option<String>,
    pub relationship: Option<String>,
}

/// A decision record (.yml file in .yidam/decisions/).
#[derive(serde::Deserialize, Default)]
pub struct Decision {
    pub id: Option<String>,
    pub summary: Option<String>,
}

/// A seed file from samudaya/ (markdown with kind/constitutional frontmatter).
#[derive(serde::Deserialize, Default)]
pub struct SamudayaSeed {
    pub kind: Option<String>,
    pub constitutional: Option<bool>,
}

pub fn parse_samudaya_seed(text: &str) -> SamudayaSeed {
    let body = text.trim_start();
    let Some(rest) = body.strip_prefix("---\n") else {
        return SamudayaSeed::default();
    };
    let Some(end) = rest.find("\n---") else {
        return SamudayaSeed::default();
    };
    serde_yaml::from_str(&rest[..end]).unwrap_or_default()
}

/// A crate or package manifest reduced to what an index row carries.
///
/// `description` is optional because a manifest may honestly not have one; the index
/// renders that absence as an em dash. What it must never render as an em dash is a
/// description the manifest *does* declare — see [`parse_cargo_manifest`].
pub struct ManifestEntry {
    pub name: String,
    pub description: Option<String>,
}

/// `[workspace.package]` — the defaults a member claims with `<key>.workspace = true`.
#[derive(Default)]
pub struct WorkspacePackage {
    pub description: Option<String>,
}

/// Reads `[workspace.package]` from a manifest that declares a workspace.
///
/// `None` when there is no `[workspace]` table: only a workspace root can be inherited
/// from, and a member that reads its own `[package]` as the source of the defaults would
/// resolve every inherited key to itself.
pub fn parse_workspace_package(text: &str) -> Option<WorkspacePackage> {
    let value: toml::Value = toml::from_str(text).ok()?;
    let workspace = value.get("workspace")?;
    Some(WorkspacePackage {
        description: workspace
            .get("package")
            .and_then(|package| package.get("description"))
            .and_then(toml::Value::as_str)
            .map(str::to_string)
            .filter(|d| !d.is_empty()),
    })
}

/// Reads a Cargo manifest's `[package]` table.
///
/// `None` when the manifest declares no package. A virtual manifest — `[workspace]` and its
/// members, no package of its own — is not a crate, and the members it names are found by
/// the walk on their own; rendering one as a row produced a link whose text and target were
/// both an em dash.
///
/// Parsed as TOML rather than scanned line by line, which is the other half of the same
/// report. The scan matched the literal prefix `description = `, so an aligned manifest
/// (`description  = "..."`, two spaces, valid TOML and ordinary formatting) fell through to
/// the em dash — and it read the first matching line in the file regardless of which table
/// held it, so a workspace root's `[workspace.package]` description answered for the
/// package below it. Both failures regenerate cleanly and pass `regen --check`.
pub fn parse_cargo_manifest(
    text: &str,
    workspace: Option<&WorkspacePackage>,
) -> Option<ManifestEntry> {
    let value: toml::Value = toml::from_str(text).ok()?;
    let package = value.get("package")?;
    let name = non_empty(package.get("name").and_then(toml::Value::as_str))?;
    let description = inherited_str(
        package.get("description"),
        workspace.and_then(|w| w.description.as_deref()),
    );
    Some(ManifestEntry { name, description })
}

/// Reads a `pyproject.toml` — PEP 621 `[project]`, or Poetry's `[tool.poetry]` for a
/// project that predates it.
pub fn parse_pyproject_manifest(text: &str) -> Option<ManifestEntry> {
    let value: toml::Value = toml::from_str(text).ok()?;
    let project = value
        .get("project")
        .or_else(|| value.get("tool").and_then(|tool| tool.get("poetry")))?;
    let name = non_empty(project.get("name").and_then(toml::Value::as_str))?;
    Some(ManifestEntry {
        name,
        description: non_empty(project.get("description").and_then(toml::Value::as_str)),
    })
}

/// Reads a `package.json`.
///
/// `None` for a workspace root: a manifest whose job is to declare `workspaces` is the npm
/// counterpart of a virtual Cargo manifest, and its members are listed on their own.
pub fn parse_npm_manifest(text: &str) -> Option<ManifestEntry> {
    let value: serde_json::Value = serde_json::from_str(text).ok()?;
    if value.get("workspaces").is_some() {
        return None;
    }
    let name = non_empty(value.get("name").and_then(serde_json::Value::as_str))?;
    Some(ManifestEntry {
        name,
        description: non_empty(value.get("description").and_then(serde_json::Value::as_str)),
    })
}

fn non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
}

/// A `[package]` field that is either a string or the inheritance marker
/// `<key>.workspace = true`, resolved against the workspace root's value.
fn inherited_str(field: Option<&toml::Value>, inherited: Option<&str>) -> Option<String> {
    match field {
        Some(toml::Value::String(s)) => non_empty(Some(s)),
        Some(value) if value.get("workspace").and_then(toml::Value::as_bool) == Some(true) => {
            non_empty(inherited)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reported defect. `description  = "..."` — two spaces, valid TOML, and ordinary
    /// formatting for an aligned manifest — read as no description at all, and the index
    /// rendered an em dash. The line scan matched the literal prefix `description = `.
    #[test]
    fn an_aligned_manifest_still_has_a_description() {
        let toml = "[package]\nname         = \"retrieval\"\ndescription  = \"Office retrieval\"\n";
        let entry = parse_cargo_manifest(toml, None).expect("an aligned [package] is a package");
        assert_eq!(entry.name, "retrieval");
        assert_eq!(entry.description.as_deref(), Some("Office retrieval"));
    }

    /// The other reported defect. A virtual manifest is not a crate, and listing one
    /// produced a row whose link text and target were both an em dash.
    #[test]
    fn a_virtual_workspace_manifest_is_not_a_crate() {
        let toml = "[workspace]\nmembers = [\"retrieval\"]\nresolver = \"2\"\n";
        assert!(parse_cargo_manifest(toml, None).is_none());
    }

    /// A root crate that is also a workspace root is still a crate — and the description
    /// that answers is its own. The line scan returned the first `description = ` line in
    /// the file whatever table held it, so `[workspace.package]` answered for the package.
    #[test]
    fn a_package_beside_a_workspace_answers_for_itself() {
        let toml = "[workspace]\nmembers = [\"member\"]\n\n[workspace.package]\ndescription = \"the workspace\"\n\n[package]\nname = \"root-crate\"\ndescription = \"the crate\"\n";
        let entry = parse_cargo_manifest(toml, None).expect("a package beside a workspace");
        assert_eq!(entry.description.as_deref(), Some("the crate"));
    }

    /// Inheritance is the normal arrangement in the workspace the conventions describe, and
    /// an unresolved `description.workspace = true` is the same silent em dash by a
    /// different route.
    #[test]
    fn an_inherited_description_resolves_against_the_workspace() {
        let root = "[workspace]\nmembers = [\"retrieval\"]\n\n[workspace.package]\ndescription = \"The workspace description\"\n";
        let workspace = parse_workspace_package(root).expect("a [workspace] declares one");
        let member = "[package]\nname = \"retrieval\"\ndescription.workspace = true\nedition.workspace = true\n";
        let entry = parse_cargo_manifest(member, Some(&workspace)).expect("a member is a crate");
        assert_eq!(
            entry.description.as_deref(),
            Some("The workspace description")
        );
    }

    /// A member cannot be its own inheritance source, or every inherited key resolves to
    /// the package that asked.
    #[test]
    fn only_a_workspace_root_supplies_defaults() {
        assert!(parse_workspace_package("[package]\nname = \"retrieval\"\n").is_none());
    }

    /// Nothing to inherit from is still an honest absence, not a crash or a `true`.
    #[test]
    fn an_inherited_description_with_no_workspace_is_absent() {
        let member = "[package]\nname = \"retrieval\"\ndescription.workspace = true\n";
        let entry = parse_cargo_manifest(member, None).expect("still a package");
        assert_eq!(entry.description, None);
    }

    #[test]
    fn an_npm_workspace_root_is_not_a_package() {
        let json = r#"{"name": "root", "private": true, "workspaces": ["a", "b"]}"#;
        assert!(parse_npm_manifest(json).is_none());
    }

    #[test]
    fn an_npm_package_reads_name_and_description() {
        let json = r#"{"name": "@corpus/connector", "description": "A connector package"}"#;
        let entry = parse_npm_manifest(json).expect("a package.json with a name");
        assert_eq!(entry.name, "@corpus/connector");
        assert_eq!(entry.description.as_deref(), Some("A connector package"));
    }

    #[test]
    fn a_pyproject_reads_pep_621_then_poetry() {
        let pep = "[project]\nname = \"calculator\"\ndescription = \"A calculator package\"\n";
        let entry = parse_pyproject_manifest(pep).expect("a [project] table");
        assert_eq!(entry.name, "calculator");
        assert_eq!(entry.description.as_deref(), Some("A calculator package"));

        let poetry =
            "[tool.poetry]\nname = \"calculator\"\ndescription = \"A calculator package\"\n";
        let entry = parse_pyproject_manifest(poetry).expect("a [tool.poetry] table");
        assert_eq!(entry.name, "calculator");
        assert_eq!(entry.description.as_deref(), Some("A calculator package"));
    }

    /// A manifest that does not parse is not half-read: the old scan would still find a
    /// `name = ` line in a file cargo itself rejects.
    #[test]
    fn a_malformed_manifest_yields_nothing() {
        assert!(parse_cargo_manifest("[package\nname = \"broken\"\n", None).is_none());
    }

    #[test]
    fn corpus_instance_empty_links_is_orphan() {
        let yaml = "class: reach\nlabel: Test Reach\nlinks: []\n";
        let inst: CorpusInstance = serde_yaml::from_str(yaml).unwrap();
        assert!(inst.links.unwrap_or_default().is_empty());
    }

    #[test]
    fn frontmatter_parses_name_and_description() {
        let text = "---\nname: my-skill\ndescription: Does something.\n---\n# Body\n";
        let fm = parse_frontmatter(text);
        assert_eq!(fm.name.as_deref(), Some("my-skill"));
        assert_eq!(fm.description.as_deref(), Some("Does something."));
    }
}
