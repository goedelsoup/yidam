use anyhow::Result;
use std::path::{Path, PathBuf};

use super::bundle::render_bundle;
use super::export_graphml::render_graphml;
use super::export_llms::render_llms;
#[cfg(feature = "export-graph")]
use super::export_rdf::{render_rdf_jsonld, render_rdf_turtle};
#[cfg(feature = "export-sqlite")]
use super::export_sqlite::render_sqlite;
use super::export_web::render_web;
use crate::model::{load_domain_model, DomainModel};
use crate::paths::repo_root;

/// RDF serialization selected by `--rdf-format`; omitted → both are written.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum RdfFormat {
    Turtle,
    Jsonld,
}

/// Format-specific options for [`export`].
#[derive(Default)]
pub struct ExportOptions {
    /// WebLLM model id pinned into `web.config.json` (web format only).
    pub webllm_model: Option<String>,
    /// RDF serialization (rdf format only); `None` writes both.
    pub rdf_format: Option<RdfFormat>,
    /// Approximate token budget for the llms format; `None` emits everything.
    pub token_budget: Option<usize>,
}

/// Available export formats.
///
/// Use `yidam export --list` to see the full list with default output paths.
#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ExportFormat {
    Bundle,
    Web,
    Rdf,
    #[value(name = "graphml")]
    GraphMl,
    Sqlite,
    Llms,
}

impl ExportFormat {
    fn default_output(&self, root: &Path) -> PathBuf {
        match self {
            Self::Bundle => root.join(".yidam").join("bundle.yiz"),
            Self::Web => root.join(".yidam").join("web"),
            Self::Rdf => root.join("corpus.ttl"),
            Self::GraphMl => root.join("corpus.graphml"),
            Self::Sqlite => root.join("corpus.db"),
            Self::Llms => root.join("llms.txt"),
        }
    }
}

/// Print available export formats and their current implementation status.
pub fn list_formats() {
    // Formats behind optional features report their status for *this* build so
    // `--list` never claims a capability the binary was not compiled with.
    let rdf = if cfg!(feature = "export-graph") {
        "✓ implemented"
    } else {
        "  needs --features export-graph"
    };
    let sqlite = if cfg!(feature = "export-sqlite") {
        "✓ implemented"
    } else {
        "  needs --features export-sqlite"
    };
    let mcp = if cfg!(feature = "vector-read") {
        "  run `yidam serve --mcp`"
    } else {
        "  needs --features vector-read"
    };
    let formats: &[(&str, &str)] = &[
        ("bundle", "✓ implemented"),
        ("web", "✓ implemented"),
        ("mcp", mcp),
        ("rdf", rdf),
        ("graphml", "✓ implemented"),
        ("sqlite", sqlite),
        ("llms", "✓ implemented"),
    ];
    for (name, status) in formats {
        println!("{name:<10} {status}");
    }
}

/// Export `model` in `format` and write the result to `out`.
///
/// This function is a pure dispatcher: it calls the format-specific renderer
/// (which takes `&DomainModel` and returns bytes), then writes to disk.
/// To add a new format: add a variant to [`ExportFormat`], add a `render_<format>`
/// function in its own module, and add an arm here.
pub fn export(
    model: &DomainModel,
    format: ExportFormat,
    out: &Path,
    options: &ExportOptions,
) -> Result<()> {
    match format {
        ExportFormat::Bundle => {
            let bytes = render_bundle(model)?;
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(out, &bytes)?;
            let index_note = if model.index.is_some() {
                " + vector index"
            } else {
                ""
            };
            println!(
                "Bundle written: {} classes, {} instances, {} skills, {} decisions{index_note}, \
                 {} bytes → {}",
                model.classes.len(),
                model.instances.len(),
                model.skills.len(),
                model.decisions.len(),
                bytes.len(),
                out.display(),
            );
        }
        ExportFormat::Web => {
            let files = render_web(model, options.webllm_model.as_deref())?;
            for (rel, bytes) in &files {
                let path = out.join(rel);
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::write(&path, bytes)?;
            }
            let retrieval_note = if model.index.is_some() {
                "semantic retrieval"
            } else {
                "keyword retrieval (no vector index — run `yidam index-build` for semantic search)"
            };
            println!(
                "Web agent written: {} file(s) → {} ({retrieval_note})\n\
                 Serve the directory or open index.html and drop the bundle onto the page.",
                files.len(),
                out.display(),
            );
        }
        ExportFormat::Rdf => {
            #[cfg(not(feature = "export-graph"))]
            anyhow::bail!(
                "`rdf` export needs the `export-graph` feature — reinstall with \
                 `cargo install yidam --features export-graph`"
            );
            #[cfg(feature = "export-graph")]
            {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let (turtle, jsonld) = match options.rdf_format {
                    Some(RdfFormat::Turtle) => (true, false),
                    Some(RdfFormat::Jsonld) => (false, true),
                    None => (true, true),
                };
                if turtle {
                    std::fs::write(out, render_rdf_turtle(model)?)?;
                    println!("RDF (Turtle) written → {}", out.display());
                }
                if jsonld {
                    // With no explicit --rdf-format both are written: Turtle at
                    // `out`, JSON-LD beside it with the extension swapped.
                    let path = if turtle {
                        out.with_extension("jsonld")
                    } else {
                        out.to_path_buf()
                    };
                    std::fs::write(&path, render_rdf_jsonld(model)?)?;
                    println!("RDF (JSON-LD) written → {}", path.display());
                }
            }
        }
        ExportFormat::GraphMl => {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(out, render_graphml(model)?)?;
            println!(
                "GraphML written: {} node(s) → {}",
                model.instances.len(),
                out.display()
            );
        }
        ExportFormat::Sqlite => {
            #[cfg(not(feature = "export-sqlite"))]
            anyhow::bail!(
                "`sqlite` export needs the `export-sqlite` feature — reinstall with \
                 `cargo install yidam --features export-sqlite`"
            );
            #[cfg(feature = "export-sqlite")]
            {
                if let Some(parent) = out.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                render_sqlite(model, out)?;
            }
        }
        ExportFormat::Llms => {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let pack = render_llms(model, options.token_budget);
            std::fs::write(out, &pack.text)?;
            let budget_note = match options.token_budget {
                Some(budget) => format!(" (token budget: {budget})"),
                None => String::new(),
            };
            // The count a reader needs is what the file holds, not what the corpus
            // holds — those differ exactly when the budget bit, which is the case
            // where being told the corpus size is worst.
            let count = if pack.written == pack.total {
                format!("{} node(s)", pack.total)
            } else {
                format!("{} of {} node(s)", pack.written, pack.total)
            };
            println!(
                "llms.txt written: {count}, ~{} tokens{budget_note} → {}",
                pack.text.len() / 4,
                out.display(),
            );
            if pack.omitted() > 0 {
                let breakdown: Vec<String> = pack
                    .omitted_by_class
                    .iter()
                    .map(|(class, n)| format!("{class}: {n}"))
                    .collect();
                println!(
                    "  omitted {} node(s) ({}) — see the trailing `# Omitted:` line",
                    pack.omitted(),
                    breakdown.join(", "),
                );
            }
            if pack.elided > 0 {
                println!(
                    "  elided {} description(s) to fit; labels and links kept",
                    pack.elided,
                );
            }
        }
    }
    Ok(())
}

/// Load the domain model and export in `format`.
///
/// Resolves the output path from `out` or uses the format-specific default.
/// This is the command-level entry point; [`export`] is the pure dispatch layer.
pub fn run_export(format: ExportFormat, out: Option<&Path>, options: &ExportOptions) -> Result<()> {
    let root = repo_root()?;
    let model = load_domain_model(&root)?;
    let default_out = format.default_output(&root);
    let out_path = out.unwrap_or(&default_out);
    export(&model, format, out_path, options)
}

/// Unix seconds → ISO-8601 UTC (days-from-civil inverse, Hinnant's algorithm).
/// Shared by the llms (light) and rdf (gated) exporters, so it lives in the
/// base export module rather than either optional one.
pub(crate) fn unix_to_iso(secs: u64) -> String {
    let days = (secs / 86400) as i64;
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { y + 1 } else { y };
    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::unix_to_iso;

    #[test]
    fn unix_to_iso_is_correct() {
        assert_eq!(unix_to_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_iso(1_780_000_000), "2026-05-28T20:26:40Z");
    }
}
