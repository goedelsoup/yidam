use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Default, Deserialize)]
pub struct YidamConfig {
    #[serde(default)]
    pub index: IndexConfig,
}

#[derive(Debug, Default, Deserialize)]
pub struct IndexConfig {
    pub model: Option<String>,
}

pub fn load_yidam_config(root: &Path) -> Result<YidamConfig> {
    let path = root.join(".yidam").join("config.toml");
    if !path.exists() {
        return Ok(YidamConfig::default());
    }
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("parsing {}", path.display()))
}
