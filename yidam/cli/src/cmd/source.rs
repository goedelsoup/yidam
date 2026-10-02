//! `yidam source` — the source packs a corpus resolves identifiers through (RFC-0048 §4).
//!
//! `check` is the first subcommand, and the only one #1315 adds. `list`, `search` and `add`
//! follow in #1316; they read the same [`crate::sources::load`].
//!
//! # `check` is offline, and is the gate for an authored pack
//!
//! A corpus that writes a pack for its own publisher has nothing else holding it to the format:
//! no crate compiles it and no template test reads it. So `check` exits nonzero on any error,
//! and a corpus runs it in CI. It reads only the repository: the manifests, the fixtures they
//! name, and `prelude_sources`. It never makes a request, so a pack is checked the same way on
//! a laptop with no network as in CI.

use std::path::Path;

use anyhow::Result;
use clap::Subcommand;

use crate::report::Format;
use crate::sources::{self, Origin, Severity};

#[derive(Debug, Subcommand)]
pub enum SourceCommand {
    /// Check every pack and every `prelude_sources` pin, offline; exit nonzero on an error
    ///
    /// Each manifest parses, each pattern compiles, each resolve template binds, and each
    /// fixture is claimed by a scheme. Each vendored pack is the one its pin asked for. Nothing
    /// is fetched.
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

pub fn run(root: Option<&Path>, sub: SourceCommand) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    crate::paths::require_yidam_repo(&root)?;
    match sub {
        SourceCommand::Check { format } => check(&root, format),
    }
}

#[derive(serde::Serialize)]
struct CheckReport {
    passed: bool,
    packs: Vec<sources::Summary>,
    /// Not `findings`, which the contract gives `check-diff`'s shape.
    pack_findings: Vec<sources::Finding>,
}

fn check(root: &Path, format: Format) -> Result<()> {
    let checked = sources::check(root)?;
    let passed = checked.ok();
    let report = CheckReport {
        passed,
        packs: checked.packs,
        pack_findings: checked.findings,
    };
    crate::report::gate(root, format, report, passed, render)
}

fn render(report: &CheckReport) {
    if report.packs.is_empty() && report.pack_findings.is_empty() {
        println!(
            "No source packs: nothing in {} or {}, and nothing pinned in prelude_sources.",
            sources::AUTHORED,
            sources::VENDORED
        );
        return;
    }
    if !report.packs.is_empty() {
        println!("Packs");
        for p in &report.packs {
            let origin = match p.origin {
                Origin::Authored => "authored",
                Origin::Vendored if p.shadowed => "shadowed",
                Origin::Vendored => "vendored",
            };
            let schemes = if p.schemes.is_empty() {
                "-".to_string()
            } else {
                p.schemes.join(", ")
            };
            println!(
                "  {:<20} {:<9} {:<9} {schemes} ({} fixture{})",
                p.name,
                p.version.as_deref().unwrap_or("?"),
                origin,
                p.fixtures,
                if p.fixtures == 1 { "" } else { "s" },
            );
        }
    }
    if !report.pack_findings.is_empty() {
        println!();
        for f in &report.pack_findings {
            let tag = match f.severity {
                Severity::Error => "error",
                Severity::Info => "info ",
            };
            println!("  {tag}  {}: {}", f.path, f.message);
        }
    }
    let errors = report
        .pack_findings
        .iter()
        .filter(|f| f.severity == Severity::Error)
        .count();
    println!();
    if report.passed {
        println!("Every pack checks.");
    } else {
        println!("{errors} error(s).");
    }
}
