use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct Scenario {
    pub id: String,
    pub domain: String,
    pub central_question: String,
    pub seed_concepts: Vec<SeedConcept>,
    pub good_bootstrap_looks_like: String,
}

#[derive(Debug, Deserialize)]
pub struct SeedConcept {
    pub name: String,
    pub hint: String,
}

pub fn load(path: &Path) -> Result<Scenario> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("reading scenario: {}", path.display()))?;
    // Scenario files are markdown with YAML frontmatter delimited by `---`
    let yaml =
        extract_frontmatter(&content).with_context(|| "scenario file has no YAML frontmatter")?;
    serde_yaml::from_str(yaml).with_context(|| "parsing scenario frontmatter")
}

fn extract_frontmatter(content: &str) -> Option<&str> {
    let body = content.trim_start();
    let body = body.strip_prefix("---\n")?;
    let end = body.find("\n---")?;
    Some(&body[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_frontmatter() {
        let content = "---\nid: test\ndomain: d\ncentral_question: q\nseed_concepts:\n  - name: c\n    hint: h\ngood_bootstrap_looks_like: g\n---\n# body\n";
        let yaml = extract_frontmatter(content).unwrap();
        let s: Scenario = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(s.id, "test");
        assert_eq!(s.seed_concepts.len(), 1);
    }
}
