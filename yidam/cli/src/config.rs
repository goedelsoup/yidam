use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct YidamConfig {
    /// Read only by `index-build`, which the light `reports` binary does not carry. The
    /// field is still parsed there so a config naming a model is not rejected by a binary
    /// that simply cannot act on it.
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    #[serde(default)]
    pub index: IndexConfig,
    #[serde(default)]
    pub lint: LintConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct IndexConfig {
    #[cfg_attr(not(feature = "index"), allow(dead_code))]
    pub model: Option<String>,
}

/// What this corpus has decided about its own gate.
#[derive(Debug, Default, Deserialize)]
pub struct LintConfig {
    /// Corpus-touching commits a dated finding may hold before it escalates to an error.
    ///
    /// Absent means no finding ever escalates, and that is the right default rather than a
    /// timid one. The number is a judgement about how fast *this* corpus is meant to
    /// consume what it collects — a breadth sweep landing twelve nodes it will link over
    /// the next eighty commits is healthy in one repository and over-collection in another
    /// — so a value compiled into the binary would be one corpus's answer imposed on every
    /// other, arriving as a build failure in a repository that never agreed to it.
    ///
    /// Declared here so the argument for the number lives in the repository that has to
    /// live with it:
    ///
    /// ```toml
    /// [lint]
    /// escalate_after = 100
    /// ```
    pub escalate_after: Option<usize>,
}

pub fn load_yidam_config(root: &Path) -> Result<YidamConfig> {
    let path = root.join(".yidam").join("config.toml");
    if !path.exists() {
        return Ok(YidamConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
