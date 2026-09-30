//! `yidam derive` — the arguments a repository derives from its corpus, checked (RFC-0045).
//!
//! The predicate is [`crate::derive`]; this is the command around it. One subcommand today,
//! `check`, because the other verbs RFC-0045 considered — scaffolding an artifact, computing a
//! tier to paste into one — are writes, and a write belongs after the read has been lived with.

use anyhow::Result;
use clap::Subcommand;
use serde::Serialize;
use std::fmt::Write as _;

use crate::derive::{admitted_by, Derivation};
use crate::report::Format;

#[derive(Debug, Subcommand)]
pub enum DeriveCommand {
    /// Check every artifact under `[derive] paths` against the corpus it cites
    ///
    /// Each cited span must be in one paragraph of the node it names; the artifact's tier is the
    /// weakest standing beneath those spans, and must be one its declared reach admits; and
    /// every refusal a cited paragraph declares under `refuses:` must be answered. Exits
    /// non-zero on any error. A cited paragraph that reads like a refusal and declares none is
    /// reported for a person to look at and never fails the check.
    Check {
        #[arg(long, value_enum, default_value_t = Format::Text)]
        format: Format,
    },
}

/// `yidam derive <sub>`.
pub fn run(root: Option<&std::path::Path>, sub: DeriveCommand) -> Result<()> {
    match sub {
        DeriveCommand::Check { format } => check(root, format),
    }
}

/// The report, under names no other command's report uses — `findings` is `check-diff`'s.
#[derive(Serialize)]
struct Payload {
    derive_paths: Vec<String>,
    derivations: Vec<Derivation>,
}

fn check(root: Option<&std::path::Path>, format: Format) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    crate::paths::require_yidam_repo(&root)?;
    let globs = crate::config::load_yidam_config(&root)?.derive.paths;
    let derivations = if globs.is_empty() {
        Vec::new()
    } else {
        let corpus = crate::corpus::Corpus::open(&root);
        crate::derive::check(&root, &globs, &corpus)
    };
    let passed = derivations.iter().all(|d| d.errors() == 0);
    let report = Payload {
        derive_paths: globs,
        derivations,
    };
    crate::report::gate(&root, format, report, passed, |r| {
        print!("{}", render(&r.derive_paths, &r.derivations))
    })
}

/// The text report: one block per artifact, its tier against its reach, then what was found.
pub(crate) fn render(globs: &[String], derivations: &[Derivation]) -> String {
    let mut out = String::new();
    if globs.is_empty() {
        out.push_str(
            "derive check: no `[derive] paths` in .yidam/config.toml, so there is nothing to \
             read.\n",
        );
        return out;
    }
    if derivations.is_empty() {
        let _ = writeln!(out, "derive check: no artifact under {}.", globs.join(", "));
        return out;
    }
    for d in derivations {
        let reach = d.reach.as_deref().unwrap_or("—");
        let tier = d.tier.map_or("—".to_string(), |t| format!("[{t}]"));
        let admits = d
            .reach
            .as_deref()
            .and_then(admitted_by)
            .map_or(String::new(), |w| format!(", admits [{w}]+"));
        let _ = writeln!(out, "{}", d.path);
        let _ = writeln!(
            out,
            "  reach {reach}{admits} · tier {tier} · {} span(s)",
            d.spans.len()
        );
        for f in &d.findings {
            let at = f
                .node
                .as_deref()
                .map_or(String::new(), |n| format!(" {n}:"));
            let _ = writeln!(out, "  {:<5} {}{at} {}", f.severity, f.check, f.message);
        }
        out.push('\n');
    }
    let errors: usize = derivations.iter().map(Derivation::errors).sum();
    let failing = derivations.iter().filter(|d| d.errors() > 0).count();
    let _ = writeln!(
        out,
        "derive check: {} artifact(s), {failing} failing, {errors} error(s).",
        derivations.len()
    );
    out
}
