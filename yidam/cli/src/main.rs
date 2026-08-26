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
    /// Change an ontology and every instance that adopted it, as one event
    ///
    /// A class definition cannot be corrected in place once the class contract gates:
    /// editing it puts every instance in violation until each is fixed by hand. These
    /// subcommands do both halves together and write a record of what they touched.
    Migrate {
        #[command(subcommand)]
        operation: MigrateCommand,
        /// Print the plan and change nothing
        #[arg(long)]
        dry_run: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Measure the committed goal set: anchored traversal against flat retrieval
    Bench {
        /// The flat arm's budget — `retrieve`'s `k`, defaulting to the 5 `serve --mcp` uses
        #[arg(long, default_value_t = 5)]
        budget: usize,
        /// Measure the arms that are functions of N over generated corpora, rather than
        /// this repository's corpus. Needs no index: the flat arm is constant in N and is
        /// excluded by argument
        #[arg(long)]
        scaling: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Execute a typed path over the resolved graph — `reach -measured-by-> gage`
    Query {
        /// The query. Whitespace around a hop is required: `-rel->` and `<-rel-` are single
        /// tokens, which is what makes a hyphenated relationship unambiguous
        query: String,
        /// Fields to project, comma-separated: node, class, label, description, body, or
        /// `properties.<name>`
        #[arg(long)]
        select: Option<String>,
        /// Maximum results shown. The count reported is always the full one — a limit
        /// bounds the projection, not the traversal
        #[arg(long, default_value_t = yidam::QUERY_DEFAULT_LIMIT)]
        limit: usize,
        /// How many entry nodes a `class~"…"` anchor opens on. An anchor is a starting
        /// point, not an answer — widening it walks from every entry
        #[arg(long, default_value_t = yidam::QUERY_DEFAULT_ANCHOR_K)]
        anchor_k: usize,
        /// Answer as of a commit — a sha, tag, or `HEAD~5`. Reconstructed from git objects;
        /// the working tree is never touched
        #[arg(long, value_name = "REF", conflicts_with = "between")]
        at: Option<String>,
        /// Answer at every commit in `a..b` that touched the corpus, as a series. Git's own
        /// range meaning: `a` itself is excluded
        #[arg(long, value_name = "A..B")]
        between: Option<String>,
        /// Query installed dependencies as well. Every result says whose corpus it came
        /// from, and no hop crosses a corpus boundary
        #[arg(long, conflicts_with_all = ["at", "between"])]
        across: bool,
        /// Output format. `json` emits the machine-readable report contract
        /// (RFC-0016); `text` is unchanged and remains the default.
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Build a context pack for one goal — a query's full answer, filled to a token budget,
    /// with an account of what did not fit
    Pack {
        /// The query naming the goal, in `yidam query`'s language — including a similarity
        /// anchor, e.g. `concept~"hydropeaking" <-exhibits- reach`
        query: String,
        /// Approximate token budget (1 token ≈ 4 chars). Unbudgeted by default: a default
        /// budget would silently truncate the first pack anybody builds
        #[arg(long)]
        budget: Option<usize>,
        /// How many entry nodes a `class~"…"` anchor opens on
        #[arg(long, default_value_t = yidam::QUERY_DEFAULT_ANCHOR_K)]
        anchor_k: usize,
        /// Output format. `text` prints the pack itself, which is the artefact; `json` wraps
        /// it in the report contract (RFC-0016) beside the receipt
        #[arg(long, value_enum, default_value_t = yidam::Format::Text)]
        format: yidam::Format,
    },
    /// Quote what a query would cost before running it — nodes matched, and what each
    /// projection would run to in characters and approximate tokens
    Estimate {
        /// The query to price, in `yidam query`'s language
        query: String,
        /// A projection to price beside the standard three, comma-separated
        #[arg(long)]
        select: Option<String>,
        /// The `--limit` the quoted call would carry. Projections are priced at it; a pack
        /// has no limit and is priced whole
        #[arg(long, default_value_t = yidam::QUERY_DEFAULT_LIMIT)]
        limit: usize,
        /// Token budget to price against. Each row is marked `fits` or `over budget`
        #[arg(long)]
        budget: Option<usize>,
        /// How many entry nodes a `class~"…"` anchor opens on
        #[arg(long, default_value_t = yidam::QUERY_DEFAULT_ANCHOR_K)]
        anchor_k: usize,
        /// Output format. `json` emits the machine-readable report contract (RFC-0016)
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
        /// Write .yidam/lint-baseline.yml only if it does not exist, then exit.
        /// Safe to run unconditionally — this is the adoption path, called by
        /// `mise run yidam-vendor-update`
        #[arg(long, conflicts_with = "bless")]
        init_baseline: bool,
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

/// The ontology migrations, each naming exactly what it changes.
///
/// Subcommands rather than flags on one command: the four take different arguments and
/// mean different things, and a single `--operation` with four optional operands would let
/// a caller ask for a retype while naming a relationship.
#[derive(Subcommand)]
enum MigrateCommand {
    /// Rename a class: its definition, its directory, and every edge that named it
    Class {
        /// The class as it is now, e.g. `gage`
        old: String,
        /// What it becomes, e.g. `station`
        new: String,
    },
    /// Rename a declared property on a class and on every instance carrying it
    Property {
        /// The class that declares it
        class: String,
        /// The property as it is now
        old: String,
        /// What it becomes
        new: String,
    },
    /// Change a declared property's type; refuses when an instance would not satisfy it
    Retype {
        /// The class that declares it
        class: String,
        /// The property to retype
        property: String,
        /// The new type — `string`, `text`, `date`, `ref`, `claim`, or one this corpus coined
        new_type: String,
    },
    /// Point a declared relationship at a different class, at both ends
    Edge {
        /// The class that declares it
        class: String,
        /// The relationship to re-target
        relationship: String,
        /// The class it should target instead
        new_target: String,
    },
}

impl From<MigrateCommand> for yidam::MigrateOperation {
    fn from(c: MigrateCommand) -> Self {
        match c {
            MigrateCommand::Class { old, new } => Self::ClassRename { old, new },
            MigrateCommand::Property { class, old, new } => {
                Self::PropertyRename { class, old, new }
            }
            MigrateCommand::Retype {
                class,
                property,
                new_type,
            } => Self::PropertyRetype {
                class,
                property,
                new_type,
            },
            MigrateCommand::Edge {
                class,
                relationship,
                new_target,
            } => Self::EdgeRetarget {
                class,
                relationship,
                new_target,
            },
        }
    }
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
        Command::Bench {
            budget,
            scaling,
            format,
        } => yidam::bench(budget, scaling, format),
        Command::Query {
            query,
            select,
            limit,
            anchor_k,
            at,
            between,
            across,
            format,
        } => yidam::query(&query, select, limit, anchor_k, at, between, across, format),
        Command::Pack {
            query,
            budget,
            anchor_k,
            format,
        } => yidam::pack(&query, budget, anchor_k, format),
        Command::Estimate {
            query,
            select,
            limit,
            budget,
            anchor_k,
            format,
        } => yidam::estimate(&query, select, limit, budget, anchor_k, format),
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
            init_baseline,
            format,
        } => yidam::lint(yidam::LintOptions {
            warn_only: warn,
            explain,
            commits,
            range,
            bless,
            init_baseline,
            format,
        }),
        Command::Migrate {
            operation,
            dry_run,
            format,
        } => yidam::migrate(operation.into(), dry_run, format),
        Command::Schema { settings } => yidam::schema(settings),
        Command::SamudayaAudit => yidam::samudaya_audit(),
        Command::Sangha { format } => yidam::sangha(format),
        Command::Vocabulary { check, format } => yidam::vocabulary(check, format),
        #[cfg(feature = "tonpa")]
        Command::Tonpa { sub } => block_on(yidam::tonpa::run(sub)),
    }
}
