/// Markdown frontmatter (---/--- block) for agents, skills, and catalog entries.
#[derive(serde::Deserialize, Default)]
pub struct Frontmatter {
    pub name: Option<String>,
    pub description: Option<String>,
}

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
