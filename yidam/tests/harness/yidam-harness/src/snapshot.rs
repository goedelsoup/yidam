use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::check::CheckReport;
use crate::PROTOCOL_VERSION;

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    /// The protocol version this result was taken under. `None` for a snapshot written
    /// before the version was recorded at all — which is every snapshot from 0.1.0, and the
    /// reason this is an option rather than a defaulted string: "written by a harness that
    /// did not know its own version" is a fact worth keeping, not one worth guessing at.
    #[serde(default)]
    pub protocol_version: Option<String>,
    pub structural: CheckReport,
}

impl Snapshot {
    /// How to name this snapshot's protocol version in a message to a person.
    pub fn version_label(&self) -> &str {
        self.protocol_version.as_deref().unwrap_or("unversioned")
    }
}

pub fn write(result_dir: &Path, structural: &CheckReport) -> Result<()> {
    let snap = Snapshot {
        protocol_version: Some(PROTOCOL_VERSION.to_string()),
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
