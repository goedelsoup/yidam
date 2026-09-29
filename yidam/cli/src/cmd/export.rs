use anyhow::Result;
use std::io::Write;
use std::path::{Path, PathBuf};

use super::bundle::render_bundle;
use super::export_graphml::render_graphml;
use super::export_llms::{render_llms, LlmsPack};
#[cfg(feature = "export-graph")]
use super::export_rdf::{render_rdf_jsonld, render_rdf_turtle};
#[cfg(feature = "export-sqlite")]
use super::export_sqlite::render_sqlite;
use super::export_web::render_web;
use crate::model::{load_domain_model, DomainModel};

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
    /// Where the format writes when `--out` is absent; `None` is stdout.
    ///
    /// `llms` is the one that answers `None`. It is the export an agent runs to *read* a
    /// corpus, and a default of `llms.txt` at the root left every such read with an untracked
    /// file in the tree it had only meant to inspect (#919). The others are artefacts: a
    /// bundle, a site, a database. Nobody pipes those, so they keep a place on disk.
    fn default_output(&self, root: &Path) -> Option<PathBuf> {
        Some(match self {
            Self::Bundle => root.join(".yidam").join("bundle.yiz"),
            Self::Web => root.join(".yidam").join("web"),
            Self::Rdf => root.join("corpus.ttl"),
            Self::GraphMl => root.join("corpus.graphml"),
            Self::Sqlite => root.join("corpus.db"),
            Self::Llms => return None,
        })
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
/// `root` is the corpus `model` was loaded from. It is a parameter rather than a thing this
/// looks up, because a renderer that reaches for [`crate::paths::repo_root`] on its own is a
/// *second* answer to "which corpus?" — and the two were free to differ the moment `--root`
/// existed. `llms` did exactly that (#421, #236).
pub fn export(
    model: &DomainModel,
    root: &Path,
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
            let pack = render_llms(model, root, options.token_budget);
            write_llms(
                &pack,
                options.token_budget,
                Some(out),
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            )?;
        }
    }
    Ok(())
}

/// Write an llms pack to `out`, or to `stdout` when `out` is `None`, and report what it holds.
///
/// The report goes where the pack does not. Written to a file, the pack leaves stdout free
/// and the report takes it. Written to stdout, the report moves to `stderr`, because a
/// summary line inside the pack would be pasted into a context window with it.
fn write_llms(
    pack: &LlmsPack,
    token_budget: Option<usize>,
    out: Option<&Path>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Result<()> {
    let report: &mut dyn Write = match out {
        Some(path) => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(path, &pack.text)?;
            stdout
        }
        None => {
            stdout.write_all(pack.text.as_bytes())?;
            stdout.flush()?;
            stderr
        }
    };
    let dest = out.map_or_else(|| "stdout".to_string(), |p| p.display().to_string());
    let budget_note = match token_budget {
        Some(budget) => format!(" (token budget: {budget})"),
        None => String::new(),
    };
    // The count a reader needs is what the pack holds, not what the corpus holds — those
    // differ exactly when the budget bit, which is the case where being told the corpus size
    // is worst.
    let count = if pack.written == pack.total {
        format!("{} node(s)", pack.total)
    } else {
        format!("{} of {} node(s)", pack.written, pack.total)
    };
    writeln!(
        report,
        "llms.txt written: {count}, ~{} tokens{budget_note} → {dest}",
        pack.text.len() / 4,
    )?;
    if pack.omitted() > 0 {
        let breakdown: Vec<String> = pack
            .omitted_by_class
            .iter()
            .map(|(class, n)| format!("{class}: {n}"))
            .collect();
        writeln!(
            report,
            "  omitted {} node(s) ({}) — see the trailing `# Omitted:` line",
            pack.omitted(),
            breakdown.join(", "),
        )?;
    }
    if pack.elided > 0 {
        writeln!(
            report,
            "  elided {} description(s) to fit; labels and links kept",
            pack.elided,
        )?;
    }
    Ok(())
}

/// Load the domain model and export in `format`.
///
/// Resolves the output path from `out` or uses the format-specific default. A format whose
/// default is stdout (`llms`, see [`ExportFormat::default_output`]) prints there instead.
/// This is the command-level entry point; [`export`] is the pure dispatch layer.
pub fn run_export(
    root: Option<&Path>,
    format: ExportFormat,
    out: Option<&Path>,
    options: &ExportOptions,
) -> Result<()> {
    let root = crate::paths::resolve_root(root)?;
    // A directory that is not a corpus produced a *successful* export of nothing: every
    // format, exit 0, an artefact written. `--format bundle` and `--format web` also created
    // the `.yidam/` directory on the way, so the run that should have been refused left behind
    // the one marker every gate keys on. `require_yidam_repo`'s doc draws the line this
    // crosses — a *report* may print that it found nothing, and a command that writes may not,
    // because the empty artefact then travels to whatever reads it.
    //
    // Before the model rather than after: an export refused for having no corpus should say so
    // by name, not report the emptiness that follows from it.
    crate::paths::require_yidam_repo(&root)?;
    let model = load_domain_model(&root)?;
    let default_out = format.default_output(&root);
    match out.or(default_out.as_deref()) {
        Some(out_path) => export(&model, &root, format, out_path, options),
        None => {
            let pack = render_llms(&model, &root, options.token_budget);
            write_llms(
                &pack,
                options.token_budget,
                None,
                &mut std::io::stdout(),
                &mut std::io::stderr(),
            )
        }
    }
}

/// Unix seconds → ISO-8601 UTC.
///
/// The date half is [`crate::dates::civil_from_days`]; this adds the time of day and the
/// formatting. Shared by the llms (light) and rdf (gated) exporters, so it lives in the
/// base export module rather than either optional one.
pub(crate) fn unix_to_iso(secs: u64) -> String {
    let rem = secs % 86400;
    let (h, m, s) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let (year, month, d) = crate::dates::civil_from_days((secs / 86400) as i64);
    format!("{year:04}-{month:02}-{d:02}T{h:02}:{m:02}:{s:02}Z")
}

#[cfg(test)]
mod tests {
    use super::{unix_to_iso, write_llms, ExportFormat, LlmsPack};
    use std::collections::BTreeMap;
    use std::path::Path;

    fn pack(omitted: usize) -> LlmsPack {
        LlmsPack {
            text: "# Domain: d\n# Nodes: 2\n\n## concept/a\n".to_string(),
            total: 2 + omitted,
            written: 2,
            elided: 1,
            omitted_by_class: if omitted > 0 {
                BTreeMap::from([("concept".to_string(), omitted)])
            } else {
                BTreeMap::new()
            },
        }
    }

    /// #919: `llms` is the one format with nowhere on disk to go by default. Asserted over
    /// every variant, so a format added later has to pick a side here rather than inherit one.
    #[test]
    fn only_llms_defaults_to_stdout() {
        let root = Path::new("/corpus");
        for format in [
            ExportFormat::Bundle,
            ExportFormat::Web,
            ExportFormat::Rdf,
            ExportFormat::GraphMl,
            ExportFormat::Sqlite,
            ExportFormat::Llms,
        ] {
            let default = format.default_output(root);
            match format {
                ExportFormat::Llms => assert_eq!(default, None),
                _ => assert!(default.is_some_and(|p| p.starts_with(root)), "{format:?}"),
            }
        }
    }

    /// Stdout carries the pack and nothing else, byte for byte, so it can be piped or pasted.
    /// Every report line, the budget's included, is on stderr.
    #[test]
    fn to_stdout_the_pack_is_all_stdout_holds() {
        let p = pack(3);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        write_llms(&p, Some(100), None, &mut out, &mut err).unwrap();

        assert_eq!(String::from_utf8(out).unwrap(), p.text);
        let err = String::from_utf8(err).unwrap();
        assert!(
            err.contains("2 of 5 node(s)") && err.contains("→ stdout"),
            "{err}"
        );
        assert!(err.contains("token budget: 100"), "{err}");
        assert!(err.contains("omitted 3 node(s) (concept: 3)"), "{err}");
        assert!(err.contains("elided 1 description(s)"), "{err}");
    }

    /// With `--out` the pack is the file, the report is stdout, and stderr stays empty.
    #[test]
    fn to_a_file_the_report_takes_stdout() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("llms.md");
        let p = pack(0);
        let (mut out, mut err) = (Vec::new(), Vec::new());
        write_llms(&p, None, Some(&path), &mut out, &mut err).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), p.text);
        let out = String::from_utf8(out).unwrap();
        assert!(out.starts_with("llms.txt written: 2 node(s)"), "{out}");
        assert!(out.contains(&path.display().to_string()), "{out}");
        assert!(!out.contains(&p.text), "{out}");
        assert!(err.is_empty(), "{}", String::from_utf8_lossy(&err));
    }

    #[test]
    fn unix_to_iso_is_correct() {
        assert_eq!(unix_to_iso(0), "1970-01-01T00:00:00Z");
        assert_eq!(unix_to_iso(1_780_000_000), "2026-05-28T20:26:40Z");
    }

    /// What this function computed while Hinnant's algorithm was inlined here, kept
    /// verbatim so the fold onto [`crate::dates::civil_from_days`] has something to be
    /// differential against. Two spot values cannot distinguish two arrangements of this
    /// arithmetic; a century-and-a-third of them can.
    fn unix_to_iso_before_the_fold(secs: u64) -> String {
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

    /// 1970 → 2100 at 90,061-second steps: 45,525 timestamps, ~1.042 days apart.
    ///
    /// The stride is deliberately not a whole number of days. A day-aligned walk visits the
    /// same time-of-day forever and would not notice a seconds-of-day defect at all, and a
    /// stride that divides a week or a month lands on the same phase of every month length.
    /// This one drifts through both, so every month length, every leap year, the 1900-style
    /// century non-leap at 2100's edge and every hour of the day are all sampled.
    #[test]
    fn unix_to_iso_is_unchanged_by_the_fold() {
        const STEP: u64 = 90_061;
        const SAMPLES: u64 = 45_525;
        for i in 0..SAMPLES {
            let ts = i * STEP;
            assert_eq!(
                unix_to_iso(ts),
                unix_to_iso_before_the_fold(ts),
                "diverged at unix {ts}"
            );
        }
    }
}
