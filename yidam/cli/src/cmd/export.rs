use anyhow::Result;
use std::path::{Path, PathBuf};

use super::bundle::render_bundle;
use super::export_web::render_web;
use crate::model::{load_domain_model, DomainModel};
use crate::paths::repo_root;

/// Format-specific options for [`export`].
#[derive(Default)]
pub struct ExportOptions {
    /// WebLLM model id pinned into `web.config.json` (web format only).
    pub webllm_model: Option<String>,
}

/// Available export formats.
///
/// Formats that are not yet implemented will produce a clear error at runtime.
/// Use `yidam export --list` to see current implementation status.
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
    fn name(&self) -> &'static str {
        match self {
            Self::Bundle => "bundle",
            Self::Web => "web",
            Self::Rdf => "rdf",
            Self::GraphMl => "graphml",
            Self::Sqlite => "sqlite",
            Self::Llms => "llms",
        }
    }

    fn default_output(&self, root: &Path) -> PathBuf {
        match self {
            Self::Bundle => root.join(".yidam").join("bundle.yiz"),
            Self::Web => root.join(".yidam").join("web"),
            other => root.join(other.name()),
        }
    }
}

/// Print available export formats and their current implementation status.
pub fn list_formats() {
    const FORMATS: &[(&str, &str)] = &[
        ("bundle", "✓ implemented"),
        ("web", "✓ implemented"),
        ("mcp", "  run `yidam serve --mcp` (phase 2)"),
        ("rdf", "  planned (phase 3)"),
        ("graphml", "  planned (phase 3)"),
        ("sqlite", "  planned (phase 4)"),
        ("llms", "  planned (phase 4)"),
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
        other => anyhow::bail!("export format '{}' is not yet implemented", other.name()),
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
