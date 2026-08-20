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
    Status {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "open-questions")]
    OpenQuestions {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "corpus-index")]
    CorpusIndex {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Check an embedding provider against an index's reproducibility contract
    #[command(name = "index-verify")]
    IndexVerify {
        /// Index directory (default: `.yidam/index`)
        #[arg(long)]
        index: Option<PathBuf>,
        /// Command that embeds the probe: reads it on stdin, writes a JSON array of
        /// floats on stdout
        #[arg(long, value_name = "CMD")]
        provider: Option<String>,
        /// Name this runtime, so a `known_delta` declared for it can apply
        #[arg(long, value_name = "NAME")]
        runtime: Option<String>,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "index-status")]
    IndexStatus {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "catalog-audit")]
    CatalogAudit {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
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
    Regen {
        /// Report which blocks are stale and write nothing. Exits nonzero when any is.
        #[arg(long)]
        check: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Rename a corpus node, rewriting every edge into it
    Rename {
        /// Node to rename, e.g. `concept/old.yml` or `concept/old`
        old: String,
        /// Its new id, e.g. `concept/new` — may move it to another class
        new: String,
        /// Print the plan and change nothing
        #[arg(long)]
        dry_run: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Report the corpus graph: nodes, resolved edges, and the classes that license them
    Graph {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Show the neighbourhood of one node — the traversal `serve --mcp` performs
    Neighbors {
        /// Node id, e.g. `concept/tailwater.yml` or `concept/tailwater`
        node: String,
        /// Maximum hops (default 1)
        #[arg(long, default_value_t = 1)]
        depth: usize,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "graph-check")]
    GraphCheck {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    #[command(name = "decisions-log")]
    DecisionsLog,
    /// Show corpus node and edge changes between two git refs
    Diff {
        /// Git range, e.g. `main..HEAD`, `HEAD~5`, `abc123..def456`
        range: String,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
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
    /// Show commit history classified as testimony or pipeline work
    Log {
        /// Show only epistemic commits — the testimony
        #[arg(long, conflicts_with = "operational")]
        epistemic: bool,
        /// Show only operational commits — the pipeline
        #[arg(long)]
        operational: bool,
        /// Git range, e.g. `main..HEAD`, `HEAD~20`. Defaults to the current ref's history.
        range: Option<String>,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Show active inquiry phases (ma/* and rigpa/* branches)
    Phases {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Serve the domain computer over a stdio protocol
    Serve {
        /// Serve MCP over stdio — the agent surface. Needs the `index` feature.
        #[arg(long)]
        mcp: bool,
        /// Serve LSP over stdio — the editor surface. In the light default.
        #[arg(long)]
        lsp: bool,
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
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
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
    /// Print the closed commit vocabulary, optionally checking one subject against it
    Vocabulary {
        /// Check this subject line against the vocabulary, the way `lint --commits`
        /// checks a committed one — but before the commit exists
        #[arg(long, value_name = "SUBJECT")]
        check: Option<String>,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Report the sangha: electors, positions, and settled resolutions
    Sangha {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
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
        Command::Status { format } => yidam::status(format),
        Command::OpenQuestions { format } => yidam::open_questions(format),
        Command::CorpusIndex { format } => yidam::corpus_index(format),
        Command::IndexVerify {
            index,
            provider,
            runtime,
            format,
        } => yidam::index_verify(index, provider, runtime, format),
        Command::IndexStatus { format } => yidam::index_status(format),
        Command::CatalogAudit { format } => yidam::catalog_audit(format),
        Command::AgentsIndex => yidam::agents_index(),
        Command::SkillsIndex => yidam::skills_index(),
        Command::CratesIndex => yidam::crates_index(),
        Command::PackagesIndex => yidam::packages_index(),
        Command::BundleStatus => yidam::bundle_status(),
        Command::Regen { check, format } => yidam::regen(check, format),
        Command::Rename {
            old,
            new,
            dry_run,
            format,
        } => yidam::rename(&old, &new, dry_run, format),
        Command::Graph { format } => yidam::graph(format),
        Command::Neighbors {
            node,
            depth,
            format,
        } => yidam::neighbors(&node, depth, format),
        Command::GraphCheck { format } => yidam::graph_check(format),
        Command::DecisionsLog => yidam::decisions_log(),
        Command::Diff { range, format } => yidam::diff_corpus(&range, format),
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
        Command::Log {
            epistemic,
            operational,
            range,
            format,
        } => {
            // Neither flag is the documented default: both kinds, each tagged. See
            // `cmd::log::Filter::All` for why testimony is a discoverable flag rather
            // than a silent default.
            let filter = match (epistemic, operational) {
                (true, _) => yidam::LogFilter::Epistemic,
                (_, true) => yidam::LogFilter::Operational,
                _ => yidam::LogFilter::All,
            };
            yidam::log(range, filter, format)
        }
        Command::Phases { format } => yidam::phases(format),
        // The two transports are gated separately, and that is the whole point: MCP pulls
        // fastembed, lancedb and protoc; LSP needs none of them. An LSP that required the ML
        // stack would be one nobody could install.
        Command::Serve { mcp, lsp } => {
            if lsp && mcp {
                anyhow::bail!("pick one transport — `--lsp` or `--mcp`")
            }
            if lsp {
                return yidam::serve_lsp();
            }
            #[cfg(feature = "index")]
            {
                if mcp {
                    yidam::serve_mcp()
                } else {
                    anyhow::bail!("name a transport — `yidam serve --lsp` or `yidam serve --mcp`")
                }
            }
            #[cfg(not(feature = "index"))]
            {
                anyhow::bail!(
                    "`serve --mcp` needs the `index` feature — reinstall with \
                     `cargo install yidam --features index`. `serve --lsp` is always available."
                )
            }
        }
        Command::Lint {
            warn,
            explain,
            commits,
            range,
            bless,
            format,
        } => yidam::lint(yidam::LintOptions {
            warn_only: warn,
            explain,
            commits,
            range,
            bless,
            format,
        }),
        Command::Schema { settings } => yidam::schema(settings),
        Command::SamudayaAudit => yidam::samudaya_audit(),
        Command::Sangha { format } => yidam::sangha(format),
        Command::Vocabulary { check, format } => yidam::vocabulary(check, format),
        #[cfg(feature = "tonpa")]
        Command::Tonpa { sub } => block_on(yidam::tonpa::run(sub)),
    }
}
