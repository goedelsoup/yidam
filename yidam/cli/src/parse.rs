/// Markdown frontmatter (---/--- block) for agents, skills, and catalog entries.
#[derive(serde::Deserialize, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
    /// Skills only. `built` for a procedure an agent can follow, `stub` for a placeholder that
    /// names one (#1063). Read by `skills-index`, which reports an absent value as `unstated`
    /// rather than as either answer.
    ///
    /// **A YAML value and not a `String`.** This struct is every frontmatter's, and a catalog
    /// entry in a derived repository may already carry a `status:` of its own. Typed as a
    /// string, `status: true` there would fail the whole header and take the entry's `obtained`
    /// and `ttl_days` with it — the #1056 failure, reached by adding a field.
    #[serde(default)]
    pub status: Option<serde_yaml::Value>,
    /// Catalog entries only. What form the source takes — one of [`CATALOG_TYPES`].
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
    /// Catalog entries only. What this entry has actually obtained, by content address.
    ///
    /// See [`CatalogArtifact`]. Absent on every entry written before RFC-0023, and adding it
    /// is a corpus deciding to record what it holds rather than a requirement arriving in a
    /// build — which is why the checks that read it fire only on entries that declare it.
    #[serde(default)]
    pub artifacts: Option<Vec<CatalogArtifact>>,
}

/// One artifact a catalog entry has actually obtained.
///
/// The record, not the bytes. RFC-0023's constraint is that *a vault stores bytes and git
/// stores the record of them*, and this is that record: the digest names the artifact, and
/// everything else here is what a reader needs in order to know what was named.
///
/// **This is what makes `obtained: true` demonstrable.** The flag means fetched, and until
/// now nothing anywhere held what was fetched — so an entry marked obtained and an entry
/// marked obtained falsely were the same observation. A digest is the difference.
///
/// Optional, and absent on every entry written before this existed. Adding it is a corpus
/// deciding to record what it has, not a requirement arriving in a build.
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, Debug)]
pub struct CatalogArtifact {
    /// The content address — 64 lowercase hex characters.
    ///
    /// `Option` so that a malformed record parses and is *reported* rather than making the
    /// whole entry unreadable. A catalog entry that fails to load takes its citations, its
    /// TTL and its `used-by` down with it, which is a large penalty for one bad field.
    pub sha256: Option<String>,
    /// Size in bytes, as fetched.
    pub bytes: Option<u64>,
    /// The media type, so a reader knows what the artifact is without fetching it.
    pub media_type: Option<String>,
    /// When these bytes were obtained. Distinct from the entry's own `retrieved:`, which is
    /// about the *record* — an entry can be revised without re-fetching anything.
    pub retrieved: Option<String>,
    /// Where it came from: an index into this entry's `location` list, or a literal URL.
    pub from: Option<ArtifactOrigin>,
    /// Which vault these bytes may be stored in. `none` means the local cache and nowhere
    /// else.
    ///
    /// **Routing, and not permission.** [`Self::redistributable`] is the other question and
    /// overrides this one; see its note.
    pub vault: Option<String>,
    /// Whether these bytes may leave this machine at all.
    ///
    /// A licensing fact about the *source*, which is why it is not folded into
    /// [`Self::vault`]. A route is edited casually — somebody reorganising storage moves a
    /// dozen entries between stores in an afternoon — and a licence is not something that
    /// edit is allowed to undo. Enforced where uploads happen, not here.
    pub redistributable: Option<bool>,
    /// A text reading of these bytes, for an artifact that is not itself text (#1172).
    ///
    /// A quotation of a PDF is compared with this rather than with the PDF. It is nested here
    /// rather than listed beside it because it is not a revision of the document: a quotation
    /// pins the PDF, and the reading follows. It goes where the PDF goes — `vault push` routes
    /// and licenses it by this record's `vault:` and `redistributable:`, because a reading
    /// carries the same words the licence is about.
    ///
    /// Written by entries before #1318, and read still. `catalog-extract` now records a PDF's
    /// reading under [`Self::readings`], which is this generalised.
    pub text: Option<TextReading>,
    /// Derived readings of these bytes (RFC-0048 §6, #1318): `text:` generalised.
    ///
    /// A PDF's text reading is one of these, and a source pack's `extract` transform writes
    /// another — the body of a Europe PMC article, the rows of a table. Each names what produced
    /// it in `by`, so a changed extractor or transform is a changed reading and not a silent one.
    /// It goes where the artifact goes, for the reason `text:` gives.
    #[serde(default)]
    pub readings: Option<Vec<ArtifactReading>>,
}

impl CatalogArtifact {
    /// Every reading this record carries: a `text:` first, as the `text/plain` reading its
    /// extractor took, then `readings:` in the order written.
    pub fn all_readings(&self) -> Vec<ArtifactReading> {
        let legacy = self.text.as_ref().map(|t| ArtifactReading {
            sha256: t.sha256.clone(),
            media_type: Some(TEXT_READING.to_string()),
            by: t.extractor.clone(),
        });
        legacy
            .into_iter()
            .chain(self.readings.iter().flatten().cloned())
            .collect()
    }

    /// The reading a quotation of these bytes is compared with: the first `text/plain` one,
    /// whichever key recorded it.
    pub fn text_reading(&self) -> Option<ArtifactReading> {
        self.all_readings()
            .into_iter()
            .find(ArtifactReading::is_text)
    }
}

/// The media type of a text reading.
pub const TEXT_READING: &str = "text/plain";

/// One derived reading of an artifact's bytes. See [`CatalogArtifact::readings`].
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct ArtifactReading {
    /// The reading's own content address, in the same cache as the artifact's.
    pub sha256: Option<String>,
    pub media_type: Option<String>,
    /// What produced it, so a person can reproduce it. A PDF's extractor is `<crate>
    /// <version>`; a pack's transform is `<pack>@<version>/<transform>@sha256:<hash>`. Never
    /// re-run to check it: lint compares against the stored bytes.
    pub by: Option<String>,
}

impl ArtifactReading {
    /// Whether this is a text reading, parameters such as `charset` aside.
    pub fn is_text(&self) -> bool {
        self.media_type
            .as_deref()
            .and_then(|m| m.split(';').next())
            .is_some_and(|m| m.trim().eq_ignore_ascii_case(TEXT_READING))
    }
}

/// A text reading as entries before #1318 recorded it. See `reading.rs`.
#[derive(serde::Deserialize, serde::Serialize, Default, Clone, Debug, PartialEq, Eq)]
pub struct TextReading {
    /// The reading's own content address, in the same cache as the artifact's.
    pub sha256: Option<String>,
    /// What produced it, as `<crate> <version>` — so a person can reproduce it. Never re-run
    /// to check it: lint compares against the stored bytes.
    pub extractor: Option<String>,
}

/// Where an obtained artifact came from.
///
/// `PartialEq` because "the same source, fetched again" is a question asked of this value and
/// nothing else: `catalog fetch` carries an operator's licensing and routing decisions onto a
/// new record only from a prior record naming the *same* origin. A hand-rolled comparison at
/// that call site would be one the type could not keep honest if a third variant arrived.
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug, PartialEq, Eq)]
#[serde(untagged)]
pub enum ArtifactOrigin {
    /// An index into the entry's own `location` list — `from: 0` is the first location.
    ///
    /// Preferred over repeating the URL, because a location that moves is then corrected in
    /// one place. `catalog-artifact-malformed` reports an index naming no location.
    Location(usize),
    /// A literal URL, for bytes obtained from somewhere this entry does not list.
    Url(String),
}

/// One typed place a catalog source can be reached.
#[derive(serde::Deserialize, Default, Clone)]
pub struct CatalogLocation {
    /// One of [`CATALOG_LOCATION_KINDS`]. The type decides how a reader (or the web export)
    /// should treat the value, so a value contradicting its type renders wrong.
    pub kind: Option<String>,
    pub value: Option<String>,
    /// Distinguishes several locations on one entry. Optional when there is only one.
    #[serde(default)]
    pub description: Option<String>,
}

/// The location kinds a catalog entry may declare.
///
/// `identifier` is RFC-0048's, and its value is `scheme:local-id` — `doi:10.1167/tvst.8.5.14`.
/// One kind for every scheme keeps this set closed: corpora wrote `kind: doi` and `kind: pmc`,
/// and a kind per scheme would grow the set with every source a corpus reads.
pub const CATALOG_LOCATION_KINDS: &[&str] =
    &["url", "url_template", "address", "file", "identifier"];

/// The kinds `yidam migrate locations` rewrites to `identifier`, the scheme each becomes.
///
/// Both were written by corpora before `identifier` existed, and both were refused by
/// `catalog-location-malformed`. The value moves under the scheme unchanged: `kind: doi,
/// value: X` is `kind: identifier, value: doi:X`.
pub const CATALOG_LOCATION_KINDS_RETIRED: &[&str] = &["doi", "pmc"];

/// What form a catalog source takes: the closed set its `type:` is one of.
///
/// `statute`, `report`, `standard` and `document` joined the first five in RFC-0048 §2.1,
/// because corpora wrote them 178 times with no word in the set for them. `primary` did not:
/// it says what standing a source has, not what form it takes, and a primary source can be a
/// statute, a dataset or a report. The schema and `catalog-type-unknown` both read this list.
pub const CATALOG_TYPES: &[&str] = &[
    "paper", "dataset", "api", "database", "statute", "report", "standard", "document", "other",
];

pub fn parse_frontmatter(text: &str) -> Frontmatter {
    parse_frontmatter_reporting(text).0
}

/// The same read, and why it came back empty when it did.
///
/// [`parse_frontmatter`] is this with the reason thrown away, and throwing it away is what
/// #1056 is about: a catalog entry with one unclosed quote in its header reached every check
/// as a `Frontmatter::default()`, which is not the empty file it looks like — it is `obtained`
/// absent (and so read as true), no `ttl_days`, no `used-by`, no `location` and no
/// `artifacts`. A corpus lost an entry's TTL for its whole history that way, and no gate could
/// say so. [`crate::cmd::lint::checks::malformed_yaml`] reports the reason this returns; it is
/// the same division of labour [`crate::corpus::parse_or_default`] makes for an instance and a
/// class, in the one other place bytes become a record.
///
/// **No frontmatter at all is not a failure.** A file that does not open with `---` has not
/// contradicted anything, and the surfaces reading a skill or a seed through this are entitled
/// to that reading. An *opened* header is a claim that a document follows, so a header that
/// never closes, or that closes over YAML the parser rejects, is reported.
pub fn parse_frontmatter_reporting(text: &str) -> (Frontmatter, Option<String>) {
    parse_header(text)
}

/// The YAML inside a document's frontmatter block.
///
/// `Ok(None)` for a document that opens no header, `Err` for one that opens a header and never
/// closes it. The distinction is [`parse_frontmatter_reporting`]'s and is explained there.
fn frontmatter_yaml(text: &str) -> Result<Option<&str>, &'static str> {
    let Some(rest) = text.trim_start().strip_prefix("---\n") else {
        return Ok(None);
    };
    match rest.find("\n---") {
        Some(end) => Ok(Some(&rest[..end])),
        None => Err("frontmatter opens with `---` and is never closed"),
    }
}

/// One document's frontmatter, read into whatever header type the caller has, and why it came
/// back empty when it did.
///
/// **One reader for every frontmatter in the repository.** There were two — this and
/// [`parse_samudaya_seed`], which found the fences itself and ended in `unwrap_or_default()` —
/// and the second was a reader whose outcome nothing anywhere read (#1081). Two copies of
/// "where does the header start and stop" is the shape that had `samudaya-audit` reporting
/// `missing 'kind:' field` about a seed whose header the parser had rejected: the wrong sentence
/// about the right file, which is worse than silence because it sends the author to the wrong
/// line.
fn parse_header<T>(text: &str) -> (T, Option<String>)
where
    T: Default + serde::de::DeserializeOwned,
{
    match frontmatter_yaml(text) {
        Ok(None) => (T::default(), None),
        Ok(Some(yaml)) => match serde_yaml::from_str(yaml) {
            Ok(parsed) => (parsed, None),
            Err(e) => (T::default(), Some(e.to_string())),
        },
        Err(why) => (T::default(), Some(why.to_string())),
    }
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

/// The corpus node, from the SDK that defines it.
///
/// **Re-exported rather than declared here, and that is the point of #714.** This file used
/// to hold its own `CorpusInstance`, `CorpusLink` and `ExternalCitation` while
/// `yidam_core::corpus` held a Markdown node model — `parse_node`, `extract_claims`,
/// `extract_links` — that three SDKs agreed about exactly, that parity certified for seven
/// minor versions, and that no product ever called. RFC-0002 named the split; RFC-0013 closed
/// it and specified the parser that would end it; nobody wrote the parser, and both documents
/// were recorded `Implemented` anyway.
///
/// So the type the products use and the type the parity surface certifies are now one type.
/// A second implementation on this side would put the drift straight back.
///
/// [`crate::prose`] holds what used to be this struct's inherent methods, because prose is
/// what an *ontology* declares and the SDK is not where that is decided.
pub use yidam_core::corpus::{parse_instance, CorpusInstance, CorpusLink, ExternalCitation};

/// A decision record (.yml file in .yidam/decisions/).
#[derive(serde::Deserialize, Default)]
pub struct Decision {
    pub id: Option<String>,
    pub summary: Option<String>,
    /// The records this one replaces, by `id:` or stem — one, or a list.
    ///
    /// Untyped, because a field no tool read until #1068 was written as both, and a typed
    /// field would turn every record spelling it the other way into a parse failure that
    /// `malformed-yaml` reports as an Error. [`Self::superseded`] is the reading.
    pub supersedes: Option<serde_yaml::Value>,
}

impl Decision {
    /// The ids [`Self::supersedes`] names: a string, or each string in a list. Anything else
    /// names nothing.
    pub fn superseded(&self) -> Vec<&str> {
        match &self.supersedes {
            Some(serde_yaml::Value::String(s)) => vec![s.trim()],
            Some(serde_yaml::Value::Sequence(items)) => items
                .iter()
                .filter_map(|v| v.as_str())
                .map(str::trim)
                .collect(),
            _ => Vec::new(),
        }
    }
}

/// A seed file from samudaya/ (markdown with kind/constitutional frontmatter).
#[derive(serde::Deserialize, Default)]
pub struct SamudayaSeed {
    pub kind: Option<String>,
    pub constitutional: Option<bool>,
}

pub fn parse_samudaya_seed(text: &str) -> SamudayaSeed {
    parse_samudaya_seed_reporting(text).0
}

/// A seed's frontmatter, and why it came back empty when it did.
///
/// Through [`parse_header`], which is the same reader a catalog entry goes through — this
/// function used to find the fences itself and end in `unwrap_or_default()`, so a seed whose
/// header the parser rejected read as a seed declaring no `kind:` and no `constitutional:`.
/// That is not silence, which is the part worth spelling out: `samudaya-audit` then reported
/// `missing 'kind:' field in frontmatter` about a file whose `kind:` was on line 2, and a
/// constitutional augmentation read as one that had not answered the question — the answer
/// `augmentation missing 'constitutional: true|false'` would have been given for. Every seed is
/// consumed once, at genesis, by a derived repository that will never see this file again.
///
/// [`crate::cmd::samudaya_audit`] reports the reason.
pub fn parse_samudaya_seed_reporting(text: &str) -> (SamudayaSeed, Option<String>) {
    parse_header(text)
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
