use anyhow::Result;
use clap::error::ErrorKind;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use std::path::PathBuf;
use std::sync::LazyLock;

use yidam::ExportFormat;

mod help;

/// What this binary is, in one line: version, build commit, and compiled features.
///
/// Every correctness story here rests on the binary matching the commit pinned in
/// `.yidam.toml`, and until now the binary could not be asked which one it was —
/// `yidam --version` was refused as an unexpected argument. A repository whose shell
/// resolved a `yidam` from somewhere else had no way to find out except by reading a
/// report's JSON envelope, which is the wrong place to look and the wrong time to learn.
///
/// The features matter as much as the commit. A light `reports` build does not carry
/// `tonpa` or `index-build`, so "command not found" and "this binary cannot do that"
/// are different diagnoses that used to look identical.
///
/// A `static` because clap wants a `&'static str` and the feature list is only knowable
/// once, at startup.
static VERSION: LazyLock<String> = LazyLock::new(|| {
    let b = yidam::report::YidamBlock::current();
    format!("{} ({}) [{}]", b.version, b.commit, b.features.join(" "))
});

#[derive(Parser)]
#[command(
    name = "yidam",
    about = "Corpus analysis and index CLI for yidam-derived repositories",
    version = VERSION.as_str()
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

/// The parser, with the flat subcommand list replaced by `help`'s grouping.
///
/// Built in two passes because the listing is rendered from the names and `about` strings
/// of the subcommands this binary compiled — which are only knowable from a built
/// [`clap::Command`]. The first pass is read; the second carries the result as
/// `after_help`. clap is asked for the descriptions rather than told them, so no
/// description exists twice.
fn command() -> clap::Command {
    let base = Cli::command();
    let subcommands: Vec<(String, String)> = base
        .get_subcommands()
        .filter(|c| !c.is_hide_set() && c.get_name() != "help")
        .map(|c| {
            (
                c.get_name().to_string(),
                c.get_about().map(|a| a.to_string()).unwrap_or_default(),
            )
        })
        .collect();
    base.help_template(help::TEMPLATE)
        .after_help(help::render(&subcommands))
}

#[derive(Subcommand)]
enum Command {
    /// Repository overview: nodes, open questions, catalog, index freshness, phases.
    ///
    /// Writes the `<!-- REGEN: yidam status -->` block in the repository's README.
    Status {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// List the corpus's unresolved questions, newest first.
    ///
    /// Writes the `<!-- REGEN: yidam open-questions -->` block in the repository's README.
    #[command(name = "open-questions")]
    OpenQuestions {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Index every corpus node by class, with its label and link count.
    ///
    /// Writes the `<!-- REGEN: yidam corpus-index -->` block in the repository's README.
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
    /// Report whether the vector index is present, and how stale it is against the corpus.
    ///
    /// Writes the `<!-- REGEN: yidam index-status -->` block in the repository's README.
    #[command(name = "index-status")]
    IndexStatus {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Audit catalog sources: which are cited by the corpus, and which are not.
    ///
    /// Writes the `<!-- REGEN: yidam catalog-audit -->` block in the repository's README.
    #[command(name = "catalog-audit")]
    CatalogAudit {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Index the domain agents in `.yidam/agents/`.
    ///
    /// Writes the `<!-- REGEN: yidam agents-index -->` block in the repository's README.
    #[command(name = "agents-index")]
    AgentsIndex,
    /// Index the domain skills in `.yidam/skills/`.
    ///
    /// Writes the `<!-- REGEN: yidam skills-index -->` block in the repository's README.
    #[command(name = "skills-index")]
    SkillsIndex,
    /// Index the domain-computer crates in `crates/`.
    ///
    /// Writes the `<!-- REGEN: yidam crates-index -->` block in the repository's README.
    #[command(name = "crates-index")]
    CratesIndex,
    /// Index the domain-computer packages in `packages/`.
    ///
    /// Writes the `<!-- REGEN: yidam packages-index -->` block in the repository's README.
    #[command(name = "packages-index")]
    PackagesIndex,
    /// Report the freshness of `.yidam/bundle.yiz` against the corpus it was built from.
    ///
    /// Writes the `<!-- REGEN: yidam bundle-status -->` block in the repository's README.
    #[command(name = "bundle-status")]
    BundleStatus,
    /// Is this setup sound? One screen of checks, each with a verdict and a remedy.
    ///
    /// Read-only and offline. Exits nonzero when something is wrong now; warnings — no
    /// index, an old pin — are reported and do not affect the exit code unless `--strict`.
    Doctor {
        /// Treat warnings as failures. For a CI job that wants the strictest reading.
        #[arg(long)]
        strict: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
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
    /// The graph gate CI runs: orphans, broken links, missing labels.
    ///
    /// Read-only. Exits nonzero when the graph does not hold together.
    #[command(name = "graph-check")]
    GraphCheck {
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// List the decision records in `.yidam/decisions/`, newest first.
    ///
    /// Read-only.
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
    /// Reconstruct corpus health across the repository's whole history
    ///
    /// Every other report answers about now. This answers about the shape of the change,
    /// which is where a corpus that is quietly accumulating uncited nodes becomes visible.
    Replay {
        /// Rows to print. 0 prints every commit that touched the corpus. Ignored by
        /// `--format json`, which always carries the whole series.
        #[arg(long, default_value_t = 12)]
        every: usize,
        /// Output format. `json` carries every row and the per-class breakdown.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Serve the domain computer over a stdio protocol
    Serve {
        /// Serve MCP over stdio — the agent surface. In the light default; `--features
        /// index` upgrades `retrieve` from keyword to semantic.
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
    let cli = match command()
        .try_get_matches()
        .and_then(|m| Cli::from_arg_matches(&m))
    {
        Ok(cli) => cli,
        // clap says a subcommand was not recognized without saying who did not recognize
        // it, and the answer is often a binary older than the command, from somewhere the
        // caller did not mean. Name it before exiting; `--help` and `--version` are not
        // errors and take clap's own path.
        Err(e)
            if matches!(
                e.kind(),
                ErrorKind::InvalidSubcommand | ErrorKind::UnknownArgument
            ) =>
        {
            let _ = e.print();
            eprintln!("\n{}", yidam::running_binary_note());
            std::process::exit(2);
        }
        Err(e) => e.exit(),
    };
    // Before doing any work, and for every subcommand: a `yidam` from elsewhere answering
    // for a repository that pins its own is the failure that reads as success.
    yidam::warn_if_shadowed();
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
        Command::Doctor { strict, format } => yidam::doctor(strict, format),
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
        Command::Replay { every, format } => yidam::replay(format, every),
        // Neither transport is gated any more, and that is the point: an agent surface
        // only the ML build carried was one almost nobody could reach. MCP's *semantic*
        // retrieval still needs `index`; every tool it serves does not, and the server
        // reports the difference on the handshake and on every `retrieve`.
        Command::Serve { mcp, lsp } => {
            if lsp && mcp {
                anyhow::bail!("pick one transport — `--lsp` or `--mcp`")
            }
            if lsp {
                return yidam::serve_lsp();
            }
            if mcp {
                return yidam::serve_mcp();
            }
            anyhow::bail!("name a transport — `yidam serve --lsp` or `yidam serve --mcp`")
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
