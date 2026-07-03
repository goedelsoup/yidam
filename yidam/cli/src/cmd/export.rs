use anyhow::Result;
use std::path::{Path, PathBuf};

use super::bundle::render_bundle;
use super::export_graphml::render_graphml;
use super::export_llms::render_llms;
use super::export_rdf::{render_rdf_jsonld, render_rdf_turtle};
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
    const FORMATS: &[(&str, &str)] = &[
        ("bundle", "✓ implemented"),
        ("web", "✓ implemented"),
        ("mcp", "  run `yidam serve --mcp` (phase 2)"),
        ("rdf", "✓ implemented"),
        ("graphml", "✓ implemented"),
        ("sqlite", "✓ implemented"),
        ("llms", "✓ implemented"),
    ];
    for (name, status) in FORMATS {
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
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            render_sqlite(model, out)?;
        }
        ExportFormat::Llms => {
            if let Some(parent) = out.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let text = render_llms(model, options.token_budget);
            std::fs::write(out, &text)?;
            let budget_note = match options.token_budget {
                Some(budget) => format!(" (token budget: {budget})"),
                None => String::new(),
            };
            println!(
                "llms.txt written: {} node(s), ~{} tokens{budget_note} → {}",
                model.instances.len(),
                text.len() / 4,
                out.display(),
            );
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
