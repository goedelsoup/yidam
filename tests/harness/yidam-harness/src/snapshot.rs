use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::check::CheckReport;

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub structural: CheckReport,
}

pub fn write(result_dir: &Path, structural: &CheckReport) -> Result<()> {
    let snap = Snapshot {
        structural: structural.clone(),
    };
    let json = serde_json::to_string_pretty(&snap)?;
    std::fs::write(result_dir.join("structural.json"), json).context("writing structural.json")
}

pub fn load(result_dir: &Path) -> Result<Snapshot> {
    let content = std::fs::read_to_string(result_dir.join("structural.json"))
        .context("reading structural.json")?;
    serde_json::from_str(&content).context("parsing structural.json")
}
