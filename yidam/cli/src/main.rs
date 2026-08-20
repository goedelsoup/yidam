use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use yidam::ExportFormat;

#[derive(Parser)]
#[command(
    name = "yidam",
    about = "Corpus analysis and index CLI for yidam-derived repositories"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Status,
    #[command(name = "open-questions")]
    OpenQuestions,
    #[command(name = "corpus-index")]
    CorpusIndex,
    #[command(name = "index-status")]
    IndexStatus,
    #[command(name = "catalog-audit")]
    CatalogAudit,
    #[command(name = "agents-index")]
    AgentsIndex,
    #[command(name = "skills-index")]
    SkillsIndex,
    #[command(name = "crates-index")]
    CratesIndex,
    #[command(name = "packages-index")]
    PackagesIndex,
    #[command(name = "bundle-status")]
    BundleStatus,
    /// Refresh every REGEN block in one pass.
    Regen,
    #[command(name = "graph-check")]
    GraphCheck,
    #[command(name = "decisions-log")]
    DecisionsLog,
    /// Show corpus node and edge changes between two git refs
    Diff {
        /// Git range, e.g. `main..HEAD`, `HEAD~5`, `abc123..def456`
        range: String,
    },
    /// Bundle ontology, corpus, skills, decisions, and vector index into .yidam/bundle.yiz
    /// (backwards-compatible alias for `export --format bundle`)
    Bundle,
    /// Export the domain model in the specified format
    Export {
        /// Output format (use --list to see available formats and their status)
        #[arg(long, value_enum, required_unless_present = "list")]
        format: Option<ExportFormat>,
        /// Output path (default: format-specific, e.g. .yidam/bundle.yiz for bundle)
        #[arg(long)]
        out: Option<PathBuf>,
        /// WebLLM model id for the web format's chat panel (default: Llama-3.2-1B-Instruct)
        #[arg(long, value_name = "MODEL_ID")]
        webllm_model: Option<String>,
        /// RDF serialization for the rdf format (default: both Turtle and JSON-LD)
        #[arg(long, value_enum, value_name = "FORMAT")]
        rdf_format: Option<yidam::RdfFormat>,
        /// Approximate token budget for the llms format (1 token ≈ 4 chars; default: unlimited)
        #[arg(long, value_name = "TOKENS")]
        token_budget: Option<usize>,
        /// List available export formats and their implementation status
        #[arg(long)]
        list: bool,
    },
    /// Extract embedding text from corpus instances to .yidam/embeddings/
    Embed {
        /// Do not embed `.yidam/catalog/` — corpus nodes only.
        ///
        /// The catalog is walked by default: in a real derived corpus it was 51.3% of the
        /// indexable text against the corpus's 41.9%, and leaving it out was a scope
        /// decision nobody had made on purpose.
        #[arg(long)]
        no_catalog: bool,
    },
    /// Build LanceDB vector index from embeddings and export Arrow IPC for the web shell
    #[command(name = "index-build")]
    IndexBuild {
        /// Embedding model to use (overrides .yidam/config.toml [index] model and the default)
        #[arg(long)]
        model: Option<String>,
    },
    /// Copy the yidam template into TARGET and initialise a fresh git repo
    Clone {
        /// Directory to create (must not already exist)
        target: PathBuf,
    },
    /// Overlay yidam infrastructure onto an existing git repo at TARGET
    Overlay {
        /// Root of an existing git repository
        target: PathBuf,
        /// Scan the repo's git history and write a decision record into
        /// .yidam/decisions/ for each epistemic commit (heuristic verb
        /// classification; corpus nodes are not extracted)
        #[arg(long)]
        backfill: bool,
        /// Only scan history reachable from HEAD but not from this ref (e.g. v1.0.0, main~100)
        #[arg(long, value_name = "REF", requires = "backfill")]
        backfill_ref: Option<String>,
    },
    /// Scan git history and write a decision record into .yidam/decisions/
    /// for each epistemic commit (heuristic verb classification)
    Backfill {
        /// Only scan history reachable from HEAD but not from this ref (e.g. v1.0.0, main~100)
        #[arg(long, value_name = "REF")]
        since: Option<String>,
    },
    /// Show active inquiry phases (ma/* and rigpa/* branches)
    Phases,
    /// Serve the domain computer to MCP-capable agents
    Serve {
        /// Serve MCP over stdio (the only transport currently implemented)
        #[arg(long)]
        mcp: bool,
    },
    /// Run corpus quality checks against the baseline ratchet
    Lint {
        /// Report findings but always exit 0
        #[arg(long)]
        warn: bool,
        /// Print each check's rationale alongside its findings
        #[arg(long)]
        explain: bool,
        /// Also check the git log against the commit vocabulary in GRAPH.md
        #[arg(long)]
        commits: bool,
        /// Restrict --commits to a revision range (e.g. main..HEAD)
        #[arg(long, value_name = "RANGE", requires = "commits")]
        range: Option<String>,
        /// Rewrite .yidam/lint-baseline.yml from this run instead of gating on it
        #[arg(long)]
        bless: bool,
    },
    /// Emit JSON Schema for the corpus shapes into .yidam/schemas/
    Schema {
        /// Print the editor `yaml.schemas` mapping instead of writing schema files
        #[arg(long)]
        settings: bool,
    },
    /// Inspect and validate samudaya/ seed files
    #[command(name = "samudaya-audit")]
    SamudayaAudit,
    /// Manage bundle dependencies in .yidam/tonpa/
    #[cfg(feature = "tonpa")]
    Tonpa {
        #[command(subcommand)]
        sub: yidam::tonpa::TonpaCommand,
    },
}

/// Build a Tokio runtime on demand for the async commands (`index-build`,
/// `tonpa`). Only compiled when one of those features is on; the default
/// `reports` build has no async work and links no runtime.
#[cfg(any(feature = "index", feature = "tonpa"))]
fn block_on<F: std::future::Future<Output = Result<()>>>(fut: F) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(fut)
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Command::Status => yidam::status(),
        Command::OpenQuestions => yidam::open_questions(),
        Command::CorpusIndex => yidam::corpus_index(),
        Command::IndexStatus => yidam::index_status(),
        Command::CatalogAudit => yidam::catalog_audit(),
        Command::AgentsIndex => yidam::agents_index(),
        Command::SkillsIndex => yidam::skills_index(),
        Command::CratesIndex => yidam::crates_index(),
        Command::PackagesIndex => yidam::packages_index(),
        Command::BundleStatus => yidam::bundle_status(),
        Command::Regen => yidam::regen(),
        Command::GraphCheck => yidam::graph_check(),
        Command::DecisionsLog => yidam::decisions_log(),
        Command::Diff { range } => yidam::diff_corpus(&range),
        Command::Bundle => yidam::bundle(),
        Command::Export {
            format,
            out,
            webllm_model,
            rdf_format,
            token_budget,
            list,
        } => {
            if list {
                yidam::list_formats();
                Ok(())
            } else {
                let options = yidam::ExportOptions {
                    webllm_model,
                    rdf_format,
                    token_budget,
                };
                yidam::run_export(format.unwrap(), out.as_deref(), &options)
            }
        }
        Command::Embed { no_catalog } => yidam::embed(yidam::EmbedOptions {
            catalog: !no_catalog,
        }),
        Command::IndexBuild { model } => {
            #[cfg(feature = "index")]
            {
                block_on(yidam::index_build(model))
            }
            #[cfg(not(feature = "index"))]
            {
                let _ = model;
                anyhow::bail!(
                    "`index-build` needs the `index` feature — reinstall with \
                     `cargo install yidam --features index` (pulls fastembed/lancedb; requires protoc)"
                )
            }
        }
        Command::Clone { target } => yidam::clone(&target),
        Command::Overlay {
            target,
            backfill,
            backfill_ref,
        } => yidam::overlay(&target, backfill, backfill_ref.as_deref()),
        Command::Backfill { since } => yidam::backfill(since.as_deref()),
        Command::Phases => yidam::phases(),
        Command::Serve { mcp } => {
            #[cfg(feature = "index")]
            {
                if mcp {
                    yidam::serve_mcp()
                } else {
                    anyhow::bail!("only the MCP transport is implemented — run `yidam serve --mcp`")
                }
            }
            #[cfg(not(feature = "index"))]
            {
                let _ = mcp;
                anyhow::bail!(
                    "`serve` needs the `index` feature — reinstall with \
                     `cargo install yidam --features index`"
                )
            }
        }
        Command::Lint {
            warn,
            explain,
            commits,
            range,
            bless,
        } => yidam::lint(yidam::LintOptions {
            warn_only: warn,
            explain,
            commits,
            range,
            bless,
        }),
        Command::Schema { settings } => yidam::schema(settings),
        Command::SamudayaAudit => yidam::samudaya_audit(),
        #[cfg(feature = "tonpa")]
        Command::Tonpa { sub } => block_on(yidam::tonpa::run(sub)),
    }
}
