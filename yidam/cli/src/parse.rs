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

/// A corpus instance object (.yml file inside a class subdirectory).
#[derive(serde::Deserialize, Default)]
pub struct CorpusInstance {
    pub class: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub links: Option<Vec<CorpusLink>>,
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

pub fn extract_toml_field(text: &str, field: &str) -> Option<String> {
    let prefix = format!("{field} = ");
    for line in text.lines() {
        if let Some(rest) = line.trim().strip_prefix(&prefix) {
            let value = rest.trim().trim_matches('"').to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }
    None
}

pub fn extract_json_field(text: &str, field: &str) -> Option<String> {
    let pattern = format!("\"{field}\":");
    for line in text.lines() {
        if let Some(rest) = line.find(&pattern).map(|i| &line[i + pattern.len()..]) {
            let value = rest
                .trim()
                .trim_matches('"')
                .trim_end_matches(',')
                .trim()
                .to_string();
            if !value.is_empty() && value != "null" {
                return Some(value);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

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
